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

use std::collections::{HashMap, HashSet};

use crate::guard::limits::{
    MIN_LEDGER_INPUT, MIN_LEDGER_RUN_GAIN, MIN_WHOLE_OUTPUT_FOLD, PROJECT_FLOOR_MULT,
};
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
            // #567. `from an earlier session` states provenance and reads as
            // "you have seen this", which is the opposite of what it means: the
            // project scope only answers for lines the session scope did not, so
            // these lines were never delivered here. A reader acted on that,
            // took a help page missing its `Commands:` block as complete, and
            // concluded the CLI had no uninstall.
            //
            // The actionable half goes first and the provenance is dropped, which
            // is also nine bytes shorter. Marker length gates folding, and the
            // session form's trim was worth 0.3 points on its own (#450), so the
            // honest wording is the cheaper one here rather than a trade.
            Self::Project => {
                format!("[OMNI: {lines} lines not shown here, omni retrieve {handle}]")
            }
        }
    }

    /// What to say when the fold covered the payload and nothing else came back.
    ///
    /// `N lines already shown` is a claim about a *run*, and it reads as one:
    /// some of this was shown before, the rest is new. When the run is the whole
    /// output there is no rest, and the same wording leaves a reader unable to
    /// tell a fully elided reply from a command that printed nothing. Re-running
    /// a command is how a failure gets verified and how a fix gets confirmed, so
    /// reading `no output` there is the expensive misreading (#519).
    ///
    /// It states the identity instead, which is the fact the re-run was asking
    /// for and is strictly more information than the run wording carried.
    fn whole_output_marker(self, lines: usize, handle: &str) -> String {
        match self {
            Self::Session => format!(
                "[OMNI: identical to the {lines} lines already shown, omni retrieve {handle}]"
            ),
            // The whole payload, and none of it delivered here, so the agent
            // holds nothing at all. Worth the extra bytes to say both: this
            // marker is already gated at `MIN_WHOLE_OUTPUT_FOLD` (#567).
            Self::Project => format!(
                "[OMNI: identical to {lines} lines from an earlier session, none shown here, omni retrieve {handle}]"
            ),
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

/// What a fold did to the numbers of the lines still under it.
///
/// Only a caller that controls where its host starts counting can act on this,
/// which today is the `Read` path: the host renders `file.content` with `cat -n`
/// numbering counted from `startLine`, so it numbers whatever it is handed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoldShift {
    /// Nothing survives below the fold, so no number moves. A whole-output fold
    /// and any fold reaching the end of the payload are this.
    None,
    /// The survivors are one block with folded lines above them. `bump` is what
    /// the caller must add to its host's starting number so the first survivor
    /// lands on the line the file gives it, and with it every line below.
    ///
    /// Not "folded lines minus one": adjacent runs of different origin emit one
    /// marker each, so the number of lines standing above the survivors is the
    /// number of markers, which is what this counts. Review found the earlier
    /// arithmetic putting every survivor `k - 1` lines too high.
    Leading { bump: usize },
    /// Content above the fold and content below it. Nothing can correct that,
    /// because one starting number cannot describe two different offsets.
    Interior,
}

impl FoldShift {
    /// The question is not where the folds are, it is whether what survives them
    /// is one block. Contiguous survivors all sit at the same distance from
    /// where they started, so a single starting number puts every one of them
    /// back; split survivors sit at two different distances and no single number
    /// describes both.
    ///
    /// `markers_above` comes from `substitute`, which is the only place that
    /// knows: a run becomes one marker or stays verbatim. Three attempts to infer
    /// it from the output were wrong before it was simply reported (#573), and
    /// each is worth not repeating. `folded.len() - 1` assumes one marker per
    /// fold and adjacent runs of different origin emit one each. Searching the
    /// view for the surviving block matches inside a marker when the block's text
    /// is something a marker also says. Recognising markers by their prefix is
    /// defeated by a file whose own lines start with it.
    fn of(folded: &HashSet<usize>, total: usize, markers_above: usize) -> Self {
        let survivors: Vec<usize> = (0..total).filter(|i| !folded.contains(i)).collect();
        let (Some(&first), Some(&last)) = (survivors.first(), survivors.last()) else {
            // Nothing survived, so nothing can be misnumbered.
            return Self::None;
        };
        if last - first + 1 != survivors.len() {
            return Self::Interior;
        }
        if first == 0 {
            // The survivors still start where the payload does, so the numbers
            // the host will give them are already right.
            return Self::None;
        }
        // A fold below the survivors changes nothing about the survivors: they are
        // still `first` lines into the file and `markers_above` lines into the
        // view, so one number closes that gap whatever follows them.
        Self::Leading {
            bump: first.saturating_sub(markers_above),
        }
    }
}

/// The session scope for one reader, which is not the same as one session.
///
/// The scope answers "has this reader already been shown these bytes", and
/// `Origin::Session`'s marker asserts exactly that. A subagent runs in its own
/// context and carries **the parent's** `session_id`, so keying on the session
/// alone answered a subagent with the parent's history and told it 200 lines
/// were "already shown" when that context had received none of them (#581).
///
/// The main agent has no `agent_id`, so its scope is unchanged and no history is
/// orphaned by this. What a subagent loses is only the parent's lines; its own
/// repeats still fold under its own scope, and genuinely repeated project bytes
/// still fold through the project scope, whose marker says "not shown here" and
/// is honest for a reader that never held them (#567, #575).
///
/// This does not fix the other reader the premise fails for: after compaction
/// the session id is unchanged and the context is gone, and nothing in the hook
/// payload says so.
pub fn scope_for(session: &str, agent: Option<&str>) -> String {
    match agent {
        Some(a) if !a.is_empty() => format!("{session}/{a}"),
        _ => session.to_string(),
    }
}

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
    /// Who is being shown these lines, recorded and read by nothing (#509).
    ///
    /// The project scope is keyed on the directory alone, so two agents in one
    /// repo already share a history and a fold cannot tell whose bytes it is
    /// replacing. Recording the agent is what makes that measurable; changing
    /// the key is the decision the measurement is for.
    agent: String,
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
            agent: "unknown".to_string(),
        }
    }

    /// Adds the project history to what this ledger can draw on.
    pub fn with_project(mut self, project: impl Into<String>) -> Self {
        self.project = Some(project.into());
        self
    }

    /// Names the agent these lines are being delivered to.
    ///
    /// Resolved by the caller rather than from the environment here: a Codex
    /// payload arriving while `CLAUDECODE` is set answers `claude_code` to the
    /// naive check, and this column exists precisely to tell hosts apart.
    pub fn by(mut self, agent: impl Into<String>) -> Self {
        self.agent = agent.into();
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
        self.project_reporting_shift(text).map(|(view, _)| view)
    }

    /// The view, and what the fold did to the numbering of the lines under it.
    ///
    /// A caller whose host numbers the lines it is handed needs the second half:
    /// a marker with content under it moves every one of those numbers, and the
    /// payload says nothing about it (#557). The question is answered from the
    /// folded indices rather than by recognising markers in the output, because
    /// a file whose own lines begin with the marker prefix would defeat that.
    pub fn project_reporting_shift(&self, text: &str) -> Option<(String, FoldShift)> {
        let shift = std::cell::Cell::new(FoldShift::None);
        // The gain gate wraps the projection rather than being re-derived inside
        // it (spec 5.4). `MIN_LEDGER_INPUT` is this projection's own floor and is
        // higher than the gate's, so both apply and the stricter one decides.
        let view = crate::pipeline::gate::gain(text, |text| {
            self.project_inner(text).map(|(view, shifted)| {
                shift.set(shifted);
                view
            })
        })?;
        Some((view, shift.get()))
    }

    fn project_inner(&self, text: &str) -> Option<(String, FoldShift)> {
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
            None => HashMap::new(),
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
            if in_session.contains_key(h) {
                Some(Origin::Session)
            } else if in_project.contains_key(h) {
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
            .filter(|(view, _, _)| view.len() < text.len());

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
            Some((_, folded, _)) => hashes
                .iter()
                .enumerate()
                .filter(|(i, _)| !folded.contains(i))
                .map(|(_, h)| h.clone())
                .collect(),
            None => hashes.clone(),
        };
        // What the folds drew on, before the write below makes every line look
        // like this session's (#533). `PROJECT_FLOOR_MULT` prices cross-agent
        // reuse and was calibrated on the single-agent case, and nothing recorded
        // enough to tell the two apart after the fact.
        if let Some((_, folded, _)) = &projected {
            // Every line folded means the agent holds markers and nothing else,
            // which is the case `MIN_WHOLE_OUTPUT_FOLD` refuses below 1 KB. Read
            // here rather than threaded out of `substitute`, because that flag is
            // per run while this table is already one row per (origin, source
            // agent) per call. The two differ only when adjacent runs of
            // different origins tile the payload, and for the question this
            // column answers, whether the agent kept any content at all, the
            // call-level reading is the right one.
            let whole_output = folded.len() == lines.len();
            let mut tally: HashMap<(&'static str, &str), (usize, usize)> = HashMap::new();
            for &i in folded {
                let Some(origin) = origin_of(&hashes[i]) else {
                    continue;
                };
                let (label, source) = match origin {
                    Origin::Session => ("session", in_session.get(&hashes[i])),
                    Origin::Project => ("project", in_project.get(&hashes[i])),
                };
                // A hit with no agent cannot happen through `ledger_record`, which
                // always writes one. Counting it as `unknown` beats dropping the
                // row and quietly understating the total.
                let entry = tally
                    .entry((label, source.map_or("unknown", String::as_str)))
                    .or_default();
                entry.0 += 1;
                entry.1 += lines[i].len();
            }
            let folds: Vec<crate::store::sqlite::FoldRecord> = tally
                .into_iter()
                .map(
                    |((origin, source_agent), (lines, bytes))| crate::store::sqlite::FoldRecord {
                        source_agent: source_agent.to_string(),
                        origin,
                        lines,
                        bytes,
                        whole_output,
                        payload_bytes: text.len(),
                    },
                )
                .collect();
            // Keyed on the project when there is one: the question is about a
            // repository two agents share, and a session scope cannot be compared
            // across agents by definition.
            let scope = self.project.as_deref().unwrap_or(&self.scope);
            self.store.ledger_record_folds(scope, &self.agent, &folds);
        }

        self.store
            .ledger_record(&self.scope, &delivered, &self.agent);
        // The project history is written too, so a later session can draw on this
        // one. Same rows, a second scope key. A folded line is already in both
        // scopes, since that is what made it foldable, so filtering it out of
        // this write changes nothing and keeps the two calls saying one thing.
        if let Some(p) = &self.project {
            self.store.ledger_record(p, &delivered, &self.agent);
        }

        // What the fold did to the numbering below it (#557). Read from the
        // folded indices, where the answer is known.
        let shifts = projected
            .as_ref()
            .map(|(_, folded, above)| FoldShift::of(folded, lines.len(), *above))
            .unwrap_or(FoldShift::None);
        projected.map(|(view, _, _)| (view, shifts))
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
    ) -> Option<(String, HashSet<usize>, usize)> {
        let runs = group_runs(hashes, origin_of);
        if !runs.iter().any(|r| r.seen.is_some()) {
            return None;
        }

        let payload_bytes: usize = lines.iter().map(|l| l.len()).sum();
        let mut out = String::with_capacity(payload_bytes);
        let mut folded: HashSet<usize> = HashSet::new();
        let mut replaced_any = false;
        // How many marker lines stand above the first line that survives. Counted
        // here because this is the only place that knows: a run becomes one marker
        // or stays verbatim, and adjacent runs of different origin are separate
        // runs. Every attempt to infer it downstream was wrong (#573).
        let mut markers_above = 0usize;
        let mut still_above = true;
        for run in runs {
            let body = lines[run.start..run.end].concat();
            // The only question that decides a fold: does this run save more
            // than the marker replacing it costs. The marker is rendered rather
            // than estimated, so the test cannot drift from the string it is
            // weighing, and the handle's length is fixed (#450).
            // One run covering every line means the reply is this marker and
            // nothing else, which needs different wording (#519). Decided here
            // rather than at the emit site so the affordability test below weighs
            // the string that will actually be sent: the whole-output wording is
            // longer, and the first draft of this weighed the short one and
            // emitted the long one, which is the drift the rest of this comment
            // exists to prevent.
            let covers_everything = run.start == 0 && run.end == lines.len();
            let render = |o: Origin, handle: &str| {
                if covers_everything {
                    o.whole_output_marker(run.end - run.start, handle)
                } else {
                    o.marker(run.end - run.start, handle)
                }
            };

            // Two questions, not one. The first is whether the run outgrows the
            // marker replacing it, which is what every floor here has ever asked.
            // The second only applies when the fold leaves nothing behind: a
            // partial fold hands the agent context it can work from and a handle
            // it may decline, while a whole-output fold hands it a handle and
            // nothing, so needing any part of the payload costs a round trip it
            // has no say in. Four of four recorded under 1 KB were retrieved
            // within nine seconds (#543), so below that floor the trade is
            // negative and the run stays verbatim.
            // The whole-output floor, applied to the folds that are the
            // whole output in every way that matters to a reader (#601).
            //
            // `covers_everything` is exact and the danger is not. Resolving a
            // merge conflict and re-reading the same window folded 23 of 26
            // lines and delivered `zulu-08/09/10`: three lines that had nothing
            // to do with the edit, under a marker reading `23 lines already
            // shown`, which an agent verifying a deletion reads as *nothing
            // changed*. The deletion and the reorder that were the entire point
            // produced no new line, so there was nothing for the fold to emit.
            //
            // A remainder that small is not context, it is a round trip the
            // agent has no say in, which is the argument `MIN_WHOLE_OUTPUT_FOLD`
            // already makes for the exact case. So the floor widens to cover it
            // and the wording does not: a partial fold still says `N lines
            // already shown`, because it still is one.
            //
            // Priced on 847 recorded folds before choosing four fifths: the
            // guard refuses 3 of them and 1,748 bytes of 1,439,813, or 0.12% of
            // everything the ledger has ever saved on this machine. It cannot
            // reach a large payload, where a 90% fold is the case this feature
            // exists for and the floor is already met many times over.
            let leaves_almost_nothing = body.len() * 5 >= payload_bytes * 4;
            let long_enough = run.seen.is_some_and(|o| {
                let marker = render(o, &"0".repeat(HANDLE_LEN)).len();
                body.len() >= marker + o.min_gain()
                    && (!(covers_everything || leaves_almost_nothing)
                        || body.len() >= MIN_WHOLE_OUTPUT_FOLD)
            });

            // A handle is only offered for content that is provably retrievable.
            // `store_rewind` returns `None` when the row did not land, and the
            // run then stays verbatim rather than becoming a promise nobody can
            // keep (#388).
            // A run whose handle was just pulled stays verbatim. That delivery is
            // the answer to the pull, and folding it hands the reader back the
            // marker it was following (#581). The flag is consumed here, so this
            // costs one delivery rather than exempting the content for good.
            match long_enough
                .then(|| self.store.store_rewind(&body))
                .flatten()
                .filter(|handle| !self.store.take_owed_delivery(handle))
                .zip(run.seen)
            {
                Some((handle, origin)) => {
                    out.push_str(&render(origin, &handle));
                    // The run carried its own terminator, so the marker needs one
                    // only when the text it replaced ended a line. A run at the
                    // very end of an output with no trailing newline does not.
                    if body.ends_with('\n') {
                        out.push('\n');
                    }
                    folded.extend(run.start..run.end);
                    replaced_any = true;
                    if still_above {
                        markers_above += 1;
                    }
                }
                None => {
                    out.push_str(&body);
                    still_above = false;
                }
            }
        }
        replaced_any.then_some((out, folded, markers_above))
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
    /// #581. The main agent's scope has to stay the bare session id, byte for
    /// byte. Composing it differently still folds correctly inside one run,
    /// because both calls agree, so the hook-level tests cannot see a change
    /// here. What it would break is an upgrade: rows written by the previous
    /// binary are keyed on the bare id, and a new formula orphans them mid
    /// session and silently stops folding against them.
    #[test]
    fn the_main_agents_scope_is_the_session_id_unchanged() {
        assert_eq!(super::scope_for("sess-1", None), "sess-1");
        assert_eq!(super::scope_for("sess-1", Some("")), "sess-1");
        assert_eq!(
            super::scope_for("sess-1", Some("agent-9")),
            "sess-1/agent-9"
        );
    }

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

    /// #519. A re-run of an identical command folded into one run covering the
    /// whole payload, and `N lines already shown` reads as a claim about part of
    /// the output. A reader could not tell a fully elided reply from a command
    /// that printed nothing, and re-running is exactly how a fix gets verified.
    #[test]
    fn a_whole_payload_fold_says_it_is_identical() {
        let (store, _d) = temp_store();
        let text = payload();
        let ledger = Ledger::new(&store, "s1");

        ledger.project(&text);
        let second = ledger.project(&text).expect("a repeat is projectable");

        assert!(
            second.contains("identical to"),
            "a fully folded reply must state the identity: {second:?}"
        );
        assert_eq!(
            second.lines().count(),
            1,
            "one marker, not a marker plus commentary: {second:?}"
        );
    }

    /// The count in that marker is the reader's only check on it, so it has to be
    /// the payload's own line count. #519 shipped `43 lines already shown` beside
    /// `42 lines omitted` for a 43 line payload, and neither number was wrong on
    /// its own.
    #[test]
    fn the_whole_payload_marker_counts_the_whole_payload() {
        let (store, _d) = temp_store();
        let text = payload();
        let ledger = Ledger::new(&store, "s1");

        ledger.project(&text);
        let second = ledger.project(&text).expect("a repeat is projectable");

        assert!(
            second.contains(&format!("the {} lines", text.lines().count())),
            "expected {} lines in {second:?}",
            text.lines().count()
        );
    }

    /// The partial case keeps the run wording, because there it is true: some of
    /// this was shown before and the rest is new. Guarding the fix from becoming
    /// "every fold claims the output is identical".
    #[test]
    fn a_partial_fold_keeps_the_run_wording() {
        let (store, _d) = temp_store();
        let text = payload();
        let ledger = Ledger::new(&store, "s1");

        ledger.project(&text);
        let extended = format!("{text}\nsomething this session has never seen before\n");
        let second = ledger.project(&extended).expect("the repeated head folds");

        assert!(
            second.contains("lines already shown") && !second.contains("identical to"),
            "a partial fold must not claim identity: {second:?}"
        );
        assert!(
            second.contains("never seen before"),
            "the new line has to survive: {second:?}"
        );
    }

    /// #543 priced whole-output folds and `MIN_WHOLE_OUTPUT_FOLD` refuses them
    /// under 1 KB, but nothing recorded which folds were whole-output, so the
    /// floor could not be checked against the corpus that calibrated it. Both
    /// shapes are asserted in one test because the column is only useful if it
    /// separates them: a flag that is always 1 answers the query the same way as
    /// no flag at all.
    #[test]
    fn records_whether_a_fold_covered_the_whole_output() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("omni.db");
        let store = Store::open_path(&db).expect("store");
        let text = payload();
        let ledger = Ledger::new(&store, "s1");

        ledger.project(&text);
        let whole = ledger.project(&text).expect("a repeat is projectable");
        assert!(
            whole.contains("identical to"),
            "this arm has to be a whole-output fold: {whole:?}"
        );

        // One line this session has never seen, so the head folds and the tail
        // stays. Same ledger, so the only difference is the shape of the fold.
        let extended = format!("{text}\nsomething this session has never seen before\n");
        let partial = ledger.project(&extended).expect("the repeated head folds");
        assert!(
            !partial.contains("identical to"),
            "this arm has to be a partial fold: {partial:?}"
        );

        let conn = rusqlite::Connection::open(&db).expect("open");
        let rows: Vec<(i64, i64)> = conn
            .prepare("SELECT whole_output, payload_bytes FROM ledger_folds ORDER BY id")
            .expect("prepare")
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .expect("query")
            .collect::<Result<_, _>>()
            .expect("rows");

        assert_eq!(
            rows.iter().map(|r| r.0).collect::<Vec<_>>(),
            vec![1, 0],
            "a whole-output fold then a partial one; \
             without the split the floor stays unverifiable"
        );
        // Both calls land in the same second, so a `GROUP BY ts` audit would add
        // these two payloads together and compare the total with the floor. The
        // size has to be readable per row for the audit to mean anything.
        assert_eq!(
            rows[0].1,
            text.len() as i64,
            "the whole-output row must carry its own payload size"
        );
        assert_eq!(
            rows[1].1,
            extended.len() as i64,
            "each call carries its own size, not the pair's total"
        );
    }

    /// Six lines, 341 bytes. Above `MIN_LEDGER_INPUT` and above a whole-output
    /// marker plus the session gain, so without the #543 floor this folds
    /// entirely; below `MIN_WHOLE_OUTPUT_FOLD`, so with it the run stays verbatim.
    /// Sized against all three constants because a fixture that clears only one of
    /// them tests a different branch than the one named here.
    fn under_the_whole_output_floor() -> String {
        (0..6)
            .map(|i| format!("2026-08-10T00:00:{i:02}Z  handler finished request {i} in 12ms"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// #543. A whole-output fold leaves the agent a handle and no content, so any
    /// part of the payload it still needs costs a retrieval round trip. Four of
    /// the four recorded under 1 KB were retrieved within nine seconds, against a
    /// 0.85% retrieve rate across the store, so the fold spent four extra tool
    /// calls to save 2,680 bytes it then handed back.
    ///
    /// The partial arm is asserted in the same test on purpose: the floor must not
    /// become "no small payload ever folds", which would cost the runs that are
    /// still profitable because the agent keeps context beside them.
    #[test]
    fn never_folds_a_whole_output_that_cannot_pay_for_the_round_trip() {
        let (store, _d) = temp_store();
        let text = under_the_whole_output_floor();
        assert!(
            text.len() > MIN_LEDGER_INPUT && text.len() < MIN_WHOLE_OUTPUT_FOLD,
            "fixture must sit between the two floors, got {} bytes",
            text.len()
        );
        let ledger = Ledger::new(&store, "s1");

        ledger.project(&text);
        assert_eq!(
            ledger.project(&text),
            None,
            "a sub-1 KB repeat must stay verbatim rather than become a bare handle"
        );

        let extended = format!("{text}\nsomething this session has never seen before\n");
        let partial = ledger
            .project(&extended)
            .expect("the repeated head still folds beside content the agent can use");
        assert!(
            partial.contains("lines already shown") && partial.contains("never seen before"),
            "the floor must not reach partial folds: {partial:?}"
        );
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

    /// #509. The project scope is one history per directory, so the column is
    /// the only thing that can say whose lines it holds. `INSERT OR IGNORE`
    /// keeps the first writer, which is what makes "a different agent was shown
    /// this" answerable at all: overwriting on a repeat would report the last
    /// reader as the one who saw it.
    #[test]
    fn records_which_agent_was_shown_each_line() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("omni.db");
        let store = Store::open_path(&db).expect("store");
        // Above `MIN_LEDGER_INPUT` and below the project scope's fold bar, so
        // the second agent is *shown* every line rather than handed a marker.
        // Sized against the constants because the obvious fixture, the same
        // 200-line payload the floor test uses, folds the whole run: the second
        // agent then delivers nothing, writes nothing, and the test passes with
        // `INSERT OR REPLACE` in place of `INSERT OR IGNORE`.
        let text: String = (0..6)
            .map(|i| format!("2026-08-10T00:00:00Z  handler finished request {i} in 12ms\n"))
            .collect();
        assert!(
            text.len() > MIN_LEDGER_INPUT && text.len() < MIN_LEDGER_RUN_GAIN * PROJECT_FLOOR_MULT,
            "fixture sits on the wrong side of a threshold: {} bytes",
            text.len()
        );

        Ledger::new(&store, "s1")
            .with_project("/repo")
            .by("claude_code")
            .project(&text);
        Ledger::new(&store, "s2")
            .with_project("/repo")
            .by("codex")
            .project(&text);

        let conn = rusqlite::Connection::open(&db).expect("open");
        let rows = |agent: &str| -> i64 {
            conn.query_row(
                "SELECT COUNT(*) FROM ledger_lines WHERE scope = '/repo' AND agent_id = ?1",
                [agent],
                |r| r.get(0),
            )
            .expect("count")
        };

        assert!(
            rows("claude_code") > 0,
            "the agent the lines were delivered to was not recorded"
        );
        assert_eq!(
            rows("codex"),
            0,
            "a repeat rewrote the agent that was actually shown the line"
        );
    }

    /// #533. Whether the project scope stays shared across agents or becomes
    /// `(repo, agent)` cannot be argued, only measured, and it could not be
    /// measured: `ledger_lines` says a line was *seen*, never that a marker was
    /// issued against it or whose bytes that marker replaced. This is the row
    /// the corpus query needs, and the cross-agent case is the only one that
    /// decides anything, so it is the one pinned here.
    #[test]
    fn records_the_agent_whose_lines_a_project_fold_replaced() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("omni.db");
        let store = Store::open_path(&db).expect("store");
        // Six times the session floor, so the project scope's higher bar is
        // cleared and a fold really happens. Under it the second agent is simply
        // shown the lines, and a test asserting on an absent fold passes for the
        // wrong reason.
        let text: String = (0..200)
            .map(|i| format!("2026-08-10T00:00:00Z  handler finished request {i} in 12ms\n"))
            .collect();

        Ledger::new(&store, "s1")
            .with_project("/repo")
            .by("claude_code")
            .project(&text);
        let view = Ledger::new(&store, "s2")
            .with_project("/repo")
            .by("codex")
            .project(&text)
            .expect("no fold on the second agent, so there is nothing to record");
        assert!(
            view.contains("from an earlier session"),
            "this has to be a project fold to be the case under test: {view}"
        );

        let conn = rusqlite::Connection::open(&db).expect("open");
        let (agent, source, lines, bytes): (String, String, i64, i64) = conn
            .query_row(
                "SELECT agent_id, source_agent, lines, bytes
                 FROM ledger_folds WHERE origin = 'project'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .expect("a project fold left no row, so cross-agent reuse stays unpriceable");

        assert_eq!(agent, "codex", "the agent the marker was delivered to");
        assert_eq!(
            source, "claude_code",
            "the agent whose lines were replaced, which is the whole measurement"
        );
        assert!(lines > 0 && bytes > 0, "{lines} lines, {bytes} bytes");
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

    /// #567. The run marker said `from an earlier session`, which states where the
    /// lines came from and reads as "you have seen this". It means the opposite:
    /// the project scope only answers for lines the session scope did not, so
    /// those lines were never delivered here.
    ///
    /// A reader acted on it, took a help page missing its `Commands:` block as a
    /// complete one, and concluded the CLI had no uninstall. The run form had no
    /// test of its wording at all: every existing assertion on the phrase is
    /// satisfied by the whole-output marker, so the string could be changed with
    /// the suite green.
    #[test]
    fn a_project_run_marker_says_the_lines_were_not_shown_here() {
        let (store, _d) = temp_store();
        // Repeated lines first, then lines nobody has seen, so the fold is a run
        // inside a payload rather than the whole of it. That is the shape the
        // report hit and the one the whole-output marker never covers.
        let repeated: String = (0..60)
            .map(|i| format!("2026-08-10T00:00:00Z  handler finished request {i} in 12ms\n"))
            .collect();
        let fresh: String = (0..60)
            .map(|i| format!("2026-08-10T00:00:00Z  cache probe {i} missed and refilled\n"))
            .collect();

        Ledger::new(&store, "s1")
            .with_project("/repo")
            .project(&repeated);

        let view = Ledger::new(&store, "s2")
            .with_project("/repo")
            .project(&format!("{repeated}{fresh}"))
            .expect("a project repeat above the floor is projectable");

        assert!(
            view.contains("not shown here"),
            "the marker did not say the lines were never delivered here: {view}"
        );
        assert!(
            !view.contains("already shown"),
            "a project run claimed a sighting this session never had: {view}"
        );
        assert!(
            view.contains(&fresh),
            "this has to be a run inside a payload, not a whole-output fold: {view}"
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

        // One line neither history has seen, so both folds below are runs rather
        // than whole outputs. Without it the #543 floor decides these cases before
        // either bar is consulted, and the test stops being about the two bars.
        // It also makes the bars above exact: a run marker is what gets rendered
        // here, where a whole-output fold would have rendered the longer wording.
        let probe = format!("{text}a line neither history has ever seen\n");

        // Same session: over the session floor, so it projects.
        assert!(Ledger::new(&store, "s1").project(&probe).is_some());
        // New session: only the project history has it, and it is too small.
        assert_eq!(
            Ledger::new(&store, "s2")
                .with_project("/repo")
                .project(&probe),
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

    /// One fixed-width line, so a fixture's size can be reasoned about against
    /// `MIN_WHOLE_OUTPUT_FOLD` rather than eyeballed. 34 bytes including the
    /// terminator.
    fn row(tag: char, i: usize) -> String {
        format!("{tag} line {i:04} of the fixture payload\n")
    }

    /// #601, as a matrix, because the guard has to hold on one axis and stay out
    /// of the way on the other two. A single fixture proves whichever it happens
    /// to sit on, and the danger here is a guard that quietly stops the folds
    /// this feature exists for.
    ///
    /// Coverage is what the fold takes of the payload; the floor only applies
    /// once that is four fifths or more **and** the run is under
    /// `MIN_WHOLE_OUTPUT_FOLD`.
    #[test]
    fn refuses_a_small_fold_that_would_leave_almost_nothing() {
        // (name, lines reused from the first show, lines new to the second, folds?)
        let cases = [
            // The reported shape: 23 of 26 lines folded, 782 B of an 884 B
            // payload, and the three survivors say nothing about the edit.
            ("small payload, 88% covered", 23, 3, false),
            // Same payload size, a third of it folded: the reader keeps enough
            // to work from, so the trade is the one the ledger is for.
            ("small payload, 38% covered", 10, 16, true),
            // Same coverage as the first row on a payload past the floor. The
            // guard must not reach this: it is the case the feature exists for.
            ("large payload, 92% covered", 55, 5, true),
        ];

        for (name, reused, fresh, should_fold) in cases {
            let (store, _d) = temp_store();
            let ledger = Ledger::new(&store, "s1");

            let first: String = (0..reused).map(|i| row('s', i)).collect();
            let seed = format!("{first}{}", (0..40).map(|i| row('p', i)).collect::<String>());
            assert!(seed.len() > MIN_LEDGER_INPUT, "{name}: seed too small to record");
            ledger.project(&seed);

            let second: String = (0..reused)
                .map(|i| row('s', i))
                .chain((0..fresh).map(|i| row('n', i)))
                .collect();
            let folded_bytes = reused * 34;
            assert_eq!(
                folded_bytes >= MIN_WHOLE_OUTPUT_FOLD,
                should_fold && name.starts_with("large"),
                "{name}: fixture is on the wrong side of the whole-output floor"
            );

            assert_eq!(
                ledger.project(&second).is_some(),
                should_fold,
                "case: {name}"
            );
        }
    }
}
