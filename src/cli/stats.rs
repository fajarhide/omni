use crate::store::sqlite::Store;
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

pub fn format_tokens(bytes: u64) -> String {
    let tokens = bytes / 4;
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
    let price = crate::guard::config::get_input_cost();
    // ~4 chars per token
    let tokens = bytes_saved as f64 / 4.0;
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
    items: Vec<(String, u64, u64, u64)>,
    limit: usize,
) -> Vec<(String, u64, f64)> {
    let mut grouped: HashMap<String, (u64, u64, u64)> = HashMap::new();

    for (cmd, calls, input, output) in items {
        // Group by the shortened version so things like "npm install x" and "npm install y" combine
        let key = shorten_command(&cmd, 18);
        let entry = grouped.entry(key).or_insert((0, 0, 0));
        entry.0 += calls;
        entry.1 += input;
        entry.2 += output;
    }

    let mut result: Vec<(String, u64, f64)> = grouped
        .into_iter()
        .map(|(cmd, (calls, input, output))| {
            let pct = if input > 0 {
                100.0 * (1.0 - output as f64 / input as f64)
            } else {
                0.0
            };
            (cmd, calls, pct)
        })
        .collect();

    result.sort_by(|a, b| b.1.cmp(&a.1));
    if limit > 0 {
        result.truncate(limit);
    }
    result
}

fn get_top_commands(store: &Store, since: i64, limit: usize) -> Vec<(String, u64, f64)> {
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
        "  {: <12} Full technical breakdown (commands, routes, session)",
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
    let filter_flag = args
        .iter()
        .any(|a| a == "--today" || a == "--week" || a == "--month" || a == "--all-commands");

    let mode = if detail_flag {
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
        "project" => run_project_stats(args, store),
        "detail" => run_detail(args, store),
        "json" => run_json(store),
        _ => run_default(store),
    }
}

// ─── Default Mode: Gain-Focused Multi-Period ────────────

