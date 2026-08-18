//! Shell-line decomposition: which single command produced the stdout in hand.
//!
//! Lifted out of `registry.rs` per section 5.4 of the direction spec, which asked
//! for "shell-line decomposition to the segment that produced stdout. Single
//! responsibility, unit-testable without a DB". `registry.rs` keeps distiller and
//! profile selection and loses command parsing; nothing here knows what a
//! distiller is.
//!
//! **The behaviour is unchanged and that is the point.** The spec's P2 also
//! proposed a *new* rule, "decompose and take the last segment", and this module
//! deliberately does not implement it: `owning_tail` below records the
//! measurement over 4,958 recorded pipelines that killed it, where routing by the
//! last stage regardless would hand 69.1% of them to `head`, `tail` or `grep`.
//! The move is a refactor; the replay must produce identical numbers, and if it
//! does not, this module is wrong.

/// says nothing about who produced the output being distilled.
///
/// Deliberately short. Every name here is silent by definition, not merely quiet
/// in the common case: `mkdir`, `cp` and `rm` all print under `-v`, and putting
/// them here would let a chain be routed to a single distiller again. Leaving a
/// producer out costs a passthrough; letting one in costs the answer.
const SILENT_BUILTINS: &[&str] = &[
    "cd", "export", "set", "unset", "source", ".", "true", "false", "pushd", "popd", "umask",
    "alias", "shift", "local", "readonly", "exit", "return", "break", "continue", "wait", "trap",
    "shopt", "declare", "typeset", "let", "[", "[[", "test", ":",
    // Control flow. `while [ $i -lt 60 ]; do echo x; i=$((i+1)); done` is one
    // program whose stdout comes from `echo`, but the `;` between its clauses is
    // the same character that separates two commands. Reading the clauses as
    // producers made every shell loop a passthrough, which is what
    // `exec_fail_passthrough` caught on CI.
    "done", "fi", "esac", "then", "else",
];

/// Keywords that introduce a clause rather than end one, so the command after
/// them is what may write to stdout.
const CLAUSE_PREFIXES: &[&str] = &["do", "then", "else", "elif", "while", "until", "if"];

/// Keywords that open a loop or branch header. What follows is a variable name
/// and a word list, not a command: `for f in *.yaml` printed nothing, but the
/// `f` after the keyword reads as an executable to anything scanning for one.
const HEADER_KEYWORDS: &[&str] = &["for", "select", "case"];

/// The one command in `command` whose stdout is being distilled, or `None` when
/// several produced it.
///
/// `distill_with_command` reads the first executable of the command string and
/// hands that distiller the whole of stdout. On a chain the rest of the output
/// belongs to other programs: `git status && echo === && find .` came back as
/// `git: on branch main | staged:0 mod:0 untracked:0`, so the 40 lines of `find`
/// that the command was run for were deleted with no marker, no count and no
/// rewind hash, and the ratio read as a 99% win on the bytes that held the answer
/// (#264). `git status` is the worst case only because its distiller emits a
/// fixed one-liner whatever the input, leaving no residue to notice.
///
/// Splitting stdout back onto the chain is not possible: it is one stream with
/// nothing marking which program wrote which line. So the rule is the honest one.
/// One producer, route it. More than one, the caller passes the output through
/// untouched.
///
/// A pipeline resolves to its first stage, with one exception. Most filters
/// preserve the shape of what they are fed, so `kubectl get pods | head -20` is
/// still a pod table and still belongs to `kubectl`. `jq` and `yq` do not: they
/// rewrite the payload into something of their own, so the output is theirs.
/// Routing it upstream is how `kubectl get pod -o json | jq -r '...'` reached the
/// cloud distiller, which kept one arbitrary row of four and dropped the three
/// that carried the answer (#269).
///
/// A `grep` tail claims the payload for a second, unrelated reason. See
/// `FILTERING_TAILS`.
pub fn sole_output_command(command: &str) -> Option<&str> {
    let segments = split_sequential(command);
    let producer = match segments.len() {
        0 => return None,
        1 => segments[0],
        _ => {
            let mut producers = segments.into_iter().filter(|seg| !is_silent(seg));
            let first = producers.next()?;
            producers.next().is_none().then_some(first)?
        }
    };
    let producer = strip_assignments(producer);
    Some(owning_tail(producer).unwrap_or(producer))
}

