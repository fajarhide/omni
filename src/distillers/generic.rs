use crate::distillers::Distiller;
use crate::pipeline::OutputSegment;

pub struct GenericDistiller;

impl Distiller for GenericDistiller {
    fn distill(
        &self,
        segments: &[OutputSegment],
        _input: &str,
        _session: Option<&crate::pipeline::SessionState>,
    ) -> String {
        let mut out = String::new();
        let max_lines = 100;

        for (i, seg) in segments.iter().enumerate() {
            if i < max_lines {
                out.push_str(&seg.content);
                out.push('\n');
            } else {
                out.push_str(&format!(
                    "... [{} more lines]\n",
                    segments.len() - max_lines
                ));
                break;
            }
        }

        out.trim().to_string()
    }
}
