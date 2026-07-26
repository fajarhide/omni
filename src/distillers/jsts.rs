// Safety: All string indexing uses positions from find()/rfind() on ASCII
// delimiters (':', '(', '/', ' ') which always return valid char boundaries.
#![allow(clippy::string_slice)]

use crate::distillers::Distiller;
use crate::pipeline::{OutputSegment, SignalTier};
use std::collections::BTreeMap;

pub struct JsTsDistiller;

impl Distiller for JsTsDistiller {
    fn distill(
        &self,
        segments: &[OutputSegment],
        input: &str,
        session: Option<&crate::pipeline::SessionState>,
    ) -> String {
        let mut lines: Vec<&str> = input.lines().collect();

        if let Some(state) = session
            && let Some(js_pm) = state.toolchain_hints.get("js")
        {
            if js_pm == "pnpm" {
                lines.retain(|l| !l.contains("pnpm: packages are hard linked"));
            } else if js_pm == "yarn" {
                lines.retain(|l| !l.contains("yarn install v1."));
            }
        }

        let filtered_input = lines.join("\n");

        // A composite task runner (`npm run verify` = `build && tsc && eslint && …`)
        // concatenates several tools' output into one buffer. npm echoes the chained
        // command it runs, so a `> … && …` line is the tell. Without this guard the
        // single-tool detectors below each match a fragment and the FIRST wins the
        // whole buffer — `tsc --` inside npm's own echo made `npm run verify` distil
        // to `tsc: no errors`, discarding four of five gates (#106). No per-tool
        // distiller can safely own a composite (there is no delimiter between the
        // tools' outputs), so decline: return it unchanged and let the pipeline's
        // generic collapse fold the repeated build noise while keeping every gate's
        // distinct verdict line.
        if is_composite_command(&lines) {
            return filtered_input;
        }

        // Dispatch based on content analysis
        if is_vitest_output(&lines) {
            distill_vitest(&filtered_input)
        } else if is_tsc_output(&lines) {
            distill_tsc(&filtered_input)
        } else if is_playwright_output(&lines) {
            distill_playwright(&filtered_input)
        } else if is_eslint_output(&lines) {
            distill_eslint(&filtered_input)
        } else if is_prettier_output(&lines) {
            distill_prettier(&filtered_input)
        } else {
            // Both arms of the `filtered_input.len() < input.len()` branch that
            // used to stand here called this same function with the same
            // arguments, so the condition decided nothing.
            distill_fallback(segments, session)
        }
    }
}

// ---------------------------------------------------------------------------
// Detection helpers
// ---------------------------------------------------------------------------

fn is_composite_command(lines: &[&str]) -> bool {
    // npm/yarn/pnpm echo the script they run; a `> a && b && c` echo means several
    // tools chained, their outputs about to be concatenated with no delimiter. The
    // per-tool detectors can't safely claim such a buffer. (`make`/`npm-run-all`
    // composites without an `&&` echo aren't covered yet — add when one is reported.)
    lines
        .iter()
        .any(|l| l.trim_start().starts_with('>') && l.contains("&&"))
}

fn is_vitest_output(lines: &[&str]) -> bool {
    lines.iter().any(|l| {
        l.contains("vitest")
            || l.contains("VITE v")
            || l.contains("Test Files")
            || l.contains("Tests  ")
    })
}

fn is_tsc_output(lines: &[&str]) -> bool {
    lines.iter().any(|l| {
        l.contains("error TS")
            || l.contains("tsc --")
            || l.contains("Found errors")
            || l.contains("Found ") && l.contains(" error")
    })
}

fn is_playwright_output(lines: &[&str]) -> bool {
    lines.iter().any(|l| {
        l.contains("playwright")
            || l.contains("[chromium]")
            || l.contains("[firefox]")
            || l.contains("Running ") && l.contains(" tests")
    })
}

