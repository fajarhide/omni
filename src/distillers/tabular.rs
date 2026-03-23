use crate::distillers::Distiller;
use crate::pipeline::{ContentType, OutputSegment};

pub struct TabularDistiller;

impl Distiller for TabularDistiller {
    fn content_type(&self) -> ContentType {
        ContentType::TabularData
    }

    fn distill(&self, segments: &[OutputSegment], _input: &str) -> String {
        let mut out = String::new();
        let max_rows = 20;

        for (i, seg) in segments.iter().enumerate() {
            if i < max_rows {
                let cleaned = seg.content.split_whitespace().collect::<Vec<_>>().join(" ");
                out.push_str(&cleaned);
                out.push('\n');
            } else {
                out.push_str(&format!("... [{} more rows]\n", segments.len() - max_rows));
                break;
            }
        }

        out.trim().to_string()
    }
}
