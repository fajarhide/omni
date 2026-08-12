// Safety: All string indexing uses positions from find()/rfind() on ASCII
// delimiters (':', '=', '.', '_') which always return valid char boundaries.
#![allow(clippy::string_slice)]

use crate::distillers::Distiller;
use crate::pipeline::{OutputSegment, SignalTier};

pub struct SystemOpsDistiller;

impl Distiller for SystemOpsDistiller {
    fn distill(
        &self,
        segments: &[OutputSegment],
        input: &str,
        _session: Option<&crate::pipeline::SessionState>,
    ) -> Option<String> {
        let lines: Vec<&str> = input.lines().collect();
        if lines.is_empty() {
            return None;
        }

        // Dispatch based on content analysis
        Some(if is_env_output(&lines) {
            distill_env_output(input)
        } else if is_ls_output(&lines) {
            distill_ls_output(input)
        } else if is_tree_output(&lines) {
            distill_tree_output(input)
        } else if is_find_output(&lines) {
            distill_find_output(input)
        } else if is_grep_output(&lines) {
            distill_grep_output(input)
        } else {
            distill_fallback(segments)
        })
    }
}

// ---------------------------------------------------------------------------
// Sensitive patterns for env redaction (Gate 6: Security)
// ---------------------------------------------------------------------------

const SENSITIVE_PATTERNS: &[&str] = &[
    "SECRET",
    "TOKEN",
    "KEY",
    "PASSWORD",
    "PASS",
    "AUTH",
    // `CRED` was a stem, and a stem only works under substring matching. Segment
    // matching needs the words themselves (#408).
    "CREDS",
    "CREDENTIAL",
    "CREDENTIALS",
    "API_",
    "AWS_",
    "GITHUB_",
    "ANTHROPIC_",
    "DATABASE_URL",
    "REDIS_URL",
    "MONGO_URL",
    "CLIENT_SECRET",
    "ACCESS_KEY",
    "OPENAI_",
    "GEMINI_",
    "PRIVATE_KEY",
];

// ---------------------------------------------------------------------------
// Detection helpers
// ---------------------------------------------------------------------------

/// A `grep -n` over a **single** file: every line is `<lineno>:<text>` with no
/// path, because grep only prefixes the filename when it was given more than one.
///
/// Worth its own arm because the payload that missed it was not distilled by
/// `distill_grep_output` at all: it fell through to the generic system-ops
/// fallback, which keeps Critical and Important segments and drops the rest, and
/// dropped 7 of 12 matches including `expr: absent(probe_success{tier="carrier"} == 1)`
/// and the alert summary, while four lines of runbook prose survived (#316).
/// A grep pattern *is* the caller's filter, so scoring its results by noise is a
/// second filter that cannot know what the first was looking for.
///
/// Misrouting into the grep distiller is harmless, which is what makes widening
/// the detector the safe direction: `distill_grep_output` hoists losslessly and
/// hands back the input whenever hoisting does not shrink it.
fn is_numbered_single_file_grep(lines: &[&str]) -> bool {
    let mut numbered = 0usize;
    for l in lines {
        let t = l.trim_end();
        if t.is_empty() {
            continue;
        }
        let Some((head, _)) = t.split_once(':') else {
            return false;
        };
        if head.is_empty() || !head.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
        numbered += 1;
    }
    numbered >= 3
}

fn is_grep_output(lines: &[&str]) -> bool {
    if is_numbered_single_file_grep(lines) {
        return true;
    }
    // grep/ripgrep: lines with "filepath:content" or "filepath:linenum:content"
    // Exclude lines that look like error output
    let grep_count = lines
        .iter()
        .filter(|l| {
            let l = l.trim();
            if l.is_empty() {
                return false;
            }
            // Must have a colon and NOT be a key=value pair
            if let Some(pos) = l.find(':') {
                // The part before the colon should look like a file path
                let before = &l[..pos];
                // Must not start with uppercase_key=value (that's env)
                !before.contains('=')
                    && !before.is_empty()
                    && (before.contains('/') || before.contains('.') || before.contains('\\'))
            } else {
                false
            }
        })
        .count();
    grep_count >= 3
}

fn is_ls_output(lines: &[&str]) -> bool {
    // ls -la: first line starts with "total N"
    let first = lines.first().map(|l| l.trim()).unwrap_or("");
    if first.starts_with("total ") {
        // Additional check: lines starting with permission string (drwx, -rw-, lrwx)
        let perm_count = lines
            .iter()
            .skip(1)
            .filter(|l| {
                let t = l.trim();
                t.starts_with("drwx")
                    || t.starts_with("-rw")
                    || t.starts_with("lrwx")
                    || t.starts_with("d---")
                    || t.starts_with("----")
                    || t.starts_with("drw-")
                    || t.starts_with("-r-")
                    || t.starts_with("-r--")
            })
            .count();
        perm_count >= 1
    } else {
        false
    }
}

