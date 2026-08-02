use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/// `src/main.rs:10:5`, the file-and-line shape that marks a context line.
///
/// Compiled once. It used to be built inside `is_context`, which `classify_block`
/// calls for every line, so a single 64-character line cost 660 µs and an 80-line
/// payload spent 52 ms deciding what it was. That is five times the whole hook
/// budget in `AGENTS.md`, spent compiling the same pattern eighty times (#283).
static PATH_WITH_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[\w\./\-]+\.\w+:\d+(:\d+)?").expect("the path regex is a literal and must compile")
});

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticClass {
    Critical,   // errors, panics, fatal — ALWAYS shown
    Diagnostic, // warnings, deprecations — shown with count
    Context,    // stack traces, file locations — shown if Critical present
    Progress,   // loading bars, "Compiling X" — always stripped
    Noise,      // blank lines, decorators — always stripped
    Data,       // actual output data (JSON, tables) — shown as-is
}

#[derive(Debug, Clone)]
pub struct SemanticBlock {
    pub class: SemanticClass,
    pub lines: Vec<String>,
    pub score: f32, // 0.0-1.0 confidence
    pub tool_family: Option<String>,
    pub line_range: (usize, usize),
}

impl SemanticBlock {
    pub fn new(
        class: SemanticClass,
        lines: Vec<String>,
        score: f32,
        tool_family: Option<String>,
        line_range: (usize, usize),
    ) -> Self {
        Self {
            class,
            lines,
            score,
            tool_family,
            line_range,
        }
    }
}

/// Classifies a block of lines into a semantic class based on patterns,
/// line density, uppercase ratio, and tool-specific heuristics.
pub fn classify_block(lines: &[&str], tool_family: Option<&str>) -> (SemanticClass, f32) {
    if lines.is_empty() {
        return (SemanticClass::Noise, 1.0);
    }

    let joined = lines.join("\n");
    let joined_lower = joined.to_lowercase();
    let is_single_line = lines.len() == 1;

    // 1. Check for Progress Bars / Noise (High priority to avoid parsing large noise)
    if is_progress_or_noise(&joined, is_single_line) {
        return (SemanticClass::Progress, 0.9);
    }

    if is_blank_or_decorative(lines) {
        return (SemanticClass::Noise, 0.9);
    }

    // 2. Critical layer (Errors, Panics, Fatal)
    if is_critical(&joined_lower, tool_family) {
        return (SemanticClass::Critical, 0.9);
    }

    // 3. Diagnostic layer (Warnings, Deprecations)
    if is_diagnostic(&joined_lower, tool_family) {
        return (SemanticClass::Diagnostic, 0.8);
    }

    // 4. Context layer (Stack traces, file paths)
    if is_context(&joined) {
        return (SemanticClass::Context, 0.7);
    }

    // 5. Data layer (JSON, tables)
    if is_data(&joined) {
        return (SemanticClass::Data, 0.8);
    }

    // Default fallback
    (SemanticClass::Context, 0.4)
}

#[allow(clippy::collapsible_if)]
fn is_progress_or_noise(text: &str, is_single_line: bool) -> bool {
    let lower = text.to_lowercase();
    if is_single_line {
        if lower.starts_with("compiling ")
            || lower.starts_with("downloading ")
            || lower.starts_with("fetching ")
            || lower.starts_with("building ")
        {
            return true;
        }
    }
    // Simple ASCII progress bar detection
    let progress_chars = text.chars().filter(|&c| c == '#' || c == '=').count();
    if progress_chars > 10 && progress_chars > text.len() / 4 {
        return true;
    }
    // Percentage detection
    if text.contains("% |") || text.contains(" | ") {
        if let Some(pos) = text.find('%') {
            if pos > 0 && text.chars().nth(pos - 1).unwrap().is_ascii_digit() {
                return true;
            }
        }
    }
    false
}

fn is_blank_or_decorative(lines: &[&str]) -> bool {
    if lines.iter().all(|l| l.trim().is_empty()) {
        return true;
    }
    // Decorative lines like "-------" or "======="
    lines.iter().all(|l| {
        let trimmed = l.trim();
        trimmed.is_empty()
            || trimmed
                .chars()
                .all(|c| c == '-' || c == '=' || c == '*' || c == '_')
    })
}

