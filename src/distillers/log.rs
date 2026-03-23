use crate::distillers::Distiller;
use crate::pipeline::{ContentType, OutputSegment, SignalTier};

pub struct LogDistiller;

impl Distiller for LogDistiller {
    fn content_type(&self) -> ContentType {
        ContentType::LogOutput
    }

    fn distill(&self, segments: &[OutputSegment], _input: &str) -> String {
        let mut out = String::new();
        let mut i = 0;

        while i < segments.len() {
            let seg = &segments[i];

            if seg.tier == SignalTier::Critical || seg.tier == SignalTier::Important {
                if i > 0 && segments[i - 1].tier == SignalTier::Context {
                    out.push_str(&segments[i - 1].content);
                    out.push('\n');
                }

                out.push_str(&seg.content);
                out.push('\n');

                if i + 1 < segments.len() && segments[i + 1].tier == SignalTier::Context {
                    out.push_str(&segments[i + 1].content);
                    out.push('\n');
                    i += 1;
                }
            }
            i += 1;
        }

        let trimmed = out.trim().to_string();
        if trimmed.is_empty() {
            // Nothing critical/important? Fallback to generic compression
            let max_lines = 10;
            let mut fallback = String::new();
            for (idx, seg) in segments.iter().enumerate() {
                if idx < max_lines {
                    fallback.push_str(&seg.content);
                    fallback.push('\n');
                } else {
                    fallback.push_str(&format!(
                        "... [{} more log lines]",
                        segments.len() - max_lines
                    ));
                    break;
                }
            }
            fallback.trim().to_string()
        } else {
            trimmed
        }
    }
}
