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

pub fn compose(
    segments: Vec<OutputSegment>,
    dropped_content: Option<String>,
    config: &ComposeConfig,
    store: Option<&Store>,
    // Metadata hooks for learn logics
    original_text: &str,
    route: &ContentType,
) -> (String, Option<String>) {
    if segments.is_empty() && !original_text.is_empty() {
        return ("".to_string(), None); // Fully dropped
    }

    // Auto-Learn execution: evaluates Passthrough outputs > 200 chars identifying potential noise natively.
    if matches!(route, ContentType::Unknown) && original_text.len() > 200 {
        crate::session::learn::queue_for_learn(original_text, "omni_passthrough_eval");
    }

    let mut kept_ordered: Vec<&OutputSegment> = segments
        .iter()
        .filter(|s| s.final_score() >= config.threshold)
        .collect();

    let dropped_count = segments.len() - kept_ordered.len();
    let dropped_lines: usize = segments
        .iter()
        .filter(|s| s.final_score() < config.threshold)
        .map(|s| s.content.lines().count())
        .sum();

    // Preserve original line order for coherent output structures
    kept_ordered.sort_by_key(|s| s.line_range.0);

    let mut output = String::new();
    for segment in &kept_ordered {
        output.push_str(&segment.content);
        if !segment.content.ends_with('\n') {
            output.push('\n');
        }
    }

    let mut rewind_hash = None;

    if dropped_count > 0
        && let Some(content) = dropped_content
    {
        let critical_kept: usize = kept_ordered
            .iter()
            .filter(|s| s.tier == crate::pipeline::SignalTier::Critical)
            .map(|s| s.content.lines().count())
            .sum();

        if let Some(s) = store {
            let hash = s.store_rewind(&content);
            output.push_str(&format!(
                "\n[omni: filtered {} noise lines. {} critical lines kept. id: {}]\n",
                dropped_lines, critical_kept, hash
            ));
            rewind_hash = Some(hash);
        } else {
            output.push_str(&format!(
                "\n[omni: filtered {} noise lines. {} critical lines kept]\n",
                dropped_lines, critical_kept
            ));
        }
    }

    if output.len() > config.max_output_chars {
        output.truncate(config.max_output_chars);
        output.push_str("\n[OMNI: output truncated]\n");
    }

    (output, rewind_hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::SignalTier;
    use tempfile::tempdir;

    fn create_segment(
        content: &str,
        tier: SignalTier,
        base_score: f32,
        line: usize,
    ) -> OutputSegment {
        OutputSegment {
            content: content.to_string(),
            tier,
            base_score,
            context_score: 0.0,
            line_range: (line, line),
        }
    }

    #[test]
    fn test_compose_keeps_critical_segments() {
        let segments = vec![
            create_segment("error: bad failure", SignalTier::Critical, 0.9, 1),
            create_segment("compiling lib", SignalTier::Noise, 0.05, 2),
            create_segment("warning: unused msg", SignalTier::Important, 0.7, 3),
        ];

        let config = ComposeConfig {
            threshold: 0.3,
            max_output_chars: 1000,
            rewind_store: None,
        };

        let (out, hash) = compose(segments, None, &config, None, "", &ContentType::Unknown);
        assert!(out.contains("error: bad failure"));
        assert!(out.contains("warning: unused msg"));
        assert!(!out.contains("compiling lib"));
        assert_eq!(hash, None); // Hash None because dropped_content is None
    }

    #[test]
    fn test_compose_drops_noise_segments_below_threshold() {
        let segments = vec![
            create_segment("A", SignalTier::Noise, 0.1, 1),
            create_segment("B", SignalTier::Critical, 0.9, 2),
            create_segment("C", SignalTier::Noise, 0.2, 3),
        ];

        let (out, _) = compose(
            segments,
            None,
            &ComposeConfig::default(),
            None,
            "",
            &ContentType::Unknown,
        );
        assert_eq!(out, "B\n");
    }

    #[test]
    fn test_compose_adds_rewind_notice_when_content_dropped() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("omni.db");
        let store = Store::open_path(&db_path).unwrap();

        let segments = vec![
            create_segment("bad loop", SignalTier::Critical, 0.9, 1),
            create_segment("ignored noise", SignalTier::Noise, 0.05, 2),
        ];

        let dropped = Some("ignored noise".to_string());
        let (out, hash) = compose(
            segments,
            dropped,
            &ComposeConfig::default(),
            Some(&store),
            "",
            &ContentType::Unknown,
        );

        assert!(out.contains("bad loop"));
        assert!(!out.contains("ignored noise")); // Not in main output
        assert!(out.contains("[omni: filtered 1 noise lines"));
        assert!(hash.is_some());
    }

    #[test]
    fn test_compose_preserves_original_line_order() {
        let segments = vec![
            create_segment("Critical 2", SignalTier::Critical, 0.9, 3),
            create_segment("Critical 1", SignalTier::Important, 0.8, 1),
        ];

        let (out, _) = compose(
            segments,
            None,
            &ComposeConfig::default(),
            None,
            "",
            &ContentType::Unknown,
        );
        assert_eq!(out, "Critical 1\nCritical 2\n");
    }

    #[test]
    fn test_compose_safety_truncation_at_max_output_chars() {
        let content = "a".repeat(100);
        let segments = vec![create_segment(&content, SignalTier::Critical, 0.9, 1)];

        let config = ComposeConfig {
            threshold: 0.3,
            max_output_chars: 50,
            rewind_store: None,
        };

        let (out, _) = compose(segments, None, &config, None, "", &ContentType::Unknown);
        // Truncated to exactly 50 bytes + \n[OMNI: output truncated]\n
        assert!(out.starts_with(&"a".repeat(50)));
        assert!(out.contains("output truncated"));
    }

    #[test]
    fn test_compose_dengan_empty_segments_returns_empty() {
        let (out, _) = compose(
            vec![],
            None,
            &ComposeConfig::default(),
            None,
            "",
            &ContentType::Unknown,
        );
        assert_eq!(out, "");
    }

    #[test]
    fn test_rewind_store_roundtrip_via_compose_retrieve() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("omni.db");
        let store = Store::open_path(&db_path).unwrap();

        let segments = vec![
            create_segment("error!", SignalTier::Critical, 0.9, 1),
            create_segment("dropped detail", SignalTier::Noise, 0.1, 2),
        ];

        let dropped_str = "error!\ndropped detail";
        let (_, hash_opt) = compose(
            segments,
            Some(dropped_str.to_string()),
            &ComposeConfig::default(),
            Some(&store),
            "",
            &ContentType::Unknown,
        );

        let hash = hash_opt.unwrap();
        // Retrieve full payload back
        let retrieved = store.retrieve_rewind(&hash).unwrap();
        assert_eq!(retrieved, dropped_str);
    }
}