fn is_find_output(lines: &[&str]) -> bool {
    // find: 3+ lines starting with "./" or "/"
    let count = lines
        .iter()
        .filter(|l| {
            let t = l.trim();
            t.starts_with("./") || (t.starts_with('/') && !t.contains(':'))
        })
        .count();
    count >= 3
}

/// A fingerprint must be a token no sibling format also prints. Box-drawing
/// connectors fail that test: `tree` prints them, and so does every document
/// that describes a file layout, 25 markdown files in this repository alone,
/// `CONTRIBUTING.md` among them. Matching on `any` connector meant one such line
/// classified the whole payload, and `distill_tree_output` replaced a 127-line
/// prose guide with `tree: 127 entries` (#236). The `directories`/`files` half
/// was looser still: it fired on a *comment* in this very file that names both
/// words, which is how a 3 KB source listing came back as one line.
///
/// What `tree` prints that a document quoting one does not is the closing
/// report, `N directories, M files`, on its own last line. Without it
/// (`tree --noreport`) the connectors have to be the shape of the output rather
/// than a passage inside it.
fn is_tree_output(lines: &[&str]) -> bool {
    let has_report = lines.iter().rev().take(3).any(|l| {
        let t = l.trim();
        t.starts_with(|c: char| c.is_ascii_digit()) && t.contains("director") && t.contains("file")
    });
    if has_report {
        return true;
    }

    let connectors = lines
        .iter()
        .filter(|l| l.contains("├── ") || l.contains("└── "))
        .count();
    let non_empty = lines.iter().filter(|l| !l.trim().is_empty()).count();
    connectors >= 3 && connectors * 2 > non_empty
}

fn is_env_output(lines: &[&str]) -> bool {
    // env: 5+ lines of "UPPERCASE_KEY=value"
    let count = lines
        .iter()
        .filter(|l| {
            let t = l.trim();
            if let Some(pos) = t.find('=') {
                let key = &t[..pos];
                !key.is_empty()
                    && key
                        .chars()
                        .next()
                        .map(|c| c.is_uppercase())
                        .unwrap_or(false)
                    && key.chars().all(|c| c.is_alphanumeric() || c == '_')
            } else {
                false
            }
        })
        .count();
    count >= 5
}

// ---------------------------------------------------------------------------
// Grep/Ripgrep distiller
// ---------------------------------------------------------------------------

/// `path:line:content` (or `path:content`), split at the first colon, the same
/// boundary `is_grep_output` keys off.
fn split_grep_line(line: &str) -> Option<(&str, &str)> {
    let (path, rest) = line.split_once(':')?;
    (!path.is_empty() && (path.contains('/') || path.contains('.'))).then_some((path, rest))
}

/// grep repeats the full path on every match line, so a file with 12 matches
/// pays for its path 12 times, that repetition is the noise, and it lives
/// between the lines rather than inside them. Hoist each path to a header and
/// indent its matches under it.
///
/// Every match survives: `header + ':' + indented line` reconstructs the input
/// exactly. The match text is the whole point of grep, summarising it to
/// `foo.rs: 12 matches` (what this used to emit) answers a question nobody
/// asked and forces the agent to grep again.
///
/// Close the run of match lines accumulated so far, writing its count directly
/// above the rows it counts.
///
/// The count used to be global and printed once at position 0, which made it a
/// claim about output it had never seen. One Bash call holding two greps, the
/// first matching nothing, was delivered as `grep: 20 matches in 10 files` above
/// a `---sep---`, so a search that found **nothing** read as one that found
/// twenty hits in ten files, and the reporter acted on it while asking whether a
/// Cloudflare API token was on disk (#247). A non-match line is already kept
/// verbatim, so the boundary was visible in the output and simply not consulted.
/// Same invariant #227 established for `collapse`: no summary may span a
/// surviving line that separates the rows it counts.
fn flush_grep_section(
    out: &mut String,
    section: &mut String,
    matches: &mut usize,
    files: &mut usize,
) {
    if *matches > 0 {
        out.push_str(&format!("grep: {} matches in {} files\n", matches, files));
        out.push_str(section);
    }
    section.clear();
    *matches = 0;
    *files = 0;
}

/// The result set of a `grep`/`rg`/`ag`, which the caller's pattern already
/// filtered.
///
/// Separate from `SystemOpsDistiller::distill` because that one dispatches on the
/// shape of the payload, and a grep result carries whatever shape the file had.
/// A `kubectl logs … | grep -iE 'error|ready'` is bare log text, so every
/// detector missed and it landed in `distill_fallback`, which drops by noise
/// score (#326). Here the command is known, so shape does not have to be guessed
/// at: the grep path hoists repeated paths and returns the input whenever
/// hoisting does not shrink it, and neither outcome can lose a matched line.
///
/// Env output keeps its own arm. `distill_env_output` redacts values against
/// `SENSITIVE_PATTERNS`, which is a security gate rather than a compression one,
/// and `grep -i secret .env` is exactly the shape that needs it.
pub fn distill_user_filtered(input: &str) -> String {
    let lines: Vec<&str> = input.lines().collect();
    if is_env_output(&lines) {
        return distill_env_output(input);
    }
    distill_grep_output(input)
}

