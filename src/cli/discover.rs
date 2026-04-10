use crate::store::sqlite::Store;

pub fn run(args: &[String], store: &Store) -> anyhow::Result<()> {
    let since_days: i64 = args
        .iter()
        .position(|a| a == "--days")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(7);

    let show_all = args.iter().any(|a| a == "--all");
    let json_mode = args.iter().any(|a| a == "--json");
    let since = chrono::Utc::now().timestamp() - (since_days * 86400);

    let records = store.coverage_analysis(since)?;

    // Kategorisasi:
    // Excellent: avg_reduction > 70% → filter bekerja dengan baik
    // Poor: avg_reduction 10-40% dan content_type != Unknown → filter suboptimal
    // Unknown: content_type == "Unknown" → belum ada filter
    // Passthrough: avg_reduction == 0% → tidak tersentuh filter

    let excellent: Vec<_> = records
        .iter()
        .filter(|r| r.avg_reduction_pct > 70.0)
        .collect();

    let poor: Vec<_> = records
        .iter()
        .filter(|r| {
            r.avg_reduction_pct > 0.0
                && r.avg_reduction_pct <= 40.0
                && r.content_type != "Unknown"
                && r.call_count >= 3
        })
        .collect();

    let unknown: Vec<_> = records
        .iter()
        .filter(|r| r.content_type == "Unknown" && r.call_count >= 2)
        .collect();

    if json_mode {
        // Output JSON untuk scripting
        let json = serde_json::json!({
            "period_days": since_days,
            "excellent": excellent.iter().map(|r| serde_json::json!({
                "command": r.command, "calls": r.call_count,
                "reduction_pct": r.avg_reduction_pct, "content_type": r.content_type
            })).collect::<Vec<_>>(),
            "poor": poor.iter().map(|r| serde_json::json!({
                "command": r.command, "calls": r.call_count,
                "reduction_pct": r.avg_reduction_pct,
                "suggestion": format!("omni learn --from-history {}", r.command.split_whitespace().next().unwrap_or(""))
            })).collect::<Vec<_>>(),
            "unknown": unknown.iter().map(|r| serde_json::json!({
                "command": r.command, "calls": r.call_count,
                "suggestion": format!("omni learn --from-history {}", r.command.split_whitespace().next().unwrap_or(""))
            })).collect::<Vec<_>>()
        });
        println!("{}", serde_json::to_string_pretty(&json)?);
        return Ok(());
    }

    // Human-readable output
    println!("─────────────────────────────────────────────────────");
    println!(" OMNI Coverage Analysis — last {} days", since_days);
    println!("─────────────────────────────────────────────────────");
    println!();

    // Excellent section
    if !excellent.is_empty() {
        println!(" ✓  Excellent Coverage (>70% reduction):");
        for r in excellent.iter().take(if show_all { 100 } else { 8 }) {
            let bar = bar_str(r.avg_reduction_pct, 20);
            println!(
                "    {:30} {} {:.1}%  ({} calls)",
                truncate(&r.command, 30),
                bar,
                r.avg_reduction_pct,
                r.call_count
            );
        }
        if !show_all && excellent.len() > 8 {
            println!("    ... +{} more (--all to show)", excellent.len() - 8);
        }
        println!();
    }

    // Poor section — actionable!
    if !poor.is_empty() {
        println!(" ⚠  Poor Distillation (<40% reduction, 3+ calls):");
        println!("    These tools are being filtered but with low effectiveness.");
        println!();
        for r in &poor {
            let cmd_base = r.command.split_whitespace().next().unwrap_or("");
            println!(
                "    {:30} {:.1}% reduction  ({} calls)",
                truncate(&r.command, 30),
                r.avg_reduction_pct,
                r.call_count
            );
            println!("    → Suggestion: omni learn --from-history {}", cmd_base);
        }
        println!();
    }

    // Unknown section — highest priority
    if !unknown.is_empty() {
        println!(" ✗  No Filter Coverage (Unknown type, 2+ calls):");
        println!("    0% token reduction — OMNI sees these as opaque output.");
        println!();
        for r in &unknown {
            let total_wasted_tokens = (r.total_input_bytes / 4) as f64 / 1000.0;
            let cmd_base = r.command.split_whitespace().next().unwrap_or("");
            println!(
                "    {:30}  ({} calls, ~{:.0}K tokens unfiltered)",
                truncate(&r.command, 30),
                r.call_count,
                total_wasted_tokens
            );
            println!("    → Fix: omni learn --from-history {}", cmd_base);
        }
        println!();
    }

    if excellent.is_empty() && poor.is_empty() && unknown.is_empty() {
        println!(" No data yet. Run some commands with Claude Code first.");
        println!(" OMNI tracks automatically — check back after a few sessions.");
    }

    println!("─────────────────────────────────────────────────────");
    println!(" Tips:");
    println!("  omni discover --days 30    Show last 30 days");
    println!("  omni discover --json       Machine-readable output");
    println!("  omni discover --all        Show all commands (not just top)");
    println!("─────────────────────────────────────────────────────");

    Ok(())
}

