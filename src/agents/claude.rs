use crate::agents::AgentIntegration;
use colored::*;
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;

pub struct ClaudeIntegration;

impl AgentIntegration for ClaudeIntegration {
    fn id(&self) -> &'static str {
        "claude"
    }

    fn name(&self) -> &'static str {
        "Claude Code"
    }

    fn install(&self, exe_path: &str) -> anyhow::Result<()> {
        let (path, mut val) = initialize_settings()?;
        let _ = backup_settings(&path);

        install_omni_hooks(&mut val, exe_path);
        let new_content = serde_json::to_string_pretty(&val)?;
        fs::write(&path, new_content)?;
        println!(
            "  {} {} installed in Claude settings",
            "✓".green(),
            "Hooks".bold()
        );

        install_mcp_server(exe_path)?;
        println!(
            "  {} {} registered in .claude.json",
            "✓".green(),
            "MCP Server".bold()
        );

        Ok(())
    }

    fn uninstall(&self) -> anyhow::Result<()> {
        Ok(())
    }

    fn doctor_check(&self, fix_mode: bool, warnings: &mut Vec<String>) -> bool {
        let mut all_ok = true;

        println!("  {}", "Claude Code:".cyan());
        let path = get_settings_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if content.contains("--hook")
                    || content.contains("--post-hook")
                    || content.contains("--pre-hook")
                    || content.contains("--session-start")
                    || content.contains("--pre-compact")
                {
                    let fmt_hook = |name: &str, tag: &str| {
                        if content.contains(tag) {
                            println!(
                                "   {:<15} {}",
                                name.bright_black(),
                                "[OK] installed".green()
                            );
                            true
                        } else {
                            println!(
                                "   {:<15} {}",
                                name.bright_black(),
                                "[WARNING] missing".yellow()
                            );
                            false
                        }
                    };

                    if !fmt_hook("PreToolUse", "PreToolUse") {
                        all_ok = false;
                    }
                    if !fmt_hook("PostToolUse", "PostToolUse") {
                        all_ok = false;
                        warnings.push(
                            "PostToolUse hook is not installed. Run `omni init`.".to_string(),
                        );
                    }
                    if !fmt_hook("SessionStart", "SessionStart") {
                        all_ok = false;
                    }
                    if !fmt_hook("PreCompact", "PreCompact") {
                        all_ok = false;
                    }

                    if fix_mode && !all_ok {
                        if let Ok(exe_path) = std::env::current_exe() {
                            let _ = self.install(&exe_path.to_string_lossy());
                        }
                        println!(
                            "   {:<15} {}",
                            "Hooks:".bright_black(),
                            "[FIXED] missing hooks installed".green().bold()
                        );
                        all_ok = true;
                        warnings.retain(|w| {
                            !w.contains("hook") && !w.contains("Claude settings not found")
                        });
                    }
                } else {
                    if fix_mode {
                        if let Ok(exe_path) = std::env::current_exe() {
                            let _ = self.install(&exe_path.to_string_lossy());
                        }
                        println!(
                            "   {:<15} {}",
                            "Hooks:".bright_black(),
                            "[FIXED] installed".green().bold()
                        );
                    } else {
                        println!(
                            "   {:<15} {}",
                            "Hooks:".bright_black(),
                            "[WARNING] no hooks found".yellow().bold()
                        );
                        warnings
                            .push("OMNI hooks are not configured. Run `omni init`.".to_string());
                        all_ok = false;
                    }
                }
            }
        } else {
            if fix_mode {
                if let Ok(exe_path) = std::env::current_exe() {
                    let _ = self.install(&exe_path.to_string_lossy());
                }
                println!(
                    "   {:<15} {}",
                    "Hooks:".bright_black(),
                    "[FIXED] installed".green().bold()
                );
            } else {
                println!(
                    "   {:<15} {}",
                    "Hooks:".bright_black(),
                    "[ERROR] settings.json missing".red()
                );
                warnings
                    .push("Claude settings not found. Have you installed Claude Code?".to_string());
                all_ok = false;
            }
        }

        let mcp_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Library/Application Support/Claude/claude_desktop_config.json");
        let mcpa_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".claude.json");

        let mut mcp_found = false;
        for p in &[mcp_path, mcpa_path] {
            if p.exists()
                && let Ok(c) = fs::read_to_string(p)
                && (c.contains("omni --mcp") || c.contains("\"omni\":"))
            {
                mcp_found = true;
                println!(
                    "   {:<15} {} {}",
                    "MCP Server:".bright_black(),
                    p.display().to_string().bright_black(),
                    "[OK]".green().bold()
                );
                break;
            }
        }
        if !mcp_found {
            if fix_mode {
                if let Ok(exe_path) = std::env::current_exe() {
                    let _ = self.install(&exe_path.to_string_lossy());
                }
                println!(
                    "   {:<15} {}",
                    "MCP Server:".bright_black(),
                    "[FIXED] registered".green().bold()
                );
            } else {
                println!(
                    "   {:<15} {}",
                    "MCP Server:".bright_black(),
                    "[WARNING] no MCP server found".yellow().bold()
                );
                warnings.push("MCP Server is not configured. Run `omni init`.".to_string());
                all_ok = false;
            }
        }

        all_ok
    }
}

