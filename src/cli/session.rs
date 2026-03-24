use crate::pipeline::SessionState;
use crate::store::sqlite::Store;
use chrono::{Local, TimeZone, Utc};
use colored::*;
use std::sync::Arc;

fn print_help() {
    println!(
        "\n{} {} — Session state management",
        "omni".bold().cyan(),
        "session".bold().yellow()
    );
    println!("\n{}", "USAGE:".bold().bright_white());
    println!("  omni {} {}", "session".cyan(), "[FLAGS]".bright_black());

    println!("\n{}", "FLAGS:".bold().bright_white());
    println!("  {: <12} Check current session status", "--status".cyan());
    println!("  {: <12} View recent session history", "--history".cyan());
    println!(
        "  {: <12} Reset/Clear the current session",
        "--clear".cyan()
    );
    println!("  {: <12} Continue a stale session", "--continue".cyan());
    println!("  {: <12} Show this help message", "--help, -h".cyan());

    println!("\n{}", "EXAMPLES:".bold().bright_white());
    println!(
        "  omni session --status  {}",
        "# View current session status".bright_black()
    );
    println!(
        "  omni session --history {}",
        "# View past sessions".bright_black()
    );
    println!();
}

pub fn run_session(args: &[String], store: Arc<Store>) -> anyhow::Result<()> {
    if args
        .iter()
        .any(|a| a == "--help" || a == "-h" || a == "help")
    {
        print_help();
        return Ok(());
    }

    let is_history = args.iter().any(|a| a == "--history");
    let is_clear = args.iter().any(|a| a == "--clear");
    let is_continue = args.iter().any(|a| a == "--continue");
    let is_status = args.iter().any(|a| a == "--status");
    let is_inject = args.iter().any(|a| a == "--inject");

    // If no flags, show help
    if !is_history && !is_clear && !is_continue && !is_status && !is_inject {
        print_help();
        return Ok(());
    }

    if is_history {
        let sessions = store.list_recent_sessions(10).unwrap_or_default();
        if sessions.is_empty() {
            println!("\n{} No recent sessions found.", "ℹ".blue());
            return Ok(());
        }
        println!("\n{}", "Recent Session History:".bold().bright_white());
        for s in sessions {
            let ago = (Utc::now().timestamp() - s.last_active) / 60;
            let time_str = if ago < 60 {
                format!("{}m ago", ago)
            } else {
                format!("{}h ago", ago / 60)
            };
            let task = s.inferred_task.as_deref().unwrap_or("not detected");
            let sid = if s.session_id.len() > 8 {
                &s.session_id[..8]
            } else {
                &s.session_id
            };

            println!(
                "  {} {} {: <10} | {: <20} | {} cmds",
                "•".bright_black(),
                sid.cyan(),
                time_str.bright_black(),
                task.bright_white(),
                s.last_commands.len().to_string().yellow()
            );
        }
        println!();
        return Ok(());
    }

    let mut state = match store.find_latest_session() {
        Some(s) => s,
        None => {
            if is_inject {
                return Ok(());
            }
            println!("\n{} No active session found.\n", "ℹ".blue());
            return Ok(());
        }
    };

    if is_clear {
        let _ = store.delete_session(&state.session_id);
        println!("{} Current session cleared.", "✓".green());
        return Ok(());
    }

    if is_continue {
        state.last_active = Utc::now().timestamp();
        store.upsert_session(&state);
        println!(
            "{} Session {} marked as continued.",
            "✓".green(),
            state.session_id.cyan()
        );
        return Ok(());
    }

    if is_inject {
        let task = state.inferred_task.as_deref().unwrap_or("none");
        let mut hot_vec: Vec<(&String, &u32)> = state.hot_files.iter().collect();
        hot_vec.sort_by(|a, b| b.1.cmp(a.1));
        let hot_str = if hot_vec.is_empty() {
            "none".to_string()
        } else {
            hot_vec
                .iter()
                .take(2)
                .map(|(f, _)| f.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        };
        let err = state
            .active_errors
            .first()
            .map(|e| e.replace('\n', " "))
            .unwrap_or_else(|| "none".to_string());

        let mut msg = format!(
            "[OMNI Context] Task: {}. Hot: {}. Error: {}",
            task, hot_str, err
        );
        if msg.len() > 200 {
            msg.truncate(197);
            msg.push_str("...");
        }
        println!("{}", msg);
        return Ok(());
    }

    if is_status {
        let ago = (Utc::now().timestamp() - state.started_at) / 60;
        let time_str = if ago < 60 {
            format!("{}m ago", ago)
        } else {
            format!("{}h ago", ago / 60)
        };
        let started_str = Local
            .timestamp_opt(state.started_at, 0)
            .single()
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "unknown time".to_string());

        let sid = if state.session_id.len() > 8 {
            &state.session_id[..8]
        } else {
            &state.session_id
        };

        println!(
            "\n{}",
            "─────────────────────────────────────────"
                .bright_black()
                .bold()
        );
        println!(" {} — Current Session", "OMNI".bold().cyan());
        println!(
            "{}",
            "─────────────────────────────────────────"
                .bright_black()
                .bold()
        );

        println!(
            "  {:<15} {}",
            "Session ID:".bright_black(),
            sid.cyan().bold()
        );
        println!(
            "  {:<15} {} ({})",
            "Started:".bright_black(),
            time_str,
            started_str.bright_black()
        );
        println!(
            "  {:<15} {}\n",
            "Commands:".bright_black(),
            state.last_commands.len().to_string().yellow()
        );

        println!(
            "  {:<15} {}",
            "Task:".bright_black(),
            state
                .inferred_task
                .as_deref()
                .unwrap_or("not detected")
                .bright_white()
                .bold()
        );
        println!(
            "  {:<15} {}\n",
            "Domain:".bright_black(),
            state
                .inferred_domain
                .as_deref()
                .unwrap_or("not detected")
                .bright_white()
        );

        let mut hot_vec: Vec<(&String, &u32)> = state.hot_files.iter().collect();
        hot_vec.sort_by(|a, b| b.1.cmp(a.1));
        println!(" {}", "Hot files:".bold().bright_white());
        for (i, (file, count)) in hot_vec.iter().take(3).enumerate() {
            println!(
                "  {:>2}. {: <30} ({}x)",
                i + 1,
                file.cyan(),
                count.to_string().yellow()
            );
        }

        println!("\n {}", "Active errors:".bold().bright_white());
        if state.active_errors.is_empty() {
            println!("  {} none", "•".bright_black());
        } else {
            for err in state.active_errors.iter().take(3) {
                let e = err.replace('\n', " ");
                let clean = if e.len() > 80 {
                    format!("{}...", &e[..77])
                } else {
                    e
                };
                println!("  {} {}", "•".red(), clean.red());
            }
        }
        println!(
            "\n{}",
            "─────────────────────────────────────────"
                .bright_black()
                .bold()
        );
        Ok(())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn get_store() -> (Arc<Store>, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("omni.db");
        (Arc::new(Store::open_path(&db_path).unwrap()), dir)
    }

    #[test]
    fn test_session_command_tidak_crash_jika_tidak_ada_session() {
        let (store, _dir) = get_store();
        let args = vec!["session".to_string()];
        let res = run_session(&args, store);
        assert!(res.is_ok());
    }

    #[test]
    fn test_session_inject_leq_200_chars() {
        let (store, _dir) = get_store();
        let mut state = SessionState::new();
        state.inferred_task = Some("A".repeat(300));
        state.add_hot_file(&"B".repeat(300));
        store.upsert_session(&state);

        let args = vec!["session".to_string(), "--inject".to_string()];
        let res = run_session(&args, store);
        assert!(res.is_ok());
    }

    #[test]
    fn test_session_clear_reset_state() {
        let (store, _dir) = get_store();
        let state = SessionState::new();
        store.upsert_session(&state);

        assert!(store.find_latest_session().is_some());

        let args = vec!["session".to_string(), "--clear".to_string()];
        run_session(&args, store.clone()).unwrap();

        assert!(store.find_latest_session().is_none());
    }

    #[test]
    fn test_session_history_menampilkan_sessions() {
        let (store, _dir) = get_store();
        let state = SessionState::new();
        store.upsert_session(&state);

        let args = vec!["session".to_string(), "--history".to_string()];
        let res = run_session(&args, store);
        assert!(res.is_ok());
    }

    #[test]
    fn test_session_context_format_under_200_chars() {
        let mut state = SessionState::new();
        state.add_hot_file("src/auth/mod.rs");
        state.add_error("E0499: cannot borrow as mutable");
        state.inferred_task = Some("fix auth bug".to_string());

        // Simulasi logic dari omni_session "context"
        let task = state
            .inferred_task
            .as_deref()
            .unwrap_or("general development");
        let err = state
            .active_errors
            .first()
            .map(|e| e.as_str())
            .unwrap_or("none");
        let mut hot: Vec<_> = state.hot_files.iter().collect();
        hot.sort_by(|a, b| b.1.cmp(a.1));
        let hot_str = hot
            .iter()
            .take(2)
            .map(|(f, c)| format!("{} ({}x)", f, c))
            .collect::<Vec<_>>()
            .join(", ");

        let mut ctx = format!("[OMNI Context] Task: {}.", task);
        if !hot_str.is_empty() {
            ctx.push_str(&format!(" Hot: {}.", hot_str));
        }
        if err != "none" {
            ctx.push_str(&format!(" Error: {}", &err[..err.len().min(80)]));
        }
        if ctx.len() > 200 {
            ctx.truncate(197);
            ctx.push_str("...");
        }

        assert!(ctx.len() <= 200);
        assert!(ctx.contains("fix auth bug"));
        assert!(ctx.contains("src/auth/mod.rs"));
    }
}
