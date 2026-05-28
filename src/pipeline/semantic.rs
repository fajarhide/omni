use regex::Regex;
use serde::{Deserialize, Serialize};

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
        || lower_text.contains("failed")
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
    let path_regex = Regex::new(r"[\w\./\-]+\.\w+:\d+(:\d+)?").unwrap();
    if path_regex.is_match(text) {
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
}
