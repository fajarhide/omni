use crate::store::sqlite::Store;
use colored::*;
use std::sync::Arc;

#[derive(serde::Serialize)]
#[allow(dead_code)]
pub struct HandoffJson {
    pub version: String,
    pub session_id: String,
    pub markdown_export: String,
}

// ─── Handoff Schema v2 (L2-04) ─────────────────────────────

#[derive(serde::Serialize)]
pub struct HandoffJsonV2 {
    pub schema_version: u32,
    pub session_id: String,
    pub agent: String,
    pub loop_context: Option<HandoffLoopContext>,
    pub progress: HandoffProgress,
    pub context: HandoffContext,
    pub pressure: HandoffPressure,
    pub recommendation: HandoffRecommendation,
}

#[derive(serde::Serialize)]
pub struct HandoffLoopContext {
    pub loop_id: String,
    pub iteration: u32,
    pub goal: String,
    pub budget_used_tokens: u64,
    pub budget_remaining_tokens: u64,
}

#[derive(serde::Serialize)]
pub struct HandoffProgress {
    pub completed: Vec<HandoffEngram>,
    pub active_errors: Vec<String>,
}

#[derive(serde::Serialize)]
pub struct HandoffEngram {
    pub label: String,
    pub trigger: String,
}

#[derive(serde::Serialize)]
pub struct HandoffHotFile {
    pub path: String,
    pub access_count: u32,
}

#[derive(serde::Serialize)]
pub struct HandoffContext {
    pub hot_files: Vec<HandoffHotFile>,
    pub recent_commands: Vec<String>,
    pub task: String,
    pub domain: String,
}

#[derive(serde::Serialize)]
pub struct HandoffPressure {
    pub level: String,
}

#[derive(serde::Serialize)]
pub struct HandoffRecommendation {
    pub action: String,
    pub reason: String,
}

fn compute_recommendation(state: &crate::pipeline::SessionState) -> (String, String) {
    if state.active_errors.is_empty() && !state.engrams.is_empty() {
        return (
            "DONE".to_string(),
            "No active errors, engrams indicate progress.".to_string(),
        );
    }
    let pressure = format!("{}", state.context_pressure);
    if pressure == "Critical" {
        return (
            "COMPACT_AND_CONTINUE".to_string(),
            "Context pressure critical — compact before next iteration.".to_string(),
        );
    }
    if !state.active_errors.is_empty() {
        return (
            "CONTINUE".to_string(),
            format!("{} active errors remaining.", state.active_errors.len()),
        );
    }
    (
        "CONTINUE".to_string(),
        "Session active, no blockers.".to_string(),
    )
}