fn distill_grep_output(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut section = String::with_capacity(input.len());
    let mut current: Option<&str> = None;
    let (mut matches, mut files) = (0usize, 0usize);
    let mut parsed_any = false;

    for line in input.lines() {
        let Some((path, rest)) = split_grep_line(line.trim_end()) else {
            // Not a match line (grep's own warnings, a shell `echo`, blank
            // separators). It ends the run, so the count lands above its own
            // rows rather than above whatever follows.
            flush_grep_section(&mut out, &mut section, &mut matches, &mut files);
            current = None;
            out.push_str(line);
            out.push('\n');
            continue;
        };
        parsed_any = true;
        if current != Some(path) {
            section.push_str(path);
            section.push('\n');
            current = Some(path);
            files += 1;
        }
        section.push_str("  ");
        section.push_str(rest);
        section.push('\n');
        matches += 1;
    }
    flush_grep_section(&mut out, &mut section, &mut matches, &mut files);

    if !parsed_any {
        // Nothing in this payload parsed as a match line, so there is no grep
        // result to report. This used to return the string `grep: no matches`,
        // which discarded the body and asserted an outcome for a command whose
        // output was never recognised, the `require_parsed` prohibition. It is
        // near-unreachable, because `is_grep_output` routes here only once three
        // lines already parse, but a latent false claim is not worth keeping for
        // the two lines it saves (#247).
        return input.to_string();
    }

    // Hoisting costs a header line per file; on output that is one match per file
    // it can lose. Never hand back something longer than we were given.
    if out.len() < input.len() {
        out
    } else {
        input.to_string()
    }
}

// ---------------------------------------------------------------------------
// ls -la distiller
// ---------------------------------------------------------------------------

fn distill_ls_output(input: &str) -> String {
    let mut files = 0u32;
    let mut dirs = 0u32;
    let mut links = 0u32;
    let mut total = 0u32;
    let mut newest_file: Option<String> = None;

    for line in input.lines().skip(1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        total += 1;

        if trimmed.starts_with('d') {
            dirs += 1;
        } else if trimmed.starts_with('l') {
            links += 1;
        } else if trimmed.starts_with('-') {
            files += 1;
        }

        // Track the last file listed (which is typically the newest in sorted output)
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() >= 9 {
            // Last column(s) = filename - may include spaces if quoted
            let filename = parts[8..].join(" ");
            if !filename.starts_with('.') || filename.len() > 1 {
                newest_file = Some(filename);
            }
        }
    }

    let mut out = format!(
        "ls: {} items | {} files, {} dirs, {} links",
        total, files, dirs, links
    );

    if let Some(ref name) = newest_file {
        out.push_str(&format!(" | last: {}", name));
    }

    out
}

// ---------------------------------------------------------------------------
// find distiller
// ---------------------------------------------------------------------------

/// Longest directory prefix shared by every path, cut at a `/` so the remainder
/// stays a valid relative path, a prefix ending mid-filename would not round-trip.
/// Empty when the paths share no directory.
fn common_dir_prefix(paths: &[&str]) -> String {
    let Some(first) = paths.first() else {
        return String::new();
    };
    let mut end = first.len();
    for p in &paths[1..] {
        // Walk char boundaries, not bytes: `end` is used to slice below, and a
        // byte count could land inside a multi-byte path component.
        let shared = first
            .char_indices()
            .zip(p.chars())
            .take_while(|((_, a), b)| a == b)
            .map(|((i, a), _)| i + a.len_utf8())
            .last()
            .unwrap_or(0);
        end = end.min(shared);
        if end == 0 {
            return String::new();
        }
    }
    first[..end]
        .rfind('/')
        .map_or(String::new(), |i| first[..=i].to_string())
}

/// A find listing IS the answer, the paths are the payload, not noise wrapped
/// around one. What repeats is the directory prefix: on a real tree it is ~73%
/// of the bytes, one string restated on every line. Hoist it into a header and
/// emit each path relative to it.
///
/// Lossless: `prefix + line` reconstructs every original path. The previous
/// version summarised to `find: total=120 files=120` and dropped all 120 paths,
/// so the agent had to re-run find, paying twice to save once.
fn distill_find_output(input: &str) -> String {
    let paths: Vec<&str> = input
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && *l != ".")
        .collect();

    let prefix = common_dir_prefix(&paths);
    if prefix.is_empty() {
        return input.to_string();
    }

    let mut out = format!("find: {} paths under {}\n", paths.len(), prefix);
    for p in &paths {
        out.push_str(p.strip_prefix(prefix.as_str()).unwrap_or(p));
        out.push('\n');
    }
    // A shallow prefix (`./`) buys less than the header costs. Never hand back
    // something longer than we were given.
    if out.len() < input.len() {
        out
    } else {
        input.to_string()
    }
}

