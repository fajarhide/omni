use crate::store::sqlite::Store;
use colored::*;
use std::sync::Arc;

#[derive(serde::Serialize)]
pub struct HandoffJson {
    pub version: String,
    pub session_id: String,
    pub markdown_export: String,
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
            md.push_str(&format!("- {}\n", &clean[..clean.len().min(120)]));
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
        md.push_str(&format!("- `{}`\n", &cmd[..cmd.len().min(80)]));
    }

    md.push_str(&format!(
        "\n## Context Pressure: {}\n",
        state.context_pressure
    ));
    md.push_str("\n---\n*Paste this into a new session to continue where you left off.*\n");

    if is_json {
        let output = HandoffJson {
            version: "1".to_string(),
            session_id: state.session_id.clone(),
            markdown_export: md,
        };
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("{}", md.bright_white());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::HandoffJson;

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
}
