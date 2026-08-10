//! The session ledger: what this agent has already been shown.
//!
//! Every filter in `src/distillers/` answers the same question, one command at a
//! time: given this output, what can be dropped. The ledger answers a different
//! one: given everything shown in this session, what is this output repeating.
//! The two are **orthogonal**, and the corpus says so. Replayed over 7,019
//! model-facing traces on 2026-08-10, 25.3% of raw bytes were lines already seen
//! and **22.9% still were after every distiller had run**. Filtering removes 5.2%
//! of bytes and barely dents repetition.
//!
//! It also reaches the class nothing else can. File reads are the largest class
//! in the corpus at 1.53 MB, and the filters save **0.0%** of it, because you
//! cannot strip lines from a file the agent asked to see without guessing which
//! parts it meant. The ledger takes 24.8% of the same class without guessing
//! anything: those lines were already delivered once.
//!
//! ## What it does
//!
//! A **run** of consecutive lines that were all emitted earlier in this session
//! becomes one marker naming the count and an `omni_retrieve` handle. Everything
//! else passes through byte for byte.
//!
//! ## The rules it inherits
//!
//! - **Append-only.** It only ever reduces the output of the command in flight,
//!   and never rewrites anything already delivered. That is what keeps the
//!   upstream prompt cache intact: a cache works on a prefix, and shortening the
//!   suffix costs nothing. Retroactive compaction is the move that would destroy
//!   it, and nothing here reaches backwards.
//! - **Deterministic.** The same ledger state must render byte-identical output.
//!   The handle is the content address and carries no timestamp. An earlier
//!   `{ts_ns}_{hash}` handle shipped and made 4 of 73 repeated inputs emit
//!   different bytes; it was fixed with no test behind it, so
//!   `renders_identical_bytes_for_identical_state` is that test.
//! - **Nothing is lost.** A run is archived before its marker is written, and a
//!   failed archive means the run stays verbatim. A handle that does not resolve
//!   is the one defect this whole mechanism cannot have (#388).
//! - **Unknown means untouched.** Structured payloads never reach here; the
//!   caller gates on `pipeline::format::sniff` exactly as collapse does.

use std::collections::HashSet;

use crate::guard::limits::{MIN_LEDGER_INPUT, MIN_LEDGER_RUN_BYTES, MIN_LEDGER_RUN_LINES};
use crate::store::sqlite::Store;

/// Addresses the ledger for one session.
///
/// `scope` is the session id today. Widening it to the project is what would
/// reach cross-session repetition, measured at 3.7% of post-filter bytes against
/// 19.1% within a session. A fifth of the value for a new eviction policy is why
/// that is a later phase and not this one.
pub struct Ledger<'a> {
    store: &'a Store,
    scope: String,
}

/// One stretch of output and whether it was already shown.
struct Run {
    start: usize,
    end: usize,
    seen: bool,
}

impl<'a> Ledger<'a> {
    pub fn new(store: &'a Store, scope: impl Into<String>) -> Self {
        Self {
            store,
            scope: scope.into(),
        }
    }

    /// The view of `text` this session has earned, or `None` when there is
    /// nothing to gain.
    ///
    /// `None` means "leave the caller's bytes alone", which is the honest answer
    /// whenever the projection did not actually shorten anything. The lines are
    /// recorded either way: a block is worth remembering because it may be seen
    /// again, not because it compressed today. That is the widened gate the plan
    /// asks for, and it is why the recording is unconditional while the
    /// substitution is not.
    pub fn project(&self, text: &str) -> Option<String> {
        if text.len() < MIN_LEDGER_INPUT {
            return None;
        }

        // `split_inclusive`, not `lines()`. `lines()` drops the terminator, so
        // rebuilding with `\n` would rewrite every CRLF payload on Windows into
        // LF, silently, on content the ledger did not replace. Keeping the
        // terminator on the slice makes the untouched runs byte-exact by
        // construction rather than by promise (CLAUDE.md cross-platform rule 2).
        let lines: Vec<&str> = text.split_inclusive('\n').collect();
        let hashes: Vec<String> = lines.iter().map(|l| line_key(l)).collect();
        let seen = self.store.ledger_seen(&self.scope, &hashes);

        // Record before projecting. The order matters only for a line that
        // repeats inside this very payload: recording first would let the second
        // copy see the first, which is true but not useful, since both copies are
        // in front of the agent already. Recording after keeps the projection a
        // function of what *earlier commands* showed, which is what makes it
        // deterministic for a caller replaying the same session.
        let projected = self.substitute(&lines, &hashes, &seen);
        self.store.ledger_record(&self.scope, &hashes);

        projected.filter(|p| p.len() < text.len())
    }

