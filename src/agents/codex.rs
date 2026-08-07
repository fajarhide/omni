use crate::agents::AgentIntegration;
use colored::*;
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;

pub struct CodexIntegration;

impl AgentIntegration for CodexIntegration {
    fn tier(&self) -> crate::agents::Tier {
        crate::agents::Tier::Full
    }

    fn id(&self) -> &'static str {
        "codex"
    }

    fn name(&self) -> &'static str {
        "Codex CLI"
    }

    fn install(&self, exe_path: &str) -> anyhow::Result<()> {
        let codex_dir = get_codex_dir();
        fs::create_dir_all(&codex_dir)?;

        // Install MCP server in config.toml
        let config_path = codex_dir.join("config.toml");
        let mut content = if config_path.exists() {
            fs::read_to_string(&config_path)?
        } else {
            String::new()
        };

        // A sub-table like `[mcp_servers.omni.tools.x]` declares `mcp_servers.omni`
        // on its own. Without the parent block beside it the server has no
        // transport, and Codex refuses to start at all:
        //
        //   Error: config.toml:45:14: invalid transport
        //   Caused by: invalid transport in `mcp_servers.omni`
        //
        // The old test was `contains("[mcp_servers.omni]")`, which an orphaned
        // sub-table does not satisfy, so install could not repair what uninstall
        // had left behind (#351). Verified in a copied CODEX_HOME on 0.144.6:
        // orphan alone and `codex features list` dies; parent restored and it runs.
        if !declares_transport(&content) {
            content.push_str(&format!(
                "\n[mcp_servers.omni]\ntype = \"stdio\"\ncommand = \"{}\"\nargs = [\"--mcp\"]\n",
                exe_path
            ));
            fs::write(&config_path, content)?;
        }

        println!(
            "  {} Configured MCP Server in {}",
            "✓".green(),
            config_path.display()
        );

        // Install hooks in hooks.json
        install_omni_hooks(exe_path)?;
        println!(
            "  {} Configured {} in {}",
            "✓".green(),
            "Hooks".bold(),
            codex_dir.join("hooks.json").display()
        );
        // Codex will not run a hook it has not been told to trust, and says
        // nothing when it skips one, so writing the config is only half of the
        // install (#359).
        println!(
            "  {} Start {} once and approve them under {}, or Codex skips them",
            "!".yellow(),
            "codex".bold(),
            "\"Hooks need review\"".bold()
        );
        // Codex keeps its bypass on the command line on purpose: it is rejected
        // from config.toml, so no installer can turn it on for you (#359).
        println!(
            "    {} for unattended runs, pass {}",
            "or".bright_black(),
            "--dangerously-bypass-hook-trust".bright_black()
        );

        Ok(())
    }

    fn uninstall(&self) -> anyhow::Result<()> {
        let codex_dir = get_codex_dir();

        // Remove MCP from config.toml
        let config_path = codex_dir.join("config.toml");
        if config_path.exists() {
            let content = fs::read_to_string(&config_path)?;
            if content.contains("[mcp_servers.omni") {
                let new_content = strip_omni_server(&content);
                fs::write(&config_path, new_content.trim_end().to_string() + "\n")?;
                println!(
                    "  {} Removed MCP Server from {}",
                    "✓".yellow(),
                    config_path.display()
                );
            }
        }

        // Remove hooks from hooks.json
        remove_omni_hooks()?;
        println!(
            "  {} Removed Hooks from {}",
            "✓".yellow(),
            codex_dir.join("hooks.json").display()
        );

        Ok(())
    }

    fn doctor_check(&self, fix_mode: bool, warnings: &mut Vec<String>) -> bool {
        let codex_dir = get_codex_dir();
        let config_path = codex_dir.join("config.toml");
        let hooks_path = codex_dir.join("hooks.json");
        let mut all_ok = true;

        println!("\n  {}", "Codex CLI:".cyan());

        // Check MCP config
        if config_path.exists()
            && fs::read_to_string(&config_path)
                .unwrap_or_default()
                .contains("omni")
        {
            println!(
                "   {:<15} {} {}",
                "Config:".bright_black(),
                config_path.display().to_string().bright_black(),
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
                    "Config:".bright_black(),
                    "[FIXED] registered".green().bold()
                );
            } else {
                println!(
                    "   {:<15} {}",
                    "Config:".bright_black(),
                    "not configured".bright_black()
                );
                warnings.push(
                    "Codex CLI MCP server not configured. Run `omni init --codex`.".to_string(),
                );
            }
        }

        // Check hooks
        let hooks_content = if hooks_path.exists() {
            fs::read_to_string(&hooks_path).unwrap_or_default()
        } else {
            String::new()
        };

        let has_pre = hooks_content.contains("--pre-hook");
        let has_post = hooks_content.contains("--post-hook");
        let has_session = hooks_content.contains("--session-start");

        if has_pre && has_post && has_session {
            let fmt_hook = |name: &str, present: bool| {
                if present {
                    println!(
                        "   {:<15} {}",
                        name.bright_black(),
                        "[OK] installed".green()
                    );
                }
            };
            fmt_hook("PreToolUse", has_pre);
            fmt_hook("PostToolUse", has_post);
            fmt_hook("SessionStart", has_session);

            // Installed is not the same as running: Codex skips any hook it has
            // not been told to trust, and says nothing about it (#359).
            let config = fs::read_to_string(&config_path).unwrap_or_default();
            let awaiting = hooks_awaiting_review(&config, &hooks_path.to_string_lossy());
            if !awaiting.is_empty() {
                all_ok = false;
                println!(
                    "   {:<15} {}",
                    "Trust:".bright_black(),
                    format!("[WARNING] {} awaiting review in Codex", awaiting.join(", ")).yellow()
                );
                warnings.push(format!(
                    "Codex has not been told to trust OMNI's hooks ({}), so it skips them silently. \
                     Start `codex` once and approve them under \"Hooks need review\", or pass \
                     `--dangerously-bypass-hook-trust` for unattended runs.",
                    awaiting.join(", ")
                ));
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
                if !has_pre {
                    println!(
                        "   {:<15} {}",
                        "PreToolUse".bright_black(),
                        "[WARNING] missing".yellow()
                    );
                }
                if !has_post {
                    println!(
                        "   {:<15} {}",
                        "PostToolUse".bright_black(),
                        "[WARNING] missing".yellow()
                    );
                }
                if !has_session {
                    println!(
                        "   {:<15} {}",
                        "SessionStart".bright_black(),
                        "[WARNING] missing".yellow()
                    );
                }
                warnings
                    .push("Codex CLI hooks not configured. Run `omni init --codex`.".to_string());
            }
        }

        all_ok
    }
}

