use crate::session::correction::{self, CommandExecution};
use crate::session::learn::{apply_to_config, detect_patterns};
use anyhow::Result;
use chrono::Utc;
use colored::*;
use std::fs;
use std::io::{self, IsTerminal, Read};

fn print_help() {
    println!(
        "\n{} {} — Auto-generate filters from history",
        "omni".bold().cyan(),
        "learn".bold().yellow()
    );
    println!("\n{}", "USAGE:".bold().bright_white());
    println!("  omni learn {}", "[FLAGS]".bright_black());

    println!("\n{}", "FLAGS:".bold().bright_white());
    println!(
        "  {: <12} Discover and view candidate patterns",
        "--discover".cyan()
    );
    println!(
        "  {: <12} Automatically append new filters to config",
        "--apply".cyan()
    );
    println!(
        "  {: <12} Preview the generated TOML without writing",
        "--dry-run".cyan()
    );
    println!(
        "  {: <12} Use background learning queue as source",
        "--from-queue".cyan()
    );
    println!(
        "  {: <12} Run inline tests for all existing filters",
        "--verify".cyan()
    );
    println!("  {: <12} Show this help message", "--help, -h".cyan());

    println!("\n{}", "EXAMPLES:".bold().bright_white());
    println!(
        "  omni learn --discover {}",
        "# Search for new noise patterns".bright_black()
    );
    println!(
        "  omni learn --dry-run  {}",
        "# Preview suggested filters".bright_black()
    );
    println!(
        "  omni learn --apply    {}",
        "# Commit suggestions to config".bright_black()
    );
    println!(
        "  omni learn --from-queue --dry-run {}",
        "# Learn from recent history".bright_black()
    );
    println!(
        "  omni learn --verify   {}",
        "# Test existing filters".bright_black()
    );
    println!();
}