    fn substitute(
        &self,
        lines: &[&str],
        hashes: &[String],
        seen: &HashSet<String>,
    ) -> Option<String> {
        let runs = group_runs(hashes, seen);
        if !runs.iter().any(|r| r.seen) {
            return None;
        }

        let mut out = String::with_capacity(lines.iter().map(|l| l.len()).sum());
        let mut replaced_any = false;
        for run in runs {
            let body = lines[run.start..run.end].concat();
            let long_enough =
                run.end - run.start >= MIN_LEDGER_RUN_LINES && body.len() >= MIN_LEDGER_RUN_BYTES;

            // A handle is only offered for content that is provably retrievable.
            // `store_rewind` returns `None` when the row did not land, and the
            // run then stays verbatim rather than becoming a promise nobody can
            // keep (#388).
            match (run.seen && long_enough)
                .then(|| self.store.store_rewind(&body))
                .flatten()
            {
                Some(handle) => {
                    out.push_str(&format!(
                        "[OMNI: {} lines already shown this session, omni_retrieve(\"{handle}\") for them]",
                        run.end - run.start
                    ));
                    // The run carried its own terminator, so the marker needs one
                    // only when the text it replaced ended a line. A run at the
                    // very end of an output with no trailing newline does not.
                    if body.ends_with('\n') {
                        out.push('\n');
                    }
                    replaced_any = true;
                }
                None => out.push_str(&body),
            }
        }
        replaced_any.then_some(out)
    }
}

/// Consecutive lines that agree about having been seen.
fn group_runs(hashes: &[String], seen: &HashSet<String>) -> Vec<Run> {
    let mut runs: Vec<Run> = Vec::new();
    for (i, h) in hashes.iter().enumerate() {
        let is_seen = seen.contains(h);
        match runs.last_mut() {
            Some(last) if last.seen == is_seen => last.end = i + 1,
            _ => runs.push(Run {
                start: i,
                end: i + 1,
                seen: is_seen,
            }),
        }
    }
    runs
}