// ---------------------------------------------------------------------------
// tree distiller
// ---------------------------------------------------------------------------

fn distill_tree_output(input: &str) -> String {
    // Look for summary line "N directories, M files"
    let summary_line = input.lines().find(|l| {
        let t = l.trim();
        t.contains("director") && t.contains("file")
    });

    // Collect top-level dirs (depth 1, lines starting with ├── or └──)
    let top_dirs: Vec<&str> = input
        .lines()
        .filter(|l| {
            // Top-level items: "├── name" or "└── name" (no leading spaces before the box char)
            let t = l.trim_start();
            (t.starts_with("├── ") || t.starts_with("└── "))
                && !l.starts_with("│")
                && !l.starts_with("    ")
        })
        .filter_map(|l| {
            let t = l.trim_start();
            let name = t.trim_start_matches("├── ").trim_start_matches("└── ");
            if name.is_empty() { None } else { Some(name) }
        })
        .collect();

    let mut out = if let Some(summary) = summary_line {
        format!("tree: {}", summary.trim())
    } else {
        let total = input.lines().count();
        format!("tree: {} entries", total)
    };

    if !top_dirs.is_empty() {
        let shown: Vec<&str> = top_dirs.iter().take(8).copied().collect();
        out.push_str(&format!("\n  top: {}", shown.join(", ")));
        if top_dirs.len() > 8 {
            out.push_str(&format!(" +{} more", top_dirs.len() - 8));
        }
    }

    out
}

// ---------------------------------------------------------------------------
// env distiller (⚠️ SECURITY CRITICAL: Gate 6)
// ---------------------------------------------------------------------------

/// `env` output is an enumeration: every `KEY=VALUE` line is the answer, the same
/// shape `passes_through_verbatim` already protects for `ls` and `ps`. The only
/// reason to touch it at all is the secret in it.
///
/// It used to emit per-prefix *counts* and no values, except the sensitive one it
/// named in order to say it had been redacted. On a nine-variable `kubectl exec …
/// env` that delivered `DB(8) APP(1)` plus the password line: eight values gone,
/// and the single value surfaced by name was the only one that must never be
/// printed (#342). `DB(8)` answers "which DB vars are set" while the question
/// anyone runs this command for is "set to *what*".
///
/// So it redacts rather than summarises. Nothing is lost but the secrets, which is
/// what a security filter is supposed to do, and the byte saving on an `env` dump
/// was never the point.
/// The payload with every `SENSITIVE_KEY=value` line's value replaced, or `None`
/// when there was nothing to hide.
///
/// Separate from `distill_env_output` because the redaction must not depend on
/// which command produced the output. Measured over 5,733 recorded traces: 25
/// carry an env-shaped payload, 17 of those hold a credential, and **not one of
/// the 25 came from a command named `env`**. They arrive from `cd … && …`,
/// `kubectl exec`, `sed`, `printf` and `export`, so a gate keyed on the command
/// can only ever cover the case nobody actually runs (#344). The keys that went
/// through unredacted include `DB_POSTGRESDB_PASSWORD` eight times, plus
/// `API_KEY_N8N`, `SSH_PRIVATE_KEY` and an access token.
pub fn redact_sensitive_assignments(input: &str) -> Option<String> {
    let mut out = String::with_capacity(input.len());
    let mut hit = false;

    for line in input.lines() {
        let trimmed = line.trim_end();
        match trimmed.find('=') {
            Some(eq) if is_sensitive_key(&trimmed[..eq]) && eq + 1 < trimmed.len() => {
                hit = true;
                out.push_str(&trimmed[..eq]);
                out.push_str("=[REDACTED]\n");
            }
            _ => {
                out.push_str(trimmed);
                out.push('\n');
            }
        }
    }
    hit.then_some(out)
}

/// A key whose *value* must never be delivered. Trimmed so an indented or
/// `export`-prefixed assignment is still recognised, because that is how these
/// arrive in a shell transcript.
///
/// Matching is per underscore-delimited segment rather than by substring. The old
/// `upper.contains(p)` made `PASSED` a password, `AUTHORS` an auth token and
/// `MONKEYS` an API key, and since this runs on every command's output it deleted
/// the value of each of them wherever they appeared (#408). These names are
/// SCREAMING_SNAKE environment keys, so the segment is the unit that carries
/// meaning, not the character run.
///
/// Three shapes, because the list holds three. A pattern ending in `_` is a vendor
/// prefix and anchors to the first segment. A pattern containing `_` spans segments
/// and has to match a consecutive run of them. Everything else matches a segment
/// outright or the tail of one, so an undelimited `MYPASSWORD` is still caught
/// while `PASSED` is not.
///
/// `BYPASS` is a known false positive under the tail rule and is left in place:
/// hiding a value that did not need hiding is recoverable, printing a secret is
/// not. The defect being fixed here is the other direction, where a measurement
/// was destroyed and nothing said so.
fn is_sensitive_key(key: &str) -> bool {
    let k = key.trim().trim_start_matches("export ").trim();
    if k.is_empty() || !k.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return false;
    }
    let upper = k.to_uppercase();
    let segments: Vec<&str> = upper.split('_').filter(|s| !s.is_empty()).collect();
    let Some(first) = segments.first() else {
        return false;
    };

    SENSITIVE_PATTERNS
        .iter()
        .any(|p| match p.strip_suffix('_') {
            Some(prefix) => *first == prefix,
            None if p.contains('_') => {
                let want: Vec<&str> = p.split('_').collect();
                segments.windows(want.len()).any(|w| w == want.as_slice())
            }
            None => segments.iter().any(|s| s == p || s.ends_with(p)),
        })
}