fn bar_str(pct: f64, width: usize) -> String {
    let filled = ((pct / 100.0) * width as f64).round() as usize;
    let filled = filled.min(width);
    format!("{}{}", "█".repeat(filled), "░".repeat(width - filled))
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        format!("{:<max$}", s)
    } else {
        format!("{}...", &s.chars().take(max - 3).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::{ContentType, DistillResult, Route};
    use crate::store::sqlite::Store;
    use tempfile::tempdir;

    fn make_store() -> (Store, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("omni.db");
        (Store::open_path(&db_path).unwrap(), dir)
    }

    fn insert_distillation(
        store: &Store,
        command: &str,
        content_type: ContentType,
        input_bytes: u64,
        output_bytes: u64,
    ) {
        let res = DistillResult {
            output: "".to_string(),
            route: Route::Keep,
            filter_name: format!("{:?}", content_type),
            content_type,
            score: 0.5,
            context_score: 0.0,
            input_bytes: input_bytes as usize,
            output_bytes: output_bytes as usize,
            latency_ms: 5,
            rewind_hash: None,
            segments_kept: 1,
            segments_dropped: 0,
            collapse_savings: None,
        };
        store.record_distillation("test_session", &res, command);
    }

    #[test]
    fn test_discover_categorizes_correctly() {
        let (store, _dir) = make_store();

        // Excellent: 80% reduction (input=1000, output=200)
        for _ in 0..5 {
            insert_distillation(&store, "git status", ContentType::GitStatus, 1000, 200);
        }

        // Poor: 20% reduction (input=1000, output=800), non-Unknown, 3+ calls
        for _ in 0..4 {
            insert_distillation(&store, "npm test", ContentType::TestOutput, 1000, 800);
        }

        // Unknown: content_type Unknown, 2+ calls
        for _ in 0..3 {
            insert_distillation(&store, "custom-tool", ContentType::Unknown, 500, 500);
        }

        let since = chrono::Utc::now().timestamp() - 86400;
        let records = store.coverage_analysis(since).unwrap();

        let excellent: Vec<_> = records
            .iter()
            .filter(|r| r.avg_reduction_pct > 70.0)
            .collect();
        let poor: Vec<_> = records
            .iter()
            .filter(|r| {
                r.avg_reduction_pct > 0.0
                    && r.avg_reduction_pct <= 40.0
                    && r.content_type != "Unknown"
                    && r.call_count >= 3
            })
            .collect();
        let unknown: Vec<_> = records
            .iter()
            .filter(|r| r.content_type == "Unknown" && r.call_count >= 2)
            .collect();

        assert_eq!(excellent.len(), 1, "git status should be excellent");
        assert_eq!(excellent[0].command, "git status");
        assert!(excellent[0].avg_reduction_pct > 70.0);

        assert_eq!(poor.len(), 1, "npm test should be poor");
        assert_eq!(poor[0].command, "npm test");
        assert_eq!(poor[0].call_count, 4);

        assert_eq!(unknown.len(), 1, "custom-tool should be unknown");
        assert_eq!(unknown[0].command, "custom-tool");
    }

    #[test]
    fn test_bar_str_output() {
        let bar = bar_str(50.0, 10);
        assert_eq!(bar, "█████░░░░░");

        let bar_full = bar_str(100.0, 10);
        assert_eq!(bar_full, "██████████");

        let bar_empty = bar_str(0.0, 10);
        assert_eq!(bar_empty, "░░░░░░░░░░");
    }

    #[test]
    fn test_truncate_short_string() {
        let result = truncate("hello", 10);
        assert_eq!(result, "hello     ");
    }

    #[test]
    fn test_truncate_long_string() {
        let result = truncate("this is a very long command string", 15);
        assert_eq!(result, "this is a ve...");
    }

    #[test]
    fn test_discover_json_mode_runs() {
        let (store, _dir) = make_store();
        insert_distillation(&store, "git log", ContentType::GitStatus, 2000, 400);
        insert_distillation(&store, "git log", ContentType::GitStatus, 2000, 400);

        let args = vec![
            "omni".to_string(),
            "discover".to_string(),
            "--json".to_string(),
        ];
        let result = run(&args, &store);
        assert!(result.is_ok());
    }
}