#[allow(clippy::collapsible_match)]
fn is_critical(lower_text: &str, tool_family: Option<&str>) -> bool {
    // Tool-specific critical markers
    if let Some(tool) = tool_family {
        match tool {
            "cargo" | "rustc" => {
                if lower_text.contains("error[e")
                    || lower_text.contains("panicked at")
                    || lower_text.contains("could not compile")
                {
                    return true;
                }
            }
            "npm" | "yarn" | "node" => {
                if lower_text.contains("npm err!")
                    || lower_text.contains("uncaught exception")
                    || lower_text.contains("failed to compile")
                {
                    return true;
                }
            }
            "pytest" | "python" => {
                if lower_text.contains("traceback (most recent call last):")
                    || lower_text.contains("failed (")
                    || lower_text.contains("fatal error")
                {
                    return true;
                }
            }
            _ => {}
        }
    }

    // Generic critical markers
    lower_text.contains("error:")
        || lower_text.contains("error[")
        || lower_text.contains("fatal:")
        || lower_text.contains("exception:")
        || lower_text.contains("panic:")
        || lower_text.starts_with("error ")
        || lower_text.contains("build failed")
        || lower_text.contains("--- fail")
        || mentions_failure(lower_text)
}

/// Whether `failed` appears as a *verdict* rather than incidentally.
///
/// The bare substring is not evidence of failure. Every green cargo tally reads
/// `test result: ok. 479 passed; 0 failed`, and a passing test can be named after
/// failure (`test guard::preserves_failed_lines ... ok`). Matching it anywhere
/// classified both as Critical, which is how a fully green suite came out of the
/// TestDistiller as `... 6 more failures` (#210).
///
/// Two exclusions. An identifier character on *either* side means the word is
/// part of a name rather than a report, and a preceding count of exactly zero
/// means the runner is reporting none.
// Safety: `match_indices` yields byte offsets that are always char boundaries,
// so slicing at one cannot split a UTF-8 character.
#[allow(clippy::string_slice)]
fn mentions_failure(lower_text: &str) -> bool {
    const WORD: &str = "failed";
    let is_ident = |c: char| c.is_alphanumeric() || c == '_';

    lower_text.match_indices(WORD).any(|(i, _)| {
        let before = &lower_text[..i];
        let after = &lower_text[i + WORD.len()..];

        // Inside an identifier, so it names something rather than reporting it.
        // Both sides, because a name can start with the word (`failed_to_parse`)
        // as readily as end with it (`preserves_failed_lines`).
        if before.ends_with(is_ident) || after.starts_with(is_ident) {
            return false;
        }

        // A tally reporting none: `0 failed`, `; 0 failed;`. The digit run is read
        // back-to-front, so it comes out reversed — `10 failed` yields `"01"`.
        // Only an exact zero matters here and `"0"` reversed is itself, so the
        // comparison holds; anything else, including a multi-digit count, is a
        // real failure.
        let digits_reversed: String = before
            .trim_end()
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        digits_reversed != "0"
    })
}

#[allow(clippy::collapsible_match)]
fn is_diagnostic(lower_text: &str, tool_family: Option<&str>) -> bool {
    if let Some(tool) = tool_family {
        match tool {
            "cargo" | "rustc" => {
                if lower_text.contains("warning:") {
                    return true;
                }
            }
            "npm" | "yarn" => {
                if lower_text.contains("npm warn") || lower_text.contains("warning") {
                    return true;
                }
            }
            _ => {}
        }
    }

    lower_text.contains("warning:")
        || lower_text.contains("deprecated:")
        || lower_text.contains("deprecation warning")
        || lower_text.contains("test result:")
        || lower_text.contains("--- pass")
        || lower_text.contains("diff --git")
        || lower_text.starts_with("warning[")
        || lower_text == "ok"
}

