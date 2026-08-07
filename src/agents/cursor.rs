use crate::agents::AgentIntegration;
use colored::*;
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;

pub struct CursorIntegration;

impl AgentIntegration for CursorIntegration {
    fn id(&self) -> &'static str {
        "cursor"
    }

    fn name(&self) -> &'static str {
        "Cursor AI"
    }

    fn install(&self, exe_path: &str) -> anyhow::Result<()> {
        // Install MCP server
        let (mcp_path, mut mcp_val) = initialize_mcp_config()?;
        install_mcp_server(&mut mcp_val, exe_path);
        fs::write(&mcp_path, serde_json::to_string_pretty(&mcp_val)?)?;
        println!(
            "  {} Configured MCP Server in ~/.cursor/mcp.json",
            "✓".green()
        );

        // Install hooks
        install_omni_hooks(exe_path)?;
        println!(
            "  {} Configured {} in ~/.cursor/hooks.json",
            "✓".green(),
            "Hooks".bold()
        );

        Ok(())
    }

    fn uninstall(&self) -> anyhow::Result<()> {
        // Remove MCP
        let mcp_path = get_mcp_path();
        if mcp_path.exists() {
            let content = fs::read_to_string(&mcp_path)?;
            if let Ok(mut val) = serde_json::from_str::<Value>(&content) {
                remove_mcp_server(&mut val);
                fs::write(&mcp_path, serde_json::to_string_pretty(&val)?)?;
                println!(
                    "  {} Removed MCP Server from ~/.cursor/mcp.json",
                    "✓".yellow()
                );
            }
        }

        // Remove hooks
        remove_omni_hooks()?;
        println!("  {} Removed Hooks from ~/.cursor/hooks.json", "✓".yellow());
        Ok(())
    }

    fn doctor_check(&self, fix_mode: bool, warnings: &mut Vec<String>) -> bool {
        let mcp_path = get_mcp_path();
        let hooks_path = get_hooks_path();
        let mut all_ok = true;

        println!("\n  {}", "Cursor AI:".cyan());

        // Check MCP
        if mcp_path.exists()
            && let Ok(content) = fs::read_to_string(&mcp_path)
            && let Ok(val) = serde_json::from_str::<Value>(&content)
            && has_valid_omni_server(&val)
        {
            println!(
                "   {:<15} {} {}",
                "MCP: ".bright_black(),
                "~/.cursor/mcp.json".bright_black(),
                "[OK]".green().bold()
            );
        } else {
            all_ok = false;
            if fix_mode {
                if let Ok(exe_path) = std::env::current_exe() {
                    let _ = self.install(&exe_path.to_string_lossy());
                }
                println!(
                    "   {:<15} {}",
                    "MCP: ".bright_black(),
                    "[FIXED] registered".green().bold()
                );
            } else {
                println!(
                    "   {:<15} {}",
                    "MCP: ".bright_black(),
                    "not configured".bright_black()
                );
                warnings.push(
                    "Cursor MCP server is not configured. Run `omni init --cursor`.".to_string(),
                );
            }
        }

        // Check hooks. Grepping the whole file for `--post-hook` said "installed"
        // however it was wired, which is how the `afterFileEdit` misregistration
        // survived unnoticed: the check could not fail (#340). Assert the event
        // the command sits under, per event.
        let installed = installed_hook_events(&hooks_path);
        let missing: Vec<&str> = REQUIRED_HOOKS
            .iter()
            .filter(|(event, flag)| !installed.iter().any(|(e, f)| e == event && f == flag))
            .map(|(event, _)| *event)
            .collect();

        if missing.is_empty() {
            for (event, flag) in REQUIRED_HOOKS {
                println!(
                    "   {:<21} {}",
                    event.bright_black(),
                    format!("[OK] {flag}").green()
                );
            }
        } else {
            all_ok = false;
            if fix_mode {
                if let Ok(exe_path) = std::env::current_exe() {
                    let _ = install_omni_hooks(&exe_path.to_string_lossy());
                }
                println!(
                    "   {:<15} {}",
                    "Hooks:".bright_black(),
                    "[FIXED] missing hooks installed".green().bold()
                );
            } else {
                println!(
                    "   {:<15} {}",
                    "Hooks:".bright_black(),
                    format!("[WARNING] missing: {}", missing.join(", "))
                        .yellow()
                        .bold()
                );
                warnings.push(format!(
                    "Cursor hooks missing on {}. Run `omni init --cursor`.",
                    missing.join(", ")
                ));
            }
        }

        all_ok
    }
}

/// The event each hook has to sit under for Cursor to feed it anything, so
/// `doctor` can assert the wiring rather than the presence of a substring.
const REQUIRED_HOOKS: &[(&str, &str)] = &[
    ("beforeShellExecution", "--pre-hook"),
    ("postToolUse", "--post-hook"),
    ("postToolUseFailure", "--hook"),
    ("stop", "--hook"),
];