fn run_default(store: &Store) -> Result<()> {
    let periods = store.multi_period_stats()?;
    let (rewind_stored, rewind_retrieved) = store.rewind_metrics()?;

    let has_data = periods.iter().any(|(_, count, _, _)| *count > 0);

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
    for (label, count, input, output) in &periods {
        if *count == 0 && label != "All Time" {
            continue;
        }

        let input_tokens = format_tokens(*input);
        let output_tokens = format_tokens(*output);
        let reduction_pct = if *input > 0 {
            100.0 * (1.0 - *output as f64 / *input as f64)
        } else {
            0.0
        };
        let bytes_saved = input.saturating_sub(*output);
        let cost = est_cost_usd(bytes_saved);

        let pct_colored = if reduction_pct > 70.0 {
            format!("{:.1}% saved", reduction_pct).bright_green()
        } else if reduction_pct > 40.0 {
            format!("{:.1}% saved", reduction_pct).bright_yellow()
        } else {
            format!("{:.1}% saved", reduction_pct).bright_red()
        };

        println!(
            "  {:<12} {:>3} commands │ {:>4} → {:<4} tokens │  {} │ ~${:.2}",
            format!("{}:", label).bright_white().bold(),
            format_number(*count).cyan(),
            input_tokens.red(),
            output_tokens.green(),
            pct_colored,
            cost,
        );
    }

    let top_commands = get_top_commands(store, 0, 8);

    if !top_commands.is_empty() {
        println!("\n  {}", "Top Commands:".bold().bright_white());
        for (cmd, count, pct) in &top_commands {
            let short_cmd = shorten_command(cmd, 18);
            let bar = format_bar_with_empty(*pct);
            let bar_colored = if *pct > 80.0 {
                bar.bright_green()
            } else if *pct > 40.0 {
                bar.bright_yellow()
            } else {
                bar.bright_red()
            };

            println!(
                "    {:<18} {}  {:>5.1}%  ({:>2}x)",
                short_cmd.bright_cyan(),
                bar_colored,
                pct,
                count
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
        "  💡 {} for full breakdown",
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

    let (count, input_total, output_total, sum_latency, _max_latency) =
        store.aggregate_stats(since)?;
    let reduction_pct = if input_total > 0 {
        100.0 * (1.0 - output_total as f64 / input_total as f64)
    } else {
        0.0
    };
    let avg_latency = if count > 0 {
        sum_latency as f64 / count as f64
    } else {
        0.0
    };
    let bytes_saved = input_total.saturating_sub(output_total);
    let cost_saved = est_cost_usd(bytes_saved);
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
        "Estimated Savings:".bright_black(),
        format!("${:.3} USD", cost_saved).bold().bright_cyan()
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
            .filter(|(_, _, pct)| *pct > 0.0)
            .take(10)
            .cloned()
            .collect()
    };

    if !display_filters.is_empty() {
        println!("\n {}", "By Command:".bold().bright_white());
        println!(
            "   #  {:<24} {:>7} {:>9}  Signal Strength",
            "CLI", "Count", "Savings"
        );
        println!("   ── {:─<24} ─────── ───────── ────────────────────", "");

        for (i, (name, cnt, pct)) in display_filters.iter().enumerate() {
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

            let display_name = if name.chars().count() > 21 {
                let mut s: String = name.chars().take(18).collect();
                s.push_str("...");
                s
            } else {
                (*name).clone()
            };

            println!(
                "  {:>2}. {:<24} {:>6}x  {:>7.1}%  {}{}",
                i + 1,
                display_name.bright_cyan(),
                cnt,
                pct,
                bar_colored,
                suffix
            );
        }

        if !all_flag {
            let filtered_count = grouped_filters
                .iter()
                .filter(|(_, _, pct)| *pct > 0.0)
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
                "  {}{}{:>15}  ({:>3.0}%)",
                route_color.bold(),
                ":".bright_white().to_string() + &padding,
                cnt,
                pct
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

fn run_json(store: &Store) -> Result<()> {
    let periods = store.multi_period_stats()?;
    let top_commands = get_top_commands(store, 0, 100);
    let (rewind_stored, rewind_retrieved) = store.rewind_metrics()?;
    let (count, _, _, sum_latency, _) = store.aggregate_stats(0)?;

    let avg_latency = if count > 0 {
        sum_latency as f64 / count as f64
    } else {
        0.0
    };

    let periods_json: Vec<serde_json::Value> = periods
        .iter()
        .map(|(label, count, input, output)| {
            let input_tokens = *input / 4;
            let output_tokens = *output / 4;
            let savings_pct = if *input > 0 {
                (100.0 * (1.0 - *output as f64 / *input as f64) * 10.0).round() / 10.0
            } else {
                0.0
            };
            let bytes_saved = input.saturating_sub(*output);
            let usd_saved = est_cost_usd(bytes_saved);
            serde_json::json!({
                "label": label.to_lowercase().replace(' ', "_"),
                "commands": count,
                "input_tokens": input_tokens,
                "output_tokens": output_tokens,
                "savings_pct": savings_pct,
                "usd_saved": (usd_saved * 100.0).round() / 100.0,
            })
        })
        .collect();

    let commands_json: Vec<serde_json::Value> = top_commands
        .iter()
        .map(|(cmd, count, pct)| {
            serde_json::json!({
                "command": cmd,
                "count": count,
                "savings_pct": pct,
            })
        })
        .collect();

    let output = serde_json::json!({
        "periods": periods_json,
        "commands": commands_json,
        "rewind": {
            "archived": rewind_stored,
            "retrieved": rewind_retrieved,
        },
        "avg_latency_ms": (avg_latency * 10.0).round() / 10.0,
    });

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
        "  {:<30} {:>7} {:>9}  Signal Strength",
        "Project Directory", "Count", "Savings"
    );
    println!("  ── {:─<30} ─────── ───────── ────────────────────", "");

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
            "  {:<30} {:>6}x  {:>7.1}%  {}",
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
    fn test_format_bytes_semua_ranges() {
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
        assert_eq!(format_tokens(400), "100"); // 400 bytes / 4 = 100 tokens
        assert_eq!(format_tokens(40_000), "10K"); // 10K tokens
        assert_eq!(format_tokens(4_000_000), "1.0M"); // 1M tokens
    }

    #[test]
    fn test_est_cost_usd_kalkulasi_benar() {
        let cost = est_cost_usd(4_000_000);
        assert!((cost - 3.0).abs() < 0.01);

        let cost2 = est_cost_usd(400_000);
        assert!((cost2 - 0.30).abs() < 0.01);

        assert_eq!(est_cost_usd(0), 0.0);
    }

    #[test]
    fn test_stats_default_not_crash_jika_db_kosong() {
        let tmp = NamedTempFile::new().unwrap();
        let store = Store::open_path(tmp.path()).unwrap();
        let args: Vec<String> = vec!["stats".into()];
        let result = run(&args, &store);
        assert!(result.is_ok());
    }

    #[test]
    fn test_stats_detail_not_crash_jika_db_kosong() {
        let tmp = NamedTempFile::new().unwrap();
        let store = Store::open_path(tmp.path()).unwrap();
        let args: Vec<String> = vec!["stats".into(), "--detail".into()];
        let result = run(&args, &store);
        assert!(result.is_ok());
    }

    #[test]
    fn test_stats_json_not_crash_jika_db_kosong() {
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
}
