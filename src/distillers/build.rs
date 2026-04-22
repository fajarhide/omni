use crate::distillers::Distiller;
use crate::pipeline::{OutputSegment, SignalTier};

pub struct BuildDistiller;

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