fn is_eslint_output(lines: &[&str]) -> bool {
    // Anchor on eslint's real output shape, never the bare word "eslint" — that
    // matched a *filename* (`eslint.config.js`) in prettier's file list and sent a
    // `prettier --write` run to `distill_eslint`, which reported the wrong tool
    // finding nothing (#114). Same substring-in-data trap as #105/#106.
    lines.iter().any(|l| {
        l.contains(" problems (")            // summary: "✖ 3 problems (0 errors, 3 warnings)"
            || l.contains("@typescript-eslint/") // a real eslint rule id
            || is_eslint_finding_line(l) // "  12:5  warning  <msg>  <rule>"
    })
}

/// eslint prints a finding as `  <line>:<col>  error|warning  …`.
fn is_eslint_finding_line(l: &str) -> bool {
    let mut tokens = l.split_whitespace();
    let Some((line, col)) = tokens.next().and_then(|t| t.split_once(':')) else {
        return false;
    };
    if line.is_empty()
        || col.is_empty()
        || !line.bytes().all(|b| b.is_ascii_digit())
        || !col.bytes().all(|b| b.is_ascii_digit())
    {
        return false;
    }
    matches!(tokens.next(), Some("error" | "warning"))
}

fn is_prettier_output(lines: &[&str]) -> bool {
    // Prettier's real output is capitalised (`Checking formatting…`, `[warn] …`),
    // so the old lowercase `checking `/`reformatted ` never fired on it — the
    // detector was dead (#114). Match what prettier actually prints, in either mode.
    lines.iter().any(|l| {
        l.contains("Checking formatting")     // --check header
            || l.contains("[warn]")           // --check finding / summary
            || l.contains("Code style issues") // --check summary
            || l.to_lowercase().contains("prettier") // command echo / banner, any case
            || is_prettier_write_line(l) // --write: "<path> <n>ms"
    })
}

/// prettier `--write` prints one line per file: `<path> <n>ms`, with ` (unchanged)`
/// appended to files it left alone.
fn is_prettier_write_line(l: &str) -> bool {
    l.split_whitespace().any(|t| {
        t.len() > 2 && t.ends_with("ms") && t[..t.len() - 2].bytes().all(|b| b.is_ascii_digit())
    })
}

// ---------------------------------------------------------------------------
// vitest
// ---------------------------------------------------------------------------

