use crate::pipeline::{ContentType, OutputSegment};
use crate::store::sqlite::Store;
use std::sync::Arc;

pub struct RewindDecision {
    pub should_store: bool,
    pub threshold: f32, // segments below this go to RewindStore
}

pub fn decide_rewind(segments: &[OutputSegment], _content_type: &ContentType) -> RewindDecision {
    let total = segments.len().max(1) as f32;
    let noise_count = segments.iter().filter(|s| s.final_score() < 0.3).count();
    let noise_ratio = noise_count as f32 / total;

    // If >40% will be dropped → activate RewindStore
    RewindDecision {
        should_store: noise_ratio > 0.4 && segments.len() > 20,
        threshold: 0.3,
    }
}

pub struct ComposeConfig {
    pub threshold: f32,          // segments di bawah threshold di-drop
    pub max_output_chars: usize, // 50000 chars max (safety)
    pub rewind_store: Option<Arc<Store>>,
}

impl Default for ComposeConfig {
    fn default() -> Self {
        Self {
            threshold: 0.3,
            max_output_chars: 50_000,
            rewind_store: None,
        }
    }
}

pub fn evaluate_learning(
    route: &ContentType,
    original_text: &str,
    input_lines_count: usize,
    dropped_lines_count: usize,
    command: &str,
) {
    if original_text.len() < 100 || matches!(route, ContentType::StructuredData) {
        return;
    }

    let poor_distillation = input_lines_count > 5
        && (dropped_lines_count as f32 / input_lines_count.max(1) as f32) < 0.3;

    if matches!(route, ContentType::Unknown) || poor_distillation {
        let category = if command.is_empty() {
            format!("omni_eval_{:?}", route).to_lowercase()
        } else {
            command.to_string()
        };
        crate::session::learn::queue_for_learn(original_text, &category);
    }
}

#[cfg(test)]
mod tests {
    // Tests specific to decide_rewind and evaluate_learning would go here
}
