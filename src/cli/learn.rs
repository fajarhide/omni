use crate::session::learn::{apply_to_config, detect_patterns};
use anyhow::Result;
use chrono::Utc;
use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::PathBuf;

pub fn run_learn(args: &[String]) -> Result<()> {
    let apply = args.iter().any(|a| a == "--apply");
    let dry_run = args.iter().any(|a| a == "--dry-run");
    let from_queue = args.iter().any(|a| a == "--from-queue");
    let verify = args.iter().any(|a| a == "--verify");

    if verify {
        println!("Running inline tests for all loaded TOML filters...");
        let report = crate::pipeline::toml_filter::run_inline_tests(
            &crate::pipeline::toml_filter::load_all_filters(),
        );
        let total = report.passes + report.failures.len();
        println!("Filters passed: {}/{}", report.passes, total);
        if !report.failures.is_empty() {
            println!("Failures:");
            for f in report.failures {
                println!("- {}", f);
            }
        }
        return Ok(());
    }

    let mut input = String::new();

    let mut use_queue = from_queue;
    if !use_queue && io::stdin().is_terminal() {
        use_queue = true;
    }

    if use_queue {
        let dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".omni");
        let path = dir.join("learn_queue.jsonl");
        if path.exists() {
            let content = fs::read_to_string(&path)?;
            for line in content.lines() {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(line)
                    && let Some(s) = val.get("sample").and_then(|v| v.as_str())
                {
                    input.push_str(s);
                    input.push('\n');
                }
            }
        } else {
            println!("No learning data available yet.");
            println!(
                "OMNI automatically collects samples of repetitive noise in the background as you use it."
            );
            println!("Run this command again after you've processed more unclassified output.");
            return Ok(());
        }
    } else {
        io::stdin().read_to_string(&mut input)?;
    }

    let candidates = detect_patterns(&input);

    if candidates.is_empty() {
        println!("No repetitive active noise patterns discovered in input (min 3 occurrences).");
        return Ok(());
    }

    println!("Identified {} candidate patterns:\n", candidates.len());
    for (i, c) in candidates.iter().enumerate() {
        println!(
            "{}. [{}] \"{}\" ({} occurences)",
            i + 1,
            format!("{:?}", c.suggested_action).to_lowercase(),
            c.trigger_prefix,
            c.count
        );
    }

    let filter_name = format!("learned_{}", Utc::now().timestamp());

    if dry_run {
        let generated = crate::session::learn::generate_toml(&candidates, &filter_name);
        println!("\n[Dry Run] Generated TOML configuration:\n{}", generated);
    } else if apply {
        let path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".omni")
            .join("filters")
            .join("learned.toml");
        let added = apply_to_config(&candidates, &filter_name, &path)?;
        if added > 0 {
            println!(
                "\nSuccessfully appended {} new triggers to {:?}",
                added, path
            );
        }
    } else {
        println!(
            "\nRun `omni learn --apply` to automatically write these into your ~/.omni filters."
        );
        println!("Or `omni learn --dry-run` to preview the generated TOML.");
    }

    Ok(())
}