fn distill_vitest(input: &str) -> String {
    let mut passed_tests = 0;
    let mut failed_tests = 0;
    let mut total_tests = 0;
    let mut has_summary = false;

    let mut failed_details: Vec<String> = Vec::new();

    let lines: Vec<&str> = input.lines().collect();

    // Attempt to parse formal summary first
    for line in &lines {
        let t = line.trim();
        let t_lower = t.to_lowercase();
        if t_lower.contains("tests ") && (t_lower.contains("failed") || t_lower.contains("passed"))
        {
            has_summary = true;
            // E.g., "Tests  3 failed | 48 passed (51)"
            let parts: Vec<&str> = t.split('|').collect();
            for part in parts {
                if part.contains("passed")
                    && let Some(num) = part.split_whitespace().find_map(|s| s.parse::<u32>().ok())
                {
                    passed_tests = num;
                }
                if part.contains("failed")
                    && let Some(num) = part.split_whitespace().find_map(|s| s.parse::<u32>().ok())
                {
                    failed_tests = num;
                }
            }
            // Parse total from "(51)" if present
            if let Some(start) = t.find('(')
                && let Some(end) = t[start..].find(')')
                && let Ok(num) = t[start + 1..start + end].trim().parse::<u32>()
            {
                total_tests = num;
            }
        }

        // Find failed tests: " ✗ src/services/__tests__/api.test.ts:47:12" or "   ✗ should handle rate limiting"
        // Look for deeper trace points
        if t.contains('❯') && t.contains(':') {
            let trace = t[t.find('❯').unwrap()..].trim_start_matches('❯').trim();
            // take basename:line
            if let Some(slash_idx) = trace.rfind('/') {
                let rest = &trace[slash_idx + 1..];
                // if it looks like file:line:col
                let mut parts = rest.split(':');
                if let Some(file) = parts.next()
                    && let Some(line) = parts.next()
                {
                    failed_details.push(format!("{}:{}", file, line));
                }
            } else {
                // fallback if no slash
                let mut parts = trace.split(':');
                if let Some(file) = parts.next()
                    && let Some(line) = parts.next()
                {
                    failed_details.push(format!("{}:{}", file, line));
                }
            }
        }
    }

    if !has_summary {
        // Fallback: count from lines
        for line in &lines {
            // Count passes and fails heuristically if no summary
            if line.contains(" ✓ ") {
                passed_tests += 1;
            }
            if *line != " ✗ " && line.contains(" ✗ ") && !line.contains("failed |") {
                failed_tests += 1;
            }
        }
        total_tests = passed_tests + failed_tests;
    }

    if total_tests == 0 {
        total_tests = passed_tests + failed_tests;
    }

    if failed_tests == 0 && failed_details.is_empty() {
        // Zero-state guard (#143): only claim a clean run if we actually parsed a
        // vitest signal (a "Tests …" summary or at least one ✓ line). Otherwise a
        // misdetected input (e.g. a `VITE v` dev server, #115) would become a
        // false `vitest: ✓ 0/0 passed`. No signal → pass the input through.
        let parsed = has_summary || passed_tests > 0;
        return super::require_parsed(
            parsed,
            input,
            format!("vitest: ✓ {}/{} passed", passed_tests, total_tests),
        );
    }

    // Deduplicate failed_details
    let mut unique_fails = Vec::new();
    for f in failed_details {
        if !unique_fails.contains(&f) {
            unique_fails.push(f);
        }
    }

    let fail_count_display = if failed_tests > 0 {
        failed_tests
    } else {
        unique_fails.len() as u32
    };

    let mut out = format!(
        "vitest: ✓ {}/{} | ✗ {}",
        passed_tests, total_tests, fail_count_display
    );

    if !unique_fails.is_empty() {
        let shown: Vec<String> = unique_fails.into_iter().take(5).collect();
        out.push_str(&format!(" [{}]", shown.join(", ")));
        // Could add +N but spec just says show them. We show up to 5 implicitly.
    }

    out
}

// ---------------------------------------------------------------------------
// TypeScript Compiler (TSC)
// ---------------------------------------------------------------------------

fn distill_tsc(input: &str) -> String {
    let mut by_file: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut total_errors = 0;
    // Zero-state guard (#143): did we positively recognize tsc output at all?
    let mut saw_tsc_signal = false;

    for line in input.lines() {
        let t = line.trim();
        // Check for error line like "src/components/Button.tsx(10,5): error TS2741: Property 'onClick' is missing"
        // Or "error TS2307: Cannot find module './utils' in 'src/app.ts'"

        if let Some(ts_idx) = t.find("error TS") {
            saw_tsc_signal = true;
            total_errors += 1;

            // Try to extract file and line
            let mut file_display = String::new();
            let mut issue_display = String::new();

            // Format 1: file(line,col): error TS####...
            if ts_idx > 0 && t[..ts_idx].contains("): ") {
                let prefix = &t[..ts_idx];
                if let Some(paren_idx) = prefix.find('(') {
                    let file = prefix[..paren_idx].trim();
                    let basename = file.rsplit('/').next().unwrap_or(file);
                    file_display = basename.to_string();

                    let line_num = prefix[paren_idx + 1..].split(',').next().unwrap_or("");

                    // Extract TS code
                    let rest = &t[ts_idx..];
                    let ts_code = rest.split(':').next().unwrap_or("").replace("error ", "");

                    issue_display = format!("{}:l{}", ts_code, line_num);
                }
            } else {
                // Format 2: error TS####: ... in 'file.ts'
                let rest = &t[ts_idx..];
                let mut parts = rest.split(':');
                let ts_code = parts.next().unwrap_or("").replace("error ", "");

                if let Some(in_idx) = t.rfind(" in '") {
                    let file_part = &t[in_idx + 5..];
                    let file = file_part.trim_end_matches('\'');
                    let basename = file.rsplit('/').next().unwrap_or(file);
                    file_display = basename.to_string();
                    issue_display = ts_code;
                } else {
                    file_display = "unknown".to_string();
                    issue_display = ts_code;
                }
            }

            if !file_display.is_empty() {
                by_file.entry(file_display).or_default().push(issue_display);
            }
        } else if t.to_lowercase().contains("found ") && t.to_lowercase().contains(" error") {
            // "Found 5 errors" (also matches the clean "Found 0 errors" summary)
            saw_tsc_signal = true;
            if let Some(num) = t
                .split_whitespace()
                .nth(1)
                .and_then(|s| s.parse::<u32>().ok())
                && total_errors == 0
            {
                total_errors = num; // fallback if we couldn't parse individual lines
            }
        }
    }

    if total_errors == 0 {
        // Only claim "no errors" if we actually parsed a tsc signal; a misrouted
        // non-tsc input (or empty output from another command) passes through.
        return super::require_parsed(saw_tsc_signal, input, "tsc: no errors".to_string());
    }

    let file_count = by_file.len();
    let mut out = format!("tsc: {} errors in {} files", total_errors, file_count);

    let mut sorted: Vec<(String, Vec<String>)> = by_file.into_iter().collect();
    // Sort by number of errors descending
    sorted.sort_by_key(|a| std::cmp::Reverse(a.1.len()));

    for (file, issues) in sorted.iter().take(5) {
        let count = issues.len();
        let issues_str = issues.join(", ");
        let truncated = crate::util::text::display_truncate_with_ellipsis(&issues_str, 57);
        out.push_str(&format!("\n  {}: {} errors [{}]", file, count, truncated));
    }

    if sorted.len() > 5 {
        out.push_str(&format!("\n  +{} more files", sorted.len() - 5));
    }

    out
}

