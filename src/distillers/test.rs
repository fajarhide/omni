use crate::distillers::Distiller;
use crate::pipeline::{OutputSegment, SignalTier};

pub struct TestDistiller;

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

        for seg in segments {
            if seg.tier == SignalTier::Critical
                || seg.content.contains("FAIL")
                || seg.content.contains('✗')
            {
                failed += 1;
                // Avoid pushing pure summary lines as failure details if they are just the aggregate count
                if !seg.content.starts_with("FAILED tests/") && !seg.content.starts_with("===") {
                    failure_details.push(seg.content.clone());
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
            if line.starts_with("FAILED ") && !failure_details.contains(&line.to_string()) {
                failure_details.push(line.to_string());
            }
        }

        let mut out = String::new();

        if failed == 0 && failure_details.is_empty() {
            return format!("Tests: {} passed ✓", passed);
        }

        out.push_str(&format!("Tests: {} passed, {} failed\n", passed, failed));

        let max_fails = 5;
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
