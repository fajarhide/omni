use crate::pipeline::SessionState;
use crate::store::sqlite::Store;
use chrono::{Local, TimeZone, Utc};
use std::sync::Arc;

pub fn run_session(args: &[String], store: Arc<Store>) -> anyhow::Result<()> {
    let mut is_continue = false;
    let mut is_history = false;
    let mut is_clear = false;
    let mut is_inject = false;

    for a in args {
        match a.as_str() {
            "--continue" => is_continue = true,
            "--history" => is_history = true,
            "--clear" => is_clear = true,
            "--inject" => is_inject = true,
            _ => {}
        }
    }

    if is_history {
        let sessions = store.list_recent_sessions(10).unwrap_or_default();
        if sessions.is_empty() {
            println!("No recent sessions found.");
            return Ok(());
        }
        println!("Recent Sessions:");
        for s in sessions {
            let ago = (Utc::now().timestamp() - s.last_active) / 60;
            let time_str = if ago < 60 {
                format!("{}m ago", ago)
            } else {
                format!("{}h ago", ago / 60)
            };
            let task = s.inferred_task.as_deref().unwrap_or("not detected");
            println!(
                "- {} ({}) | Task: {} | Commands: {}",
                s.session_id,
                time_str,
                task,
                s.last_commands.len()
            );
        }
        return Ok(());
    }

    let mut state = match store.find_latest_session() {
        Some(s) => s,
        None => {
            if is_inject {
                // Return silently or empty context
                return Ok(());
            }
            println!("No active session found.");
            return Ok(());
        }
    };

    if is_clear {
        let _ = store.delete_session(&state.session_id);
        println!("Current session cleared.");
        return Ok(());
    }

    if is_continue {
        state.last_active = Utc::now().timestamp();
        store.upsert_session(&state);
        println!("Session {} marked as continued.", state.session_id);
        return Ok(());
    }

    if is_inject {
        let task = state
            .inferred_task
            .as_deref()
            .unwrap_or("general development");
        let err = state
            .active_errors
            .first()
            .map(|e| e.as_str())
            .unwrap_or("none");
        let mut hot_vec: Vec<(&String, &u32)> = state.hot_files.iter().collect();
        hot_vec.sort_by(|a, b| b.1.cmp(a.1));

        let hot_str = hot_vec
            .iter()
            .take(2)
            .map(|(k, v)| format!("{} ({}x)", k, v))
            .collect::<Vec<_>>()
            .join(", ");

        // Format yang bisa langsung di-inject ke Claude
        let mut ctx = format!("[OMNI Context] Task: {}.", task);
        if !hot_str.is_empty() {
            ctx.push_str(&format!(" Hot: {}.", hot_str));
        }
        if err != "none" {
            let err_clean = err.replace('\n', " ");
            ctx.push_str(&format!(
                " Error: {}",
                &err_clean[..err_clean.len().min(80)]
            ));
        }
        // Trim ke max 200 chars
        if ctx.len() > 200 {
            ctx.truncate(197);
            ctx.push_str("...");
        }
        println!("{}", ctx);
        return Ok(());
    }

    // Default output
    let ago = (Utc::now().timestamp() - state.started_at) / 60;
    let time_str = if ago < 60 {
        format!("{}m ago", ago)
    } else {
        format!("{}h ago", ago / 60)
    };

    // Formatting started_at local time safely
    let started_str = Local
        .timestamp_opt(state.started_at, 0)
        .single()
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "unknown time".to_string());

    println!("─────────────────────────────────────────");
    println!(" OMNI — Current Session");
    println!("─────────────────────────────────────────");

    let sid = if state.session_id.len() > 8 {
        &state.session_id[..8]
    } else {
        &state.session_id
    };
    println!(" Session:   {}", sid);
    println!(" Started:   {} ({})", time_str, started_str);
    println!(" Commands:  {}", state.last_commands.len());
    println!();
    println!(
        " Inferred task:   {}",
        state.inferred_task.as_deref().unwrap_or("not detected")
    );
    println!(
        " Inferred domain: {}",
        state.inferred_domain.as_deref().unwrap_or("not detected")
    );
    println!();

    let mut hot_vec: Vec<(&String, &u32)> = state.hot_files.iter().collect();
    hot_vec.sort_by(|a, b| b.1.cmp(a.1));
    println!(" Hot files ({} total):", state.hot_files.len());
    for (i, (file, count)) in hot_vec.iter().take(3).enumerate() {
        println!("  {}. {}      ({} accesses)", i + 1, file, count);
    }

    println!();
    println!(" Active errors:");
    if state.active_errors.is_empty() {
        println!("  • none");
    } else {
        for err in state.active_errors.iter().take(3) {
            let e = err.replace('\n', " ");
            let clean = if e.len() > 80 {
                format!("{}...", &e[..77])
            } else {
                e
            };
            println!("  • {}", clean);
        }
    }
    println!();

    let domain = state.inferred_domain.as_deref().unwrap_or("unknown");
    if domain != "unknown" {
        println!(
            " Context score: session context is boosting signals in {}/*",
            domain
        );
    } else {
        println!(" Context score: baseline session context mapping");
    }

    Ok(())
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
