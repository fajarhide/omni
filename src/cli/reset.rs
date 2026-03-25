use colored::*;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn print_help() {
    println!(
        "\n{} {} — Clean uninstall (with backups)",
        "omni".bold().cyan(),
        "reset".bold().yellow()
    );
    println!("\n{}", "USAGE:".bold().bright_white());
    println!("  omni {} [--yes]", "reset".cyan());

    println!("\n{}", "DESCRIPTION:".bold().bright_white());
    println!("  Performs a clean uninstall of OMNI by:");
    println!("  1. Backing up ~/.omni to a .bak folder");
    println!("  2. Removing hooks from Claude settings");
    println!("  3. Unregistering the MCP server");
    println!();
}

pub fn run(args: &[String]) -> Result<(), String> {
    if args
        .iter()
        .any(|a| a == "--help" || a == "-h" || a == "help")
    {
        print_help();
        return Ok(());
    }

    if args.iter().any(|a| a == "--yes" || a == "-y") {
        // Skip prompt
    } else {
        use std::io::{self, Write};
        print!(
            "{} Are you sure you want to uninstall OMNI? [y/N]: ",
            "⚠".yellow().bold()
        );
        io::stdout().flush().unwrap();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            return Err("Failed to read input".to_string());
        }
        let input = input.trim().to_lowercase();
        if input != "y" && input != "yes" {
            println!("Reset aborted.");
            return Ok(());
        }
    }

    let home_dir = dirs::home_dir().ok_or("Could not determine home directory")?;
    let omni_dir = home_dir.join(".omni");

    if !omni_dir.exists() {
        println!(
            "\n{} The ~/.omni directory does not exist. Nothing to reset.",
            "ℹ".blue()
        );
        return Ok(());
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let backup_dir_name = format!(".omni.{}.bak", timestamp);
    let backup_dir = home_dir.join(&backup_dir_name);

    if let Err(e) = fs::rename(&omni_dir, &backup_dir) {
        return Err(format!("{} Failed to backup ~/.omni: {}", "✗".red(), e));
    }

    println!(
        "\n{} {} backed up successfully.",
        "✓".green(),
        "Data".bold()
    );
    println!("  Moved ~/.omni to ~/{}\n", backup_dir_name.bright_black());

    println!("{} Cleaning up agent integrations...", "ℹ".blue());
    let args = vec!["--uninstall".to_string()];
    if let Err(e) = crate::cli::init::run_init(&args) {
        println!(
            "  {} (Note: could not fully remove hooks/MCP: {})",
            "⚠".yellow(),
            e
        );
    }

    println!(
        "\n{} You can now safely uninstall OMNI.",
        "✓".green().bold()
    );
    println!(
        "  Run: {} brew uninstall fajarhide/tap/omni\n",
        "→".yellow()
    );

    Ok(())
}