/// `(event, flag)` for every OMNI command registered in the file.
fn installed_hook_events(hooks_path: &PathBuf) -> Vec<(String, String)> {
    let Ok(content) = fs::read_to_string(hooks_path) else {
        return Vec::new();
    };
    let Ok(val) = serde_json::from_str::<Value>(&content) else {
        return Vec::new();
    };
    let Some(hooks) = val.get("hooks").and_then(|h| h.as_object()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (event, arr) in hooks {
        for entry in arr.as_array().into_iter().flatten() {
            let Some(cmd) = entry.get("command").and_then(|c| c.as_str()) else {
                continue;
            };
            if !cmd.contains("omni") {
                continue;
            }
            // Longest flag first: `--post-hook` also contains `--hook`.
            for flag in ["--pre-hook", "--post-hook", "--hook"] {
                if cmd.contains(flag) {
                    out.push((event.clone(), flag.to_string()));
                    break;
                }
            }
        }
    }
    out
}

fn get_mcp_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cursor/mcp.json")
}

fn get_hooks_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cursor/hooks.json")
}

fn initialize_mcp_config() -> anyhow::Result<(PathBuf, Value)> {
    let mcp_path = get_mcp_path();
    if let Some(parent) = mcp_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let val = if mcp_path.exists() {
        let content = fs::read_to_string(&mcp_path)?;
        serde_json::from_str(&content).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };
    Ok((mcp_path, val))
}

fn install_mcp_server(val: &mut Value, exe_path: &str) {
    let obj = match val.as_object_mut() {
        Some(o) => o,
        None => {
            *val = json!({});
            val.as_object_mut().unwrap()
        }
    };
    let servers = obj
        .entry("mcpServers")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .unwrap();
    servers.insert(
        "omni".to_string(),
        json!({
            "type": "stdio", "command": exe_path, "args": ["--mcp"],
            "env": { "OMNI_AGENT_ID": "cursor" }
        }),
    );
}

fn remove_mcp_server(val: &mut Value) {
    if let Some(obj) = val.as_object_mut()
        && let Some(servers) = obj.get_mut("mcpServers").and_then(|v| v.as_object_mut())
    {
        servers.remove("omni");
    }
}

fn has_valid_omni_server(val: &Value) -> bool {
    val.get("mcpServers")
        .and_then(|v| v.as_object())
        .and_then(|servers| servers.get("omni"))
        .is_some_and(|omni| {
            omni.get("command").and_then(|v| v.as_str()).is_some()
                && omni
                    .get("args")
                    .and_then(|v| v.as_array())
                    .is_some_and(|args| args.iter().any(|arg| arg.as_str() == Some("--mcp")))
        })
}

pub fn install_omni_hooks(exe_path: &str) -> anyhow::Result<()> {
    install_omni_hooks_at(&get_hooks_path(), exe_path)
}

/// The path is a parameter so the tests drive this without setting `HOME`.
/// `cargo` runs tests in parallel and a `set_var` here would decide where an
/// unrelated test writes, which is the failure mode `AGENTS.md` calls out.
fn install_omni_hooks_at(hooks_path: &PathBuf, exe_path: &str) -> anyhow::Result<()> {
    if let Some(parent) = hooks_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut val = if hooks_path.exists() {
        let content = fs::read_to_string(&hooks_path)?;
        serde_json::from_str(&content).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };

    let obj = match val.as_object_mut() {
        Some(o) => o,
        None => {
            val = json!({});
            val.as_object_mut().unwrap()
        }
    };

    let hooks = obj
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .unwrap();

    let pre_cmd = format!("{} --pre-hook", exe_path);
    let post_cmd = format!("{} --post-hook", exe_path);
    let hook_cmd = format!("{} --hook", exe_path);

    // Drop every OMNI entry first, then re-add. Two things this buys: an install
    // after the binary moved leaves one command rather than two, and the stale
    // `afterFileEdit` registration from before #340 is purged on upgrade instead
    // of sitting in the file forever, firing on edits that carry no output.
    retain_non_omni(hooks);

    let ensure_hook = |arr_val: &mut Value, cmd: &str| {
        let arr = arr_val.as_array_mut().unwrap();
        for v in arr.iter() {
            if v.get("command").and_then(|c| c.as_str()) == Some(cmd) {
                return;
            }
        }
        arr.push(json!({ "command": cmd }));
    };

    // `postToolUse` is Cursor's analogue of Claude Code's `PostToolUse`, and it
    // is the only one of these that carries command output. `afterFileEdit`,
    // registered here until #340, fires on a file write: the distiller was handed
    // nothing for the entire life of the integration, which is why this
    // installation has 9,857 `claude_code` rows and zero `cursor` ones.
    for (event, cmd) in [
        ("beforeShellExecution", &pre_cmd),
        ("postToolUse", &post_cmd),
        // Failed commands carry the error payload OMNI reads for #120.
        ("postToolUseFailure", &hook_cmd),
        // Cursor has no PreCompact, and `stop` is the closest thing to
        // SessionEnd: the flush that lets the next session start informed.
        ("stop", &hook_cmd),
    ] {
        ensure_hook(hooks.entry(event).or_insert_with(|| json!([])), cmd);
    }

    fs::write(&hooks_path, serde_json::to_string_pretty(&val)?)?;
    Ok(())
}

