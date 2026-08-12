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

use crate::guard::limits::{MIN_LEDGER_INPUT, MIN_LEDGER_RUN_GAIN, PROJECT_FLOOR_MULT};
use crate::store::sqlite::Store;

/// Which history a run was found in, because the two cannot make the same claim.
///
/// This distinction is the whole of the project scope. An earlier draft cancelled
/// it on the grounds that a handle for another session's content is a lie, which
/// was right about the wording and wrong about the remedy: the fix is to stop
/// saying "already shown", not to stop remembering.
///
/// - `Session` means the agent is still holding those bytes, so the handle is
///   free unless it chooses to re-read them.
/// - `Project` means the bytes went to a different session of this project and
///   this agent has **never seen them**. The marker says so, and the trade stops
///   being free: it is a handle plus a possible retrieval against the block
///   itself, which is the same bet `store_rewind` already makes everywhere else.
///   That is why it carries its own, much higher floor.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Origin {
    Session,
    Project,
}

impl Origin {
    /// What a replaced run is worth saying about itself.
    ///
    /// Every word here is paid for on every fold, so the wording is as short as
    /// it can be while still saying which history the lines came from and how to
    /// get them back. The session form was 87 bytes and is 65; trimming it moved
    /// the aggregate by 0.3 points on its own, because a shorter marker makes
    /// smaller runs worth folding (#450).
    fn marker(self, lines: usize, handle: &str) -> String {
        match self {
            Self::Session => {
                format!("[OMNI: {lines} lines already shown, omni retrieve {handle}]")
            }
            Self::Project => {
                format!("[OMNI: {lines} lines from an earlier session, omni retrieve {handle}]")
            }
        }
    }

    /// The bytes a fold has to save, after paying for its own marker.
    ///
    /// Session scope pays a marker. Project scope pays a marker **and** the
    /// expected cost of a retrieval the agent has no choice about if it needs
    /// the content, because it has never seen those lines. Three times the
    /// session gain is where the replay put the knee: at that bar the project
    /// scope keeps the runs worth several hundred bytes each and drops the tail
    /// that was earning 179 bytes per interruption (#448).
    fn min_gain(self) -> usize {
        match self {
            Self::Session => MIN_LEDGER_RUN_GAIN,
            Self::Project => MIN_LEDGER_RUN_GAIN * PROJECT_FLOOR_MULT,
        }
    }
}

/// A handle is always this long, so a marker's length is known before the run is
/// archived and the profitability test can weigh the real thing.
///
/// `store_rewind` slices the hex digest to 16 characters. If that ever changes,
/// `renders_a_marker_the_gain_test_can_predict` fails rather than the ledger
/// silently folding runs that do not pay.
const HANDLE_LEN: usize = 16;

/// Addresses the ledger for one session, and optionally for its project.
///
/// Cross-session repetition measures 3.7% of post-filter bytes against 19.1%
/// within a session, so the project scope is worth about a fifth of the session
/// scope at best, and less than that after its higher floor. It is built anyway
/// because the plan asks for it and because the data is what settles whether the
/// trade is worth taking.
pub struct Ledger<'a> {
    store: &'a Store,
    scope: String,
    /// `None` disables project scope entirely, which is what every caller that
    /// cannot name a project passes.
    project: Option<String>,
}

/// One stretch of output, and where it was seen before if it was.
struct Run {
    start: usize,
    end: usize,
    seen: Option<Origin>,
}

impl<'a> Ledger<'a> {
    pub fn new(store: &'a Store, scope: impl Into<String>) -> Self {
        Self {
            store,
            scope: scope.into(),
            project: None,
        }
    }

    /// Adds the project history to what this ledger can draw on.
    pub fn with_project(mut self, project: impl Into<String>) -> Self {
        self.project = Some(project.into());
        self
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
        // The gain gate wraps the projection rather than being re-derived inside
        // it (spec 5.4). `MIN_LEDGER_INPUT` is this projection's own floor and is
        // higher than the gate's, so both apply and the stricter one decides.
        crate::pipeline::gate::gain(text, |text| self.project_inner(text))
    }