// ---------------------------------------------------------------------------
// Playwright
// ---------------------------------------------------------------------------

fn distill_playwright(input: &str) -> String {
    let mut passed = 0;
    let mut failed = 0;

    let mut fail_info: Vec<String> = Vec::new();

    let lines: Vec<&str> = input.lines().collect();

    // Collect specific failures
    for line in &lines {
        let t = line.trim();
        // Look for: ✗  9 [chromium] › tests/login.spec.ts:20:1 › submits valid credentials (5.0s)
        if t.contains('✗') && t.contains(" › ") {
            // Extract file:line and test name
            let parts: Vec<&str> = t.split(" › ").collect();
            if parts.len() >= 3 {
                let file_path = parts[1]; // tests/login.spec.ts:20:1
                let test_name = parts[2].split(" (").next().unwrap_or(parts[2]);

                // Keep just file basename and line
                let mut display_file = file_path.to_string();
                if let Some(slash_idx) = file_path.rfind('/') {
                    display_file = file_path[slash_idx + 1..].to_string();
                }

                // Strip the final column if any, e.g. login.spec.ts:20:1 -> login.spec.ts:20
                if display_file.matches(':').count() == 2
                    && let Some(last_colon) = display_file.rfind(':')
                {
                    display_file = display_file[..last_colon].to_string();
                }

                fail_info.push(format!("{}:{}", test_name, display_file));
            }
        }

        // Parse summary line: "  2 failed" or "  22 passed (45.2s)"
        if t.ends_with("passed") || (t.contains("passed (") && t.ends_with(")")) {
            if let Some(num) = t
                .split_whitespace()
                .next()
                .and_then(|s| s.parse::<u32>().ok())
            {
                passed = num;
            }
        } else if (t.ends_with("failed") || (t.contains("failed (") && t.ends_with(")")))
            && let Some(num) = t
                .split_whitespace()
                .next()
                .and_then(|s| s.parse::<u32>().ok())
        {
            failed = num;
        }
    }

    // If we missed summary but have individual lines
    if passed == 0 && failed == 0 {
        for line in &lines {
            if line.contains(" ✓ ") {
                passed += 1;
            }
            if line.contains(" ✗ ") {
                failed += 1;
            }
        }
    }

    let total = passed + failed;

    if failed == 0 {
        // Zero-state guard (#143): only claim a clean run if we parsed at least one
        // passing test (a summary count or a ✓ line). No signal → pass through, so a
        // misrouted input never becomes a false `playwright: ✓ 0/0 passed`.
        return super::require_parsed(
            passed > 0,
            input,
            format!("playwright: ✓ {}/{} passed", passed, total),
        );
    }

    let mut out = format!("playwright: ✓ {}/{} | ✗ {}", passed, total, failed);

    if !fail_info.is_empty() {
        let shown: Vec<String> = fail_info.into_iter().take(3).collect();
        out.push_str(&format!(" [{}]", shown.join(", ")));
    }

    out
}

