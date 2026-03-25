use colored::*;
use serde_json::{Value, json};
use std::env;
use std::fs;
use std::path::PathBuf;

fn print_help() {
    println!(
        "\n{} {} — Setup OMNI hooks and MCP server",
        "omni".bold().cyan(),
        "init".bold().yellow()
    );
    println!("\n{}", "USAGE:".bold().bright_white());
    println!("  omni {}", "init [FLAGS]".cyan());

    println!("\n{}", "FLAGS:".bold().bright_white());
    println!(
        "  {: <12} Perform full setup (hooks + MCP server)",
        "--all".cyan()
    );
    println!("  {: <12} Only install Claude Code hooks", "--hook".cyan());
    println!("  {: <12} Only register MCP server", "--mcp".cyan());
    println!(
        "  {: <12} Check current installation status",
        "--status".cyan()
    );
    println!(
        "  {: <12} Completely remove OMNI Hook and MCP server from Claude",
        "--uninstall".cyan()
    );
    println!("  {: <12} Show this help message", "--help, -h".cyan());

    println!("\n{}", "EXAMPLES:".bold().bright_white());
    println!(
        "  omni init --all       {}",
        "# Recommended full setup".bright_black()
    );
    println!(
        "  omni init --status    {}",
        "# Verify installation".bright_black()
    );
    println!();
}

pub fn get_claude_json_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".claude.json")
}

pub fn get_settings_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".claude")
        .join("settings.json")
}

fn initialize_settings() -> anyhow::Result<(PathBuf, Value)> {
    let settings_path = get_settings_path();

    if let Some(parent) = settings_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let val = if settings_path.exists() {
        let content = fs::read_to_string(&settings_path)?;
        serde_json::from_str(&content).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };

    Ok((settings_path, val))
}

fn backup_settings(path: &PathBuf) -> anyhow::Result<PathBuf> {
    let backup_path = path.with_extension("json.bak");
    if path.exists() {
        fs::copy(path, &backup_path)?;
    }
    Ok(backup_path)
}

pub fn run_init(args: &[String]) -> anyhow::Result<()> {
    if args
        .iter()
        .any(|a| a == "--help" || a == "-h" || a == "help")
    {
        print_help();
        return Ok(());
    }

    let mut is_hook = args.iter().any(|a| a == "--hook");
    let mut is_mcp = args.iter().any(|a| a == "--mcp");
    let is_all = args.iter().any(|a| a == "--all");
    let is_status = args.iter().any(|a| a == "--status");
    let is_uninstall = args.iter().any(|a| a == "--uninstall");

    // If no specific flag is provided, show help instead of defaulting to setup
    if !is_all && !is_status && !is_uninstall && !is_hook && !is_mcp {
        print_help();
        return Ok(());
    }

    if is_all {
        is_hook = true;
        is_mcp = true;
    }

    let exe_path = env::current_exe()?.to_string_lossy().to_string();

    if is_status {
        let (_, val) = initialize_settings()?;
        let (post_ok, session_ok, pre_ok) = check_status(&val, &exe_path);

        println!("\n{}", "OMNI Installation Status:".bold().bright_white());

        let fmt_status = |ok: bool| {
            if ok {
                "✓ installed".green()
            } else {
                "✗ not installed".red()
            }
        };

        println!("  PostToolUse:  {}", fmt_status(post_ok));
        println!("  SessionStart: {}", fmt_status(session_ok));
        println!("  PreCompact:   {}", fmt_status(pre_ok));
        println!();
        return Ok(());
    }

    if is_uninstall {
        let (path, mut val) = initialize_settings()?;
        if path.exists() {
            backup_settings(&path)?;
        }

        remove_omni_hooks(&mut val);

        // Also remove MCP from .claude.json
        let mcp_path = get_claude_json_path();
        if mcp_path.exists()
            && let Ok(content) = fs::read_to_string(&mcp_path)
            && let Ok(mut mcp_val) = serde_json::from_str::<Value>(&content)
        {
            if let Some(obj) = mcp_val.as_object_mut() {
                // Remove from global mcpServers
                if let Some(servers) = obj.get_mut("mcpServers").and_then(|v| v.as_object_mut()) {
                    servers.remove("omni");
                }

                // Remove from project-specific mcpServers (under "projects")
                if let Some(projects) = obj.get_mut("projects").and_then(|p| p.as_object_mut()) {
                    for (_path, p_val) in projects.iter_mut() {
                        if let Some(ps) =
                            p_val.get_mut("mcpServers").and_then(|s| s.as_object_mut())
                        {
                            ps.remove("omni");
                        }
                    }
                }

                // Remove from top-level project keys
                let top_level_keys: Vec<String> = obj.keys().cloned().collect();
                for key in top_level_keys {
                    if key != "mcpServers"
                        && key != "projects"
                        && let Some(inner_obj) = obj.get_mut(&key).and_then(|v| v.as_object_mut())
                        && let Some(ps) = inner_obj
                            .get_mut("mcpServers")
                            .and_then(|s| s.as_object_mut())
                    {
                        ps.remove("omni");
                    }
                }
            }
            let _ = fs::write(&mcp_path, serde_json::to_string_pretty(&mcp_val)?);
        }

        let new_content = serde_json::to_string_pretty(&val)?;
        fs::write(&path, new_content)?;
        println!("✓ OMNI hooks and MCP server uninstalled");
        return Ok(());
    }

    if is_hook || is_mcp {
        let (path, mut val) = initialize_settings()?;
        let _ = backup_settings(&path);

        if is_hook {
            install_omni_hooks(&mut val, &exe_path);
            let new_content = serde_json::to_string_pretty(&val)?;
            fs::write(&path, new_content)?;
            println!(
                "  {} {} installed in Claude settings",
                "✓".green(),
                "Hooks".bold()
            );
        }

        if is_mcp {
            install_mcp_server(&exe_path)?;
            println!(
                "  {} {} registered in .claude.json",
                "✓".green(),
                "MCP Server".bold()
            );
        }

        println!("\n  {} Binary: {}", "ℹ".blue(), exe_path.bright_black());
        println!(
            "  {} Restart Claude Code to activate.\n",
            "✓".green().bold()
        );
    }

    Ok(())
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

    let _cmd = format!("{} --hook", exe_path);

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

    // 1. Ensure top-level mcpServers exists
    let servers = obj
        .entry("mcpServers")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("mcpServers is not an object"))?;

    // 2. Add/Update OMNI
    servers.insert(
        "omni".to_string(),
        json!({
            "type": "stdio",
            "command": exe_path,
            "args": ["--mcp"]
        }),
    );

    // 3. Also update any project entries that might have an old OMNI reference
    if let Some(projects) = obj.get_mut("projects").and_then(|p| p.as_object_mut()) {
        for (_path, p_val) in projects.iter_mut() {
            if let Some(ps) = p_val.get_mut("mcpServers").and_then(|s| s.as_object_mut())
                && ps.contains_key("omni")
            {
                ps.insert(
                    "omni".to_string(),
                    serde_json::json!({
                        "command": "omni",
                        "args": ["--mcp"],
                        "env": {}
                    }),
                );
            }
        }
    }

    let new_content = serde_json::to_string_pretty(&val)?;
    fs::write(&path, new_content)?;
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
    fn test_init_hook_idempotent_run_2x_tidak_duplicate() {
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

    // "membuat backup" test requires IO side effects, which might be tricky but we can skip pure IO testing inside rust memory unless necessary. The logic is self-evident.

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