fn is_context(text: &str) -> bool {
    // Look for file paths (e.g., src/main.rs:10:5)
    if PATH_WITH_LINE.is_match(text) {
        return true;
    }

    // Look for stack trace frames
    if text.trim().starts_with("at ") && text.contains("(") && text.contains(")") {
        return true;
    }

    // Indented context (common after errors)
    if text.starts_with("    ") || text.starts_with("\t") {
        return true;
    }

    false
}

fn is_data(text: &str) -> bool {
    let trimmed = text.trim();
    // JSON
    if (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
    {
        return true;
    }

    // Simple table heuristic: multiple pipes
    if text.lines().count() > 1 && text.lines().all(|l| l.contains('|')) {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #210: `contains("failed")` made every green cargo tally Critical, and a
    /// passing test named after failure with it. Both directions are asserted,
    /// because a predicate that stops matching real failures is the worse bug.
    #[test]
    fn treats_failed_as_a_verdict_not_a_substring() {
        // Not failures: a tally reporting none, and a name containing the word.
        for green in [
            "test result: ok. 479 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out",
            "test guard::preserves_failed_lines ... ok",
            "test failed_to_parse_config ... ok",
            "tests: 0 failed, 51 passed, 51 total",
        ] {
            assert!(
                !is_critical(&green.to_lowercase(), Some("cargo")),
                "green output classified Critical: {green}"
            );
        }

        // Still failures.
        for red in [
            "test result: FAILED. 479 passed; 1 failed; 0 ignored",
            "assertion failed: left == right",
            "tests: 3 failed, 51 passed, 54 total",
        ] {
            assert!(
                is_critical(&red.to_lowercase(), Some("cargo")),
                "real failure not classified Critical: {red}"
            );
        }
    }

    #[test]
    fn test_is_progress() {
        assert!(is_progress_or_noise("Compiling omni v0.5.8", true));
        assert!(is_progress_or_noise(
            "[===============>      ] 75% | downloading",
            true
        ));
    }

    #[test]
    fn test_is_critical_cargo() {
        assert!(is_critical("error[e0308]: mismatched types", Some("cargo")));
        assert!(is_critical("thread 'main' panicked at", Some("cargo")));
    }

    #[test]
    fn test_is_critical_generic() {
        assert!(is_critical("fatal: not a git repository", None));
    }

    #[test]
    fn test_is_diagnostic() {
        assert!(is_diagnostic("warning: unused variable", Some("cargo")));
        assert!(is_diagnostic("npm warn deprecated", Some("npm")));
    }

    #[test]
    fn test_is_context() {
        assert!(is_context("  --> src/main.rs:10:5"));
        assert!(is_context(
            "    at processTicksAndRejections (node:internal/process/task_queues:96:5)"
        ));
    }

    #[test]
    fn test_is_data_json() {
        assert!(is_data(r#"{"key": "value"}"#));
        assert!(is_data("[\n  1,\n  2\n]"));
    }

    /// #283. `is_context` built its path regex with `Regex::new` on every call,
    /// and `classify_block` calls it once per line, so a single 64-character
    /// line cost 660 microseconds and an 80-line payload spent 52 ms being
    /// classified. `AGENTS.md` budgets the whole hook at 10 ms.
    ///
    /// The bound is deliberately loose. Fixed, a thousand calls take under a
    /// millisecond in release and a few milliseconds in a debug test build; the
    /// broken version needed 660 ms for the same thousand. 200 ms sits two
    /// orders of magnitude above the fixed cost and three times under the broken
    /// one, so it cannot flake the way the collapse throughput gate does (#245)
    /// while still failing the moment the compile moves back inside the call.
    #[test]
    fn classifies_a_line_without_recompiling_its_regex() {
        let lines = vec!["6d47f1a Point sign-in at our own page, not Clerk's hosted portal"];

        let start = std::time::Instant::now();
        for _ in 0..1_000 {
            let _ = classify_block(&lines, Some("git"));
        }
        let elapsed = start.elapsed();

        assert!(
            elapsed < std::time::Duration::from_millis(200),
            "1,000 classifications took {elapsed:?}; a regex is being compiled per call again"
        );
    }
}