// ---------------------------------------------------------------------------
// ESLint
// ---------------------------------------------------------------------------

fn distill_eslint(input: &str) -> String {
    let mut total_errors = 0;
    let mut total_warnings = 0;
    let mut by_rule: BTreeMap<String, u32> = BTreeMap::new();
    let mut files_affected: std::collections::HashSet<String> = std::collections::HashSet::new();
    // Zero-state guard (#143): a bare file list (e.g. prettier output, #114) also
    // populates `files_affected`, so that is NOT proof this is eslint. Only an
    // eslint "problems (" summary or a parsed rule counts as a positive signal.
    let mut saw_eslint_signal = false;

    for line in input.lines() {
        let t = line.trim();
        let t_lower = t.to_lowercase();

        // Skip empty or summary lines
        if t.is_empty() || t.contains('✖') || t_lower.contains("checking") {
            // But still parse summary counts
            if t.contains("problems (") {
                saw_eslint_signal = true;
                if let Some(err_idx) = t.find(" errors") {
                    if let Some(start) = t[..err_idx].rfind('(') {
                        if let Ok(n) = t[start + 1..err_idx].trim().parse::<u32>() {
                            total_errors = n;
                        }
                    } else if let Some(start) = t[..err_idx].rfind(' ')
                        && let Ok(n) = t[start + 1..err_idx].trim().parse::<u32>()
                    {
                        total_errors = n;
                    }
                } else if let Some(err_idx) = t.find(" error")
                    && let Some(start) = t[..err_idx].rfind('(')
                    && let Ok(n) = t[start + 1..err_idx].trim().parse::<u32>()
                {
                    total_errors = n;
                }

                if let Some(warn_idx) = t.find(" warnings") {
                    if let Some(start) = t[..warn_idx].rfind(" ")
                        && let Ok(n) = t[start + 1..warn_idx].trim().parse::<u32>()
                    {
                        total_warnings = n;
                    }
                } else if let Some(warn_idx) = t.find(" warning")
                    && let Some(start) = t[..warn_idx].rfind(", ")
                    && let Ok(n) = t[start + 2..warn_idx].trim().parse::<u32>()
                {
                    total_warnings = n;
                }
            }
            continue;
        }

        // Standard formatter grouping (file path on its own line)
        if !t.contains(" error ")
            && !t.contains(" warning ")
            && (t.contains('/') || t.contains('\\'))
            && !t.contains(' ')
        {
            files_affected.insert(t.to_string());
        }

        // Inline formatter (file:line:col error ...)
        if let Some(colon_idx) = t.find(':')
            && (t.contains(" error ") || t.contains(" warning "))
        {
            let file_path = &t[..colon_idx];
            if file_path.contains('/') || file_path.contains('.') || file_path.contains('\\') {
                files_affected.insert(file_path.to_string());
            }
        }

        // Parse individual rules: "src/index.ts:10:15 error Unexpected console statement @typescript-eslint/no-console"
        if t.contains(" error ") || t.contains(" warning ") {
            saw_eslint_signal = true;
            let parts = t.split_whitespace();
            if let Some(last) = parts.last()
                && (last.contains('/') || last.contains('-'))
            {
                // Looks like a rule name
                *by_rule.entry(last.to_string()).or_insert(0) += 1;
            }
        }
    }

    if total_errors == 0 && total_warnings == 0 {
        // No counts parsed. Only report a clean lint if we saw a genuine eslint
        // signal; otherwise (e.g. prettier's file list, #114) pass the input through.
        return super::require_parsed(
            saw_eslint_signal,
            input,
            "eslint: no problems found".to_string(),
        );
    }

    let mut out = format!(
        "eslint: {} errors, {} warnings in {} files",
        total_errors,
        total_warnings,
        files_affected.len()
    );

    if !by_rule.is_empty() {
        let mut sorted: Vec<(String, u32)> = by_rule.into_iter().collect();
        sorted.sort_by_key(|a| std::cmp::Reverse(a.1));

        out.push_str("\n  top rules: ");
        let rules_str: Vec<String> = sorted
            .iter()
            .take(3)
            .map(|(r, c)| format!("{}: {}", r, c))
            .collect();
        out.push_str(&rules_str.join(", "));
    }

    out
}

