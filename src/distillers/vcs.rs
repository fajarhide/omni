use crate::distillers::Distiller;
use crate::pipeline::OutputSegment;

pub struct VcsDistiller;

impl Distiller for VcsDistiller {
    fn distill(
        &self,
        _segments: &[OutputSegment],
        input: &str,
        _session: Option<&crate::pipeline::SessionState>,
    ) -> String {
        let lines: Vec<&str> = input.lines().filter(|l| !l.trim().is_empty()).collect();
        let total = lines.len();

        if total <= 10 {
            return input.trim().to_string();
        }

        // PR/Issue list — show first 10, summarize rest
        let shown: Vec<&str> = lines.iter().take(10).copied().collect();
        let mut out = shown.join("\n");
        if total > 10 {
            out.push_str(&format!(
                "\n... [{} more items — use --limit to see more]",
                total - 10
            ));
        }
        out
    }
}