/// The trailing pipeline stage when it rewrites the payload rather than
/// selecting from it, so the output stops belonging to whatever fed it.
///
/// **A list of names, and that is the answer #277 asked for.** That issue
/// proposed classifying every stage as selector or transformer and routing to
/// the last transformer. Measured over **4,958 recorded pipelines**, the general
/// rule is worse than the narrow one at both ends:
///
/// * Routing by the last stage regardless hands **69.1%** of them to `head`,
///   `tail` or `grep`, all verbatim passthroughs, and stops distilling two
///   thirds of every pipeline anyone runs.
/// * Only **7.5%** end in a stage that reshapes at all, and of the residual the
///   dominant first stages are `cd` (119), `echo` (32) and `for` (23), which
///   have no distiller to claim the payload in the first place. The pipelines
///   genuinely at risk, a real distiller upstream of a real reshaper, are about
///   **1.3%**.
///
/// So the shape stays a name list; what changes is that it is now the measured
/// list rather than the two names #269 needed. Every entry provably emits
/// something that is not its input's grammar: `cut` and `awk` project columns,
/// `tr` and `base64` rewrite bytes, `wc` and `column` replace the payload with a
/// count or a layout, `xargs` runs a different program entirely.
///
/// `sed` and `sort` are deliberately **absent**. `sed 's/x/y/'` and `sort` leave
/// the shape intact and are 334 of the recorded tails between them; treating
/// them as reshapers would stop distilling a pod table because someone sorted
/// it.
fn owning_tail(segment: &str) -> Option<&str> {
    let last = split_pipeline(segment).pop()?;
    let base = last
        .split_whitespace()
        .next()
        .map(|w| w.trim_matches(|c| c == '"' || c == '\''))?;
    (RESHAPING_TAILS.contains(&base) || FILTERING_TAILS.contains(&base)).then_some(last)
}

/// The trailing stage when the caller's own pattern produced the result set, so
/// every line in it was asked for by name.
///
/// A different reason from `RESHAPING_TAILS` reaching the same answer: `grep`
/// emits its input's grammar unchanged, so nothing about the *shape* says the
/// payload changed hands. What changed is that a filter already ran. Scoring the
/// result by noise is a second filter that cannot know what the first was looking
/// for, and it drops the lines the pattern was written to find.
///
/// #316 established that for a bare `grep` and fixed it inside `system_ops`,
/// where the payload arrives without its command. So a `grep` on the end of a
/// pipeline never reached the rule: `kubectl logs … | grep -iE 'error|ready'` is
/// routed by `kubectl`, and `distill_kubectl_generic` keeps `is_critical` lines
/// only, so 14 of 15 matched lines went. The one that survived was an `ERROR`,
/// which made the delivered answer say the pod had failed to start while the
/// dropped lines said `3/3 MCP servers connected` and `Bolt app is running!`
/// (#326).
///
/// Measured on 3,392 recorded traces before choosing the rule: 429 pipelines put
/// a `grep` after a pipe, and the ones with a real distiller upstream are where
/// it costs. `kubectl … | grep` keeps 76.8% of its bytes on average, with
/// individual rows at 5% (1,793 → 95 bytes on a `get pods -A`). What the rule
/// gives up is those same reductions, which is the point: they were produced by
/// deleting matched lines.
///
/// `ag` rides along because it is the same contract; it has no recorded traces
/// here, so it is included on grammar rather than on measurement.
const FILTERING_TAILS: &[&str] = &["grep", "rg", "ag"];

/// The stage names `owning_tail` recognises, in one place because two callers
/// need the same answer and a comment is not a mechanism.
///
/// `distillers::passes_through_verbatim` has to agree with this list: naming a
/// tail as the payload's owner only helps if that tail is then handled, and none
/// of these has a grammar to distil. #277 added seven names to both lists in two
/// files and the only thing keeping them in step was a note telling the next
/// person to keep them in step. That is the duplication #194 is about, so the
/// half of it with a demonstrated cost is a shared constant now.
///
/// There is deliberately no test walking this list against
/// `passes_through_verbatim`. One was written and it could not fail: once both
/// sides read the same constant, asserting that every member of the list is in
/// the list proves nothing. The constant *is* the mechanism, which is the point
/// of removing the comment that used to be.
/// `uniq` is here for `uniq -c`, which prepends a count column and so emits a
/// grammar its input did not have: `kubectl logs … | awk … | sort | uniq -c` is a
/// histogram, and routing it to `kubectl` handed an already-aggregated 40-row
/// answer to the pod-table distiller, which delivered 10 rows and dropped the two
/// spikes the query existed to find (#338). Bare `uniq` only dedupes and by the
/// reasoning above belongs with `sort` rather than here, but splitting the two
/// costs an argument check for no measured gain: of 4,335 recorded pipelines, 55
/// end in `uniq -c` and **none** end in a bare `uniq`. A deduped list is an
/// enumeration anyway, which is the shape `passes_through_verbatim` already
/// protects.
pub const RESHAPING_TAILS: &[&str] = &[
    "jq", "yq", "cut", "tr", "awk", "base64", "wc", "column", "xargs", "uniq",
];

