use crate::distillers::Distiller;
use crate::pipeline::{OutputSegment, SignalTier};

pub struct TestDistiller;

/// Every major runner states its own totals on one summary line — cargo
/// `test result: FAILED. 490 passed; 10 failed; ...`, pytest `3 failed, 42 passed
/// in 3.15s`, jest `Tests: 3 failed, 51 passed, 54 total`. Quote it instead of
/// recounting: the runner is authoritative, and counting result lines is both
/// fragile and wrong here — cargo_test_500 prints 330 `... ok` lines for 490
/// passing tests, and CollapseMode::Test folds those lines away before the
/// distiller ever sees them.
fn runner_summary(input: &str) -> Option<&str> {
    input.lines().map(str::trim).find(|line| {
        line.starts_with("test result:")
            || line.starts_with("Tests:")
            || (line.contains(" passed") && line.contains(" in "))
    })
}

impl Distiller for TestDistiller {
    fn distill(
        &self,
        segments: &[OutputSegment],
        input: &str,
        _session: Option<&crate::pipeline::SessionState>,
    ) -> String {
        let mut passed = 0;
        let mut failed = 0;
        let mut failure_details = Vec::new();
        let summary = runner_summary(input);

        for seg in segments {
            if seg.tier == SignalTier::Critical
                || seg.content.contains("FAIL")
                || seg.content.contains('✗')
            {
                failed += 1;
                // Avoid pushing pure summary lines as failure details if they are just the aggregate count
                if !seg.content.to_lowercase().contains("failed tests/")
                    && !seg.content.contains("===")
                {
                    // Truncate to max 12 lines to keep just the assertion and stack trace
                    let lines: Vec<&str> = seg.content.lines().collect();
                    if lines.len() > 12 {
                        let truncated =
                            lines[..12].join("\n") + "\n       ... [stack trace truncated]";
                        failure_details.push(truncated);
                    } else {
                        failure_details.push(seg.content.clone());
                    }
                }
            } else if seg.tier == SignalTier::Important
                || seg.content.contains("PASS")
                || seg.content.contains('✓')
                || seg.content.contains("ok")
            {
                passed += 1;
            }
        }

        // Try to find explicit summary in input
        for line in input.lines() {
            // The summary line is the headline below; pytest's
            // `=== 1 failed, 2 passed ===` also matches "failed", so skip it here
            // or it gets printed twice.
            if Some(line.trim()) == summary {
                continue;
            }
            let lower = line.to_lowercase();
            if (lower.contains("failed") || lower.contains("error:") || lower.contains("err "))
                && !failure_details.contains(&line.to_string())
            {
                failure_details.push(line.to_string());
            }
        }

        let mut out = String::new();

        // Prefer the runner's own tally; fall back to counting only when it
        // didn't print one (interrupted run, custom harness).
        let headline = summary
            .map(str::to_string)
            .unwrap_or_else(|| format!("Tests: {} passed, {} failed", passed, failed));

        if failed == 0 && failure_details.is_empty() {
            return headline;
        }

        out.push_str(&headline);
        out.push('\n');

        let max_fails = 10;
        for (i, fail) in failure_details.iter().enumerate() {
            if i < max_fails {
                out.push_str(fail);
                out.push('\n');
            } else {
                out.push_str(&format!(
                    "... {} more failures\n",
                    failure_details.len() - max_fails
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
    use crate::pipeline::{collapse, registry, scorer};

    /// Runs the real collapse → score → distill composition, because that is where
    /// the bug lived: each stage was correct alone. CollapseMode::Test folds the
    /// 330 `... ok` lines into one marker, and the distiller then counted segments
    /// and reported `1 passed` for a run cargo itself called 490 passed.
    #[test]
    fn reports_runner_totals_not_a_recount_of_collapsed_lines() {
        // Arrange
        let input = include_str!("../../tests/fixtures/cargo_test_500.txt");
        let profile = registry::resolve_profile("cargo test");
        let collapsed = collapse::collapse(input, &profile.collapse)
            .collapsed_lines
            .join("\n");
        let segments = scorer::score_segments(&collapsed, profile.segmentation, None, "cargo test");

        // Act
        let output = TestDistiller.distill(&segments, &collapsed, None);

        // Assert
        assert!(
            output.starts_with("test result: FAILED. 490 passed; 10 failed"),
            "expected cargo's own tally as the headline, got: {}",
            output.lines().next().unwrap_or("")
        );
    }

    /// Without a summary line there is nothing to quote, so counting is the
    /// fallback — but it must not crash or invent a tally.
    #[test]
    fn falls_back_to_counting_when_runner_printed_no_summary() {
        // Arrange
        let input = "test alpha ... ok\ntest beta ... ok";
        let segments = scorer::score_segments(
            input,
            registry::resolve_profile("cargo test").segmentation,
            None,
            "cargo test",
        );

        // Act
        let output = TestDistiller.distill(&segments, input, None);

        // Assert
        assert!(
            output.starts_with("Tests:"),
            "expected the counted fallback, got: {}",
            output
        );
    }
}