/// Strips OMNI's own hook entries from every event, leaving other tools' hooks
/// untouched. Shared by install (so it can re-register cleanly) and uninstall.
fn retain_non_omni(hooks: &mut serde_json::Map<String, Value>) {
    for arr_val in hooks.values_mut() {
        if let Some(arr) = arr_val.as_array_mut() {
            arr.retain(|v| {
                v.get("command").and_then(|c| c.as_str()).is_none_or(|c| {
                    !(c.contains("omni")
                        && (c.contains("--pre-hook")
                            || c.contains("--post-hook")
                            || c.contains("--hook")))
                })
            });
        }
    }
}

pub fn remove_omni_hooks() -> anyhow::Result<()> {
    let hooks_path = get_hooks_path();
    if !hooks_path.exists() {
        return Ok(());
    }
    let content = fs::read_to_string(&hooks_path)?;
    let Ok(mut val) = serde_json::from_str::<Value>(&content) else {
        return Ok(());
    };

    if let Some(obj) = val.as_object_mut()
        && let Some(hooks) = obj.get_mut("hooks").and_then(|h| h.as_object_mut())
    {
        retain_non_omni(hooks);
    }

    fs::write(&hooks_path, serde_json::to_string_pretty(&val)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_mcp_server_is_idempotent() {
        let mut val = json!({});
        install_mcp_server(&mut val, "/usr/local/bin/omni");
        install_mcp_server(&mut val, "/usr/local/bin/omni");
        let servers = val
            .get("mcpServers")
            .and_then(|v| v.as_object())
            .expect("mcpServers exists");
        assert_eq!(servers.len(), 1);
        assert!(servers.contains_key("omni"));
    }

    #[test]
    fn remove_mcp_server_removes_only_omni() {
        let mut val = json!({ "mcpServers": { "omni": {"command": "/usr/local/bin/omni", "args": ["--mcp"]}, "other": {"command": "other"} } });
        remove_mcp_server(&mut val);
        let servers = val
            .get("mcpServers")
            .and_then(|v| v.as_object())
            .expect("mcpServers exists");
        assert!(!servers.contains_key("omni"));
        assert!(servers.contains_key("other"));
    }

    /// The old version of this built its own JSON and then asserted on the JSON it
    /// had just built, so it passed while `--post-hook` sat on `afterFileEdit` and
    /// received nothing for the life of the integration (#340). Drive the real
    /// writer and assert the event each command lands on.
    #[test]
    fn registers_the_post_hook_on_the_event_that_carries_output() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("hooks.json");
        install_omni_hooks_at(&path, "/usr/local/bin/omni").expect("install");

        let installed = installed_hook_events(&path);
        for (event, flag) in REQUIRED_HOOKS {
            assert!(
                installed.iter().any(|(e, f)| e == event && f == flag),
                "{flag} must be registered on {event}, got {installed:?}"
            );
        }
        assert!(
            !installed.iter().any(|(e, _)| e == "afterFileEdit"),
            "afterFileEdit carries no command output and must not be registered"
        );
    }

    /// An upgrade has to remove the dead registration, not leave it beside the
    /// working one. A user who ran an older `omni init --cursor` still has the
    /// `afterFileEdit` entry on disk.
    #[test]
    fn purges_a_stale_registration_left_by_an_earlier_install() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("hooks.json");
        fs::write(
            &path,
            serde_json::to_string_pretty(&json!({"hooks": {
                "afterFileEdit": [{"command": "/old/path/omni --post-hook"}],
                "stop": [{"command": "/some/other/tool.sh"}]
            }}))
            .expect("json"),
        )
        .expect("seed");

        install_omni_hooks_at(&path, "/usr/local/bin/omni").expect("install");
        let installed = installed_hook_events(&path);

        assert!(
            !installed.iter().any(|(e, _)| e == "afterFileEdit"),
            "the stale entry must be purged, got {installed:?}"
        );
        assert!(
            fs::read_to_string(&path)
                .expect("read")
                .contains("/some/other/tool.sh"),
            "another tool's hook must survive"
        );
    }
}