pub fn get_settings_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".claude/settings.json")
}

pub fn get_claude_json_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".claude.json")
}

pub fn backup_settings(path: &PathBuf) -> anyhow::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let backup_path = path.with_extension("json.bak");
    fs::copy(path, backup_path)?;
    Ok(())
}

pub fn initialize_settings() -> anyhow::Result<(PathBuf, Value)> {
    let path = get_settings_path();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut val = if path.exists() {
        let content = fs::read_to_string(&path)?;
        serde_json::from_str(&content).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };

    install_omni_hooks(&mut val, ""); // Temp to ensure object exists
    Ok((path, val))
}

pub fn check_status(val: &Value, exe_path: &str) -> (bool, bool, bool) {
    let hooks = match val.get("hooks").and_then(|v| v.as_object()) {
        Some(h) => h,
        None => return (false, false, false),
    };

    let check = |event: &str| -> bool {
        if let Some(arr) = hooks.get(event).and_then(|v| v.as_array()) {
            for v in arr {
                if let Some(inner_arr) = v.get("hooks").and_then(|v2| v2.as_array()) {
                    for hook_def in inner_arr {
                        if let Some(cmd) = hook_def.get("command").and_then(|c| c.as_str())
                            && cmd.contains(exe_path)
                            && (cmd.contains("--hook")
                                || cmd.contains("--post-hook")
                                || cmd.contains("--pre-hook")
                                || cmd.contains("--session-start")
                                || cmd.contains("--pre-compact"))
                        {
                            return true;
                        }
                    }
                }
            }
        }
        false
    };

    (
        check("PostToolUse"),
        check("SessionStart"),
        check("PreCompact"),
    )
}

pub fn remove_omni_hooks(val: &mut Value) {
    if let Some(obj) = val.as_object_mut()
        && let Some(hooks) = obj.get_mut("hooks").and_then(|h| h.as_object_mut())
    {
        for (_key, arr_val) in hooks.iter_mut() {
            if let Some(arr) = arr_val.as_array_mut() {
                arr.retain(|v| {
                    if let Some(inner) = v.get("hooks").and_then(|h| h.as_array()) {
                        !inner.iter().any(|h| {
                            h.get("command").and_then(|c| c.as_str()).is_some_and(|c| {
                                c.contains("omni")
                                    && (c.contains("--hook")
                                        || c.contains("--post-hook")
                                        || c.contains("--pre-hook")
                                        || c.contains("--session-start")
                                        || c.contains("--pre-compact"))
                            })
                        })
                    } else {
                        true
                    }
                });
            }
        }
    }
}

pub fn install_omni_hooks(val: &mut Value, exe_path: &str) {
    let obj = match val.as_object_mut() {
        Some(o) => o,
        None => {
            *val = json!({});
            val.as_object_mut().unwrap()
        }
    };

    let hooks = obj
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .unwrap();

    if exe_path.is_empty() {
        return;
    }

    let ensure_hook = |arr_val: &mut serde_json::Value, matcher: &str, hook_cmd: &str| {
        let arr = arr_val.as_array_mut().unwrap();
        for v in arr.iter() {
            if let Some(inner) = v.get("hooks").and_then(|h| h.as_array()) {
                for h in inner {
                    if h.get("command").and_then(|c| c.as_str()) == Some(hook_cmd) {
                        return;
                    }
                }
            }
        }

        arr.push(json!({
            "matcher": matcher,
            "hooks": [
                {
                    "type": "command",
                    "command": hook_cmd
                }
            ]
        }));
    };

    let script_path = format!("{} --pre-hook", exe_path);
    let post_cmd = format!("{} --post-hook", exe_path);
    let session_cmd = format!("{} --session-start", exe_path);
    let compact_cmd = format!("{} --pre-compact", exe_path);

    ensure_hook(
        hooks.entry("PreToolUse").or_insert_with(|| json!([])),
        "Bash",
        &script_path,
    );
    ensure_hook(
        hooks.entry("PostToolUse").or_insert_with(|| json!([])),
        "Bash",
        &post_cmd,
    );
    ensure_hook(
        hooks.entry("SessionStart").or_insert_with(|| json!([])),
        "",
        &session_cmd,
    );
    ensure_hook(
        hooks.entry("PreCompact").or_insert_with(|| json!([])),
        "",
        &compact_cmd,
    );
}

