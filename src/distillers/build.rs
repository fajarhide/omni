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

        for seg in segments {
            if seg.tier == SignalTier::Critical {
                errors.push(seg.content.clone());
            } else if seg.tier == SignalTier::Important {
                warnings.push(seg.content.clone());
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