/// Splits on unquoted single `|`, the pipe operator. `||` is a sequential
/// operator and `split_sequential` has already dealt with it.
fn split_pipeline(segment: &str) -> Vec<&str> {
    let bytes = segment.as_bytes();
    let mut stages = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    let mut quote: Option<u8> = None;

    while i < bytes.len() {
        let b = bytes[i];
        match quote {
            Some(q) => {
                if b == q {
                    quote = None;
                }
            }
            None => match b {
                b'\'' | b'"' | b'`' => quote = Some(b),
                b'\\' => i += 1,
                b'|' => {
                    push_segment(&mut stages, segment, start, i);
                    start = i + 1;
                }
                _ => {}
            },
        }
        i += 1;
    }
    push_segment(&mut stages, segment, start, bytes.len());
    stages
}

fn is_silent(segment: &str) -> bool {
    let mut words = segment
        .split_whitespace()
        .map(|w| w.trim_matches(|c| c == '"' || c == '\''));
    let Some(first) = words.next() else {
        return true;
    };
    if HEADER_KEYWORDS.contains(&first) {
        return true;
    }
    let mut rest = std::iter::once(first)
        .chain(words)
        .skip_while(|w| CLAUSE_PREFIXES.contains(w) || is_assignment(w));
    match rest.next() {
        // Every word was a clause prefix or an assignment, so nothing ran.
        None => true,
        Some(base) => SILENT_BUILTINS.contains(&base),
    }
}

/// The command with any leading `VAR=value` words removed, so every caller that
/// reads a head reads the program rather than the environment set for it.
///
/// `is_assignment` already existed but only decided whether a *whole* segment was
/// silent, and a single-segment command never reaches that branch. So
/// `OMNI_DB_PATH=/tmp/d.db kubectl get pods` resolved to `Generic` where the bare
/// command resolves to `Infra`, and `sole_output_command` handed back the string
/// with the assignment still on the front, which no TOML filter keyed on
/// `^kubectl\b` can match either (#339).
///
/// Measured before choosing the shape: env-prefixed commands are 1,082 of 9,812
/// recorded here and save 14.9% against 22.9% for the rest, so this is worth about
/// 112 KB over the whole corpus. Small, and a one-line strip rather than a parser
/// is what that buys.
pub(crate) fn strip_assignments(command: &str) -> &str {
    let mut rest = command.trim_start();
    while let Some(word) = rest.split_whitespace().next() {
        if !is_assignment(word) {
            break;
        }
        // `rest` is left-trimmed, so `word` is exactly its prefix. `strip_prefix`
        // rather than a range index because the crate denies `clippy::string_slice`
        // and this needs no proof of a char boundary to be correct.
        rest = rest.strip_prefix(word).unwrap_or(rest).trim_start();
    }
    rest
}

/// The program name to file a recorded row under, for any command string.
///
/// #339 taught `sole_output_command` to strip assignments and closed. It fixed
/// routing and left `distillations.filter_name` wrong in two places, because
/// neither writer of that column went through here:
///
/// * `hooks::pipe` took the first token of the raw command and nothing else, so
///   the exec and pipe door never received the fix at all.
/// * `hooks::post_tool` did call `sole_output_command`, but through
///   `.unwrap_or(command)`. That function answers `None` for any chain with two
///   producers, and the fallback then handed the raw chain back.
///
/// Measured on 0.7.6 before the change: 1,525 of 11,335 rows named an assignment
/// and 291 named a binary's full path, 16.0% of the corpus, against #339's own
/// 1,079 before it was closed. Every aggregate keyed on this column was wrong by
/// that much, including the workload numbers this repo sizes distillers from.
///
/// The file name, not the path, for the same reason `resolve_profile` takes it:
/// `/opt/homebrew/opt/python@3.11/bin/python3.11` and `python3.11` are one
/// program and must be one row.
pub(crate) fn producer_label(command: &str) -> &str {
    // The producer when there is a single one, and otherwise the first segment
    // that actually runs something. `sole_output_command` answers `None` for a
    // chain with two producers, and the raw string it was falling back to begins
    // with whatever `cd` or assignment stands in front, which is the half #339
    // missed. `is_silent` is the same predicate that decides which segments can
    // own the output, so the label agrees with the routing by construction.
    //
    // Every segment silent means nothing wrote to stdout, and the whole string
    // is then as good a name as any.
    let segment = sole_output_command(command)
        .or_else(|| {
            split_sequential(command)
                .into_iter()
                .find(|seg| !is_silent(seg))
        })
        .unwrap_or(command);
    program_name(strip_assignments(segment))
}

/// The bare program name of an already-stripped command.
///
/// Shared with `resolve_profile`, which decided routing on exactly this and had
/// its own copy. Two copies of one predicate drift and only one of them gets
/// reported, which is how the two halves of #339 came apart in the first place.
pub(crate) fn program_name(command: &str) -> &str {
    let first = command
        .split_whitespace()
        .next()
        .unwrap_or(command)
        .trim_matches(|c| c == '"' || c == '\'');
    std::path::Path::new(first)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(first)
}