pub fn run_learn(args: &[String]) -> Result<()> {
    if args
        .iter()
        .any(|a| a == "--help" || a == "-h" || a == "help")
    {
        print_help();
        return Ok(());
    }

    let apply = args.iter().any(|a| a == "--apply");
    let dry_run = args.iter().any(|a| a == "--dry-run");
    let from_queue = args.iter().any(|a| a == "--from-queue");
    let verify = args.iter().any(|a| a == "--verify");
    let is_discover = args.iter().any(|a| a == "--discover");

    // If no flags, show help
    if !apply && !dry_run && !from_queue && !verify && !is_discover {
        print_help();
        return Ok(());
    }

    if verify {
        println!(
            "\n{}",
            "Running inline tests for loaded filters:"
                .bold()
                .bright_white()
        );
        let all_filters = crate::pipeline::toml_filter::load_all_filters();
        let report = crate::pipeline::toml_filter::run_inline_tests(all_filters);

        let (mut built_in_fails, mut user_fails) = (0, 0);
        let (mut built_in_total, mut user_total) = (0, 0);

        for filter in all_filters {
            let count = filter.inline_tests.len();
            if filter.name.starts_with("sys_") {
                built_in_total += count;
            } else {
                user_total += count;
            }
        }

        for fail in &report.failures {
            if fail.contains("Filter 'sys_") {
                built_in_fails += 1;
            } else {
                user_fails += 1;
            }
        }

        let total_tests = built_in_total + user_total;
        let total_passes = report.passes;

        println!(
            "  Total Tests: {} ({} passed, {} failed)",
            total_tests,
            total_passes,
            report.failures.len()
        );
        println!(
            "  - Built-in:  {} tests ({} failed)",
            built_in_total, built_in_fails
        );
        println!(
            "  - User:      {} tests ({} failed)",
            user_total, user_fails
        );

        if !report.failures.is_empty() {
            println!("\n{}", "Failure Details:".bold().red());
            for f in report.failures {
                println!("  {} {}", "✗".red(), f);
            }
            if user_fails > 0 {
                println!(
                    "\n{}",
                    "TIP: Learned filters often fail if the noise pattern has changed. You can clear them by deleting ~/.omni/filters/learned.toml"
                        .dimmed()
                );
            }
        } else {
            println!(
                "\n{}",
                "✓ All loaded filters pass their inline tests!".green()
            );
        }

        return Ok(());
    }

    let mut input = String::new();

    // In terminal mode, if an action is used without --from-queue, default to queue
    let mut use_queue = from_queue;
    let is_action = is_discover || dry_run || apply;
    if is_action && !from_queue && io::stdin().is_terminal() {
        use_queue = true;
    }

    let mut executions = Vec::new();

    if use_queue {
        let dir = crate::paths::omni_home();
        let path = dir.join("learn_queue.jsonl");
        if path.exists() {
            let content = fs::read_to_string(&path)?;
            for line in content.lines() {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(line)
                    && let Some(s) = val.get("sample").and_then(|v| v.as_str())
                    && let Some(c) = val.get("command").and_then(|v| v.as_str())
                {
                    input.push_str(s);
                    input.push('\n');
                    executions.push(CommandExecution {
                        command: c.to_string(),
                        is_error: false, // Will be inferred by classify_error
                        output: s.to_string(),
                    });
                }
            }
        } else {
            println!("\n{} No learning data available yet.", "ℹ".blue());
            println!("  OMNI collects samples of repetitive noise as you use it.");
            println!("  Run this again after you've processed more output.\n");
            return Ok(());
        }
    } else {
        io::stdin().read_to_string(&mut input)?;
    }

    let candidates = detect_patterns(&input);

    println!(
        "\n{}",
        "─────────────────────────────────────────"
            .bright_black()
            .bold()
    );
    println!(" {} — Pattern Discovery", "OMNI".bold().cyan());
    println!(
        "{}",
        "─────────────────────────────────────────"
            .bright_black()
            .bold()
    );

    if candidates.is_empty() {
        println!("  {} No repetitive noise patterns discovered.", "ℹ".blue());
        println!(
            "  {} Requirement: minimum 3 occurrences.",
            "•".bright_black()
        );
        println!(
            "{}\n",
            "─────────────────────────────────────────"
                .bright_black()
                .bold()
        );
        return Ok(());
    }

    println!(
        "  {} Identified {} potential noise patterns:\n",
        "⚡".yellow(),
        candidates.len().to_string().yellow().bold()
    );

    // Header Table
    println!(
        "    {: <4} {: <10} {: <45} {: <8}",
        "#".bright_black(),
        "ACTION".bright_black(),
        "PATTERN PREVIEW".bright_black(),
        "COUNT".bright_black()
    );
    println!("    {}", "─".repeat(70).bright_black());

    for (i, c) in candidates.iter().enumerate() {
        let action = format!("{:?}", c.suggested_action).to_lowercase();
        let mut preview = c.trigger_prefix.clone();
        if preview.len() > 42 {
            preview.truncate(39);
            preview.push_str("...");
        }

        println!(
            "    {:02}. {: <10} {: <45} {: <8}",
            (i + 1).to_string().bright_black(),
            action.cyan(),
            preview.bright_white(),
            format!("{}x", c.count).yellow()
        );
    }

    // DISCOVER CORRECTIONS
    let correction_pairs = correction::find_corrections(&executions);
    let correction_rules = correction::deduplicate_corrections(correction_pairs);

    if !correction_rules.is_empty() {
        println!(
            "\n  {} Identified {} common command corrections:\n",
            "💡".bright_cyan(),
            correction_rules.len().to_string().cyan().bold()
        );

        for (i, rule) in correction_rules.iter().take(5).enumerate() {
            println!(
                "    {:02}. {: <25} → {: <25} ({}x)",
                (i + 1).to_string().bright_black(),
                rule.wrong_pattern.red(),
                rule.right_pattern.green(),
                format!("{}x", rule.occurrences).yellow()
            );
            println!(
                "        {} {} | Base: {}",
                "Cause:".bright_black(),
                rule.error_type.as_str().bright_black(),
                rule.base_command.bright_black()
            );
        }
    }

    let filter_name = format!("learned_{}", Utc::now().timestamp());

    let command_hint = executions.first().map(|e| e.command.as_str());

    if dry_run {
        let generated =
            crate::session::learn::generate_toml(&candidates, &filter_name, command_hint);
        println!(
            "\n{}",
            "─────────────────────────────────────────"
                .bright_black()
                .bold()
        );
        println!(
            " {} Suggested TOML Configuration:",
            "Preview".bold().bright_white()
        );
        println!(
            "{}",
            "─────────────────────────────────────────"
                .bright_black()
                .bold()
        );
        println!("{}", generated.cyan());
        println!(
            "{}",
            "─────────────────────────────────────────"
                .bright_black()
                .bold()
        );
    } else if apply {
        let path = crate::paths::learned_filters_path();
        let _ = crate::paths::ensure_omni_home();
        let added = apply_to_config(&candidates, &filter_name, &path, command_hint)?;
        if added > 0 {
            println!(
                "\n{}",
                "─────────────────────────────────────────"
                    .bright_black()
                    .bold()
            );
            println!(
                "  {} Successfully added {} triggers to {:?}",
                "✓".green(),
                added,
                path
            );

            // AUTO-CLEAR QUEUE after successful apply
            if use_queue {
                let queue_path = crate::paths::omni_home().join("learn_queue.jsonl");
                if queue_path.exists() {
                    let _ = fs::write(&queue_path, ""); // Truncate the file
                    println!(
                        "  {} Learning queue cleared. {} pending samples processed.",
                        "✨".bright_white(),
                        executions.len().to_string().yellow()
                    );
                }
            }

            println!(
                "{}",
                "─────────────────────────────────────────"
                    .bright_black()
                    .bold()
            );
        } else {
            println!(
                "  {} No new patterns to apply (all already exist).",
                "ℹ".blue()
            );
        }
    } else {
        println!(
            "\n{}",
            "─────────────────────────────────────────"
                .bright_black()
                .bold()
        );
        println!(
            "  {} Run {} to commit these filters.",
            "→".yellow(),
            "omni learn --apply".cyan().bold()
        );
        println!(
            "  {} Run {} to preview TOML configuration.",
            "→".yellow(),
            "omni learn --dry-run".cyan().bold()
        );
        println!(
            "{}",
            "─────────────────────────────────────────"
                .bright_black()
                .bold()
        );
    }

    Ok(())
}
