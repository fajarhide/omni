use colored::*;
use serde_json::Value;
use std::env;
use std::fs;

/// Read by `print_help` and `super::check_flags` (#151). The first
/// `AGENT_FLAGS` entries are the agents; the rest are Claude-specific, which is
/// how help groups them.
const FLAGS: super::Flags = &[
    ("--claude", "Configure Claude Code (Anthropic)"),
    ("--cursor", "Configure Cursor AI"),
    ("--zed", "Configure Zed Editor"),
    ("--cline", "Configure Cline"),
    ("--roo", "Configure Roo Code"),
    ("--roo-code", "Configure Roo Code (alias)"),
    ("--copilot", "Configure GitHub Copilot CLI"),
    ("--gemini", "Configure Gemini CLI"),
    ("--opencode", "Configure OpenCode"),
    ("--codex", "Configure Codex CLI"),
    ("--openclaw", "Configure OpenClaw"),
    (
        "--antigravity",
        "Configure Antigravity IDE / Generic Webhook",
    ),
    ("--hermes", "Configure Hermes Agent"),
    ("--vscode", "Configure VS Code (MCP)"),
    ("--pi", "Configure Pi Agent"),
    ("--all", "Perform full Claude setup (hooks + MCP)"),
    ("--hook", "Only install hooks"),
    ("--mcp", "Only register MCP server"),
    ("--status", "Check current installation status"),
    ("--uninstall", "Remove OMNI hooks and MCP server"),
];

/// Where `FLAGS` stops listing agents and starts listing Claude-specific flags.
///
// ponytail: hand-maintained split index rather than two lists, because
// `check_flags` wants one flat list and a second const would mean teaching it to
// take several. `splits_flags_between_the_two_help_groups` below is what keeps
// it honest. Upgrade path: separate consts + a `check_flags` that accepts a
// slice of lists — worth it the moment a third group appears.
const AGENT_FLAGS: usize = 15;

fn print_help() {
    println!(
        "\n{} {} — Setup OMNI for your preferred AI Agent",
        "omni".bold().cyan(),
        "init".bold().yellow()
    );
    println!("\n{}", "USAGE:".bold().bright_white());
    println!("  omni {}", "init [FLAGS]".cyan());

    let entries: Vec<_> = FLAGS
        .iter()
        .chain(std::iter::once(&super::HELP_FLAG))
        .collect();
    super::print_flag_group("SUPPORTED AGENTS:", &entries[..AGENT_FLAGS]);
    super::print_flag_group("CLAUDE SPECIFIC FLAGS:", &entries[AGENT_FLAGS..]);

    println!("\n{}", "EXAMPLES:".bold().bright_white());
    println!(
        "  omni init             {}",
        "# Interactive menu".bright_black()
    );
    println!(
        "  omni init --claude    {}",
        "# Setup for Claude Code".bright_black()
    );
    println!();
}

