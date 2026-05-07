use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::io::Read;

// Phase 6: mutating command detection for hot-file warnings
fn is_mutating_command(cmd: &str) -> bool {
    let lower = cmd.to_lowercase();
    // Direct file mutations
    lower.contains("rm ")
        || lower.contains("delete ")
        || lower.contains("mv ")
        || lower.contains("cp ")
        // Git state changes
        || lower.contains("git checkout")
        || lower.contains("git reset")
        || lower.contains("git add")
        // Build/install (often write to target/ or node_modules/)
        || lower.contains("cargo build")
        || lower.contains("cargo install")
        || lower.contains("cargo clean")
        // JS installs/builds
        || lower.contains("npm install")
        || lower.contains("npm run build")
        || lower.contains("rm -rf")
        // Docker / k8s writes
        || lower.contains("docker build")
        || lower.contains("docker run")
        || lower.contains("kubectl apply")
        || lower.contains("kubectl delete")
        // Generic edit-like keywords
        || lower.contains("write ")
        || lower.contains("edit ")
        || lower.contains("replace ")
        || lower.contains("touch ")
        || lower.contains("mkdir ")
}

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

    // Conservative Context Injection Hint for Read/Search commands
    if let Some(target_file) = extract_target_file(cmd_str) {
        // Phase 6: mutating command on hot file → warn
        if is_mutating_command(cmd_str) {
            let session_state = crate::pipeline::SessionState::new();
            let hot_count = session_state
                .hot_files
                .get(&target_file)
                .copied()
                .unwrap_or(0);
            if hot_count > 2 {
                let updated_input = parsed.tool_input.clone();
                let reason = format!(
                    "OMNI Guard: {} is a hot file (accessed {}x this session). Mutating it may have wide impact. Consider reviewing dependents via omni_context.",
                    target_file, hot_count
                );
                let output = PreHookOutput {
                    hook_specific_output: HookSpecificOutput {
                        hook_event_name: "PreToolUse",
                        permission_decision: "allow",
                        permission_decision_reason: reason,
                        updated_input,
                    },
                };
                return serde_json::to_string(&output).ok();
            }
        }

        // We only provide a hint, we don't modify the command
        let updated_input = parsed.tool_input.clone();
        let reason = format!(
            "OMNI context available for {}; call omni_context if needed",
            target_file
        );

        let output = PreHookOutput {
            hook_specific_output: HookSpecificOutput {
                hook_event_name: "PreToolUse",
                permission_decision: "allow",
                permission_decision_reason: reason,
                updated_input,
            },
        };
        return serde_json::to_string(&output).ok();
    }

    // Phase 6: mutating command without specific file target — still check if any hot file is implicated
    if is_mutating_command(cmd_str) {
        let session_state = crate::pipeline::SessionState::new();
        if !session_state.hot_files.is_empty() {
            let top_hot: Vec<String> = session_state
                .hot_files
                .iter()
                .take(3)
                .map(|(f, c)| format!("{} ({}x)", f, c))
                .collect();
            if !top_hot.is_empty() {
                let updated_input = parsed.tool_input.clone();
                let reason = format!(
                    "OMNI Guard: mutating command detected. Current hot files: {}. Review impact before proceeding.",
                    top_hot.join(", ")
                );
                let output = PreHookOutput {
                    hook_specific_output: HookSpecificOutput {
                        hook_event_name: "PreToolUse",
                        permission_decision: "allow",
                        permission_decision_reason: reason,
                        updated_input,
                    },
                };
                return serde_json::to_string(&output).ok();
            }
        }
    }

    None
}

fn extract_target_file(cmd: &str) -> Option<String> {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }
    match parts[0] {
        "cat" | "head" | "tail" => parts.get(1).map(|s| s.to_string()),
        "grep" | "rg" => {
            // Very naive extraction, just grabs the last argument if it doesn't look like a flag
            parts
                .last()
                .filter(|s| !s.starts_with('-'))
                .map(|s| s.to_string())
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn pre_hook_rewrites_git_status() {
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
    fn pre_hook_provides_context_hint_for_cat() {
        let input = json!({
            "tool_input": {
                "command": "cat src/main.rs"
            }
        })
        .to_string();

        let output = process_payload(&input).expect("Should inject context");
        assert!(output.contains("OMNI context available for src/main.rs"));
        assert!(output.contains("PreToolUse"));
        assert!(output.contains("allow"));
    }

    #[test]
    fn pre_hook_ignores_unknown_command() {
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
    fn pre_hook_handles_shell_pipes() {
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