// ---------------------------------------------------------------------------
// Prettier
// ---------------------------------------------------------------------------

fn distill_prettier(input: &str) -> String {
    // The old body parsed black's `reformatted N files` summary — prettier prints no
    // such line, so both counters stayed 0 and a *failing* `--check` and a
    // *successful* `--write` both rendered as "0 files reformatted, 0 unchanged"
    // (#114). Parse prettier's real output per mode; if neither is recognisable,
    // decline (return the input) rather than fabricate a count.
    let lines: Vec<&str> = input.lines().collect();

    // --check: offending files are listed as `[warn] <path>`, ending with a boilerplate
    // `[warn] Code style issues …` line. The filenames are the actionable signal.
    let is_check = lines
        .iter()
        .any(|l| l.contains("Checking formatting") || l.contains("[warn]"));
    if is_check {
        let files: Vec<&str> = lines
            .iter()
            .filter_map(|l| l.trim().strip_prefix("[warn] "))
            .filter(|f| !f.is_empty() && !f.starts_with("Code style issues"))
            .collect();
        return if files.is_empty() {
            "prettier --check: all files formatted".to_string()
        } else {
            format!(
                "prettier --check: {} file(s) need formatting\n{}",
                files.len(),
                capped_lines(&files, 20)
            )
        };
    }

    // --write: `<path> <n>ms` per file, ` (unchanged)` on files left alone.
    let file_lines: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|l| is_prettier_write_line(l))
        .collect();
    if file_lines.is_empty() {
        return input.to_string();
    }
    let unchanged = file_lines
        .iter()
        .filter(|l| l.contains("(unchanged)"))
        .count();
    let changed: Vec<&str> = file_lines
        .iter()
        .filter(|l| !l.contains("(unchanged)"))
        .map(|l| l.split_whitespace().next().unwrap_or(""))
        .collect();
    let mut out = format!(
        "prettier --write: {} reformatted, {} unchanged",
        changed.len(),
        unchanged
    );
    if !changed.is_empty() {
        out.push('\n');
        out.push_str(&capped_lines(&changed, 20));
    }
    out
}

/// The `… and N more` tail every capped renderer in this file shares.
///
/// A cap without one is silent data loss wearing a compression badge: the reader
/// gets a well-formed output and no way to tell it is a tenth of what there was
/// (#111, #176). `distill_fallback` was the one capped renderer that did not
/// emit it, which is #188.
fn more_tail(total: usize, shown: usize) -> Option<String> {
    (total > shown).then(|| format!("… and {} more", total - shown))
}

/// Render `items` one per indented line, capped, with an `… and N more` tail.
fn capped_lines(items: &[&str], cap: usize) -> String {
    let mut out: Vec<String> = items.iter().take(cap).map(|s| format!("  {s}")).collect();
    if let Some(tail) = more_tail(items.len(), cap) {
        out.push(format!("  {tail}"));
    }
    out.join("\n")
}

// ---------------------------------------------------------------------------
// Fallback
// ---------------------------------------------------------------------------