pub fn distill_env_output(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut redacted_count = 0u32;
    let mut parsed_any = false;

    for line in input.lines() {
        let trimmed = line.trim_end();
        let Some(eq_pos) = trimmed.find('=') else {
            // grep headers, blank separators, a shell's own notice. Not ours to
            // reshape, and dropping them is how a command's context disappears.
            out.push_str(trimmed);
            out.push('\n');
            continue;
        };
        parsed_any = true;
        let key = &trimmed[..eq_pos];
        // One predicate, not two. This arm carried its own copy of the substring
        // match, so fixing only `is_sensitive_key` would have left `env` output
        // redacting `passed=` while every other command stopped (#408).
        if is_sensitive_key(key) {
            redacted_count += 1;
            out.push_str(key);
            out.push_str("=[REDACTED]\n");
        } else {
            out.push_str(trimmed);
            out.push('\n');
        }
    }

    // Zero-state guard (#143): no `KEY=VALUE` line means this was never env
    // output, and a summary of nothing is the false claim this project keeps
    // shipping.
    if !parsed_any {
        return input.to_string();
    }

    // One line, only when something was hidden, so a reader who sees `[REDACTED]`
    // knows OMNI did it rather than the command.
    if redacted_count > 0 {
        return format!("[OMNI: {redacted_count} sensitive value(s) redacted]\n{out}");
    }
    out
}

// ---------------------------------------------------------------------------
// Fallback: take max 30 lines from segments
// ---------------------------------------------------------------------------

