use crate::store::sqlite::Store;
use colored::*;

pub fn print_help() {
    println!(
        "\n{} {} — Retrieve unfiltered original output",
        "omni".bold().cyan(),
        "rewind".bold().yellow()
    );
    println!("\n{}", "USAGE:".bold().bright_white());
    println!("  omni {} [hash]", "rewind".cyan());

    println!("\n{}", "DESCRIPTION:".bold().bright_white());
    println!(
        "  OMNI heavily filters output from chatty commands (e.g. git diff, npm install) to save tokens."
    );
    println!(
        "  If you ever need to see the FULL, unfiltered output for debugging, use this command."
    );
    println!(
        "  If no hash is provided, it automatically fetches the last filtered command's output."
    );
    println!();
}

pub fn run(args: &[String], store: &Store) -> anyhow::Result<()> {
    if args
        .iter()
        .any(|a| a == "--help" || a == "-h" || a == "help")
    {
        print_help();
        return Ok(());
    }

    let target_hash = if args.len() >= 3 {
        args[2].clone()
    } else {
        match store.get_latest_rewind_hash() {
            Ok(Some(h)) => {
                eprintln!(
                    "{}",
                    format!(
                        "{} No hash provided. Auto-fetching latest intercepted log (id: {})...\n",
                        "➜".cyan(),
                        h.bold()
                    )
                    .bright_black()
                );
                h
            }
            Ok(None) => {
                eprintln!(
                    "{}",
                    "⚠ No recent intercepted logs found in the RewindStore.".yellow()
                );
                return Ok(());
            }
            Err(e) => {
                anyhow::bail!("Database error: {}", e);
            }
        }
    };

    match store.retrieve_rewind(&target_hash) {
        Some(content) => {
            println!("{}", content);
        }
        None => {
            eprintln!(
                "{}",
                format!(
                    "✗ Could not find original log for id '{}'. It may have expired.",
                    target_hash
                )
                .red()
            );
            std::process::exit(1);
        }
    }

    Ok(())
}
