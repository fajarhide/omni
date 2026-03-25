use crate::store::sqlite::Store;
use anyhow::Result;
use colored::*;

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

pub fn format_bar(pct: f64) -> String {
    let width = 20;
    let filled = ((pct / 100.0) * width as f64).round() as usize;
    let filled = filled.min(width);
    "█".repeat(filled)
}

pub fn est_cost_usd(bytes_saved: u64) -> f64 {
    // ~4 chars per token, $3 per 1M tokens
    let tokens = bytes_saved as f64 / 4.0;
    (tokens / 1_000_000.0) * 3.0
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

fn print_help() {
    println!(
        "\n{} {} — Token savings analytics",
        "omni".bold().cyan(),
        "stats".bold().yellow()
    );
    println!("\n{}", "USAGE:".bold().bright_white());
    println!("  omni {} {}", "stats".cyan(), "[FLAGS]".bright_black());

    println!("\n{}", "FLAGS:".bold().bright_white());
    println!("  {: <12} View today's stats", "--today".cyan());
    println!("  {: <12} View last 7 days", "--week".cyan());
    println!("  {: <12} View last 30 days (default)", "--month".cyan());
    println!(
        "  {: <12} Show detailed session insights",
        "--session".cyan()
    );
    println!(
        "  {: <12} Show unclassified passthrough candidates",
        "--passthrough".cyan()
    );
    println!("  {: <12} Show this help message", "--help, -h".cyan());

    println!("\n{}", "EXAMPLES:".bold().bright_white());
    println!(
        "  omni stats --today    {} View your savings today",
        "#".bright_black()
    );
    println!(
        "  omni stats --session  {} View hot files and errors",
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

    let show_passthrough = args.iter().any(|a| a == "--passthrough");
    let show_session = args.iter().any(|a| a == "--session");

    let (period_label, since) = if args.iter().any(|a| a == "--today") {
        let now = chrono::Utc::now().timestamp();
        let start = now - (now % 86400); // midnight UTC
        ("today", start)
    } else if args.iter().any(|a| a == "--week") {
        ("last 7 days", chrono::Utc::now().timestamp() - 7 * 86400)
    } else {
        // default --month or no flag
        ("last 30 days", chrono::Utc::now().timestamp() - 30 * 86400)
    };

    // Aggregate
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

    // Rewind
    let (rewind_stored, rewind_retrieved) = store.rewind_metrics()?;

    println!(
        "\n{}",
        "─────────────────────────────────────────────────"
            .bright_black()
            .bold()
    );
    println!(
        " {}",
        format!("OMNI Signal Report — {}", period_label.bold()).bright_white()
    );
    println!(
        "{}",
        "─────────────────────────────────────────────────"
            .bright_black()
            .bold()
    );

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

    // Filter breakdown
    let filters = store.filter_breakdown(since)?;
    if !filters.is_empty() {
        println!("\n {}", "By Command:".bold().bright_white());
        println!(
            "   #  {:<24} {:>7} {:>9}  Signal Strength",
            "CLI", "Count", "Savings"
        );
        println!("   ── {:─<24} ─────── ───────── ────────────────────", "");

        for (i, (name, cnt, pct)) in filters.iter().enumerate() {
            let bar = format_bar(*pct);
            let bar_colored = if *pct > 80.0 {
                bar.bright_green()
            } else {
                bar.bright_yellow()
            };
            let suffix = if name == "passthrough" || name == "unknown" {
                " ← learn?".bright_black().italic()
            } else {
                "".clear()
            };

            let display_name = if name.chars().count() > 21 {
                let mut s: String = name.chars().take(18).collect();
                s.push_str("...");
                s
            } else {
                name.clone()
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
                "distill" | "keep" => route.bright_green(),
                "rewind" => route.bright_blue(),
                "soft" => route.bright_yellow(),
                "drop" => route.bright_red(),
                _ => route.bright_black(),
            };

            // Fix alignment: print name with fixed width before formatting
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

    // Session insights (--session or default)
    if show_session || !show_passthrough {
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
    }

    println!(
        "{}",
        "─────────────────────────────────────────────────"
            .bright_black()
            .bold()
    );
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
    fn test_est_cost_usd_kalkulasi_benar() {
        // 4M bytes = 1M tokens, cost = $3
        let cost = est_cost_usd(4_000_000);
        assert!((cost - 3.0).abs() < 0.01);

        // 400K bytes = 100K tokens = $0.30
        let cost2 = est_cost_usd(400_000);
        assert!((cost2 - 0.30).abs() < 0.01);

        assert_eq!(est_cost_usd(0), 0.0);
    }

    #[test]
    fn test_stats_tidak_crash_jika_db_kosong() {
        let tmp = NamedTempFile::new().unwrap();
        let store = Store::open_path(tmp.path()).unwrap();
        let args: Vec<String> = vec!["stats".into()];
        let result = run(&args, &store);
        assert!(result.is_ok());
    }

    #[test]
    fn test_stats_passthrough_menampilkan_candidates() {
        let tmp = NamedTempFile::new().unwrap();
        let store = Store::open_path(tmp.path()).unwrap();
        let args: Vec<String> = vec!["stats".into(), "--passthrough".into()];
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
    fn test_format_number() {
        assert_eq!(format_number(0), "0");
        assert_eq!(format_number(999), "999");
        assert_eq!(format_number(1000), "1,000");
        assert_eq!(format_number(1247000), "1,247,000");
    }
}