    fn project_inner(&self, text: &str) -> Option<String> {
        if text.len() < MIN_LEDGER_INPUT {
            return None;
        }

        // `split_inclusive`, not `lines()`. `lines()` drops the terminator, so
        // rebuilding with `\n` would rewrite every CRLF payload on Windows into
        // LF, silently, on content the ledger did not replace. Keeping the
        // terminator on the slice makes the untouched runs byte-exact by
        // construction rather than by promise (`CONTRIBUTING.md` cross-platform rule 2).
        let lines: Vec<&str> = text.split_inclusive('\n').collect();
        let hashes: Vec<String> = lines.iter().map(|l| line_key(l)).collect();

        // Session first, and the project scope only answers for what the session
        // did not. A line in both belongs to the session: that is the claim that
        // is free, and taking the weaker one would cost the agent a retrieval for
        // content it is holding.
        let in_session = self.store.ledger_seen(&self.scope, &hashes);
        let in_project = match &self.project {
            Some(p) => self.store.ledger_seen(p, &hashes),
            None => HashSet::new(),
        };
        // A line that states a failure is never folded, however often it has been
        // shown (#458). The heuristic "you have seen this already" is sound for
        // informational lines and wrong for the error channel, where the
        // repetition *is* the signal: the same TypeError appearing on a re-run
        // means the bug is still there, and that is the one line worth the
        // tokens. Eliding it delivers source context and no statement of what
        // went wrong, which an agent reasonably reads as the failure being gone.
        //
        // Marking the line unseen rather than filtering it afterwards also
        // splits the run around it, so the frames either side still fold.
        // Identical hash means identical trimmed text, so the verdict is a
        // property of the hash and this set answers in O(1) rather than the
        // closure searching the payload per line. The hook has a 10 ms budget.
        let never_fold: HashSet<&String> = hashes
            .iter()
            .zip(lines.iter())
            .filter(|(_, line)| crate::pipeline::semantic::carries_failure(line))
            .map(|(hash, _)| hash)
            .collect();

        let origin_of = |h: &String| {
            if never_fold.contains(h) {
                return None;
            }
            if in_session.contains(h) {
                Some(Origin::Session)
            } else if in_project.contains(h) {
                Some(Origin::Project)
            } else {
                None
            }
        };

        // Record before projecting. The order matters only for a line that
        // repeats inside this very payload: recording first would let the second
        // copy see the first, which is true but not useful, since both copies are
        // in front of the agent already. Recording after keeps the projection a
        // function of what *earlier commands* showed, which is what makes it
        // deterministic for a caller replaying the same session.
        let projected = self
            .substitute(&lines, &hashes, &origin_of)
            .filter(|(view, _)| view.len() < text.len());

        // Record what the agent was handed, which is not always what it was
        // given (#465). A run replaced by a marker never reached the context, so
        // recording it here would let the *next* occurrence in this session read
        // as `Origin::Session` and say "already shown" about bytes that were
        // never shown. It also halves the bar: session origin charges
        // `MIN_LEDGER_RUN_GAIN` where project origin charges three times that,
        // so the false claim makes the ledger more willing to fold, not less.
        //
        // When the projection is discarded, by `substitute` finding nothing or by
        // the gain filter above, the caller emits `text` verbatim and every line
        // was delivered. That is the empty-set case and needs no special arm.
        let delivered: Vec<String> = match &projected {
            Some((_, folded)) => hashes
                .iter()
                .enumerate()
                .filter(|(i, _)| !folded.contains(i))
                .map(|(_, h)| h.clone())
                .collect(),
            None => hashes.clone(),
        };
        self.store.ledger_record(&self.scope, &delivered);
        // The project history is written too, so a later session can draw on this
        // one. Same rows, a second scope key. A folded line is already in both
        // scopes, since that is what made it foldable, so filtering it out of
        // this write changes nothing and keeps the two calls saying one thing.
        if let Some(p) = &self.project {
            self.store.ledger_record(p, &delivered);
        }

        projected.map(|(view, _)| view)
    }

    /// The view, and the indices of the lines it replaced with a marker.
    ///
    /// The caller needs the second half to record only what it delivered (#465);
    /// returning it beats recomputing which runs folded, which would mean
    /// re-deciding profitability and could disagree with what was emitted.
    fn substitute(
        &self,
        lines: &[&str],
        hashes: &[String],
        origin_of: &dyn Fn(&String) -> Option<Origin>,
    ) -> Option<(String, HashSet<usize>)> {
        let runs = group_runs(hashes, origin_of);
        if !runs.iter().any(|r| r.seen.is_some()) {
            return None;
        }

        let mut out = String::with_capacity(lines.iter().map(|l| l.len()).sum());
        let mut folded: HashSet<usize> = HashSet::new();
        let mut replaced_any = false;
        for run in runs {
            let body = lines[run.start..run.end].concat();
            // The only question that decides a fold: does this run save more
            // than the marker replacing it costs. The marker is rendered rather
            // than estimated, so the test cannot drift from the string it is
            // weighing, and the handle's length is fixed (#450).
            let long_enough = run.seen.is_some_and(|o| {
                let marker = o.marker(run.end - run.start, &"0".repeat(HANDLE_LEN)).len();
                body.len() >= marker + o.min_gain()
            });

            // A handle is only offered for content that is provably retrievable.
            // `store_rewind` returns `None` when the row did not land, and the
            // run then stays verbatim rather than becoming a promise nobody can
            // keep (#388).
            match long_enough
                .then(|| self.store.store_rewind(&body))
                .flatten()
                .zip(run.seen)
            {
                Some((handle, origin)) => {
                    out.push_str(&origin.marker(run.end - run.start, &handle));
                    // The run carried its own terminator, so the marker needs one
                    // only when the text it replaced ended a line. A run at the
                    // very end of an output with no trailing newline does not.
                    if body.ends_with('\n') {
                        out.push('\n');
                    }
                    folded.extend(run.start..run.end);
                    replaced_any = true;
                }
                None => out.push_str(&body),
            }
        }
        replaced_any.then_some((out, folded))
    }
}

