use crate::store::sqlite::Store;
use crate::util::token_estimate::{ContentHint, estimate_tokens};
use anyhow::Result;
use colored::*;
use std::collections::HashMap;

// ─── Helper Functions ───────────────────────────────────

pub fn format_bytes(n: u64) -> String {
    if n < 1024 {
        format!("{} B", n)
    } else if n < 1024 * 1024 {
        format!("{:.1} KB", n as f64 / 1024.0)
    } else if n < 1024 * 1024 * 1024 {
        format!("{:.1} MB", n as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", n as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

#[allow(dead_code)]
pub fn format_tokens(bytes: u64) -> String {
    let tokens = estimate_tokens(bytes as usize, ContentHint::Mixed) as u64;
    if tokens < 1000 {
        format!("{}", tokens)
    } else if tokens < 1_000_000 {
        format!("{:.0}K", tokens as f64 / 1_000.0)
    } else {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    }
}

pub fn format_exact_tokens(tokens: u64) -> String {
    if tokens < 1000 {
        format!("{}", tokens)
    } else if tokens < 1_000_000 {
        format!("{:.0}K", tokens as f64 / 1_000.0)
    } else {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    }
}

pub fn format_bar(pct: f64) -> String {
    let width = 20;
    let filled = ((pct / 100.0) * width as f64).round() as usize;
    let filled = filled.min(width);
    "█".repeat(filled)
}

fn format_bar_with_empty(pct: f64) -> String {
    let width = 20;
    let filled = ((pct / 100.0) * width as f64).round() as usize;
    let filled = filled.min(width);
    let empty = width - filled;
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}

pub fn est_cost_usd(bytes_saved: u64) -> f64 {
    est_cost_usd_with_hint(bytes_saved, ContentHint::Mixed)
}

pub fn est_cost_usd_with_hint(bytes_saved: u64, hint: ContentHint) -> f64 {
    let price = crate::guard::config::get_input_cost();
    let tokens = estimate_tokens(bytes_saved as usize, hint) as f64;
    (tokens / 1_000_000.0) * price
}

fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

fn group_and_calculate_stats(
    items: Vec<(String, u64, u64, u64, u64, u64)>,
    limit: usize,
) -> Vec<(String, u64, f64, u64)> {
    let mut grouped: HashMap<String, (u64, u64, u64, u64, u64)> = HashMap::new();

    for (cmd, calls, input, output, raw_tok, filt_tok) in items {
        // Group by the shortened version so things like "npm install x" and "npm install y" combine
        let key = shorten_command(&cmd, 18);
        let entry = grouped.entry(key).or_insert((0, 0, 0, 0, 0));
        entry.0 += calls;
        entry.1 += input;
        entry.2 += output;
        entry.3 += raw_tok;
        entry.4 += filt_tok;
    }

    let mut result: Vec<(String, u64, f64, u64)> = grouped
        .into_iter()
        .map(|(cmd, (calls, input, output, raw_tok, filt_tok))| {
            let pct = if raw_tok > 0 {
                100.0 * (1.0 - filt_tok as f64 / raw_tok as f64)
            } else if input > 0 {
                100.0 * (1.0 - output as f64 / input as f64)
            } else {
                0.0
            };

            let tokens_saved = if raw_tok > 0 {
                raw_tok.saturating_sub(filt_tok)
            } else if input > 0 {
                let r = crate::util::token_estimate::estimate_tokens(
                    input as usize,
                    crate::util::token_estimate::ContentHint::Mixed,
                );
                let f = crate::util::token_estimate::estimate_tokens(
                    output as usize,
                    crate::util::token_estimate::ContentHint::Mixed,
                );
                r.saturating_sub(f) as u64
            } else {
                0
            };

            (cmd, calls, pct, tokens_saved)
        })
        .collect();

    result.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    if limit > 0 {
        result.truncate(limit);
    }
    result
}

fn get_top_commands(store: &Store, since: i64, limit: usize) -> Vec<(String, u64, f64, u64)> {
    let raw = store
        .get_per_command_stats(since, limit * 3)
        .unwrap_or_default();

    group_and_calculate_stats(raw, limit)
}

fn shorten_command(cmd: &str, max_len: usize) -> String {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    let short = match parts.len() {
        0 => return "[pipe]".to_string(),
        1 => parts[0].to_string(),
        _ => format!("{} {}", parts[0], parts[1]),
    };
    if short.len() <= max_len {
        short
    } else {
        format!("{}...", &short[..max_len.saturating_sub(3)])
    }
}

fn agent_display_name(agent_id: &str) -> &str {
    match agent_id {
        "claude_code" | "claude" => "Claude Code",
        "cursor" => "Cursor AI",
        "zed" => "Zed Editor",
        "cline" => "Cline",
        "roo-code" | "roo_code" => "Roo Code",
        "copilot" => "Copilot CLI",
        "gemini" => "Gemini CLI",
        "opencode" => "OpenCode",
        "codex" => "Codex CLI",
        "openclaw" => "OpenClaw",
        "antigravity" => "Antigravity",
        "vscode" => "VS Code",
        "unknown" | "terminal" | "" => "Terminal",
        other => other,
    }
}

fn print_separator() {
    println!(
        "{}",
        "─────────────────────────────────────────────────"
            .bright_black()
            .bold()
    );
}

fn print_help() {
    println!(
        "\n{} {} — Token savings analytics",
        "omni".bold().cyan(),
        "stats".bold().yellow()
    );
    println!("\n{}", "USAGE:".bold().bright_white());
    println!("  omni {} {}", "stats".cyan(), "[FLAGS]".bright_black());

    println!("\n{}", "FLAGS:".bold().bright_white());
    println!(
        "  {: <12} Full technical breakdown (commands, routes, sessions, agents)",
        "--detail".cyan()
    );
    println!("  {: <12} Scope to today only", "--today".cyan());
    println!("  {: <12} Scope to last 7 days", "--week".cyan());
    println!(
        "  {: <12} Scope to last 30 days (default for --detail)",
        "--month".cyan()
    );
    println!("  {: <12} Machine-readable JSON output", "--json".cyan());
    println!(
        "  {: <12} Display breakdown per project path",
        "--project".cyan()
    );
    println!(
        "  {: <12} Show context composition signals",
        "--context".cyan()
    );
    println!("  {: <12} Show this help message", "--help, -h".cyan());

    println!("\n{}", "EXAMPLES:".bold().bright_white());
    println!(
        "  omni stats              {} Gain-focused overview",
        "#".bright_black()
    );
    println!(
        "  omni stats --detail     {} Full breakdown with commands",
        "#".bright_black()
    );
    println!(
        "  omni stats --json       {} Machine-readable for CI/CD",
        "#".bright_black()
    );
    println!();
}

// ─── Main Entry ─────────────────────────────────────────

pub fn run(args: &[String], store: &Store) -> Result<()> {
    if args
        .iter()
        .any(|a| a == "--help" || a == "-h" || a == "help")
    {
        print_help();
        return Ok(());
    }

    let detail_flag = args.iter().any(|a| a == "--detail");
    let json_flag = args.iter().any(|a| a == "--json");
    let project_flag = args.iter().any(|a| a == "--project");
    let context_flag = args.iter().any(|a| a == "--context");
    let filter_flag = args
        .iter()
        .any(|a| a == "--today" || a == "--week" || a == "--month" || a == "--all-commands");

    let mode = if context_flag {
        "context"
    } else if detail_flag {
        "detail"
    } else if json_flag {
        "json"
    } else if project_flag {
        "project"
    } else if filter_flag {
        "detail"
    } else {
        "default"
    };

    match mode {
        "context" => run_context_stats(store),
        "project" => run_project_stats(args, store),
        "detail" => run_detail(args, store),
        "json" => run_json(store),
        _ => run_default(store),
    }
}

// ─── Context Mode: Context Composition Analyzer ────────
fn run_context_stats(store: &Store) -> Result<()> {
    println!();
    print_separator();
    println!(" {}", "OMNI Signal Report — Context".bold().bright_white());
    print_separator();

    if let Some(session) = store.find_latest_session() {
        let turn = &session.current_turn;
        println!(
            "  {:<25} {}",
            "Session ID:".bright_black(),
            session.session_id.cyan()
        );
        println!(
            "  {:<25} {}",
            "Commands (Turns):".bright_black(),
            format_number(session.command_count as u64).cyan()
        );
        println!("\n  {}", "Token Breakdown:".bold().bright_white());
        println!(
            "    {:<25} {} tokens",
            "File Reads:".bright_black(),
            format_exact_tokens(turn.file_read_tokens).yellow()
        );
        println!(
            "    {:<25} {} tokens",
            "Tool Outputs:".bright_black(),
            format_exact_tokens(turn.tool_output_tokens).green()
        );

        let est_total = turn.file_read_tokens + turn.tool_output_tokens;
        println!(
            "\n  {:<27} {} tokens",
            "Estimated Context Total:".bold().bright_white(),
            format_exact_tokens(est_total).bright_cyan()
        );

        if turn.has_duplicate_file_reads {
            println!(
                "\n  {}",
                "WARNING: Duplicate File Reads Detected!"
                    .bold()
                    .bright_red()
            );
            for f in turn.duplicate_files.iter().take(5) {
                println!("    - {}", f.red());
            }
        }

        if turn.largest_single_read.1 > 0 {
            println!(
                "\n  {:<27} {} ({} tokens)",
                "Largest File Read:".bright_black(),
                turn.largest_single_read.0.cyan(),
                format_exact_tokens(turn.largest_single_read.1).yellow()
            );
        }
    } else {
        println!("  {}", "No active session found.".bright_black().italic());
    }

    print_separator();
    println!();
    Ok(())
}

// ─── Default Mode: Gain-Focused Multi-Period ────────────

fn run_default(store: &Store) -> Result<()> {
    let periods = store.multi_period_stats()?;
    let (rewind_stored, rewind_retrieved) = store.rewind_metrics()?;

    let has_data = periods.iter().any(|(_, count, _, _, _, _)| *count > 0);

    println!();
    print_separator();
    println!(" {}", "OMNI Signal Report".bold().bright_white());
    print_separator();

    if !has_data {
        println!(
            "  {}",
            "No data yet! OMNI tracks savings automatically as you work."
                .bright_black()
                .italic()
        );
        println!("  {}", "Try: ls -la | omni".bright_cyan().italic());
        print_separator();
        println!();
        return Ok(());
    }

    // Multi-period rows
    for (label, count, input, output, raw_tokens, filtered_tokens) in &periods {
        if *count == 0 && label != "All Time" {
            continue;
        }

        let input_tokens_str = format_exact_tokens(*raw_tokens);
        let output_tokens_str = format_exact_tokens(*filtered_tokens);

        let reduction_pct = if *raw_tokens > 0 {
            100.0 * (1.0 - *filtered_tokens as f64 / *raw_tokens as f64)
        } else if *input > 0 {
            // Fallback for legacy records that haven't been backfilled properly
            100.0 * (1.0 - *output as f64 / *input as f64)
        } else {
            0.0
        };

        let tokens_saved = raw_tokens.saturating_sub(*filtered_tokens);
        let cost = (tokens_saved as f64 / 1_000_000.0) * crate::guard::config::get_input_cost();

        let pct_colored = if reduction_pct > 70.0 {
            format!("{:.1}% saved", reduction_pct).bright_green()
        } else if reduction_pct > 40.0 {
            format!("{:.1}% saved", reduction_pct).bright_yellow()
        } else {
            format!("{:.1}% saved", reduction_pct).bright_red()
        };

        println!(
            "  {:<12} {:>3} commands │ {:>4} → {:<4} tokens │  {} │ ~${:.2} USD",
            format!("{}:", label).bright_white().bold(),
            format_number(*count).cyan(),
            input_tokens_str.red(),
            output_tokens_str.green(),
            pct_colored,
            cost,
        );
    }

    let top_commands = get_top_commands(store, 0, 8);

    if !top_commands.is_empty() {
        println!("\n  {}", "Top Commands:".bold().bright_white());
        for (cmd, count, pct, tokens_saved) in &top_commands {
            let short_cmd = shorten_command(cmd, 18);
            let bar = format_bar_with_empty(*pct);
            let bar_colored = if *pct > 80.0 {
                bar.bright_green()
            } else if *pct > 40.0 {
                bar.bright_yellow()
            } else {
                bar.bright_red()
            };

            let tokens_str = if *tokens_saved > 0 {
                format!("(-{} tokens)", format_exact_tokens(*tokens_saved)).bright_black()
            } else {
                "".bright_black()
            };

            println!(
                "    {:<18} {}  {:>5.1}%  ({:>2}x)  {}",
                short_cmd.bright_cyan(),
                bar_colored,
                pct,
                count,
                tokens_str,
            );
        }
    }

    // Agent Distribution
    let agent_data = store.get_agent_breakdown(0).unwrap_or_default();

    // Group by display name
    let mut grouped_agents: HashMap<String, (u64, u64, u64)> = HashMap::new();
    for (id, count, input, output) in &agent_data {
        if id == "unknown" || id == "terminal" || id.is_empty() {
            continue;
        }
        let name = agent_display_name(id).to_string();
        let entry = grouped_agents.entry(name).or_insert((0, 0, 0));
        entry.0 += count;
        entry.1 += input;
        entry.2 += output;
    }

    if !grouped_agents.is_empty() {
        let total_cmds: u64 = agent_data.iter().map(|(_, c, _, _)| c).sum();
        println!("\n  {}", "Agent Distribution:".bold().bright_white());

        let mut sorted_agents: Vec<_> = grouped_agents.into_iter().collect();
        sorted_agents.sort_by_key(|a| std::cmp::Reverse(a.1.0));

        for (name, (count, input, output)) in sorted_agents {
            let pct = if total_cmds > 0 {
                count as f64 / total_cmds as f64 * 100.0
            } else {
                0.0
            };
            let savings = if input > 0 {
                100.0 * (1.0 - output as f64 / input as f64)
            } else {
                0.0
            };
            let bar = format_bar_with_empty(pct);
            println!(
                "   {:<18} {}  {:>5.1}%  ({:>2}x)  {:>5.1}% saved",
                name.bright_cyan(),
                bar.bright_blue(),
                pct,
                count,
                savings,
            );
        }
    }

    // RewindStore
    println!(
        "\n  {:<20} {}",
        "RewindStore:".bright_black(),
        format!(
            "{} archived │ {} retrieved",
            rewind_stored, rewind_retrieved
        )
        .bright_magenta()
    );

    print_separator();
    println!(
        "  {} for full breakdown",
        "omni stats --detail".bright_cyan()
    );

    // Update Notification (4h cache)
    if let Some(latest) = crate::guard::update::check() {
        crate::guard::update::print_notification(&latest);
    }

    println!();
    Ok(())
}

// ─── Detail Mode: Current View (Improved) ───────────────

fn run_detail(args: &[String], store: &Store) -> Result<()> {
    let (period_label, since) = if args.iter().any(|a| a == "--today") {
        let now = chrono::Utc::now().timestamp();
        let start = now - (now % 86400);
        ("today", start)
    } else if args.iter().any(|a| a == "--week") {
        ("last 7 days", chrono::Utc::now().timestamp() - 7 * 86400)
    } else {
        ("last 30 days", chrono::Utc::now().timestamp() - 30 * 86400)
    };

    let (count, input_total, output_total, sum_latency, _max_latency, raw_tokens, filtered_tokens) =
        store.aggregate_stats(since)?;
    let reduction_pct = if raw_tokens > 0 {
        100.0 * (1.0 - filtered_tokens as f64 / raw_tokens as f64)
    } else if input_total > 0 {
        100.0 * (1.0 - output_total as f64 / input_total as f64)
    } else {
        0.0
    };
    let avg_latency = if count > 0 {
        sum_latency as f64 / count as f64
    } else {
        0.0
    };
    let (rewind_stored, rewind_retrieved) = store.rewind_metrics()?;

    println!();
    print_separator();
    println!(
        " {}",
        format!("OMNI Signal Report — Detail ({})", period_label.bold()).bright_white()
    );
    print_separator();

    println!(
        "  {:<20} {}",
        "Commands processed:".bright_black(),
        format_number(count).bold().cyan()
    );
    println!(
        "  {:<20} {} {} {}",
        "Data Distilled:".bright_black(),
        format_bytes(input_total).red(),
        "→".bright_black(),
        format_bytes(output_total).green()
    );

    let tokens_saved = raw_tokens.saturating_sub(filtered_tokens);
    let cost_saved = (tokens_saved as f64 / 1_000_000.0) * crate::guard::config::get_input_cost();

    println!(
        "  {:<20} {} {} {}",
        "Tokens Reduced:".bright_black(),
        format_exact_tokens(raw_tokens).red(),
        "→".bright_black(),
        format_exact_tokens(filtered_tokens).green()
    );

    let ratio_msg = format!("{:.1}% reduction", reduction_pct);
    let ratio_colored = if reduction_pct > 70.0 {
        ratio_msg.bold().bright_green()
    } else if reduction_pct > 40.0 {
        ratio_msg.bold().bright_yellow()
    } else {
        ratio_msg.bold().bright_red()
    };
    println!("  {:<20} {}", "Signal Ratio:".bright_black(), ratio_colored);
    println!(
        "  {:<20} {}",
        "Actual Savings:".bright_black(),
        format!("~${:.3} USD", cost_saved).bold().bright_cyan()
    );
    println!(
        "  {:<20} {}",
        "Average Latency:".bright_black(),
        format!("{:.1}ms", avg_latency).bright_blue()
    );
    println!(
        "  {:<20} {}",
        "RewindStore:".bright_black(),
        format!(
            "{} archived / {} retrieved",
            rewind_stored, rewind_retrieved
        )
        .bright_magenta()
    );

    // Collapse savings
    let collapse_stats = store.collapse_aggregate(since);
    if let Ok((events, total_original, total_collapsed)) = collapse_stats
        && events > 0
    {
        println!(
            "  {:<20} {}",
            "Collapse:".bright_black(),
            format!(
                "{} → {} lines across {} events",
                format_number(total_original),
                format_number(total_collapsed),
                events
            )
            .bright_green()
        );
    }

    // By Command — top 10 (or all if requested), filter 0% savings
    let raw_filters = store.filter_breakdown(since)?;
    let all_flag = args.iter().any(|a| a == "--all-commands");
    let grouped_filters = group_and_calculate_stats(raw_filters, 0);

    let display_filters: Vec<_> = if all_flag {
        grouped_filters.clone()
    } else {
        grouped_filters
            .iter()
            .filter(|(_, _, pct, _)| *pct > 0.0)
            .take(10)
            .cloned()
            .collect()
    };

    // Per-command with agent info
    let cmd_agent_data = store
        .get_per_command_with_agent(since, 200)
        .unwrap_or_default();
    let mut cmd_agent_counts: HashMap<String, HashMap<String, u64>> = HashMap::new();
    for (cmd, agent_id, calls, _, _) in &cmd_agent_data {
        let key = shorten_command(cmd, 19);
        let entry = cmd_agent_counts.entry(key).or_default();
        *entry.entry(agent_id.clone()).or_insert(0) += *calls;
    }

    if !display_filters.is_empty() {
        println!("\n {}", "By Command:".bold().bright_white());
        println!(
            "   {}  {:<20} {:<12} {:>4} {:>7}  {:>10}  {}",
            "#".bright_black(),
            "CLI".bright_black(),
            "Agent".bright_black(),
            "Count".bright_black(),
            "Savings".bright_black(),
            "Tokens Reduced".bright_black(),
            "Signal Strength".bright_black()
        );
        println!(
            "   {} {}",
            "──".bright_black(),
            "──────────────────── ──────────── ───── ──────── ────────────── ────────────────────"
                .bright_black()
        );

        for (i, (name, cnt, pct, tokens_saved)) in display_filters.iter().enumerate() {
            let bar = format_bar(*pct);
            let bar_colored = if *pct > 80.0 {
                bar.bright_green()
            } else {
                bar.bright_yellow()
            };
            let suffix = if *name == "passthrough" || *name == "unknown" {
                " ← learn?".bright_black().italic()
            } else {
                "".clear()
            };

            let display_name = if name.chars().count() > 19 {
                let mut s: String = name.chars().take(16).collect();
                s.push_str("...");
                s
            } else {
                (*name).clone()
            };

            // Pick the dominant agent for this command key by highest call count.
            let agent_label = cmd_agent_counts
                .get(&display_name)
                .and_then(|agents| agents.iter().max_by_key(|(_, calls)| *calls))
                .map(|(agent_id, _)| agent_display_name(agent_id))
                .unwrap_or("Terminal");

            let tokens_str = if *tokens_saved > 0 {
                format!("-{}", format_exact_tokens(*tokens_saved))
            } else {
                String::new()
            };

            println!(
                "  {:>2}. {:<20} {:<12} {:>4}x  {:>5.1}%  {:>14}  {}{}",
                i + 1,
                display_name.bright_cyan(),
                agent_label.bright_blue(),
                cnt,
                pct,
                tokens_str.bright_magenta(),
                bar_colored,
                suffix
            );
        }

        if !all_flag {
            let filtered_count = grouped_filters
                .iter()
                .filter(|(_, _, pct, _)| *pct > 0.0)
                .count();
            let hidden_zero = grouped_filters.len() - filtered_count;

            if filtered_count > 10 {
                println!(
                    "\n   {}",
                    format!(
                        "Showing top 10 of {} commands with active savings. --all-commands to see all",
                        filtered_count
                    )
                    .bright_black()
                    .italic()
                );
            }

            if hidden_zero > 0 {
                println!(
                     "   {}",
                     format!("({} noise commands with 0% savings hidden. Use --all-commands to see all).", hidden_zero)
                         .bright_black()
                         .italic()
                 );
            }
        }
    }

    // Route distribution
    let routes = store.route_distribution(since)?;
    if !routes.is_empty() {
        let total_routes: u64 = routes.iter().map(|(_, c)| c).sum();
        println!("\n {}", "Route Distribution:".bold().bright_white());
        for (route, cnt) in &routes {
            let pct = if total_routes > 0 {
                *cnt as f64 / total_routes as f64 * 100.0
            } else {
                0.0
            };
            let route_color = match route.to_lowercase().as_str() {
                "keep" => route.bright_green(),
                "rewind" => route.bright_blue(),
                "soft" => route.bright_yellow(),
                "drop" | "passthrough" => route.bright_red(),
                _ => route.bright_black(),
            };

            let label = format!("{}:", route);
            let padding = " ".repeat(15_usize.saturating_sub(label.len()));

            println!(
                "  {}{}{}  ({:>2.0}%)",
                route_color.bold(),
                ":".bright_white().to_string() + &padding,
                cnt,
                pct
            );
        }
    }

    // Agent Distribution
    let agent_data = store.get_agent_breakdown(since).unwrap_or_default();

    // Group by display name
    let mut grouped_agents: HashMap<String, (u64, u64, u64)> = HashMap::new();
    for (id, count, input, output) in &agent_data {
        if id == "unknown" || id == "terminal" || id.is_empty() {
            continue;
        }
        let name = agent_display_name(id).to_string();
        let entry = grouped_agents.entry(name).or_insert((0, 0, 0));
        entry.0 += count;
        entry.1 += input;
        entry.2 += output;
    }

    if !grouped_agents.is_empty() {
        let total_cmds: u64 = agent_data.iter().map(|(_, c, _, _)| c).sum();
        println!("\n {}", "Agent Distribution:".bold().bright_white());
        println!(
            "   {:<16} {:>6} {:>7}  {}",
            "Agent".bright_black(),
            "Count".bright_black(),
            "Share".bright_black(),
            "Savings".bright_black()
        );
        println!(
            "   {} {}",
            "──".bright_black(),
            "────────────── ────── ─────── ────────────────────".bright_black()
        );

        let mut sorted_agents: Vec<_> = grouped_agents.into_iter().collect();
        sorted_agents.sort_by_key(|a| std::cmp::Reverse(a.1.0));

        for (name, (count, input, output)) in sorted_agents {
            let pct = if total_cmds > 0 {
                count as f64 / total_cmds as f64 * 100.0
            } else {
                0.0
            };
            let savings = if input > 0 {
                100.0 * (1.0 - output as f64 / input as f64)
            } else {
                0.0
            };
            let bar = format_bar(savings);
            let bar_colored = if savings > 80.0 {
                bar.bright_green()
            } else if savings > 40.0 {
                bar.bright_yellow()
            } else {
                bar.bright_red()
            };
            println!(
                "   {:<16} {:>5}x  {:>5.1}%  {} {:.1}%",
                name.bright_cyan(),
                count,
                pct,
                bar_colored,
                savings,
            );
        }
    }

    // Session insights — always shown in detail mode
    let hot_files = store.hot_files_global(since)?;
    if !hot_files.is_empty() {
        println!("\n {}", "Session Insights:".bold().bright_white());
        let files_str: Vec<String> = hot_files
            .iter()
            .take(3)
            .map(|(f, c)| format!("{} ({})", f.bright_cyan(), c.to_string().bright_black()))
            .collect();
        println!("  Hot files:  {}", files_str.join(", "));
    }

    print_separator();
    println!();
    Ok(())
}

// ─── JSON Mode: Machine-Readable ────────────────────────

#[derive(serde::Serialize)]
pub struct StatsJson {
    pub version: String,
    pub generated_at: i64,
    pub periods: Vec<StatsPeriod>,
    pub commands: Vec<CommandStat>,
    pub agents: Vec<AgentStat>,
    pub rewind: RewindStat,
    pub avg_latency_ms: f64,
}

#[derive(serde::Serialize)]
pub struct StatsPeriod {
    pub label: String,
    pub commands: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub savings_pct: f64,
    pub usd_saved: f64,
    pub measurement_method: String,
}

#[derive(serde::Serialize)]
pub struct CommandStat {
    pub command: String,
    pub count: u64,
    pub savings_pct: f64,
    pub tokens_saved: u64,
}

#[derive(serde::Serialize)]
pub struct AgentStat {
    pub agent: String,
    pub agent_id: String,
    pub count: u64,
    pub savings_pct: f64,
}

#[derive(serde::Serialize)]
pub struct RewindStat {
    pub archived: u64,
    pub retrieved: u64,
}

fn run_json(store: &Store) -> Result<()> {
    let periods = store.multi_period_stats()?;
    let top_commands = get_top_commands(store, 0, 100);
    let (rewind_stored, rewind_retrieved) = store.rewind_metrics()?;
    let (count, _, _, sum_latency, _, _, _) = store.aggregate_stats(0)?;

    let avg_latency = if count > 0 {
        sum_latency as f64 / count as f64
    } else {
        0.0
    };

    let periods_json: Vec<StatsPeriod> = periods
        .iter()
        .map(
            |(label, count, input, output, raw_tokens, filtered_tokens)| {
                let savings_pct = if *raw_tokens > 0 {
                    (100.0 * (1.0 - *filtered_tokens as f64 / *raw_tokens as f64) * 10.0).round()
                        / 10.0
                } else if *input > 0 {
                    (100.0 * (1.0 - *output as f64 / *input as f64) * 10.0).round() / 10.0
                } else {
                    0.0
                };
                let tokens_saved = raw_tokens.saturating_sub(*filtered_tokens);
                let usd_saved =
                    (tokens_saved as f64 / 1_000_000.0) * crate::guard::config::get_input_cost();
                StatsPeriod {
                    label: label.to_lowercase().replace(' ', "_"),
                    commands: *count,
                    input_tokens: *raw_tokens,
                    output_tokens: *filtered_tokens,
                    savings_pct,
                    usd_saved: (usd_saved * 100.0).round() / 100.0,
                    measurement_method: if *raw_tokens > 0 {
                        "actual".to_string()
                    } else {
                        "estimated".to_string()
                    },
                }
            },
        )
        .collect();

    let commands_json: Vec<CommandStat> = top_commands
        .iter()
        .map(|(cmd, count, pct, tokens_saved)| CommandStat {
            command: cmd.clone(),
            count: *count,
            savings_pct: *pct,
            tokens_saved: *tokens_saved,
        })
        .collect();

    let agent_json: Vec<AgentStat> = store
        .get_agent_breakdown(0)
        .unwrap_or_default()
        .iter()
        .map(|(agent_id, count, input, output)| {
            let savings = if *input > 0 {
                (100.0 * (1.0 - *output as f64 / *input as f64) * 10.0).round() / 10.0
            } else {
                0.0
            };
            AgentStat {
                agent: agent_display_name(agent_id).to_string(),
                agent_id: agent_id.clone(),
                count: *count,
                savings_pct: savings,
            }
        })
        .collect();

    let output = StatsJson {
        version: "1".to_string(),
        generated_at: chrono::Utc::now().timestamp(),
        periods: periods_json,
        commands: commands_json,
        agents: agent_json,
        rewind: RewindStat {
            archived: rewind_stored,
            retrieved: rewind_retrieved,
        },
        avg_latency_ms: (avg_latency * 10.0).round() / 10.0,
    };

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

fn run_project_stats(args: &[String], store: &Store) -> Result<()> {
    let today_flag = args.iter().any(|a| a == "--today");
    let week_flag = args.iter().any(|a| a == "--week");

    let now = chrono::Utc::now().timestamp();
    let (since, period_label) = if today_flag {
        (now - (now % 86400), "Today")
    } else if week_flag {
        (now - 7 * 86400, "Last 7 Days")
    } else {
        (now - 30 * 86400, "Last 30 Days")
    };

    let projects = store.get_project_stats(since)?;
    println!(
        "\n  {} — {} Breakdown",
        "OMNI Project Analytics".bold().bright_white(),
        period_label
    );
    print_separator();

    if projects.is_empty() {
        println!("  No project data recorded yet for this period.");
        return Ok(());
    }

    println!(
        " {:<28} {:>9} {:>10}  Signal Strength",
        "Project Directory", "Count", "Savings"
    );
    println!(" {:─<32} ─────── ───────── ────────────────────", "");

    for (path, count, savings) in projects {
        let display_path = if path.chars().count() > 30 {
            let mut s: String = path.chars().take(12).collect();
            s.push_str("...");
            s.extend(
                path.chars()
                    .rev()
                    .take(15)
                    .collect::<String>()
                    .chars()
                    .rev(),
            );
            s
        } else {
            path
        };

        let bar = format_bar(savings);
        let bar_colored = if savings > 80.0 {
            bar.bright_green()
        } else if savings > 40.0 {
            bar.bright_yellow()
        } else {
            bar.bright_red()
        };

        println!(
            " {:<28} {:>8}x  {:>7.1}%  {}",
            display_path.cyan(),
            count,
            savings,
            bar_colored
        );
    }
    println!();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_format_bytes_handles_all_ranges() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1048576), "1.0 MB");
        assert_eq!(format_bytes(1073741824), "1.0 GB");
    }

    #[test]
    fn test_format_tokens_ranges() {
        assert_eq!(format_tokens(0), "0");
        assert_eq!(format_tokens(380), "100"); // 380 bytes / 3.8 = 100 tokens
        assert_eq!(format_tokens(38_000), "10K"); // 10K tokens
        assert_eq!(format_tokens(3_800_000), "1.0M"); // 1M tokens
    }

    #[test]
    fn test_est_cost_usd_returns_expected_value() {
        let expected_price_per_m = (3.0 + 3.0 + 2.5) / 3.0; // ~2.833

        let cost = est_cost_usd(3_800_000); // 1M tokens
        assert!((cost - expected_price_per_m).abs() < 0.01);

        let cost2 = est_cost_usd(380_000); // 100k tokens
        assert!((cost2 - (expected_price_per_m * 0.1)).abs() < 0.01);

        assert_eq!(est_cost_usd(0), 0.0);
    }

    #[test]
    fn test_stats_default_does_not_crash_when_db_is_empty() {
        let tmp = NamedTempFile::new().unwrap();
        let store = Store::open_path(tmp.path()).unwrap();
        let args: Vec<String> = vec!["stats".into()];
        let result = run(&args, &store);
        assert!(result.is_ok());
    }

    #[test]
    fn test_stats_detail_does_not_crash_when_db_is_empty() {
        let tmp = NamedTempFile::new().unwrap();
        let store = Store::open_path(tmp.path()).unwrap();
        let args: Vec<String> = vec!["stats".into(), "--detail".into()];
        let result = run(&args, &store);
        assert!(result.is_ok());
    }

    #[test]
    fn test_stats_json_does_not_crash_when_db_is_empty() {
        let tmp = NamedTempFile::new().unwrap();
        let store = Store::open_path(tmp.path()).unwrap();
        let args: Vec<String> = vec!["stats".into(), "--json".into()];
        let result = run(&args, &store);
        assert!(result.is_ok());
    }

    #[test]
    fn test_format_bar() {
        assert_eq!(format_bar(100.0), "████████████████████");
        assert_eq!(format_bar(50.0), "██████████");
        assert_eq!(format_bar(0.0), "");
    }

    #[test]
    fn test_format_bar_with_empty() {
        assert_eq!(format_bar_with_empty(100.0), "████████████████████");
        assert_eq!(format_bar_with_empty(50.0), "██████████░░░░░░░░░░");
        assert_eq!(format_bar_with_empty(0.0), "░░░░░░░░░░░░░░░░░░░░");
    }

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(0), "0");
        assert_eq!(format_number(999), "999");
        assert_eq!(format_number(1000), "1,000");
        assert_eq!(format_number(1247000), "1,247,000");
    }

    #[test]
    fn test_stats_json_schema_validation() {
        let json_struct = StatsJson {
            version: "1".to_string(),
            generated_at: 1234567890,
            periods: vec![StatsPeriod {
                label: "All Time".to_string(),
                commands: 10,
                input_tokens: 10000,
                output_tokens: 1000,
                savings_pct: 90.0,
                usd_saved: 1.5,
                measurement_method: "test".to_string(),
            }],
            commands: vec![],
            agents: vec![],
            rewind: RewindStat {
                archived: 100,
                retrieved: 5,
            },
            avg_latency_ms: 15.5,
        };

        let json_str = serde_json::to_string(&json_struct).unwrap();
        assert!(json_str.contains("\"version\":\"1\""));
        assert!(json_str.contains("\"generated_at\":1234567890"));
        assert!(json_str.contains("\"savings_pct\":90.0"));
        assert!(json_str.contains("\"avg_latency_ms\":15.5"));
    }
}