/// True when `mcp_servers.omni` is declared *with* a transport, which is the only
/// state Codex will start from.
///
/// A bare `[mcp_servers.omni.tools.x]` also declares the server, so presence of
/// the name says nothing. Codex needs the parent block carrying `command`, and
/// rejects the whole config without it.
fn declares_transport(config: &str) -> bool {
    let mut in_parent = false;
    for line in config.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_parent = t == "[mcp_servers.omni]";
            continue;
        }
        if in_parent && t.starts_with("command") {
            return true;
        }
    }
    false
}

/// Removes `[mcp_servers.omni]` **and every `[mcp_servers.omni.*]` sub-table**.
///
/// The old version stopped skipping at the next `[`, so a sub-table survived its
/// own parent. Codex then read a server with no transport and refused to start,
/// which is a working install turned into a dead one by running uninstall (#351).
fn strip_omni_server(config: &str) -> String {
    let mut out = String::with_capacity(config.len());
    let mut skip = false;
    for line in config.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            skip = t == "[mcp_servers.omni]" || t.starts_with("[mcp_servers.omni.");
        }
        if !skip {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// Codex reads its config from `$CODEX_HOME` and only falls back to `~/.codex`.
/// Installing to the fallback while the host is using the override writes a
/// config Codex never loads, and `doctor` then reports the file it wrote rather
/// than the file in use. Seen on a machine where a launcher exported
/// `CODEX_HOME` to its own runtime directory: `~/.codex/config.toml` held the
/// MCP server, the live home did not, and OMNI reported healthy either way.
/// The OMNI hook events Codex has no trust record for, and will therefore skip.
///
/// Codex 0.144.6 trusts hooks one entry at a time, keyed
/// `<hooks.json path>:<event>:<group>:<index>` under `[hooks.state]`, with the
/// entry's own hash. An entry it has not been shown is ignored, and `codex exec`
/// prints nothing about it, so a config that reads as installed does nothing at
/// all. Proved by deleting one trust record: the hook stopped firing, and came
/// back when the record was restored (#359).
///
/// OMNI deliberately does not write these records. The hash exists so a human
/// approves each command before Codex runs it, and a tool that grants itself
/// that approval removes the only thing standing between "can write a config
/// file" and "can execute anything".
fn hooks_awaiting_review(config: &str, hooks_path: &str) -> Vec<&'static str> {
    [
        ("pre_tool_use", "PreToolUse"),
        ("post_tool_use", "PostToolUse"),
        ("session_start", "SessionStart"),
    ]
    .into_iter()
    .filter(|(wire, _)| !config.contains(&format!("{}:{}:", hooks_path, wire)))
    .map(|(_, display)| display)
    .collect()
}

