use crate::distillers::Distiller;
use crate::pipeline::{OutputSegment, SignalTier};

pub struct BuildDistiller;

/// Detect single-line diagnostic format used by Python tools (mypy, ruff, pylint).
/// Pattern: "filepath:line:col: severity: message" or "filepath:line: severity: message"
/// Must NOT match Rust compiler location lines like " --> src/main.rs:1:5"
fn is_single_line_diagnostic(content: &str) -> bool {
    let trimmed = content.trim();
    // Exclude Rust compiler location lines
    if trimmed.starts_with("-->") || trimmed.starts_with('|') || trimmed.starts_with("=") {
        return false;
    }
    let parts: Vec<&str> = trimmed.splitn(4, ':').collect();
    if parts.len() >= 3 {
        let filepath = parts[0].trim();
        let potential_line = parts[1].trim();
        // filepath must look like a path (contain . or /) and not be empty
        let looks_like_path = !filepath.is_empty()
            && (filepath.contains('.') || filepath.contains('/'))
            && !filepath.contains(' ');
        return looks_like_path
            && !potential_line.is_empty()
            && potential_line.chars().all(|c| c.is_ascii_digit());
    }
    false
}

impl Distiller for BuildDistiller {
    fn distill(
        &self,
        segments: &[OutputSegment],
        _input: &str,
        _session: Option<&crate::pipeline::SessionState>,
    ) -> String {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut current_block = Vec::new();
        let mut is_error_block = false;

        for seg in segments {
            // F-08: Handle single-line diagnostic format (Python/mypy/ruff/pylint)
            // These may be classified as Context by classify_line since ruff codes
            // (E501, F401) don't match standard error/warning keywords
            if is_single_line_diagnostic(&seg.content) {
                // Flush any pending block first
                if !current_block.is_empty() {
                    if is_error_block {
                        errors.push(current_block.join("\n"));
                    } else {
                        warnings.push(current_block.join("\n"));
                    }
                    current_block.clear();
                }
                // Classify based on content: "error" keyword → error, else warning
                let is_error = seg.tier == SignalTier::Critical
                    || seg.content.contains(": error:")
                    || seg.content.contains("ERROR:");
                if is_error {
                    errors.push(seg.content.clone());
                } else {
                    warnings.push(seg.content.clone());
                }
                continue;
            }

            if seg.tier == SignalTier::Critical || seg.tier == SignalTier::Important {
                if current_block.is_empty() {
                    is_error_block = seg.tier == SignalTier::Critical;
                }
                // If we see a new critical and we're currently in a warning block,
                // or if it's a clear new error boundary, flush it
                if seg.tier == SignalTier::Critical && !current_block.is_empty() && !is_error_block
                {
                    warnings.push(current_block.join("\n"));
                    current_block.clear();
                    is_error_block = true;
                }
                current_block.push(seg.content.clone());
            } else {
                if !current_block.is_empty() {
                    if is_error_block {
                        errors.push(current_block.join("\n"));
                    } else {
                        warnings.push(current_block.join("\n"));
                    }
                    current_block.clear();
                }
            }
        }
        if !current_block.is_empty() {
            if is_error_block {
                errors.push(current_block.join("\n"));
            } else {
                warnings.push(current_block.join("\n"));
            }
        }

        let mut out = String::new();

        if errors.is_empty() && warnings.is_empty() {
            return "Build: ok".to_string();
        }

        out.push_str(&format!(
            "Build: {} errors, {} warnings\n",
            errors.len(),
            warnings.len()
        ));

        for err in &errors {
            out.push_str(err);
            out.push('\n');
        }

        let max_warns = 5;
        for (i, warn) in warnings.iter().enumerate() {
            if i < max_warns {
                out.push_str(warn);
                out.push('\n');
            } else {
                out.push_str(&format!(
                    "... {} more warnings\n",
                    warnings.len() - max_warns
                ));
                break;
            }
        }

        out.trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::scorer;

    #[test]
    fn test_is_single_line_diagnostic() {
        // True: Python diagnostic formats
        assert!(is_single_line_diagnostic(
            "src/auth.py:42: error: incompatible type"
        ));
        assert!(is_single_line_diagnostic(
            "src/main.py:15:80: E501 Line too long"
        ));
        assert!(is_single_line_diagnostic(
            "src/utils.py:8:1: F401 imported but unused"
        ));
        // False: Rust compiler output
        assert!(!is_single_line_diagnostic("error[E0308]: mismatched types"));
        assert!(!is_single_line_diagnostic(" --> src/main.rs:1:5"));
        assert!(!is_single_line_diagnostic("  |"));
        assert!(!is_single_line_diagnostic("1 | use std::collections::Foo;"));
        // False: general
        assert!(!is_single_line_diagnostic("normal output line"));
        assert!(!is_single_line_diagnostic(""));
    }

    #[test]
    fn test_build_distiller_handles_mypy_format() {
        let mypy_output = "\
src/auth.py:42: error: Argument 1 to \"login\" has incompatible type \"str\"; expected \"int\"
src/auth.py:67: error: Name \"user_id\" is not defined
src/models.py:15: note: See https://mypy.rtfd.io for help
Found 2 errors in 2 files (checked 5 source files)
";
        let segments =
            scorer::score_segments(mypy_output, crate::pipeline::SegmentationMode::Line, None);
        let output = BuildDistiller.distill(&segments, mypy_output, None);
        assert!(
            output.contains("errors"),
            "Must report error count: {}",
            output
        );
        assert!(
            output.contains("auth.py:42"),
            "Must include first error location: {}",
            output
        );
        assert!(
            output.contains("auth.py:67"),
            "Must include second error location: {}",
            output
        );
    }

    #[test]
    fn test_build_distiller_handles_ruff_format() {
        let ruff_output = "\
src/main.py:1:1: I001 Import block is un-sorted or un-formatted
src/main.py:15:80: E501 Line too long (92 > 79 characters)
src/utils.py:8:1: F401 `os` imported but unused
Found 3 errors.
";
        let segments =
            scorer::score_segments(ruff_output, crate::pipeline::SegmentationMode::Line, None);
        let output = BuildDistiller.distill(&segments, ruff_output, None);
        assert!(
            output.contains("main.py:15"),
            "Must include line location for ruff error: {}",
            output
        );
    }
}
