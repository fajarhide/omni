use crate::store::sqlite::Store;
use anyhow::Result;
use colored::*;

pub fn run_patterns(args: &[String], store: &Store) -> Result<()> {
    if args
        .iter()
        .any(|a| a == "--help" || a == "-h" || a == "help")
    {
        print_help();
        return Ok(());
    }

    let mut tool_family = None;
    let mut i = 2;
    while i < args.len() {
        if args[i] == "--tool" && i + 1 < args.len() {
            tool_family = Some(args[i + 1].as_str());
            i += 2;
        } else {
            i += 1;
        }
    }

    let patterns = store.get_patterns(tool_family, 20);

    println!(
        "\n {} {}",
        "🧠".cyan(),
        "Cross-Session Pattern Memory".bold().bright_white()
    );
    if let Some(tool) = tool_family {
        println!(" Filtering by tool: {}", tool.yellow());
    }
    println!(" ──────────────────────────────────────────────────────────────");

    if patterns.is_empty() {
        println!("   {}", "No recurring patterns found yet.".bright_black());
        println!();
        return Ok(());
    }

    for (i, p) in patterns.iter().enumerate() {
        let status = if p.was_resolved {
            "RESOLVED".green().bold()
        } else {
            "ACTIVE".red().bold()
        };

        println!(
            "\n  {} {} | {} {} | Seen {}x",
            format!("[{}]", i + 1).bright_black(),
            status,
            "Tool:".bright_black(),
            p.tool_family.cyan(),
            p.occurrence_count.to_string().yellow()
        );

        let lines: Vec<&str> = p.pattern_text.lines().collect();
        for line in lines.iter().take(3) {
            println!("       {}", line.bright_white());
        }
        if lines.len() > 3 {
            println!("       {} ...", "---".bright_black());
        }

        if p.was_resolved && !p.resolution_hint.is_empty() {
            println!(
                "       {} {}",
                "Fix hint:".green(),
                p.resolution_hint.green()
            );
        }
    }

    println!();
    Ok(())
}

fn print_help() {
    println!(
        "\n{} {} — View recurring cross-session error patterns",
        "omni".bold().cyan(),
        "patterns".bold().yellow()
    );
    println!("\n{}", "USAGE:".bold().bright_white());
    println!(
        "  omni {} {}",
        "patterns".cyan(),
        "[OPTIONS]".bright_black()
    );

    println!("\n{}", "OPTIONS:".bold().bright_white());
    println!(
        "  {: <15} Filter patterns by tool (e.g., cargo, npm)",
        "--tool <name>".cyan()
    );
    println!("  {: <15} Show this help message", "--help, -h".cyan());

    println!("\n{}", "EXAMPLES:".bold().bright_white());
    println!(
        "  omni patterns                {} Show top patterns",
        "#".bright_black()
    );
    println!(
        "  omni patterns --tool cargo   {} Cargo only",
        "#".bright_black()
    );
    println!();
}
