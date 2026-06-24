/// [INT-01] Adaptive Scoring Feedback Loop
///
/// Analyzes the `retrieval_feedback` table to surface actionable insights
/// about OMNI's distillation effectiveness — without any LLM calls.
/// All analysis is rule-based and runs on-demand (not in background).
use crate::store::sqlite::Store;

/// The type of insight OMNI has detected.
#[derive(Debug, Clone, PartialEq)]
pub enum InsightType {
    /// A command is recalled frequently → distillation may be too aggressive.
    OverFiltered,
    /// Stored knowledge items that have never been retrieved → may be stale.
    Underused,
}

/// A single adaptive insight produced by pattern analysis.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AdaptiveInsight {
    pub insight_type: InsightType,
    pub description: String,
    pub affected_item: Option<String>,
    pub suggested_action: String,
}

/// Analyze retrieval patterns and return actionable insights.
///
/// This is the core of [INT-01]. It runs two passes:
/// 1. Commands recalled frequently in the last 7 days (OverFiltered signal).
/// 2. Knowledge items never retrieved (Underused signal).
pub fn analyze(store: &Store, project_hash: &str) -> Vec<AdaptiveInsight> {
    let mut insights = Vec::new();

    // Pass 1: over-filtered commands
    let frequent = store.get_frequent_recall_commands(project_hash, 3, 7);
    for (cmd, count) in frequent {
        let binary = cmd.split_whitespace().next().unwrap_or(&cmd).to_string();
        insights.push(AdaptiveInsight {
            insight_type: InsightType::OverFiltered,
            description: format!(
                "`{}` was recalled {} time(s) in the last 7 days — distillation may be too aggressive.",
                cmd, count
            ),
            affected_item: Some(cmd.clone()),
            suggested_action: format!(
                "Run `omni learn --loosen {}` to relax the filter.",
                binary
            ),
        });
    }

    // Pass 2: underused knowledge
    let unreferenced = store.get_unreferenced_knowledge(project_hash);
    for key in unreferenced {
        insights.push(AdaptiveInsight {
            insight_type: InsightType::Underused,
            description: format!(
                "Knowledge entry `{}` has never been recalled — it may be outdated or irrelevant.",
                key
            ),
            affected_item: Some(key),
            suggested_action:
                "Review with `omni recall <topic>` and delete stale entries with `omni knowledge forget <key>`."
                    .to_string(),
        });
    }

    insights
}

/// Format a Vec of insights into a human-readable MCP response string.
pub fn format_insights(insights: &[AdaptiveInsight]) -> String {
    if insights.is_empty() {
        return "No adaptive insights available yet. Keep using OMNI — patterns will emerge over time.".to_string();
    }

    let mut out = format!("## OMNI Adaptive Insights ({} found)\n\n", insights.len());
    for (i, ins) in insights.iter().enumerate() {
        let tag = match ins.insight_type {
            InsightType::OverFiltered => "⚡ Over-Filtered",
            InsightType::Underused => "📦 Underused",
        };
        out.push_str(&format!("{}. [{}]\n", i + 1, tag));
        out.push_str(&format!("   {}\n", ins.description));
        out.push_str(&format!("   → {}\n\n", ins.suggested_action));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tempfile::tempdir;

    fn get_store() -> (Arc<Store>, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("omni.db");
        (Arc::new(Store::open_path(&db_path).unwrap()), dir)
    }

    #[test]
    fn empty_db_returns_no_insights() {
        let (store, _dir) = get_store();
        let insights = analyze(&store, "abc123");
        assert!(insights.is_empty());
    }

    #[test]
    fn format_empty_insights_returns_placeholder() {
        let result = format_insights(&[]);
        assert!(result.contains("No adaptive insights"));
    }

    #[test]
    fn format_insights_renders_both_types() {
        let insights = vec![
            AdaptiveInsight {
                insight_type: InsightType::OverFiltered,
                description: "cargo test recalled 5x".to_string(),
                affected_item: Some("cargo test".to_string()),
                suggested_action: "omni learn --loosen cargo".to_string(),
            },
            AdaptiveInsight {
                insight_type: InsightType::Underused,
                description: "key 'old-fact' never recalled".to_string(),
                affected_item: Some("old-fact".to_string()),
                suggested_action: "Review or delete".to_string(),
            },
        ];
        let out = format_insights(&insights);
        assert!(out.contains("Over-Filtered"));
        assert!(out.contains("Underused"));
        assert!(out.contains("cargo test"));
    }
}