fn get_codex_dir() -> PathBuf {
    codex_dir_from(std::env::var_os("CODEX_HOME"), dirs::home_dir())
}

/// The choice itself, kept free of the environment so it can be tested without
/// a concurrently running test seeing the mutation.
fn codex_dir_from(codex_home: Option<std::ffi::OsString>, home: Option<PathBuf>) -> PathBuf {
    match codex_home {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => home.unwrap_or_else(|| PathBuf::from(".")).join(".codex"),
    }
}

pub fn install_omni_hooks(exe_path: &str) -> anyhow::Result<()> {
    let hooks_path = get_codex_dir().join("hooks.json");
    fs::create_dir_all(get_codex_dir())?;

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
    let session_cmd = format!("{} --session-start", exe_path);

    let ensure_hook = |arr_val: &mut Value, cmd: &str| {
        let arr = arr_val.as_array_mut().unwrap();
        // Check if already present
        for v in arr.iter() {
            if v.get("command").and_then(|c| c.as_str()) == Some(cmd) {
                return;
            }
        }
        arr.push(json!({ "command": cmd }));
    };

    ensure_hook(
        hooks.entry("PreToolUse").or_insert_with(|| json!([])),
        &pre_cmd,
    );
    ensure_hook(
        hooks.entry("PostToolUse").or_insert_with(|| json!([])),
        &post_cmd,
    );
    ensure_hook(
        hooks.entry("SessionStart").or_insert_with(|| json!([])),
        &session_cmd,
    );

    fs::write(&hooks_path, serde_json::to_string_pretty(&val)?)?;
    Ok(())
}

