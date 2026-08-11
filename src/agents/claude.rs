use crate::agents::AgentIntegration;
use colored::*;
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;

/// Which tool results Claude Code hands to the PostToolUse hook.
///
/// Claude Code matches this as a regex, so alternation is how one registration
/// covers several tools. It is a constant because `install_omni_hooks` writes it
/// and the tests assert on it, and a widened matcher that only half the code
/// knows about is worse than a narrow one (#172).
const POST_TOOL_MATCHER: &str = "Bash|Read|Grep|WebFetch";

pub struct ClaudeIntegration;

impl AgentIntegration for ClaudeIntegration {
    fn tier(&self) -> crate::agents::Tier {
        crate::agents::Tier::Full
    }

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
        crate::agent_report!(
            "  {} {} installed in Claude settings",
            "✓".green(),
            "Hooks".bold()
        );

        install_mcp_server(exe_path)?;
        crate::agent_report!(
            "  {} {} registered in .claude.json",
            "✓".green(),
            "MCP Server".bold()
        );

        Ok(())
    }

    fn uninstall(&self) -> anyhow::Result<()> {
        let settings_path = get_settings_path();
        if settings_path.exists() {
            let content = fs::read_to_string(&settings_path)?;
            if let Ok(mut val) = serde_json::from_str::<Value>(&content) {
                remove_omni_hooks(&mut val);
                fs::write(&settings_path, serde_json::to_string_pretty(&val)?)?;
                crate::agent_report!("  {} Removed Hooks from Claude settings", "✓".yellow());
            }
        }

        let mcp_path = get_claude_json_path();
        if mcp_path.exists() {
            let content = fs::read_to_string(&mcp_path)?;
            if let Ok(mut val) = serde_json::from_str::<Value>(&content) {
                let mut changed = false;

                if let Some(obj) = val.as_object_mut() {
                    if let Some(servers) = obj.get_mut("mcpServers").and_then(|v| v.as_object_mut())
                        && servers.remove("omni").is_some()
                    {
                        changed = true;
                    }

                    if let Some(projects) = obj.get_mut("projects").and_then(|p| p.as_object_mut())
                    {
                        for (_, p_val) in projects.iter_mut() {
                            if let Some(ps) =
                                p_val.get_mut("mcpServers").and_then(|s| s.as_object_mut())
                                && ps.remove("omni").is_some()
                            {
                                changed = true;
                            }
                        }
                    }

                    let top_level_keys: Vec<String> = obj.keys().cloned().collect();
                    for key in top_level_keys {
                        if key != "mcpServers"
                            && key != "projects"
                            && let Some(inner_obj) =
                                obj.get_mut(&key).and_then(|v| v.as_object_mut())
                            && let Some(ps) = inner_obj
                                .get_mut("mcpServers")
                                .and_then(|s| s.as_object_mut())
                            && ps.remove("omni").is_some()
                        {
                            changed = true;
                        }
                    }
                }

                if changed {
                    fs::write(&mcp_path, serde_json::to_string_pretty(&val)?)?;
                    crate::agent_report!("  {} Removed MCP Server from .claude.json", "✓".yellow());
                }
            }
        }

        Ok(())
    }

    fn doctor_check(&self, fix_mode: bool, warnings: &mut Vec<String>) -> bool {
        let mut all_ok = true;

        crate::agent_report!("  {}", "Claude Code:".cyan());
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
                            crate::agent_report!(
                                "   {:<15} {}",
                                name.bright_black(),
                                "[OK] installed".green()
                            );
                            true
                        } else {
                            crate::agent_report!(
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
                            crate::agents::report_fix(
                                "Hooks:",
                                "missing hooks installed",
                                self.install(&exe_path.to_string_lossy()),
                                warnings,
                            );
                        }
                        all_ok = true;
                        warnings.retain(|w| {
                            !w.contains("hook") && !w.contains("Claude settings not found")
                        });
                    }
                } else if fix_mode {
                    if let Ok(exe_path) = std::env::current_exe() {
                        crate::agents::report_fix(
                            "Hooks:",
                            "installed",
                            self.install(&exe_path.to_string_lossy()),
                            warnings,
                        );
                    }
                } else {
                    crate::agent_report!(
                        "   {:<15} {}",
                        "Hooks:".bright_black(),
                        "[WARNING] no hooks found".yellow().bold()
                    );
                    warnings.push("OMNI hooks are not configured. Run `omni init`.".to_string());
                    all_ok = false;
                }
            }
        } else if fix_mode {
            if let Ok(exe_path) = std::env::current_exe() {
                crate::agents::report_fix(
                    "Hooks:",
                    "installed",
                    self.install(&exe_path.to_string_lossy()),
                    warnings,
                );
            }
        } else {
            crate::agent_report!(
                "   {:<15} {}",
                "Hooks:".bright_black(),
                "[ERROR] settings.json missing".red()
            );
            warnings.push("Claude settings not found. Have you installed Claude Code?".to_string());
            all_ok = false;
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
                crate::agent_report!(
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
                    crate::agents::report_fix(
                        "MCP Server:",
                        "registered",
                        self.install(&exe_path.to_string_lossy()),
                        warnings,
                    );
                }
            } else {
                crate::agent_report!(
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

/// Whether `command` is OMNI's own hook for the same entry point as `ours`.
///
/// Identity is the binary name plus the flag, never the whole string. Comparing
/// the whole string meant that reinstalling from a different path matched
/// nothing and appended, so a machine that had been set up twice ran OMNI twice
/// per tool call and `omni doctor` reported `[OK]` throughout (#454).
fn is_our_hook(command: Option<&str>, ours: &str) -> bool {
    let Some(command) = command else {
        return false;
    };
    let flag = match ours.rsplit_once(' ') {
        Some((_, flag)) if flag.starts_with("--") => flag,
        _ => return false,
    };
    // `--hook` is a prefix of nothing else here, but `--pre-hook` ends with
    // `-hook`, so the flag is compared as the last token rather than by
    // `contains`.
    command.contains("omni") && command.rsplit(' ').next() == Some(flag)
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
        for v in arr.iter_mut() {
            let installed = v
                .get("hooks")
                .and_then(|h| h.as_array())
                .is_some_and(|inner| {
                    inner
                        .iter()
                        .any(|h| is_our_hook(h.get("command").and_then(|c| c.as_str()), hook_cmd))
                });
            if installed {
                // Bring the path up to date as well as the matcher. Matching on
                // the exact string meant a reinstall from a different binary
                // matched nothing and pushed a second entry, so OMNI ran twice
                // per call (#454).
                if let Some(inner) = v
                    .get_mut("hooks")
                    .and_then(|h| h.as_array_mut())
                    .and_then(|inner| {
                        inner.iter_mut().find(|h| {
                            is_our_hook(h.get("command").and_then(|c| c.as_str()), hook_cmd)
                        })
                    })
                    .and_then(|h| h.as_object_mut())
                {
                    inner.insert("command".to_string(), json!(hook_cmd));
                }
                // Already installed, but the matcher still has to be brought up to
                // date. Returning here unconditionally is why widening it in #172
                // would have reached new installs only: every existing settings
                // file names `Bash` and nothing would have rewritten it.
                if let Some(obj) = v.as_object_mut() {
                    obj.insert("matcher".to_string(), json!(matcher));
                }
                return;
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

    let ensure_async_hook = |arr_val: &mut serde_json::Value, hook_cmd: &str| {
        let arr = arr_val.as_array_mut().unwrap();
        for v in arr.iter_mut() {
            if let Some(inner) = v.get_mut("hooks").and_then(|h| h.as_array_mut()) {
                for h in inner.iter_mut() {
                    if is_our_hook(h.get("command").and_then(|c| c.as_str()), hook_cmd) {
                        if let Some(obj) = h.as_object_mut() {
                            obj.insert("command".to_string(), json!(hook_cmd));
                        }
                        return;
                    }
                }
            }
        }
        arr.push(json!({
            "matcher": "",
            "hooks": [{
                "type": "command",
                "command": hook_cmd,
                "async": true
            }]
        }));
    };

    let pre_cmd = format!("{} --pre-hook", exe_path);
    let post_cmd = format!("{} --post-hook", exe_path);
    let session_cmd = format!("{} --session-start", exe_path);
    let compact_cmd = format!("{} --pre-compact", exe_path);
    let hook_cmd = format!("{} --hook", exe_path);

    // Core hooks (blocking)
    ensure_hook(
        hooks.entry("PreToolUse").or_insert_with(|| json!([])),
        "Bash",
        &pre_cmd,
    );
    // `Bash|Read|Grep|WebFetch`, widened on the maintainer's call (#172).
    //
    // The `Read`/`Grep`/`WebFetch` arms in `hooks::post_tool` had been written,
    // gated and tested since the Rust rewrite and had **never executed on Claude
    // Code**, because this matcher named one tool. What that widening does was
    // measured before it was taken, and it is not free: driven through the built
    // binary's `--post-hook` with a real payload, `src/pipeline/collapse.rs`, 878
    // lines, comes back as **20** of them, an import list, three signatures and a
    // marker. `readfile.rs`'s only floor is `MIN_DISTILL_TOKENS = 2000`, and 7.6%
    // of 1,770 real `Read` calls clear it, `.rs` only 3.1%. An earlier version of
    // this comment said "nearly every real source file", which is a
    // generalisation from one example that the 0.6.8 changelog had already
    // measured and withdrawn (#284). The figure that carries the decision is the
    // other half of the same measurement: those 7.6% of calls hold 44% of all
    // `Read` bytes, so this changes what "read a file" means on exactly the reads
    // that matter, on the tool an agent edits from. The maintainer chose that
    // trade with those numbers in front of them.
    //
    // Three prerequisites landed first, each of which would have made this fail
    // silently rather than loudly: `normalize` could not reach a `Read` payload's
    // text at all (`file.content` matched no arm), `tool_input.file_path` was
    // never read (so the distiller saw `"unknown"` and could not pick a
    // language), and `shape_for_host` had no `Read` shape. #246 stopped the
    // `readfile` path reporting a document as a clean log, and #273 made these
    // arms archive and mark what they cut, so nothing they drop is unrecoverable.
    ensure_hook(
        hooks.entry("PostToolUse").or_insert_with(|| json!([])),
        POST_TOOL_MATCHER,
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

    //  New hooks (async, non-blocking, no output needed)
    ensure_async_hook(
        hooks.entry("SessionEnd").or_insert_with(|| json!([])),
        &hook_cmd,
    );
    ensure_async_hook(
        hooks
            .entry("PostToolUseFailure")
            .or_insert_with(|| json!([])),
        &hook_cmd,
    );
    ensure_hook(
        hooks.entry("SubagentStart").or_insert_with(|| json!([])),
        "",
        &session_cmd,
    );
    ensure_async_hook(
        hooks.entry("FileChanged").or_insert_with(|| json!([])),
        &hook_cmd,
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
            "args": ["--mcp"],
            "env": {
                "OMNI_AGENT_ID": "claude_code"
            },
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
                        "args": ["--mcp"],
                        "env": {
                            "OMNI_AGENT_ID": "claude_code"
                        },
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
                    "args": ["--mcp"],
                    "env": {
                        "OMNI_AGENT_ID": "claude_code"
                    },
                }),
            );
        }
    }

    fs::write(&path, serde_json::to_string_pretty(&val)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    /// #454, found on a real machine: a working install was repointed from a
    /// development build to the released one, and every OMNI hook ended up
    /// registered twice, once per path. Two OMNI processes per hooked call, and
    /// `omni doctor` reported `[OK]` throughout because it asks whether a hook is
    /// present, never how many.
    #[test]
    fn reinstalling_from_another_path_moves_the_hook_rather_than_adding_one() {
        let mut val = json!({});
        install_omni_hooks(&mut val, "/repo/target/debug/omni");
        install_omni_hooks(&mut val, "/opt/homebrew/bin/omni");

        let hooks = val["hooks"].as_object().expect("hooks written");
        assert!(!hooks.is_empty(), "no hooks were written at all");
        for (event, arr) in hooks {
            let ours: Vec<&str> = arr
                .as_array()
                .expect("event is an array")
                .iter()
                .flat_map(|m| m["hooks"].as_array().cloned().unwrap_or_default())
                .filter_map(|h| h["command"].as_str().map(str::to_string))
                .filter(|c| c.contains("omni"))
                .map(|c| Box::leak(c.into_boxed_str()) as &str)
                .collect();
            assert_eq!(
                ours.len(),
                1,
                "{event} carries {} OMNI hooks, so OMNI would run {} times: {ours:?}",
                ours.len(),
                ours.len()
            );
            assert!(
                ours[0].starts_with("/opt/homebrew/bin/omni"),
                "{event} still points at the old path: {ours:?}"
            );
        }
    }

    /// The identity test is the binary plus the flag, and the flag is compared as
    /// a whole token. `--pre-hook` ends with `-hook`, so a `contains` would make
    /// the two the same hook and one would overwrite the other.
    #[test]
    fn tells_the_hook_flags_apart_even_when_one_ends_with_another() {
        assert!(is_our_hook(
            Some("/a/omni --pre-hook"),
            "/b/omni --pre-hook"
        ));
        assert!(!is_our_hook(Some("/a/omni --pre-hook"), "/b/omni --hook"));
        assert!(!is_our_hook(Some("/a/omni --post-hook"), "/b/omni --hook"));
        assert!(!is_our_hook(
            Some("/usr/bin/other --pre-hook"),
            "/b/omni --pre-hook"
        ));
        assert!(!is_our_hook(None, "/b/omni --pre-hook"));
    }

    use super::*;

    #[test]
    fn test_init_hook_creates_valid_settings_json() {
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

    /// #172. The `Read`, `Grep` and `WebFetch` arms in `hooks::post_tool` had
    /// been written, gated and tested since the Rust rewrite and had never once
    /// executed on Claude Code, because this matcher named a single tool.
    #[test]
    fn registers_post_tool_for_the_tools_it_can_distil() {
        let mut val = json!({});
        install_omni_hooks(&mut val, "/usr/bin/omni");

        let matcher = val["hooks"]["PostToolUse"][0]["matcher"]
            .as_str()
            .expect("a string matcher");

        for tool in ["Bash", "Read", "Grep", "WebFetch"] {
            assert!(
                matcher.contains(tool),
                "{tool} results never reach the hook: matcher is {matcher:?}"
            );
        }
    }

    /// An install from before #172 names `Bash` and would never be rewritten,
    /// because `ensure_hook` returned as soon as it recognised the command. Every
    /// existing user would have kept the narrow matcher and seen no change at
    /// all, which is the quietest way for a fix to not ship.
    #[test]
    fn brings_an_existing_narrow_matcher_up_to_date() {
        let mut val = json!({
            "hooks": {
                "PostToolUse": [{
                    "matcher": "Bash",
                    "hooks": [{"type": "command", "command": "/usr/bin/omni --post-hook"}]
                }]
            }
        });

        install_omni_hooks(&mut val, "/usr/bin/omni");

        assert_eq!(
            val["hooks"]["PostToolUse"][0]["matcher"].as_str(),
            Some(POST_TOOL_MATCHER),
            "an existing install must be migrated, not left behind"
        );
        assert_eq!(
            val["hooks"]["PostToolUse"].as_array().unwrap().len(),
            1,
            "migrating must not duplicate the registration"
        );
    }

    #[test]
    fn test_init_status_reports_expected_status() {
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
    fn test_init_hook_writes_matcher_for_all_events() {
        let mut val = json!({});
        install_omni_hooks(&mut val, "/usr/bin/omni");

        let hooks = val.get("hooks").unwrap().as_object().unwrap();
        for (event, entries) in hooks {
            for entry in entries.as_array().unwrap() {
                let matcher = entry.get("matcher");
                assert!(
                    matches!(matcher, Some(v) if v.is_string()),
                    "{event} entry missing string matcher: {entry}"
                );
            }
        }
    }

    #[test]
    fn test_init_uninstall_removes_all_entries() {
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
