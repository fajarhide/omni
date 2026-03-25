use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct PreHookInput {
    tool_input: ToolInput,
}

#[derive(Deserialize, Serialize, Clone)]
struct ToolInput {
    command: Option<String>,
}

#[derive(Serialize)]
struct PreHookOutput {
    #[serde(rename = "hookSpecificOutput")]
    hook_specific_output: HookSpecificOutput,
}

#[derive(Serialize)]
struct HookSpecificOutput {
    #[serde(rename = "hookEventName")]
    hook_event_name: &'static str,
    #[serde(rename = "permissionDecision")]
    permission_decision: &'static str,
    #[serde(rename = "permissionDecisionReason")]
    permission_decision_reason: String,
    #[serde(rename = "updatedInput")]
    updated_input: ToolInput,
}

pub fn run() -> Result<()> {
    let mut buffer = String::new();
    std::io::stdin().read_to_string(&mut buffer)?;

    if let Some(output_json) = process_payload(&buffer) {
        println!("{}", output_json);
        std::process::exit(0);
    }

    // Exit 0 with no output tells Claude to proceed with original command
    Ok(())
}

fn process_payload(input_str: &str) -> Option<String> {
    let parsed: PreHookInput = serde_json::from_str(input_str).ok()?;
    let cmd_str = parsed.tool_input.command.as_ref()?;

    if let Some(rewritten) = crate::cli::rewrite::rewrite_logic(cmd_str) {
        let mut updated_input = parsed.tool_input.clone();
        updated_input.command = Some(rewritten);

        let output = PreHookOutput {
            hook_specific_output: HookSpecificOutput {
                hook_event_name: "PreToolUse",
                permission_decision: "allow",
                permission_decision_reason: "OMNI auto-rewrite to reduce token noise".to_string(),
                updated_input,
            },
        };
        return serde_json::to_string(&output).ok();
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_pre_hook_rewrites_git_status() {
        let input = json!({
            "tool_input": {
                "command": "git status"
            }
        })
        .to_string();

        let output = process_payload(&input).expect("Should rewrite");
        assert!(output.contains("exec git status"));
        assert!(output.contains("PreToolUse"));
        assert!(output.contains("allow"));
    }

    #[test]
    fn test_pre_hook_ignores_unknown_command() {
        let input = json!({
            "tool_input": {
                "command": "ls -la"
            }
        })
        .to_string();

        let output = process_payload(&input);
        assert!(output.is_none());
    }

    #[test]
    fn test_pre_hook_handles_shell_pipes() {
        let input = json!({
            "tool_input": {
                "command": "git status | grep foo"
            }
        })
        .to_string();

        let output = process_payload(&input).expect("Should rewrite");
        assert!(output.contains("exec git status | grep foo"));
    }
}