/// Lines the fallback keeps before it says how many it dropped.
///
/// The number is unchanged; what changed in #188 is that exceeding it is now
/// reported. It silently deleted 270 of 300 lines and the result was published
/// as a 90% saving.
const FALLBACK_MAX_LINES: usize = 30;

/// Segments sampled when nothing in the output scored Critical or Important.
const FALLBACK_SAMPLE_SEGMENTS: usize = 10;

/// Package-manager chatter the session hint already told us to expect.
///
/// Both loops below filtered this with the same two `contains` checks written
/// out twice; a predicate keeps them from drifting apart.
fn is_pm_noise(line: &str, js_pm: Option<&str>) -> bool {
    match js_pm {
        Some("pnpm") => line.contains("pnpm: packages are hard linked"),
        Some("yarn") => line.contains("yarn install v1."),
        _ => false,
    }
}

fn distill_fallback(
    segments: &[OutputSegment],
    session: Option<&crate::pipeline::SessionState>,
) -> String {
    let js_pm = session.and_then(|s| s.toolchain_hints.get("js").map(|v| v.as_str()));

    let eligible: Vec<&str> = segments
        .iter()
        .filter(|seg| matches!(seg.tier, SignalTier::Critical | SignalTier::Important))
        .flat_map(|seg| seg.content.lines())
        .filter(|line| !is_pm_noise(line, js_pm))
        .collect();

    if !eligible.is_empty() {
        return with_tail(&eligible, FALLBACK_MAX_LINES);
    }

    // Nothing scored: sample the first line of each of the first N segments
    // rather than return nothing at all. That is still a sample, so it is
    // counted against every line there was — not against the ones sampled.
    let sample: Vec<&str> = segments
        .iter()
        .take(FALLBACK_SAMPLE_SEGMENTS)
        .filter_map(|seg| seg.content.lines().find(|l| !is_pm_noise(l, js_pm)))
        .collect();
    let total_lines = segments
        .iter()
        .flat_map(|seg| seg.content.lines())
        .filter(|l| !is_pm_noise(l, js_pm))
        .count();

    let mut out = sample.join("\n");
    if let Some(tail) = more_tail(total_lines, sample.len()) {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&tail);
    }
    out.trim().to_string()
}