/// `i=0` and `f=path/to.yaml` set a variable and print nothing. Distinguished
/// from a command by the `=` before any `/`, so `./bin/x=y` is still a command.
pub(crate) fn is_assignment(word: &str) -> bool {
    match word.split_once('=') {
        Some((name, _)) => {
            !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        }
        None => false,
    }
}

/// Splits on unquoted `&&`, `||`, `;` and newlines, the operators that run
/// commands one after another so each one can write to stdout.
///
/// Quote tracking is what stops `echo "a && b"` from reading as two commands. It
/// is deliberately one-directional: an unbalanced quote leaves the scanner inside
/// a string and yields one segment, which routes as it does today rather than
/// inventing a split.
fn split_sequential(command: &str) -> Vec<&str> {
    let bytes = command.as_bytes();
    let mut segments = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    let mut quote: Option<u8> = None;

    while i < bytes.len() {
        let b = bytes[i];
        match quote {
            Some(q) => {
                if b == q {
                    quote = None;
                } else if b == b'\\' && q == b'"' {
                    i += 1;
                }
            }
            None => match b {
                b'\'' | b'"' | b'`' => quote = Some(b),
                b'\\' => i += 1,
                b'\n' | b';' => {
                    push_segment(&mut segments, command, start, i);
                    start = i + 1;
                }
                b'&' | b'|' if i + 1 < bytes.len() && bytes[i + 1] == b => {
                    push_segment(&mut segments, command, start, i);
                    i += 1;
                    start = i + 1;
                }
                _ => {}
            },
        }
        i += 1;
    }
    push_segment(&mut segments, command, start, bytes.len());
    segments
}

// Safety: `start` and `end` only ever come from positions of `;`, `\n`, `&`, `|`
// or the string's own length. Those are ASCII, and every byte inside a multi-byte
// UTF-8 sequence is >= 0x80, so none of them can match. The escape skip can leave
// the cursor mid-character, but a continuation byte matches no separator either,
// so the recorded bounds stay on char boundaries.
#[allow(clippy::string_slice)]
fn push_segment<'a>(out: &mut Vec<&'a str>, command: &'a str, start: usize, end: usize) {
    let seg = command[start..end].trim();
    if !seg.is_empty() {
        out.push(seg);
    }
}

#[cfg(test)]
mod producer_label_tests {
    use super::producer_label;

    /// #603, as a matrix rather than one case, because the two writers of
    /// `filter_name` failed on different shapes: the exec door failed on a bare
    /// assignment and the hook door only on a chain with two producers. A single
    /// fixture passes on one door and proves nothing about the other.
    ///
    /// Every row is a command shape that really occurs in the recorded corpus.
    #[test]
    fn labels_a_command_with_its_program() {
        let cases = [
            ("bare", "echo hi", "echo"),
            ("assignment", "FOO=bar echo hi", "echo"),
            ("two assignments", "A=1 B=2 echo hi", "echo"),
            // The shape the hook door got wrong: `sole_output_command` answers
            // `None` here, and the old fallback took the raw first token.
            (
                "assignment then chain",
                "FOO=bar echo one && echo two",
                "echo",
            ),
            ("chain", "echo one && echo two", "echo"),
            ("cd prefix", "cd /tmp && kubectl get pods", "kubectl"),
            // Two producers, so `sole_output_command` declines and the fallback
            // decides. It has to skip the `cd`, or the row is filed under the
            // one segment that produced no output, which is #339's other half.
            (
                "cd prefix and two producers",
                "cd /tmp && kubectl get pods && kubectl get svc",
                "kubectl",
            ),
            (
                "assignment, cd, then two producers",
                "K=1 cd /tmp && echo one && echo two",
                "echo",
            ),
            // 291 rows named a binary's full path before this.
            (
                "absolute path",
                "/opt/homebrew/bin/python3.11 x.py",
                "python3.11",
            ),
            (
                "assignment and absolute path",
                "S=/tmp/scratch /usr/bin/env node app.js",
                "env",
            ),
            ("quoted program", "\"kubectl\" get pods", "kubectl"),
            // ponytail: the split is on whitespace before the quotes come off,
            // so a program name containing a space keeps only its first word.
            // Inherited from `resolve_profile`, which has routed this way since
            // it was written, and no recorded command has that shape. A quote
            // aware split is the upgrade if one ever does.
            ("quoted program with a space", "\"my prog\" --flag", "my"),
            // Pipe mode has no command at all, and the caller turns this into
            // `[pipe]`. Anything else here would invent a program name.
            ("empty", "", ""),
            ("assignment only", "FOO=bar", ""),
        ];

        for (name, command, expected) in cases {
            assert_eq!(producer_label(command), expected, "case: {name}");
        }
    }
}