fn distill_fallback(segments: &[OutputSegment]) -> String {
    let mut out = String::new();
    let mut line_count = 0;

    for seg in segments {
        if matches!(seg.tier, SignalTier::Critical | SignalTier::Important) {
            for line in seg.content.lines() {
                if line_count >= 30 {
                    break;
                }
                out.push_str(line);
                out.push('\n');
                line_count += 1;
            }
        }
        if line_count >= 30 {
            break;
        }
    }

    // If no critical/important found, take first 30 lines from any segment
    if out.trim().is_empty() {
        for seg in segments {
            for line in seg.content.lines() {
                if line_count >= 30 {
                    break;
                }
                out.push_str(line);
                out.push('\n');
                line_count += 1;
            }
            if line_count >= 30 {
                break;
            }
        }
    }

    out.trim().to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {

    /// #342: the env distiller emitted per-prefix counts and no values, except the
    /// sensitive one it named to say it had been redacted. Nine variables came
    /// back as `DB(8) APP(1)` plus the password line: the eight values anyone runs
    /// the command for were gone, and the one value shown by name was the only one
    /// that must never be printed.
    /// #408. `upper.contains(p)` made every one of these a credential and deleted
    /// its value, on every command's output rather than only on `env`. The one that
    /// surfaced it was a `make ci` timing line reading `passed=3208`.
    #[test]
    fn leaves_ordinary_words_that_merely_contain_a_pattern() {
        for key in [
            "passed", "elapsed", "bypassed", "authors", "monkeys", "keywords", "credits", "author",
        ] {
            assert!(
                !is_sensitive_key(key),
                "{key} is not a credential and its value must survive"
            );
        }
    }

    /// The other half of #408, and the half that must not regress: the keys #342
    /// and #344 found going out unredacted, plus the undelimited spellings the
    /// segment rule has to reach by its tail.
    #[test]
    fn still_hides_every_key_that_names_a_credential() {
        for key in [
            "DB_POSTGRESDB_PASSWORD",
            "API_KEY_N8N",
            "SSH_PRIVATE_KEY",
            "AWS_SECRET_ACCESS_KEY",
            "ANTHROPIC_API_KEY",
            "GITHUB_TOKEN",
            "DATABASE_URL",
            "REDIS_URL",
            "CLIENT_SECRET",
            "GOOGLE_CREDENTIALS",
            "MYPASSWORD",
            "APIKEY",
            "export SECRET_VALUE",
        ] {
            assert!(
                is_sensitive_key(key),
                "{key} names a value that must not ship"
            );
        }
    }

    #[test]
    fn keeps_every_value_and_redacts_only_the_secret() {
        let input = "DB_TYPE=postgresdb\n\
                     DB_POSTGRESDB_HOST=db.svc.internal\n\
                     DB_POSTGRESDB_PORT=5432\n\
                     DB_POSTGRESDB_USER=appuser\n\
                     DB_POSTGRESDB_PASSWORD=hunter2\n\
                     APP_HOST=app.example.com\n";
        let out = super::distill_env_output(input);

        for kept in [
            "DB_TYPE=postgresdb",
            "DB_POSTGRESDB_HOST=db.svc.internal",
            "DB_POSTGRESDB_PORT=5432",
            "DB_POSTGRESDB_USER=appuser",
            "APP_HOST=app.example.com",
        ] {
            assert!(out.contains(kept), "value must survive: {kept}\n{out}");
        }
        assert!(
            !out.contains("hunter2"),
            "the secret must not appear:\n{out}"
        );
        assert!(out.contains("DB_POSTGRESDB_PASSWORD=[REDACTED]"), "{out}");
    }

    /// Not env output at all, so there is nothing to summarise and nothing to
    /// redact. A summary of nothing is the false claim this project keeps
    /// shipping (#143).
    #[test]
    fn returns_input_when_no_key_value_line_is_present() {
        let input = "Warming up\nnothing here looks like an assignment\n";
        assert_eq!(super::distill_env_output(input), input);
    }

    /// #316: `grep -n` over a single file prints `<lineno>:<text>` with no path,
    /// because grep only prefixes the filename when given more than one. That
    /// missed `is_grep_output`, so it fell through to the generic system-ops
    /// fallback, which keeps Critical and Important segments and drops the rest.
    /// On a 12-match grep over an alerting manifest it delivered 5 and cut 7,
    /// including `expr: absent(probe_success{tier="carrier"} == 1)` and the alert
    /// summary, while four lines of runbook prose survived. An agent reading it
    /// concludes the file has one alert expression when it has two.
    ///
    /// Driven through `distill_with_command`, because the routing is the defect:
    /// asserting on `distill_grep_output` would have passed all along.
    #[test]
    fn a_single_file_numbered_grep_keeps_every_match() {
        let input = "\
1:# Carrier / partner connectivity alerts. Fire on the blackbox probe_success metric
9:#   1. CarrierConnectivityAllDown (critical) -- absent(probe_success{tier=carrier}==1):
14:#      targets (e.g. a service that is simply off), so day-one does not page for
26:  instanceSelector:
49:        description: Every Carrier-tier target on the connectivity probe has been unreachable for 5m (or the probe stopped reporting).
67:            expr: absent(probe_success{tier=\"carrier\"} == 1)
114:        summary: \"{{ $labels.service }} ({{ $labels.instance }}) is unreachable from 10.0.0.4\"
115:        description: The connectivity probe to {{ $labels.service }} at {{ $labels.instance }} (tier {{ $labels.tier }}) has failed for 10m.
117:          1. Reproduce: az ssh vm --ip 10.0.0.4 -- 'nc -zvw3 <host> <port>'
118:          2. Is only this one target down, or several? Several carrier-tier targets = suspect the tunnel
119:          3. One target only = likely the remote service/port, not the network.
131:            expr: (probe_success{tier=~\"carrier|partner\"} == 0) and (max_over_time(probe_success[6h]) == 1)
";
        let cmd = "grep -n probe_success alerting/carrier-connectivity.yaml";
        let segments = crate::pipeline::scorer::score_with_command(input, cmd, None);
        let out = crate::distillers::distill_with_command(&segments, input, cmd, None);

        for line_no in [
            "1:", "9:", "14:", "26:", "49:", "67:", "114:", "115:", "117:", "118:", "119:", "131:",
        ] {
            assert!(
                out.contains(line_no),
                "match {line_no} was dropped; a grep pattern is the caller's filter: {out}"
            );
        }
    }

    use super::*;

    /// #247: the match count was global and printed once at position 0, so it
    /// described output it had never seen. One Bash call holding two greps, the
    /// first matching nothing, came back as `grep: 20 matches in 10 files` above
    /// the separator, a search that found nothing reading as one that found
    /// twenty hits in ten files, with no marker to detect it by.
    ///
    /// The assertion is on **what stands above the separator**, not on the total:
    /// a count that happens to be right about the payload can still be in the
    /// wrong place, and the wrong place is the whole defect.
    #[test]
    fn counts_grep_matches_per_section_not_per_payload() {
        // First command found nothing; only the separator marks its place.
        let mut input = String::from("---sep---\n");
        for i in 0..6 {
            input.push_str(&format!("/mem/note{i}.md:1:cloudflare token notes\n"));
            input.push_str(&format!("/mem/note{i}.md:2:more cloudflare config here\n"));
        }

        let out = distill_grep_output(&input);

        // What the agent reads first *is* the first command's answer. The old
        // code put the count here, so an empty result read as twelve hits.
        assert!(
            out.starts_with("---sep---"),
            "the first command found nothing, so the separator must come first:\n{out}"
        );

        let (before, after) = out.split_once("---sep---").expect("separator survives");
        assert!(
            !before.contains("grep:"),
            "no count may stand above output it does not describe:\n{out}"
        );
        assert!(
            after.starts_with("\ngrep: 12 matches in 6 files"),
            "the count belongs immediately above the rows it counts:\n{out}"
        );
    }

    /// The counter-case: a single grep is one section, so it still gets exactly
    /// one count and it still sits at the top. Without this the fix reads as
    /// "stop counting", which is not the fix.
    #[test]
    fn still_counts_a_single_grep_once_at_the_top() {
        let mut input = String::new();
        for i in 0..6 {
            input.push_str(&format!("/src/mod{i}.rs:1:fn handler() {{\n"));
            input.push_str(&format!("/src/mod{i}.rs:2:    let handler = 1;\n"));
        }

        let out = distill_grep_output(&input);

        assert!(
            out.starts_with("grep: 12 matches in 6 files"),
            "one command is one section:\n{out}"
        );
        assert_eq!(out.matches("grep:").count(), 1, "exactly one count:\n{out}");
    }

    /// #236: the old fingerprint was `any` connector, so one quoted layout
    /// classified a whole document, and the `directories`/`files` half fired on
    /// a source comment naming both words. `tree`'s closing report is the token
    /// no sibling format prints.
    #[test]
    fn detects_tree_output_only_by_what_tree_alone_prints() {
        let real: Vec<&str> = vec![
            "src",
            "├── main.rs",
            "├── pipeline",
            "└── distillers",
            "",
            "2 directories, 2 files",
        ];
        assert!(is_tree_output(&real));

        // `tree --noreport`: no closing line, so the connectors have to be the
        // shape of the output rather than a passage inside it.
        let noreport: Vec<&str> = vec!["src", "├── main.rs", "├── pipeline", "└── distillers"];
        assert!(is_tree_output(&noreport));

        // A document that quotes a layout in one code block.
        let mut doc: Vec<&str> = vec!["# Development Guide", "", "Guide for contributors.", ""];
        doc.extend(["```", "src/", "├── main.rs", "└── pipeline/", "```", ""]);
        doc.extend(std::iter::repeat_n("Prose line about the pipeline.", 20));
        assert!(!is_tree_output(&doc));

        // A source listing whose comment happens to name both words.
        let source: Vec<&str> = vec![
            "fn distill_tree_output(input: &str) -> String {",
            "    // Look for summary line \"N directories, M files\"",
            "    let summary_line = input.lines().find(|l| {",
            "        t.contains(\"director\") && t.contains(\"file\")",
            "    });",
            "}",
        ];
        assert!(!is_tree_output(&source));
    }

    #[test]
    fn test_env_redaction_removes_secrets() {
        let input = "ANTHROPIC_API_KEY=sk-ant-abc123\nHOME=/home/user\nGITHUB_TOKEN=ghp_secret";
        let result = distill_env_output(input);
        assert!(
            !result.contains("sk-ant-abc123"),
            "API key should be redacted"
        );
        assert!(
            !result.contains("ghp_secret"),
            "GitHub token should be redacted"
        );
        assert!(
            result.contains("[REDACTED]"),
            "Should contain [REDACTED] marker"
        );
    }

    #[test]
    fn test_env_redaction_covers_all_sensitive_patterns() {
        let input = [
            "SECRET_KEY=mysecret",
            "TOKEN=mytoken",
            "API_KEY=myapikey",
            "PASSWORD=mypassword",
            "AUTH_TOKEN=myauth",
            "DATABASE_URL=postgres://secret",
            "AWS_SECRET_ACCESS_KEY=awssecret",
            "OPENAI_API_KEY=sk-abc",
            "GEMINI_API_KEY=gem-abc",
            "HOME=/home/user",
            "PATH=/usr/bin",
            "SHELL=/bin/zsh",
            "TERM=xterm",
            "EDITOR=vim",
        ]
        .join("\n");

        let result = distill_env_output(&input);
        assert!(!result.contains("mysecret"));
        assert!(!result.contains("mytoken"));
        assert!(!result.contains("myapikey"));
        assert!(!result.contains("mypassword"));
        assert!(!result.contains("myauth"));
        assert!(!result.contains("postgres://secret"));
        assert!(!result.contains("awssecret"));
        assert!(!result.contains("sk-abc"));
        assert!(!result.contains("gem-abc"));
    }

    #[test]
    fn test_grep_detection() {
        let lines = vec![
            "src/main.rs:10:fn main() {",
            "src/lib.rs:5:pub mod test;",
            "src/utils.rs:20:fn helper() {",
        ];
        assert!(is_grep_output(&lines));
    }

    #[test]
    fn test_ls_detection() {
        let lines = vec![
            "total 48",
            "drwxr-xr-x  5 user staff  160 Apr  5 10:00 .",
            "-rw-r--r--  1 user staff 1024 Apr  5 10:00 file.txt",
        ];
        assert!(is_ls_output(&lines));
    }

    #[test]
    fn test_find_detection() {
        let lines = vec![
            "./src/main.rs",
            "./src/lib.rs",
            "./src/utils.rs",
            "./Cargo.toml",
        ];
        assert!(is_find_output(&lines));
    }

    #[test]
    fn test_tree_detection() {
        let lines = vec![
            ".",
            "├── src",
            "│   ├── main.rs",
            "│   └── lib.rs",
            "└── Cargo.toml",
        ];
        assert!(is_tree_output(&lines));
    }

    #[test]
    fn test_env_detection() {
        let lines = vec![
            "HOME=/home/user",
            "PATH=/usr/bin",
            "SHELL=/bin/zsh",
            "TERM=xterm",
            "EDITOR=vim",
            "LANG=en_US.UTF-8",
        ];
        assert!(is_env_output(&lines));
    }

    /// Rebuild `path:rest` from the hoisted headers and their indented matches.
    fn rebuild_grep(output: &str) -> Vec<String> {
        let mut header = "";
        let mut lines = Vec::new();
        for line in output.lines().skip(1) {
            match line.strip_prefix("  ") {
                Some(rest) => lines.push(format!("{}:{}", header, rest)),
                None => header = line,
            }
        }
        lines
    }

    #[test]
    fn states_each_grep_path_once_instead_of_per_match() {
        // Arrange: a path long enough that hoisting it beats the header it costs
        let input = "src/pipeline/registry.rs:10:fn main() {\n\
                     src/pipeline/registry.rs:20:    println!(\"hello\");\n\
                     src/pipeline/registry.rs:30:}\n\
                     src/pipeline/scorer.rs:5:pub mod test;";

        // Act
        let result = distill_grep_output(input);

        // Assert
        assert!(
            result.contains("grep: 4 matches in 2 files"),
            "got: {result}"
        );
        assert_eq!(
            result.matches("src/pipeline/registry.rs\n").count(),
            1,
            "path should be stated once, not per match: {result}"
        );
    }

    /// The old distiller reduced grep to a per-file histogram, dropping every
    /// matched line, the text that is the entire point of grep. It then had to
    /// special-case error lines back in, because otherwise they vanished too.
    /// Keeping everything makes that special case unnecessary, and losslessness
    /// is the stronger invariant to pin.
    #[test]
    fn preserves_every_grep_match_including_errors() {
        // Arrange
        let input = "src/pipeline/registry.rs:47:    return Err(AuthError::InvalidToken);\n\
                     src/pipeline/registry.rs:50:    panic!(\"fatal auth error\");\n\
                     src/pipeline/scorer.rs:10:fn connect() {\n\
                     src/pipeline/scorer.rs:20:fn query() {";

        // Act
        let result = distill_grep_output(input);

        // Assert
        assert_eq!(
            rebuild_grep(&result),
            input.lines().collect::<Vec<_>>(),
            "output must reconstruct the input exactly: {result}"
        );
    }

    #[test]
    fn hands_back_grep_input_when_hoisting_would_grow_it() {
        // Arrange: one match per file, every header costs more than it saves
        let input = "a.rs:1:x\nb.rs:1:y";

        // Act / Assert
        assert_eq!(distill_grep_output(input), input);
    }

    #[test]
    fn factors_the_shared_find_prefix_losslessly() {
        // Arrange
        let input = "/home/u/proj/src/lib.rs\n/home/u/proj/src/pipeline/mod.rs\n/home/u/proj/src/distillers/git.rs";

        // Act
        let result = distill_find_output(input);

        // Assert
        assert!(
            result.starts_with("find: 3 paths under /home/u/proj/src/\n"),
            "got: {result}"
        );
        let rebuilt: Vec<String> = result
            .lines()
            .skip(1)
            .map(|l| format!("/home/u/proj/src/{}", l))
            .collect();
        assert_eq!(rebuilt, input.lines().collect::<Vec<_>>());
    }

    #[test]
    fn hands_back_find_input_when_paths_share_no_directory() {
        // Arrange
        let input = "/usr/bin/ls\n/etc/hosts\n/var/log/syslog";

        // Act / Assert
        assert_eq!(distill_find_output(input), input);
    }

    #[test]
    fn cuts_the_prefix_at_a_separator_not_mid_filename() {
        // Arrange: "config" and "connect" share "con", which is not a directory.
        // A prefix of "/srv/app/con" would not round-trip back to the paths.
        let paths = ["/srv/app/config.rs", "/srv/app/connect.rs"];

        // Act / Assert
        assert_eq!(common_dir_prefix(&paths), "/srv/app/");
    }

    #[test]
    fn cuts_the_prefix_on_a_char_boundary_for_non_ascii_paths() {
        // Arrange: shared bytes run into a multi-byte char; slicing by byte
        // count instead of char boundary would panic here.
        let paths = ["/srv/données/a.rs", "/srv/donné/b.rs"];

        // Act / Assert
        assert_eq!(common_dir_prefix(&paths), "/srv/");
    }
}