/// Consecutive lines that agree about where they were seen.
///
/// Grouping by `Option<Origin>` rather than by a boolean is what stops a session
/// run and a project run merging into one marker, which would have to pick one of
/// two claims for content that is half and half.
fn group_runs(hashes: &[String], origin_of: &dyn Fn(&String) -> Option<Origin>) -> Vec<Run> {
    let mut runs: Vec<Run> = Vec::new();
    for (i, h) in hashes.iter().enumerate() {
        let seen = origin_of(h);
        match runs.last_mut() {
            Some(last) if last.seen == seen => last.end = i + 1,
            _ => runs.push(Run {
                start: i,
                end: i + 1,
                seen,
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
        assert!(second.contains("lines already shown"));
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
            .split_once("omni retrieve ")
            .and_then(|(_, rest)| rest.split_once(']'))
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

    /// The whole licence for the project scope. A second session may reuse the
    /// first session's history, but it has **not seen those bytes**, so the
    /// marker must not say it has. An earlier draft cancelled this phase over
    /// exactly that wording; the remedy was the wording.
    #[test]
    fn never_tells_a_new_session_it_has_already_seen_the_project_history() {
        let (store, _d) = temp_store();
        // Six times the session floor, so it clears the project scope's own bar.
        let text: String = (0..200)
            .map(|i| format!("2026-08-10T00:00:00Z  handler finished request {i} in 12ms\n"))
            .collect();
        Ledger::new(&store, "s1")
            .with_project("/repo")
            .project(&text);

        let view = Ledger::new(&store, "s2")
            .with_project("/repo")
            .project(&text)
            .expect("a project repeat above the floor is projectable");

        assert!(
            view.contains("from an earlier session"),
            "the marker claimed a sighting this session never had: {view}"
        );
        assert!(
            !view.contains("already shown"),
            "a project repeat was reported as a session repeat: {view}"
        );
    }

    /// A marker is not a sighting (#465).
    ///
    /// The session above was handed a pointer, never the bytes behind it, so the
    /// next occurrence in that same session is still a project repeat. Recording
    /// the folded run as shown turned the second marker into `already shown`,
    /// which is false, and dropped the bar from `MIN_LEDGER_RUN_GAIN * 3` to
    /// `MIN_LEDGER_RUN_GAIN`, so the false claim also made the ledger three times
    /// more willing to fold.
    #[test]
    fn a_folded_run_is_not_recorded_as_shown() {
        let (store, _d) = temp_store();
        let text: String = (0..200)
            .map(|i| format!("2026-08-10T00:00:00Z  handler finished request {i} in 12ms\n"))
            .collect();
        Ledger::new(&store, "s1")
            .with_project("/repo")
            .project(&text);

        // s2 sees it for the first time and is handed a marker, not the bytes.
        let first = Ledger::new(&store, "s2")
            .with_project("/repo")
            .project(&text)
            .expect("a project repeat above the floor is projectable");
        assert!(first.contains("from an earlier session"), "{first}");

        // Same session, same payload. It has still never received those lines.
        let second = Ledger::new(&store, "s2")
            .with_project("/repo")
            .project(&text)
            .expect("still projectable");

        assert!(
            second.contains("from an earlier session"),
            "a run this session only ever saw as a marker was reported as shown: {second}"
        );
        assert!(
            !second.contains("already shown"),
            "the ledger claimed a sighting that was a marker: {second}"
        );
    }

    /// The project scope pays a retrieval the session scope does not, so it
    /// carries its own floor. A run that clears the session bar and not the
    /// project one stays verbatim rather than costing the agent a round trip.
    #[test]
    fn holds_a_project_repeat_to_its_own_higher_floor() {
        let (store, _d) = temp_store();
        // Sized to land between the two bars on purpose: it saves more than a
        // session marker plus its gain, and less than a project one. `payload()`
        // clears both, which is what the guard below caught when this test first
        // reached for it.
        let text: String = (0..9)
            .map(|i| format!("2026-08-10T00:00:0{i}Z  handler finished request {i}\n"))
            .collect();
        let session_bar =
            Origin::Session.marker(9, &"0".repeat(HANDLE_LEN)).len() + Origin::Session.min_gain();
        let project_bar =
            Origin::Project.marker(9, &"0".repeat(HANDLE_LEN)).len() + Origin::Project.min_gain();
        assert!(
            text.len() > MIN_LEDGER_INPUT && text.len() >= session_bar,
            "fixture must clear the session bar, or it tests the early return"
        );
        assert!(
            text.len() < project_bar,
            "fixture must sit between the two bars, or it tests neither"
        );
        Ledger::new(&store, "s1")
            .with_project("/repo")
            .project(&text);

        // Same session: over the session floor, so it projects.
        assert!(Ledger::new(&store, "s1").project(&text).is_some());
        // New session: only the project history has it, and it is too small.
        assert_eq!(
            Ledger::new(&store, "s2")
                .with_project("/repo")
                .project(&text),
            None
        );
    }

    /// A line in both histories belongs to the session, because that is the
    /// claim that costs the agent nothing. Taking the weaker one would buy a
    /// retrieval for content already in the context window.
    #[test]
    fn prefers_the_session_claim_when_a_line_is_in_both() {
        let (store, _d) = temp_store();
        let text: String = (0..200)
            .map(|i| format!("2026-08-10T00:00:00Z  handler finished request {i} in 12ms\n"))
            .collect();
        let ledger = Ledger::new(&store, "s1").with_project("/repo");
        ledger.project(&text);

        let view = ledger.project(&text).expect("projectable");

        assert!(view.contains("already shown"), "{view}");
        assert!(!view.contains("from an earlier session"), "{view}");
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

    /// #458, from a real session. A failing `bun` script re-run in the same
    /// session came back with source context and no `TypeError`, because the
    /// error lines had been shown earlier and the fold treated them like any
    /// other repetition. An agent reads that as the failure being gone.
    #[test]
    fn never_folds_a_line_that_states_a_failure() {
        let (store, _d) = temp_store();
        let payload = "19 |   \"x-csrf-token\": csrf,\n                       20 |   Cookie: cookies,\n                       21 | };\n                       22 | \n                       23 | const profiles = await (await fetch(BASE)).json();\n                       24 | const profileId = profiles.profiles[0].id;\n                       TypeError: undefined is not an object (evaluating 'profiles.profiles[0]')\n                             at /tmp/repro237.ts:24:28\n                       Bun v1.3.14 (macOS arm64)\n";

        let ledger = Ledger::new(&store, "s1");
        ledger.project(payload);
        let second = ledger
            .project(payload)
            .expect("a full repeat still folds something");

        assert!(
            second.contains("TypeError: undefined is not an object"),
            "the error was elided on the re-run: {second}"
        );
        assert!(
            second.contains("[OMNI:"),
            "the repeated context should still fold, or this proves nothing: {second}"
        );
    }

    /// The new rule, and the one the old bounds got wrong. Three lines is under
    /// `MIN_LEDGER_RUN_LINES` as it was, but three long lines pay for their
    /// marker several times over, so refusing them was refusing free bytes.
    #[test]
    fn folds_a_short_run_of_long_lines_because_it_pays_for_its_marker() {
        let (store, _d) = temp_store();
        let run: String = (0..3)
            .map(|i| format!("2026-08-11T00:00:0{i}Z  a long structured log line that carries a request id, a latency and a route\n"))
            .collect();
        let ledger = Ledger::new(&store, "s1");
        ledger.project(&format!("{run}{}", payload()));

        let bar =
            Origin::Session.marker(3, &"0".repeat(HANDLE_LEN)).len() + Origin::Session.min_gain();
        assert!(
            run.len() >= bar,
            "fixture must clear the gain bar ({} vs {bar}), or it tests the old bound",
            run.len()
        );

        let second = format!(
            "{run}{}",
            (0..40)
                .map(|i| format!("a completely different line number {i} of this second payload\n"))
                .collect::<String>()
        );
        let view = ledger
            .project(&second)
            .expect("a run that pays should fold");

        assert!(view.contains("3 lines already shown"), "{view}");
    }

    /// The gain test weighs a marker rendered with a placeholder handle, so it
    /// is only honest while the real handle is the same length. If
    /// `store_rewind` ever widens its slice, the ledger would start folding runs
    /// that do not pay, silently.
    #[test]
    fn renders_a_marker_the_gain_test_can_predict() {
        let (store, _d) = temp_store();
        let handle = store
            .store_rewind("some content worth archiving\n")
            .expect("a healthy store archives");

        assert_eq!(handle.len(), HANDLE_LEN);
        assert_eq!(
            Origin::Session.marker(9, &handle).len(),
            Origin::Session.marker(9, &"0".repeat(HANDLE_LEN)).len()
        );
    }
}