pub fn remove_omni_hooks() -> anyhow::Result<()> {
    let hooks_path = get_codex_dir().join("hooks.json");
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
        for (_key, arr_val) in hooks.iter_mut() {
            if let Some(arr) = arr_val.as_array_mut() {
                arr.retain(|v| {
                    v.get("command").and_then(|c| c.as_str()).is_none_or(|c| {
                        !(c.contains("omni")
                            && (c.contains("--pre-hook")
                                || c.contains("--post-hook")
                                || c.contains("--session-start")))
                    })
                });
            }
        }
    }

    fs::write(&hooks_path, serde_json::to_string_pretty(&val)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #359: `omni init --codex` wrote a valid hooks.json and every hook was
    /// ignored, because Codex runs only entries it has a trust record for and
    /// skips the rest without a word. Doctor said `[OK] installed` throughout.
    #[test]
    fn reports_hooks_codex_has_not_been_told_to_trust() {
        let untrusted = hooks_awaiting_review("", "/h/hooks.json");

        assert_eq!(untrusted, vec!["PreToolUse", "PostToolUse", "SessionStart"]);
    }

    /// The record is per entry, so approving one hook must not vouch for the
    /// others, and the key is path-scoped so another file's record cannot count.
    #[test]
    fn counts_only_the_records_for_this_hooks_file() {
        let config = "\
[hooks.state.\"/h/hooks.json:pre_tool_use:0:0\"]\nenabled = true\n\
[hooks.state.\"/other/hooks.json:post_tool_use:0:0\"]\nenabled = true\n";

        assert_eq!(
            hooks_awaiting_review(config, "/h/hooks.json"),
            vec!["PostToolUse", "SessionStart"]
        );
    }

    /// Codex resolves its own config through `$CODEX_HOME`, so installing to
    /// `~/.codex` regardless writes a file the running Codex never opens. Found
    /// on a machine whose launcher points `CODEX_HOME` at its own runtime
    /// directory: `omni init --codex` reported success into `~/.codex` while the
    /// home actually in use kept no MCP server at all.
    #[test]
    fn installs_into_the_home_codex_is_actually_using() {
        let dir = codex_dir_from(
            Some("/somewhere/runtime-home".into()),
            Some(PathBuf::from("/Users/x")),
        );

        assert_eq!(dir, PathBuf::from("/somewhere/runtime-home"));
    }

    /// An exported-but-empty `CODEX_HOME` is not a location; treating it as one
    /// would install into the process's working directory.
    #[test]
    fn falls_back_to_the_default_home_when_the_override_is_absent_or_empty() {
        let home = Some(PathBuf::from("/Users/x"));

        assert_eq!(
            codex_dir_from(None, home.clone()),
            PathBuf::from("/Users/x/.codex")
        );
        assert_eq!(
            codex_dir_from(Some("".into()), home),
            PathBuf::from("/Users/x/.codex")
        );
    }

    /// #351: uninstall stopped skipping at the next `[`, so
    /// `[mcp_servers.omni.tools.x]` outlived its own parent. Codex then reads a
    /// server with no transport and refuses to start at all, which turns a
    /// working install into a dead editor by running uninstall. Reproduced on
    /// 0.144.6: `codex features list` dies with `invalid transport`, and comes
    /// back the moment the parent block is restored.
    #[test]
    fn uninstall_removes_the_sub_tables_too() {
        let config = "\
model = \"x\"

[mcp_servers.omni]
type = \"stdio\"
command = \"/usr/local/bin/omni\"
args = [\"--mcp\"]

[mcp_servers.omni.tools.omni_session]
approval_mode = \"approve\"

[model_providers.other]
name = \"Other\"
";
        let out = strip_omni_server(config);
        assert!(
            !out.contains("mcp_servers.omni"),
            "an orphaned sub-table bricks Codex:\n{out}"
        );
        assert!(
            out.contains("[model_providers.other]") && out.contains("name = \"Other\""),
            "unrelated config must survive:\n{out}"
        );
    }

    /// The repair half: a config left holding only the sub-table must be
    /// recognised as *missing* its transport, or install cannot fix what
    /// uninstall broke. The old check was `contains("[mcp_servers.omni]")`,
    /// which an orphan does not satisfy.
    #[test]
    fn an_orphaned_sub_table_counts_as_missing_a_transport() {
        let orphan = "[mcp_servers.omni.tools.omni_session]\napproval_mode = \"approve\"\n";
        assert!(!declares_transport(orphan));

        let complete = "[mcp_servers.omni]\ntype = \"stdio\"\ncommand = \"/usr/local/bin/omni\"\nargs = [\"--mcp\"]\n";
        assert!(declares_transport(complete));

        // A `command` belonging to some other server is not ours.
        let other =
            "[mcp_servers.other]\ncommand = \"/bin/other\"\n\n[mcp_servers.omni.tools.x]\na = 1\n";
        assert!(!declares_transport(other));
    }

    #[test]
    fn test_install_hooks_creates_valid_json() {
        let dir = tempfile::tempdir().unwrap();
        let hooks_path = dir.path().join("hooks.json");

        // Manually write to a known path for testing the JSON structure
        let mut val = json!({});
        let obj = val.as_object_mut().unwrap();
        let hooks = obj
            .entry("hooks")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .unwrap();

        let cmd = "/usr/bin/omni --pre-hook";
        let arr = hooks.entry("PreToolUse").or_insert_with(|| json!([]));
        arr.as_array_mut().unwrap().push(json!({ "command": cmd }));

        fs::write(&hooks_path, serde_json::to_string_pretty(&val).unwrap()).unwrap();

        let content = fs::read_to_string(&hooks_path).unwrap();
        assert!(content.contains("PreToolUse"));
        assert!(content.contains("--pre-hook"));
    }

    #[test]
    fn test_ensure_hook_is_idempotent() {
        let mut val = json!({ "hooks": { "PreToolUse": [] } });
        let hooks = val.get_mut("hooks").unwrap().as_object_mut().unwrap();

        let cmd = "/usr/bin/omni --pre-hook";
        let ensure = |arr_val: &mut Value, cmd: &str| {
            let arr = arr_val.as_array_mut().unwrap();
            for v in arr.iter() {
                if v.get("command").and_then(|c| c.as_str()) == Some(cmd) {
                    return;
                }
            }
            arr.push(json!({ "command": cmd }));
        };

        ensure(hooks.get_mut("PreToolUse").unwrap(), cmd);
        assert_eq!(
            hooks.get("PreToolUse").unwrap().as_array().unwrap().len(),
            1
        );

        ensure(hooks.get_mut("PreToolUse").unwrap(), cmd);
        assert_eq!(
            hooks.get("PreToolUse").unwrap().as_array().unwrap().len(),
            1,
            "Should be idempotent"
        );
    }
}
