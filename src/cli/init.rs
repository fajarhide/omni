use colored::*;
use serde_json::Value;
use std::env;
use std::fs;

fn print_help() {
    println!(
        "\n{} {} — Setup OMNI for your preferred AI Agent",
        "omni".bold().cyan(),
        "init".bold().yellow()
    );
    println!("\n{}", "USAGE:".bold().bright_white());
    println!("  omni {}", "init [FLAGS]".cyan());

    println!("\n{}", "SUPPORTED AGENTS:".bold().bright_white());
    println!(
        "  {: <14} Configure Claude Code (Anthropic)",
        "--claude".cyan()
    );
    println!("  {: <14} Configure Cursor AI", "--cursor".cyan());
    println!("  {: <14} Configure Zed Editor", "--zed".cyan());
    println!("  {: <14} Configure Cline", "--cline".cyan());
    println!("  {: <14} Configure Roo Code", "--roo".cyan());
    println!("  {: <14} Configure GitHub Copilot CLI", "--copilot".cyan());
    println!("  {: <14} Configure Gemini CLI", "--gemini".cyan());
    println!("  {: <14} Configure OpenCode", "--opencode".cyan());
    println!("  {: <14} Configure Codex CLI", "--codex".cyan());
    println!("  {: <14} Configure OpenClaw", "--openclaw".cyan());
    println!(
        "  {: <14} Configure Antigravity IDE / Generic Webhook",
        "--antigravity".cyan()
    );
    println!("  {: <14} Configure Hermes Agent", "--hermes".cyan());
    println!("  {: <14} Configure VS Code (MCP)", "--vscode".cyan());
    println!("  {: <14} Configure Pi Agent", "--pi".cyan());

    println!("\n{}", "CLAUDE SPECIFIC FLAGS:".bold().bright_white());
    println!(
        "  {: <14} Perform full Claude setup (hooks + MCP)",
        "--all".cyan()
    );
    println!("  {: <14} Only install hooks", "--hook".cyan());
    println!("  {: <14} Only register MCP server", "--mcp".cyan());
    println!(
        "  {: <14} Check current installation status",
        "--status".cyan()
    );
    println!(
        "  {: <14} Remove OMNI hooks and MCP server",
        "--uninstall".cyan()
    );

    println!("  {: <14} Show this help message", "--help, -h".cyan());

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
            "\n{} {} — Choose an AI Agent to configure:\n",
            "omni".bold().cyan(),
            "init".bold().yellow()
        );
        println!("  [{}]  Claude Code (Anthropic)", "1".cyan());
        println!("  [{}]  Cursor AI", "2".cyan());
        println!("  [{}]  Zed Editor", "3".cyan());
        println!("  [{}]  Cline", "4".cyan());
        println!("  [{}]  Roo Code", "5".cyan());
        println!("  [{}]  GitHub Copilot CLI", "6".cyan());
        println!("  [{}]  Gemini CLI", "7".cyan());
        println!("  [{}]  OpenCode", "8".cyan());
        println!("  [{}]  Codex CLI", "9".cyan());
        println!("  [{}] OpenClaw", "10".cyan());
        println!("  [{}] Antigravity IDE", "11".cyan());
        println!("  [{}] Hermes Agent", "12".cyan());
        println!("  [{}] VS Code (MCP)", "13".cyan());
        println!("  [{}]  Pi Agent", "14".cyan());
        println!("  [{}]  Quit\n", "q".yellow());

        use std::io::Write;
        print!("Select an option [1-14, q]: ");
        std::io::stdout().flush()?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        match input.trim() {
            "1" => {
                is_claude = true;
                is_hook = true;
                is_mcp = true;
            }
            "2" => is_cursor = true,
            "3" => is_zed = true,
            "4" => is_cline = true,
            "5" => is_roo = true,
            "6" => is_copilot = true,
            "7" => is_gemini = true,
            "8" => is_opencode = true,
            "9" => is_codex = true,
            "10" => is_openclaw = true,
            "11" => is_antigravity = true,
            "12" => is_hermes = true,
            "13" => is_vscode = true,
            "14" => is_pi = true,
            _ => return Ok(()),
        }
        println!();
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
