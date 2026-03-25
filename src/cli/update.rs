use colored::*;
use std::io::{self, Write};
use std::process::Command;

pub fn print_help() {
    println!(
        "\n{} {} — Upgrade OMNI to latest version",
        "omni".bold().cyan(),
        "update".bold().yellow()
    );
    println!("\n{}", "USAGE:".bold().bright_white());
    println!("  omni {}", "update".cyan());

    println!("\n{}", "DESCRIPTION:".bold().bright_white());
    println!("  Fetches the latest version from GitHub and upgrades OMNI.");
    println!("  Currently supports Homebrew-based installations.");
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

    let latest = crate::guard::update::check();

    if let Some(latest_ver) = latest {
        println!(
            "\n{} A new version is available: {} → {}",
            "✨".yellow(),
            env!("CARGO_PKG_VERSION").bright_black(),
            latest_ver.green().bold()
        );

        print!(
            "   Confirm update to version {}? [y/N]: ",
            latest_ver.green().bold()
        );
        io::stdout().flush().unwrap();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            return Err("Failed to read input".to_string());
        }

        let input = input.trim().to_lowercase();
        if input != "y" && input != "yes" {
            println!("Update aborted.");
            return Ok(());
        }

        println!("{} Updating OMNI via Homebrew...", "🚀".cyan());

        // Run brew upgrade
        let status = Command::new("brew")
            .args(["upgrade", "fajarhide/tap/omni"])
            .status();

        match status {
            Ok(s) if s.success() => {
                println!("\n{} OMNI updated successfully!", "✓".green());
            }
            Ok(s) => {
                return Err(format!(
                    "Brew upgrade failed with exit code: {}. You may need to run 'brew update' first.",
                    s.code().unwrap_or(1)
                ));
            }
            Err(e) => {
                return Err(format!(
                    "Failed to execute 'brew': {}. Please run 'brew upgrade fajarhide/tap/omni' manually.",
                    e
                ));
            }
        }
    } else {
        println!(
            "\n{} OMNI is already up to date (v{}).",
            "✓".green(),
            env!("CARGO_PKG_VERSION")
        );
    }

    Ok(())
}