pub fn run_handoff(args: &[String], store: Arc<Store>) -> anyhow::Result<()> {
    if args
        .iter()
        .any(|a| a == "--help" || a == "-h" || a == "help")
    {
        println!("omni handoff — Export session state for context transfer");
        return Ok(());
    }

    let is_json = args.iter().any(|a| a == "--json");

    let state = match store.find_latest_session() {
        Some(s) => s,
        None => {
            if is_json {
                println!("{{}}");
            } else {
                println!("No active session found to handoff.");
            }
            return Ok(());
        }
    };

    let task = state
        .inferred_task
        .as_deref()
        .unwrap_or("general development");
    let domain = state.inferred_domain.as_deref().unwrap_or("unknown");
    let agent_id = std::env::var("OMNI_AGENT_ID")
        .unwrap_or_else(|_| crate::agents::multiagent::detect_agent_id());

    let mut md = String::from("# OMNI Session Handoff\n\n");
    md.push_str(&format!("**Session:** {}\n", state.session_id));
    md.push_str(&format!("**Agent:** {}\n", agent_id));
    md.push_str(&format!("**Task:** {}\n", task));
    md.push_str(&format!("**Domain:** {}\n", domain));
    md.push_str(&format!("**Commands:** {}\n", state.command_count));
    md.push_str(&format!(
        "**Tokens Saved:** ~{}\n\n",
        state.estimated_tokens_saved()
    ));

    md.push_str("## Active Errors\n");
    if state.active_errors.is_empty() {
        md.push_str("None\n");
    } else {
        for err in &state.active_errors {
            let clean = err.replace('\n', " ");
            md.push_str(&format!(
                "- {}\n",
                clean.chars().take(120).collect::<String>()
            ));
        }
    }

    md.push_str("\n## Subtask Progress\n");
    if state.engrams.is_empty() {
        md.push_str("No engrams recorded.\n");
    } else {
        for engram in &state.engrams {
            md.push_str(&engram.compact());
            md.push('\n');
        }
    }

    md.push_str("\n## Hot Files\n");
    let mut hot_vec: Vec<(&String, &u32)> = state.hot_files.iter().collect();
    hot_vec.sort_by_key(|a| std::cmp::Reverse(a.1));
    if hot_vec.is_empty() {
        md.push_str("None\n");
    } else {
        for (path, count) in hot_vec.iter().take(10) {
            md.push_str(&format!("- {} ({}x)\n", path, count));
        }
    }

    let tool_summary = crate::session::engram::format_tool_summary(&state.tool_call_log);
    if !tool_summary.is_empty() {
        md.push('\n');
        md.push_str(&tool_summary);
    }

    md.push_str("\n## Recent Commands\n");
    for cmd in state.last_commands.iter().take(10) {
        md.push_str(&format!(
            "- `{}`\n",
            cmd.chars().take(80).collect::<String>()
        ));
    }

    md.push_str(&format!(
        "\n## Context Pressure: {}\n",
        state.context_pressure
    ));
    md.push_str("\n---\n*Paste this into a new session to continue where you left off.*\n");

    if is_json {
        // Emit v2 structured schema
        let loop_ctx = if state.loop_context.loop_id.is_some() {
            let lc = &state.loop_context;
            let budget_total = lc.budget_tokens.unwrap_or(0);
            let budget_used = lc.budget_used;
            Some(HandoffLoopContext {
                loop_id: lc.loop_id.clone().unwrap_or_default(),
                iteration: lc.iteration,
                goal: lc.goal.clone().unwrap_or_default(),
                budget_used_tokens: budget_used,
                budget_remaining_tokens: budget_total.saturating_sub(budget_used),
            })
        } else {
            None
        };

        let (action, reason) = compute_recommendation(&state);

        let output = HandoffJsonV2 {
            schema_version: 2,
            session_id: state.session_id.clone(),
            agent: agent_id,
            loop_context: loop_ctx,
            progress: HandoffProgress {
                completed: state
                    .engrams
                    .iter()
                    .map(|e| HandoffEngram {
                        label: e.label.clone(),
                        trigger: format!("{:?}", e.trigger), // Serialize the enum
                    })
                    .collect(),
                active_errors: state
                    .active_errors
                    .iter()
                    .map(|e| e.chars().take(200).collect())
                    .collect(),
            },
            context: HandoffContext {
                hot_files: hot_vec
                    .iter()
                    .take(10)
                    .map(|(p, c)| HandoffHotFile {
                        path: p.to_string(),
                        access_count: **c,
                    })
                    .collect(),
                recent_commands: state
                    .last_commands
                    .iter()
                    .take(10)
                    .map(|c| c.chars().take(80).collect())
                    .collect(),
                task: task.to_string(),
                domain: domain.to_string(),
            },
            pressure: HandoffPressure {
                level: format!("{:?}", state.context_pressure),
            },
            recommendation: HandoffRecommendation { action, reason },
        };
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("{}", md.bright_white());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handoff_json_schema_validation() {
        let json_struct = HandoffJson {
            version: "1".to_string(),
            session_id: "test-session".to_string(),
            markdown_export: "# Markdown Data".to_string(),
        };

        let json_str = serde_json::to_string(&json_struct).unwrap();
        assert!(json_str.contains("\"version\":\"1\""));
        assert!(json_str.contains("\"session_id\":\"test-session\""));
        assert!(json_str.contains("\"markdown_export\":\"# Markdown Data\""));
    }

    #[test]
    fn test_handoff_v2_schema_serializes() {
        let v2 = HandoffJsonV2 {
            schema_version: 2,
            session_id: "test-123".to_string(),
            agent: "claude_code".to_string(),
            loop_context: Some(HandoffLoopContext {
                loop_id: "loop-abc".to_string(),
                iteration: 5,
                goal: "fix tests".to_string(),
                budget_used_tokens: 30000,
                budget_remaining_tokens: 20000,
            }),
            progress: HandoffProgress {
                completed: vec![HandoffEngram {
                    label: "fixed auth".to_string(),
                    trigger: "commit".to_string(),
                }],
                active_errors: vec!["expected str found String".to_string()],
            },
            context: HandoffContext {
                hot_files: vec![HandoffHotFile {
                    path: "src/main.rs".to_string(),
                    access_count: 12,
                }],
                recent_commands: vec!["cargo test".to_string()],
                task: "fix tests".to_string(),
                domain: "rust".to_string(),
            },
            pressure: HandoffPressure {
                level: "Normal".to_string(),
            },
            recommendation: HandoffRecommendation {
                action: "CONTINUE".to_string(),
                reason: "1 active error remaining".to_string(),
            },
        };

        let json = serde_json::to_string_pretty(&v2).unwrap();
        assert!(json.contains("\"schema_version\": 2"));
        assert!(json.contains("\"loop_id\": \"loop-abc\""));
        assert!(json.contains("\"action\": \"CONTINUE\""));
    }
}
