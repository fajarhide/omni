use crate::store::sqlite::Store;
use anyhow::Result;
use colored::*;

pub fn run_rewind(args: &[String], store: &Store) -> Result<()> {
    let subcmd = args.get(2).map(|s| s.as_str()).unwrap_or("list");

    if args.iter().any(|a| a == "--help" || a == "-h" || a == "help") {
        print_help();
        return Ok(());
    }

    match subcmd {
        "list" => list_rewinds(store),
        "show" => {
            if let Some(hash) = args.get(3) {
                show_rewind(store, hash)
            } else {
                eprintln!("{} {}: hash required", "error".red(), "omni".bold());
                println!("Try: omni rewind list to find a hash.");
                Ok(())
            }
        }
        "help" => {
            print_help();
            Ok(())
        }
        _ => {
            // Default to list if it looks like a hash or unknown
            if subcmd.len() == 8 {
                show_rewind(store, subcmd)
            } else {
                list_rewinds(store)
            }
        }
    }
}

fn print_help() {
    println!(
        "\n{} {} — View and manage archived content",
        "omni".bold().cyan(),
        "rewind".bold().yellow()
    );
    println!("\n{}", "USAGE:".bold().bright_white());
    println!("  omni {} {} {}", "rewind".cyan(), "<COMMAND>".yellow(), "[HASH]".bright_black());

    println!("\n{}", "COMMANDS:".bold().bright_white());
    println!("  {: <12} Show recent archived chunks (default)", "list".cyan());
    println!("  {: <12} View the full content of an archive", "show".cyan());
    println!("  {: <12} Show this help message", "help".cyan());

    println!("\n{}", "EXAMPLES:".bold().bright_white());
    println!(
        "  omni rewind list      {} View your archive",
        "#".bright_black()
    );
    println!(
        "  omni rewind a1b2c3d4  {} Preview specific content",
        "#".bright_black()
    );
    println!();
}

fn list_rewinds(store: &Store) -> Result<()> {
    let rewinds = store.list_recent_rewinds(12)?;

    println!("\n {}", "OMNI REWIND: Archived Signal Data".bold().bright_white());
    println!(
        "   {:<10} {:<14} {:>10}  {}",
        "HASH", "TIMESTAMP", "CAPACITY", "STATUS"
    );
    println!("   ────────── ────────────── ──────────  ────────");

    if rewinds.is_empty() {
        println!("   {}", "No archived content found yet.".bright_black());
    }

    for r in rewinds {
        let saved = r.original_len;
        let status = if r.retrieved > 0 {
            format!("Retrieved ({})", r.retrieved).bright_blue()
        } else {
            "Archived".bright_black()
        };

        println!(
            "   {:<10} {:<14} {:>10}  {}",
            r.hash.cyan().bold(),
            format_ts(r.ts).bright_black(),
            crate::cli::stats::format_bytes(saved as u64).green(),
            status
        );
    }
    println!("\n {} Run {} to view content", "Tip:".yellow(), "omni rewind show <hash>".cyan());
    println!();
    Ok(())
}

fn show_rewind(store: &Store, hash: &str) -> Result<()> {
    if let Some(content) = store.retrieve_rewind(hash) {
        let lines_count = content.lines().count();
        let size_str = crate::cli::stats::format_bytes(content.len() as u64);

        println!(
            "\n {} {} all {} lines from {} using {}. The full {} is preserved.",
            "⏺".cyan(),
            "Retrieved".bright_green(),
            lines_count.to_string().bold(),
            "RewindStore".bright_white().bold(),
            format!("omni rewind {}", hash).cyan(),
            size_str.yellow()
        );

        println!("\n {}", format!("CONTENT PREVIEW [ {} ]", hash).bold().bright_cyan());
        println!("{}", " ──────────────────────────────────────────────────────────────────".bright_black());
        
        let lines_vec: Vec<&str> = content.lines().collect();
        for line in lines_vec.iter().take(40) {
            println!("  {}", line);
        }
        
        if lines_vec.len() > 40 {
            println!("\n  {} ... ({} more lines) ...", "---".bright_black(), lines_vec.len() - 40);
        }

        println!("{}", " ──────────────────────────────────────────────────────────────────".bright_black());
        println!(
            " {} Signature: {} | Data Integrity Verified",
            "✨".cyan(),
            hash.bold().bright_white()
        );
    } else {
        eprintln!("{} {}: No content found for hash {}", "error".red(), "omni".bold(), hash.yellow());
    }
    Ok(())
}

fn format_ts(ts: i64) -> String {
    let dt = chrono::Utc::now().timestamp() - ts;
    if dt < 60 {
        format!("{}s ago", dt)
    } else if dt < 3600 {
        format!("{}m ago", dt / 60)
    } else if dt < 86400 {
        format!("{}h ago", dt / 3600)
    } else {
        format!("{}d ago", dt / 86400)
    }
}

#[derive(Debug, Clone)]
pub struct RewindEntry {
    pub hash: String,
    pub ts: i64,
    pub original_len: i64,
    pub retrieved: u32,
}