/// A line's identity in the ledger.
///
/// Trimmed, so the same line reached through `sed -n` and through `cat` is one
/// line rather than two. Hashed rather than stored whole because the table is
/// keyed on it and a 4 KB line would otherwise become a 4 KB index entry.
pub fn line_key(line: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(line.trim().as_bytes());
    crate::util::text::safe_slice(&hex::encode(h.finalize()), 16).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> (Store, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = Store::open_path(&dir.path().join("omni.db")).expect("store");
        (store, dir)
    }

    /// Long enough to clear `MIN_LEDGER_INPUT`, with a body that clears the run
    /// bounds. Sized against the constants rather than by eye, because a fixture
    /// under a threshold tests the early return instead of the logic.
    fn payload() -> String {
        (0..60)
            .map(|i| format!("2026-08-10T00:00:{i:02}Z  handler finished request {i} in 12ms"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn leaves_the_first_sighting_of_everything_alone() {
        let (store, _d) = temp_store();
        let text = payload();

        assert_eq!(Ledger::new(&store, "s1").project(&text), None);
    }

    #[test]
    fn hands_back_a_handle_for_output_it_already_showed() {
        let (store, _d) = temp_store();
        let text = payload();
        let ledger = Ledger::new(&store, "s1");

        ledger.project(&text);
        let second = ledger.project(&text).expect("a repeat is projectable");

        assert!(second.len() < text.len());
        assert!(second.contains("already shown this session"));
    }

    /// The promise the marker makes. A handle that does not resolve to the exact
    /// bytes it replaced makes every other guarantee here worthless.
    #[test]
    fn every_handle_resolves_to_the_bytes_it_replaced() {
        let (store, _d) = temp_store();
        let text = payload();
        let ledger = Ledger::new(&store, "s1");
        ledger.project(&text);

        let second = ledger.project(&text).expect("projectable");

        let handle = second
            .split_once("omni_retrieve(\"")
            .and_then(|(_, rest)| rest.split_once('"'))
            .map(|(h, _)| h.to_string())
            .expect("a handle was offered");
        assert_eq!(store.retrieve_rewind(&handle), Some(text.clone()));
    }

    /// Non negotiable, per the header. A nondeterministic view breaks the
    /// agent's upstream prompt cache, and a cache miss costs more than the
    /// compaction saved.
    #[test]
    fn renders_identical_bytes_for_identical_state() {
        let (store, _d) = temp_store();
        let text = payload();
        Ledger::new(&store, "s1").project(&text);

        let first = Ledger::new(&store, "s1").project(&text);
        let second = Ledger::new(&store, "s1").project(&text);

        assert_eq!(first, second);
        assert!(first.is_some(), "the fixture must reach the substitution");
    }

    /// One session's history is not another's. Without this the ledger would
    /// tell a fresh session it had already seen output it never received, which
    /// is the false-claim defect this project is named after.
    #[test]
    fn never_claims_another_sessions_history() {
        let (store, _d) = temp_store();
        let text = payload();
        Ledger::new(&store, "s1").project(&text);

        assert_eq!(Ledger::new(&store, "s2").project(&text), None);
    }

    /// Everything the ledger does not replace comes back byte for byte, line
    /// terminators included. Rebuilding from `lines()` would rewrite a CRLF
    /// payload into LF on the untouched runs, which is a silent edit to content
    /// nobody asked to change and a corrupt diff on Windows.
    #[test]
    fn leaves_untouched_runs_byte_identical_including_crlf() {
        let (store, _d) = temp_store();
        let shared: String = (0..30)
            .map(|i| format!("a shared header line number {i} of this payload\r\n"))
            .collect();
        let ledger = Ledger::new(&store, "s1");
        ledger.project(&shared);

        let mixed = format!(
            "{shared}{}",
            (0..30)
                .map(|i| format!("a fresh line number {i} that has never been shown\r\n"))
                .collect::<String>()
        );
        let view = ledger.project(&mixed).expect("the header is a repeat");

        let fresh_tail: String = (0..30)
            .map(|i| format!("a fresh line number {i} that has never been shown\r\n"))
            .collect();
        assert!(
            view.ends_with(&fresh_tail),
            "the unreplaced tail lost its terminators: {view:?}"
        );
    }

    /// A run under the bounds costs more as a marker than it saves as text.
    #[test]
    fn leaves_a_short_repeated_run_in_place() {
        let (store, _d) = temp_store();
        let shared = "the shared preamble line, over twelve characters";
        let first = format!("{shared}\n{}", payload());
        let ledger = Ledger::new(&store, "s1");
        ledger.project(&first);

        // Only `shared` is a repeat now: one line, far under the run bounds.
        let second = format!(
            "{shared}\n{}",
            (0..60)
                .map(|i| format!("a completely different line number {i} of this second payload"))
                .collect::<Vec<_>>()
                .join("\n")
        );

        assert_eq!(ledger.project(&second), None);
    }
}
