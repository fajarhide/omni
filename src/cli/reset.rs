use crate::agents::all_integrations;
use colored::*;
use std::env;
use std::io::Write;

fn print_help() {
    println!(
        "\n{} {}",
        "OMNI RESET".bold().red(),
        "- Wipe All Omni AI Connections".bold().white()
    );
    println!("Use this command to cleanly remove OMNI configurations from all IDEs and tools.");
    println!();
    println!("Usage: omni reset [OPTIONS]");
    println!();
    println!("Options:");
    println!(
        "  {: <14} Uninstall all integrations and wipe the omni.db",
        "--all".red()
    );
    println!(
        "  {: <14} Uninstall Claude Code (Anthropic)",
        "--claude".cyan()
    );
    println!("  {: <14} Uninstall Cursor AI", "--cursor".cyan());
    println!("  {: <14} Uninstall Zed Editor", "--zed".cyan());
    println!("  {: <14} Uninstall Cline", "--cline".cyan());
    println!("  {: <14} Uninstall Roo Code", "--roo".cyan());
    println!("  {: <14} Uninstall GitHub Copilot CLI", "--copilot".cyan());
    println!("  {: <14} Uninstall Gemini CLI", "--gemini".cyan());
    println!("  {: <14} Uninstall OpenCode", "--opencode".cyan());
    println!("  {: <14} Uninstall Codex CLI", "--codex".cyan());
    println!(
        "  {: <14} Uninstall Antigravity IDE",
        "--antigravity".cyan()
    );
    println!(
        "  {: <14} Display this help message",
        "--help, -h".bright_black()
    );
    println!();
}

pub fn handle_reset() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return Ok(());
    }

    let is_all = args.iter().any(|a| a == "--all");

    let mut is_claude = args.iter().any(|a| a == "--claude");
    let mut is_cursor = args.iter().any(|a| a == "--cursor");
    let mut is_zed = args.iter().any(|a| a == "--zed");
    let mut is_cline = args.iter().any(|a| a == "--cline");
    let mut is_roo = args.iter().any(|a| a == "--roo" || a == "--roo-code");
    let mut is_copilot = args.iter().any(|a| a == "--copilot");
    let mut is_gemini = args.iter().any(|a| a == "--gemini");
    let mut is_opencode = args.iter().any(|a| a == "--opencode");
    let mut is_codex = args.iter().any(|a| a == "--codex");
    let mut is_antigravity = args.iter().any(|a| a == "--antigravity");

    // Check if no flags
    let no_flags = !is_claude
        && !is_cursor
        && !is_zed
        && !is_cline
        && !is_roo
        && !is_copilot
        && !is_gemini
        && !is_opencode
        && !is_codex
        && !is_antigravity;

    if no_flags && !is_all {
        println!(
            "\n{} {}",
            "OMNI RESET".bold().red(),
            "- Interactive Mode".bold().white()
        );
        println!("Which integrations would you like to remove?");
        println!(
            "  [{}] Wipe ALL Agent Integrations & Database",
            "1".red().bold()
        );
        println!("  [{}] Claude Code (Anthropic)", "2".cyan());
        println!("  [{}] Cursor AI", "3".cyan());
        println!("  [{}] Zed Editor", "4".cyan());
        println!("  [{}] Cline VS Code Extension", "5".cyan());
        println!("  [{}] Roo Code VS Code Extension", "6".cyan());
        println!("  [{}] GitHub Copilot CLI", "7".cyan());
        println!("  [{}] Gemini CLI", "8".cyan());
        println!("  [{}] OpenCode", "9".cyan());
        println!("  [{}] Codex CLI", "10".cyan());
        println!("  [{}] Antigravity IDE", "11".cyan());
        println!("  [{}] Cancel\n", "q".yellow());

        print!("Select an option [1-11, q]: ");
        std::io::stdout().flush()?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        match input.trim() {
            "1" => return perform_reset(true, vec![]),
            "2" => is_claude = true,
            "3" => is_cursor = true,
            "4" => is_zed = true,
            "5" => is_cline = true,
            "6" => is_roo = true,
            "7" => is_copilot = true,
            "8" => is_gemini = true,
            "9" => is_opencode = true,
            "10" => is_codex = true,
            "11" => is_antigravity = true,
            _ => return Ok(()),
        }
        println!();
    }

    let mut target_ids = Vec::new();
    if is_claude {
        target_ids.push("claude");
    }
    if is_cursor {
        target_ids.push("cursor");
    }
    if is_zed {
        target_ids.push("zed");
    }
    if is_cline {
        target_ids.push("cline");
    }
    if is_roo {
        target_ids.push("roo-code");
    }
    if is_copilot {
        target_ids.push("copilot");
    }
    if is_gemini {
        target_ids.push("gemini");
    }
    if is_opencode {
        target_ids.push("opencode");
    }
    if is_codex {
        target_ids.push("codex");
    }
    if is_antigravity {
        target_ids.push("antigravity");
    }

    perform_reset(is_all, target_ids)
}

fn perform_reset(is_all: bool, target_ids: Vec<&str>) -> anyhow::Result<()> {
    if is_all {
        println!("\n{} Removing ALL omni agent integrations...", "⟳".yellow());
        for agent in all_integrations() {
            if let Err(e) = agent.uninstall() {
                println!(
                    "  {} Failed to uninstall {}: {}",
                    "x".red(),
                    agent.name(),
                    e
                );
            }
        }

        // Wipe Database Optional behavior
        println!(
            "\n{} Would you like to wipe the SQLite database (~/.omni/omni.db)? [y/N]",
            "?".yellow()
        );
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if input.trim().eq_ignore_ascii_case("y") {
            let db_path = dirs::home_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join(".omni/omni.db");
            if db_path.exists() {
                std::fs::remove_file(&db_path).ok();
                println!("  {} Omni database wiped.", "✓".green());
            }
        }

        println!("  {} All resets completed.", "✓".green());
        return Ok(());
    }

    if target_ids.is_empty() {
        println!("No integrations selected. Aborting.");
        return Ok(());
    }

    println!("\n{} Uninstalling selected integrations...", "⟳".yellow());
    let agents = all_integrations();
    for agent in agents {
        if target_ids.contains(&agent.id())
            && let Err(e) = agent.uninstall()
        {
            println!(
                "  {} Failed to uninstall {}: {}",
                "x".red(),
                agent.name(),
                e
            );
        }
    }

    println!("\n{} Selected integrations have been reset.", "✓".green());
    Ok(())
}