/// `items` capped at `cap`, one per line, with the omission tail when it bites.
fn with_tail(items: &[&str], cap: usize) -> String {
    let mut out = items
        .iter()
        .take(cap)
        .copied()
        .collect::<Vec<&str>>()
        .join("\n");
    if let Some(tail) = more_tail(items.len(), cap) {
        out.push('\n');
        out.push_str(&tail);
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::{SessionState, SignalTier};

    /// One `Important` segment per line, which is what the scorer produces for a
    /// wall of undifferentiated log output — the shape that hit #188.
    fn important_segments(lines: &[String]) -> Vec<OutputSegment> {
        lines
            .iter()
            .enumerate()
            .map(|(i, l)| OutputSegment {
                content: l.clone(),
                tier: SignalTier::Important,
                base_score: 0.8,
                context_score: 0.0,
                line_range: (i + 1, i + 1),
            })
            .collect()
    }

    fn npm_warnings(n: usize) -> Vec<String> {
        (0..n)
            .map(|i| format!("npm WARN deprecated fake-package@1.0.{i}: no longer supported"))
            .collect()
    }

    /// #188. The fallback stopped at 30 lines with a bare `break` and returned
    /// `out.trim()` — no marker on any path. 270 of 300 lines disappeared and the
    /// result was published as a 90% saving, so nothing downstream could tell a
    /// 30-warning install from a 300-warning one.
    ///
    /// The assertion is on the **count**, not merely on the presence of an
    /// ellipsis: a marker without a number does not let a reader judge whether to
    /// re-read, which is what #176 settled.
    #[test]
    fn reports_how_many_lines_the_cap_dropped() {
        let lines = npm_warnings(300);
        let out = distill_fallback(&important_segments(&lines), None);

        assert!(
            out.contains("… and 270 more"),
            "270 dropped lines went unreported: {out}"
        );
        assert_eq!(
            out.lines().count(),
            FALLBACK_MAX_LINES + 1,
            "expected {FALLBACK_MAX_LINES} kept lines plus one tail: {out}"
        );
    }

    /// The tail must not appear when the cap did not bite — a marker claiming
    /// zero omissions is its own small false claim.
    #[test]
    fn stays_silent_when_nothing_was_dropped() {
        let lines = npm_warnings(FALLBACK_MAX_LINES);
        let out = distill_fallback(&important_segments(&lines), None);

        assert!(
            !out.contains("more"),
            "marked an omission that never happened: {out}"
        );
        assert_eq!(out.lines().count(), FALLBACK_MAX_LINES);
    }

    /// Lines dropped by the package-manager filter must not be counted as
    /// capped-away, or the tail overstates what is missing.
    #[test]
    fn counts_only_lines_the_cap_removed_not_ones_already_filtered() {
        let mut lines = vec!["pnpm: packages are hard linked".to_string()];
        lines.extend(npm_warnings(300));
        let mut session = SessionState::default();
        session
            .toolchain_hints
            .insert("js".to_string(), "pnpm".to_string());

        let out = distill_fallback(&important_segments(&lines), Some(&session));

        assert!(
            out.contains("… and 270 more"),
            "pm-filtered line was counted as capped away: {out}"
        );
    }

    /// The zero-state sample path (nothing scored Critical or Important) takes
    /// one line per segment and used to drop the rest just as silently.
    #[test]
    fn marks_omissions_in_the_zero_state_sample_too() {
        let segments: Vec<OutputSegment> = (0..20)
            .map(|i| OutputSegment {
                content: format!("context line {i}a\ncontext line {i}b"),
                tier: SignalTier::Context,
                base_score: 0.3,
                context_score: 0.0,
                line_range: (i + 1, i + 1),
            })
            .collect();

        let out = distill_fallback(&segments, None);

        assert!(
            out.contains("more"),
            "sampled 10 of 40 lines with nothing saying so: {out}"
        );
    }

    #[test]
    fn test_toolchain_filtering() {
        let distiller = JsTsDistiller;
        let input = "pnpm: packages are hard linked\n✓ test 1\nyarn install v1.22.19\n✗ test 2";
        let segments = vec![
            OutputSegment {
                content: "pnpm: packages are hard linked".to_string(),
                tier: SignalTier::Important,
                base_score: 0.8,
                context_score: 0.0,
                line_range: (1, 1),
            },
            OutputSegment {
                content: "✓ test 1".to_string(),
                tier: SignalTier::Important,
                base_score: 0.8,
                context_score: 0.0,
                line_range: (2, 2),
            },
            OutputSegment {
                content: "yarn install v1.22.19".to_string(),
                tier: SignalTier::Important,
                base_score: 0.8,
                context_score: 0.0,
                line_range: (3, 3),
            },
            OutputSegment {
                content: "✗ test 2".to_string(),
                tier: SignalTier::Critical,
                base_score: 0.9,
                context_score: 0.0,
                line_range: (4, 4),
            },
        ];

        // 1. Without session, no filtering
        let output_none = distiller.distill(&segments, input, None);
        assert!(output_none.contains("pnpm: packages are hard linked"));
        assert!(output_none.contains("yarn install v1."));

        // 2. With pnpm session
        let mut state_pnpm = SessionState::new();
        state_pnpm
            .toolchain_hints
            .insert("js".to_string(), "pnpm".to_string());
        let output_pnpm = distiller.distill(&segments, input, Some(&state_pnpm));
        assert!(!output_pnpm.contains("pnpm: packages are hard linked"));
        assert!(output_pnpm.contains("yarn install v1."));

        // 3. With yarn session
        let mut state_yarn = SessionState::new();
        state_yarn
            .toolchain_hints
            .insert("js".to_string(), "yarn".to_string());
        let output_yarn = distiller.distill(&segments, input, Some(&state_yarn));
        assert!(output_yarn.contains("pnpm: packages are hard linked"));
        assert!(!output_yarn.contains("yarn install v1."));
    }
}