pub fn install_mcp_server(exe_path: &str) -> anyhow::Result<()> {
    let path = get_claude_json_path();
    let mut val = if path.exists() {
        let content = fs::read_to_string(&path)?;
        serde_json::from_str(&content).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };

    let obj = val
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("Invalid .claude.json format"))?;

    let servers = obj
        .entry("mcpServers")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("mcpServers is not an object"))?;

    servers.insert(
        "omni".to_string(),
        json!({
            "type": "stdio",
            "command": exe_path,
            "args": ["--mcp"]
        }),
    );

    if let Some(projects) = obj.get_mut("projects").and_then(|p| p.as_object_mut()) {
        for (_path, p_val) in projects.iter_mut() {
            if let Some(ps) = p_val.get_mut("mcpServers").and_then(|s| s.as_object_mut())
                && ps.contains_key("omni")
            {
                ps.insert(
                    "omni".to_string(),
                    json!({
                        "type": "stdio",
                        "command": exe_path,
                        "args": ["--mcp"]
                    }),
                );
            }
        }
    }

    let top_level_keys: Vec<String> = obj.keys().cloned().collect();
    for key in top_level_keys {
        if key != "mcpServers"
            && key != "projects"
            && let Some(inner_obj) = obj.get_mut(&key).and_then(|v| v.as_object_mut())
            && let Some(ps) = inner_obj
                .get_mut("mcpServers")
                .and_then(|s| s.as_object_mut())
            && ps.contains_key("omni")
        {
            ps.insert(
                "omni".to_string(),
                json!({
                    "type": "stdio",
                    "command": exe_path,
                    "args": ["--mcp"]
                }),
            );
        }
    }

    fs::write(&path, serde_json::to_string_pretty(&val)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_hook_membuat_settings_json_yang_valid_json() {
        let mut val = json!({});
        install_omni_hooks(&mut val, "/usr/bin/omni");

        let hooks = val.get("hooks").unwrap().as_object().unwrap();
        assert!(hooks.contains_key("PostToolUse"));
        assert!(hooks.contains_key("SessionStart"));
        assert!(hooks.contains_key("PreCompact"));
    }

    #[test]
    fn test_init_hook_idempotent_run_2x_not_duplicate() {
        let mut val = json!({});
        install_omni_hooks(&mut val, "/usr/bin/omni");

        let get_count = |v: &Value| -> usize {
            v.get("hooks")
                .unwrap()
                .get("PostToolUse")
                .unwrap()
                .as_array()
                .unwrap()
                .len()
        };

        assert_eq!(get_count(&val), 1);

        install_omni_hooks(&mut val, "/usr/bin/omni");
        assert_eq!(get_count(&val), 1, "Should be idempotent");
    }

    #[test]
    fn test_init_status_menampilkan_status_yang_benar() {
        let mut val = json!({});
        let exe = "/usr/bin/omni";
        install_omni_hooks(&mut val, exe);

        // Check status with correct path
        let (post, sess, pre) = check_status(&val, exe);
        assert!(post && sess && pre);

        // Check status with incorrect path
        let (post_f, sess_f, pre_f) = check_status(&val, "/different/omni");
        assert!(!post_f && !sess_f && !pre_f);
    }

    #[test]
    fn test_init_uninstall_membersihkan_semua_entries() {
        let mut val = json!({});
        let exe = "/usr/bin/omni";
        install_omni_hooks(&mut val, exe);

        assert!(check_status(&val, exe).0); // terpasang

        remove_omni_hooks(&mut val);

        assert!(!check_status(&val, exe).0); // hilang

        let arr = val
            .get("hooks")
            .unwrap()
            .get("PostToolUse")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(
            arr.len(),
            0,
            "Array must be empty after retain cleans it out"
        );
    }
}
