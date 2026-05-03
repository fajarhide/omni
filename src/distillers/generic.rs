use crate::distillers::Distiller;
use crate::pipeline::{OutputSegment, SignalTier};
use std::collections::HashSet;

pub struct GenericDistiller;

impl Distiller for GenericDistiller {
    fn distill(
        &self,
        segments: &[OutputSegment],
        _input: &str,
        _session: Option<&crate::pipeline::SessionState>,
    ) -> String {
        let max_lines = 100;

        if segments.len() <= max_lines {
            return segments
                .iter()
                .map(|s| s.content.as_str())
                .collect::<Vec<_>>()
                .join("\n")
                .trim()
                .to_string();
        }

        let mut selected_indices = HashSet::new();

        // Pass 1: Select Critical & Important
        for (i, seg) in segments.iter().enumerate() {
            if matches!(seg.tier, SignalTier::Critical | SignalTier::Important)
                && selected_indices.len() < max_lines
            {
                selected_indices.insert(i);
            }
        }

        // Pass 2: Fill remaining budget with Context/Noise
        for i in 0..segments.len() {
            if selected_indices.len() >= max_lines {
                break;
            }
            selected_indices.insert(i);
        }

        // Build output maintaining original order
        let mut out = String::new();
        let mut last_idx: Option<usize> = None;

        for (i, seg) in segments.iter().enumerate() {
            if selected_indices.contains(&i) {
                if let Some(last) = last_idx
                    && i > last + 1
                {
                    out.push_str("... [omitted]\n");
                }
                out.push_str(&seg.content);
                out.push('\n');
                last_idx = Some(i);
            }
        }

        if let Some(last) = last_idx
            && last < segments.len() - 1
        {
            out.push_str(&format!("... [{} more lines]\n", segments.len() - 1 - last));
        }

        out.trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generic_distiller_prioritizes_important() {
        let mut segments = Vec::new();
        for i in 0..150 {
            let tier = if i == 120 {
                SignalTier::Critical
            } else {
                SignalTier::Noise
            };
            segments.push(OutputSegment {
                content: format!("Line {}", i),
                tier,
                base_score: 0.0,
                context_score: 0.0,
                line_range: (i, i),
            });
        }

        let distiller = GenericDistiller;
        let output = distiller.distill(&segments, "", None);

        assert!(
            output.contains("Line 120"),
            "Critical line must be preserved even if it's beyond the 100 line limit"
        );
        assert!(output.contains("... [omitted]"));
    }
}