pub fn run_init(args: &[String]) -> anyhow::Result<()> {
    if args
        .iter()
        .any(|a| a == "--help" || a == "-h" || a == "help")
    {
        print_help();
        return Ok(());
    }
    super::check_flags("init", args, FLAGS)?;

    let mut is_claude = args.iter().any(|a| a == "--claude");
    let mut is_cursor = args.iter().any(|a| a == "--cursor");
    let mut is_zed = args.iter().any(|a| a == "--zed");
    let mut is_cline = args.iter().any(|a| a == "--cline");
    let mut is_roo = args.iter().any(|a| a == "--roo" || a == "--roo-code");
    let mut is_copilot = args.iter().any(|a| a == "--copilot");
    let mut is_gemini = args.iter().any(|a| a == "--gemini");
    let mut is_opencode = args.iter().any(|a| a == "--opencode");
    let mut is_codex = args.iter().any(|a| a == "--codex");
    let mut is_openclaw = args.iter().any(|a| a == "--openclaw");
    let mut is_antigravity = args.iter().any(|a| a == "--antigravity");
    let mut is_hermes = args.iter().any(|a| a == "--hermes");
    let mut is_vscode = args.iter().any(|a| a == "--vscode");
    let mut is_pi = args.iter().any(|a| a == "--pi");

    let mut is_hook = args.iter().any(|a| a == "--hook");
    let mut is_mcp = args.iter().any(|a| a == "--mcp");
    let is_all = args.iter().any(|a| a == "--all");
    let is_status = args.iter().any(|a| a == "--status");
    let is_uninstall = args.iter().any(|a| a == "--uninstall");

    if is_all {
        is_claude = true;
        is_hook = true;
        is_mcp = true;
    }

    // No flags -> Interactive Mode
    let no_flags = !is_claude
        && !is_cursor
        && !is_zed
        && !is_cline
        && !is_roo
        && !is_copilot
        && !is_gemini
        && !is_opencode
        && !is_codex
        && !is_openclaw
        && !is_antigravity
        && !is_hermes
        && !is_vscode
        && !is_pi
        && !is_status
        && !is_uninstall
        && !is_hook
        && !is_mcp;

    if no_flags {
        println!(
            "\n{} {} — Setup OMNI for your preferred AI Agent\n",
            "omni".bold().cyan(),
            "init".bold().yellow()
        );

        let items = vec![
            "Claude Code (Anthropic)",
            "Cursor AI",
            "Zed Editor",
            "Cline",
            "Roo Code",
            "GitHub Copilot CLI",
            "Gemini CLI",
            "OpenCode",
            "Codex CLI",
            "OpenClaw",
            "Antigravity IDE",
            "Hermes Agent",
            "VS Code (MCP)",
            "Pi Agent",
            "Quit",
        ];

        let selection = dialoguer::Select::new()
            .with_prompt("Select an AI Agent to configure")
            .items(&items)
            .default(0)
            .interact()?;

        match selection {
            0 => {
                is_claude = true;
                is_hook = true;
                is_mcp = true;
            }
            1 => is_cursor = true,
            2 => is_zed = true,
            3 => is_cline = true,
            4 => is_roo = true,
            5 => is_copilot = true,
            6 => is_gemini = true,
            7 => is_opencode = true,
            8 => is_codex = true,
            9 => is_openclaw = true,
            10 => is_antigravity = true,
            11 => is_hermes = true,
            12 => is_vscode = true,
            13 => is_pi = true,
            _ => return Ok(()),
        }

        println!(
            "\n{}",
            "─────────────────────────────────────────"
                .bright_black()
                .bold()
        );
        println!(" {} OMNI Before & After Preview", "⚡".yellow());
        println!(
            "{}",
            "─────────────────────────────────────────"
                .bright_black()
                .bold()
        );
        println!("{}", "Without OMNI:".red());
        println!("  npm WARN deprecated ... (300 lines of warnings)");
        println!("  git log (2000 lines of history)");
        println!("{}", "\nWith OMNI:".green());
        println!("  npm WARN deprecated ... [OMNI: ⚠️ 300 repetitive lines dropped]");
        println!("  git log [OMNI: ⚠️ truncated to latest 50 lines]");
        println!(
            "{}\n",
            "─────────────────────────────────────────"
                .bright_black()
                .bold()
        );

        let proceed = dialoguer::Confirm::new()
            .with_prompt("Proceed with installation?")
            .default(true)
            .interact()?;

        if !proceed {
            return Ok(());
        }
    }

    let target_ids = if is_all {
        vec![
            "claude",
            "cursor",
            "zed",
            "cline",
            "roo-code",
            "copilot",
            "gemini",
            "opencode",
            "codex",
            "openclaw",
            "antigravity",
            "hermes",
            "vscode",
            "pi",
        ]
    } else {
        let mut ids = Vec::new();
        if is_claude || is_hook || is_mcp {
            ids.push("claude");
        }
        if is_cursor {
            ids.push("cursor");
        }
        if is_zed {
            ids.push("zed");
        }
        if is_cline {
            ids.push("cline");
        }
        if is_roo {
            ids.push("roo-code");
        }
        if is_copilot {
            ids.push("copilot");
        }
        if is_gemini {
            ids.push("gemini");
        }
        if is_opencode {
            ids.push("opencode");
        }
        if is_codex {
            ids.push("codex");
        }
        if is_openclaw {
            ids.push("openclaw");
        }
        if is_antigravity {
            ids.push("antigravity");
        }
        if is_hermes {
            ids.push("hermes");
        }
        if is_vscode {
            ids.push("vscode");
        }
        if is_pi {
            ids.push("pi");
        }
        ids
    };

    let exe_path = env::current_exe()?.to_string_lossy().to_string();

    if is_status {
        let (_, val) = crate::agents::claude::initialize_settings()?;
        let (post_ok, session_ok, pre_ok) = crate::agents::claude::check_status(&val, &exe_path);

        println!(
            "\n{}",
            "Claude Code OMNI Installation Status:"
                .bold()
                .bright_white()
        );

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
        let (path, mut val) = crate::agents::claude::initialize_settings()?;
        if path.exists() {
            crate::agents::claude::backup_settings(&path)?;
        }

        crate::agents::claude::remove_omni_hooks(&mut val);

        let mcp_path = crate::agents::claude::get_claude_json_path();
        if mcp_path.exists()
            && let Ok(content) = fs::read_to_string(&mcp_path)
            && let Ok(mut mcp_val) = serde_json::from_str::<Value>(&content)
        {
            if let Some(obj) = mcp_val.as_object_mut() {
                if let Some(servers) = obj.get_mut("mcpServers").and_then(|v| v.as_object_mut()) {
                    servers.remove("omni");
                }
                if let Some(projects) = obj.get_mut("projects").and_then(|p| p.as_object_mut()) {
                    for (_path, p_val) in projects.iter_mut() {
                        if let Some(ps) =
                            p_val.get_mut("mcpServers").and_then(|s| s.as_object_mut())
                        {
                            ps.remove("omni");
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
                    {
                        ps.remove("omni");
                    }
                }
            }
            let _ = fs::write(&mcp_path, serde_json::to_string_pretty(&mcp_val)?);
        }

        let new_content = serde_json::to_string_pretty(&val)?;
        fs::write(&path, new_content)?;
        println!("✓ OMNI hooks and MCP server uninstalled from Claude");
        return Ok(());
    }

    let integrations = crate::agents::all_integrations();

    for agent in integrations {
        if target_ids.contains(&agent.id()) {
            println!("{}", format!("🤖 {} Setup", agent.name()).bold().cyan());

            if let Err(e) = agent.install(&exe_path) {
                eprintln!("  {} Failed: {}", "✗".red(), e);
            }

            if agent.id() == "claude" {
                println!("\n  {} Binary: {}", "ℹ".blue(), exe_path.bright_black());
                println!(
                    "  {} Restart Claude Code to activate.\n",
                    "✓".green().bold()
                );
            }
            println!();
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `AGENT_FLAGS` is a hand-maintained index into `FLAGS`. Getting it wrong
    /// files an agent under "CLAUDE SPECIFIC FLAGS" in help and nothing else
    /// breaks, so this is the only thing that would notice.
    #[test]
    fn splits_flags_between_the_two_help_groups() {
        assert_eq!(
            FLAGS[AGENT_FLAGS - 1].0,
            "--pi",
            "last agent flag moved; AGENT_FLAGS is stale"
        );
        assert_eq!(
            FLAGS[AGENT_FLAGS].0, "--all",
            "first Claude-specific flag moved; AGENT_FLAGS is stale"
        );
    }
}
