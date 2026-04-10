use crate::cli::init::get_settings_path;
use crate::store::sqlite::Store;
use colored::*;
use std::fs;
use std::path::PathBuf;

fn print_help() {
    println!(
        "\n{} {} — Installation diagnostics",
        "omni".bold().cyan(),
        "doctor".bold().yellow()
    );
    println!("\n{}", "USAGE:".bold().bright_white());
    println!("  omni doctor {}", "[--fix]".cyan());

    println!("\n{}", "DESCRIPTION:".bold().bright_white());
    println!("  Checks the health of your OMNI installation, including:");
    println!("  • Binary version and accessibility");
    println!("  • Configuration directory and database");
    println!("  • Claude Code hook installation");
    println!("  • MCP server registration");
    println!("  • Filter trust and loading status");
    println!("\n{}", "FLAGS:".bold().bright_white());
    println!(
        "  {: <12} Automatically fix configuration and integration issues",
        "--fix".cyan()
    );
    println!();

    if let Some(latest) = crate::guard::update::check() {
        crate::guard::update::print_notification(&latest);
    }
}

fn format_time_ago(ts: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    if ts >= now {
        return "just now".to_string();
    }
    let diff = now - ts;
    if diff < 60 {
        format!("{} seconds ago", diff)
    } else if diff < 3600 {
        format!("{} minutes ago", diff / 60)
    } else if diff < 86400 {
        format!("{} hours ago", diff / 3600)
    } else {
        format!("{} days ago", diff / 86400)
    }
}

pub fn run(args: &[String]) -> anyhow::Result<()> {
    if args
        .iter()
        .any(|a| a == "--help" || a == "-h" || a == "help")
    {
        print_help();
        return Ok(());
    }

    let fix_mode = args.iter().any(|a| a == "--fix");

    let mut all_ok = true;
    let mut warnings = Vec::new();
    println!(
        "\n{}",
        "─────────────────────────────────────────"
            .bright_black()
            .bold()
    );
    println!(
        " {} — Installation Diagnostics",
        "OMNI Doctor".bold().cyan()
    );
    println!(
        "{}",
        "─────────────────────────────────────────"
            .bright_black()
            .bold()
    );

    // 1. Binary Version
    let status = crate::guard::update::get_status();
    let version_info = match status {
        crate::guard::update::Status::Latest => {
            format!("omni v{} {}", env!("CARGO_PKG_VERSION"), "[LATEST]".green())
        }
        crate::guard::update::Status::UpdateAvailable(v) => format!(
            "omni v{} {} (Latest: {})",
            env!("CARGO_PKG_VERSION"),
            "[UPDATE]".yellow().bold(),
            v.green()
        ),
        crate::guard::update::Status::Ahead => format!(
            "omni v{} {}",
            env!("CARGO_PKG_VERSION"),
            "[AHEAD/RC]".blue().bold()
        ),
    };

    println!("  {:<15} {}", "Binary:".bright_black(), version_info);

    // 2. Config Dir (with actual write test for sandbox detection)
    let conf_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".omni");
    if conf_dir.exists() {
        // Actual write test catches sandbox restrictions
        let test_file = conf_dir.join(".write_test");
        match fs::write(&test_file, "ok") {
            Ok(_) => {
                let _ = fs::remove_file(&test_file);
                println!(
                    "  {:<15} ~/.omni/ {}",
                    "Config dir:".bright_black(),
                    "[OK]".green().bold()
                );
            }
            Err(_) => {
                println!(
                    "  {:<15} ~/.omni/ {}",
                    "Config dir:".bright_black(),
                    "[ERROR]".red().bold()
                );
                warnings.push(
                    "Cannot write to ~/.omni/. If using Claude Code, add ~/.omni to sandbox.filesystem.allowWrite in ~/.claude/settings.json",
                );
                all_ok = false;
            }
        }
    } else {
        if fix_mode && fs::create_dir_all(&conf_dir).is_ok() {
            println!(
                "  {:<15} ~/.omni/ {}",
                "Config dir:".bright_black(),
                "[FIXED]".green().bold()
            );
        } else {
            println!(
                "  {:<15} ~/.omni/ {}",
                "Config dir:".bright_black(),
                "[ERROR]".red().bold()
            );
            warnings.push("Config directory ~/.omni/ is missing or not writable. Run `omni init`.");
            all_ok = false;
        }
    }

    // 3. Database
    match Store::open() {
        Ok(store) => {
            let (sessions, rewinds) = store.stats().unwrap_or_default();
            println!(
                "  {:<15} ~/.omni/omni.db ({} records) {}",
                "Database:".bright_black(),
                sessions.to_string().yellow(),
                "[OK]".green().bold()
            );

            // DB write test (catches sandbox restrictions on the database itself)
            if store.test_write() {
                println!(
                    "  {:<15} writable {}",
                    "DB Write:".bright_black(),
                    "[OK]".green().bold()
                );
            } else {
                println!(
                    "  {:<15} read-only {}",
                    "DB Write:".bright_black(),
                    "[ERROR]".red().bold()
                );
                warnings.push(
                    "Database is read-only. Claude Code sandbox may be blocking writes to ~/.omni/omni.db. Add ~/.omni to sandbox.filesystem.allowWrite in ~/.claude/settings.json",
                );
                all_ok = false;
            }

            if store.check_fts5() {
                println!(
                    "  {:<15} available {}",
                    "FTS5:".bright_black(),
                    "[OK]".green().bold()
                );
            } else {
                println!(
                    "  {:<15} missing {}",
                    "FTS5:".bright_black(),
                    "[WARNING]".yellow().bold()
                );
                warnings.push(
                    "SQLite FTS5 extension is not enabled. Search capabilities will be degraded.",
                );
                all_ok = false;
            }

            // 9. RewindStore
            println!(
                "  {:<15} {} items tracked",
                "RewindStore:".bright_black(),
                rewinds.to_string().magenta()
            );

            let (s_ts, r_ts) = store.latest_activity_timestamps().unwrap_or_default();
            println!("\n {}", "Recent activity:".bold().bright_white());
            if let Some(s) = s_ts {
                println!("   Last session: {}", format_time_ago(s).bright_black());
            } else {
                println!("   Last session: none");
            }
            if let Some(r) = r_ts {
                println!("   Last distill: {}", format_time_ago(r).bright_black());
            } else {
                println!("   Last distill: none");
            }
        }
        Err(_) => {
            println!(
                "  {:<15} ~/.omni/omni.db (missing) {}",
                "Database:".bright_black(),
                "[ERROR]".red().bold()
            );
            println!(
                "  {:<15} unknown {}",
                "FTS5:".bright_black(),
                "[ERROR]".red().bold()
            );
            warnings.push("Database is totally inaccessible.");
            all_ok = false;
        }
    }

    // 4. Hook entries in ~/.claude/settings.json
    println!("\n {}", "OMNI Hooks:".bold().bright_white());
    let path = get_settings_path();
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if content.contains("--hook")
                || content.contains("--post-hook")
                || content.contains("--pre-hook")
                || content.contains("--session-start")
                || content.contains("--pre-compact")
            {
                let fmt_hook = |name: &str, tag: &str| {
                    if content.contains(tag) {
                        println!(
                            "   {:<15} {}",
                            name.bright_black(),
                            "[OK] installed".green()
                        );
                        true
                    } else {
                        println!(
                            "   {:<15} {}",
                            name.bright_black(),
                            "[WARNING] missing".yellow()
                        );
                        false
                    }
                };

                if !fmt_hook("PreToolUse", "PreToolUse") {
                    all_ok = false;
                }
                if !fmt_hook("PostToolUse", "PostToolUse") {
                    all_ok = false;
                    warnings.push("PostToolUse hook is not installed. Run `omni init`.");
                }
                if !fmt_hook("SessionStart", "SessionStart") {
                    all_ok = false;
                }
                if !fmt_hook("PreCompact", "PreCompact") {
                    all_ok = false;
                }

                if fix_mode && !all_ok {
                    let _ = crate::cli::init::run_init(&[
                        "omni".to_string(),
                        "init".to_string(),
                        "--hook".to_string(),
                    ]);
                    println!(
                        "   {:<15} {}",
                        "Hooks:".bright_black(),
                        "[FIXED] missing hooks installed".green().bold()
                    );
                    all_ok = true;
                    warnings.retain(|w| {
                        !w.contains("hook") && !w.contains("Claude settings not found")
                    });
                }
            } else {
                if fix_mode {
                    let _ = crate::cli::init::run_init(&[
                        "omni".to_string(),
                        "init".to_string(),
                        "--hook".to_string(),
                    ]);
                    println!(
                        "   {:<15} {}",
                        "Hooks:".bright_black(),
                        "[FIXED] installed".green().bold()
                    );
                } else {
                    println!(
                        "   {:<15} {}",
                        "Hooks:".bright_black(),
                        "[WARNING] no hooks found".yellow().bold()
                    );
                    warnings.push("OMNI hooks are not configured. Run `omni init`.");
                    all_ok = false;
                }
            }
        }
    } else {
        if fix_mode {
            let _ = crate::cli::init::run_init(&[
                "omni".to_string(),
                "init".to_string(),
                "--hook".to_string(),
            ]);
            println!(
                "   {:<15} {}",
                "Hooks:".bright_black(),
                "[FIXED] installed".green().bold()
            );
        } else {
            println!(
                "   {:<15} {}",
                "Hooks:".bright_black(),
                "[ERROR] settings.json missing".red()
            );
            warnings.push("Claude settings not found. Have you installed Claude Code?");
            all_ok = false;
        }
    }

    // 5. MCP Server registration
    println!("\n {}", "OMNI MCP Server:".bold().bright_white());
    let mcp_path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Library/Application Support/Claude/claude_desktop_config.json");
    let mcpa_path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".claude.json");

    let mut mcp_found = false;
    for p in &[mcp_path, mcpa_path] {
        if p.exists()
            && let Ok(c) = fs::read_to_string(p)
            && (c.contains("omni --mcp") || c.contains("\"omni\":"))
        {
            mcp_found = true;
            println!(
                "   {:<15} {} {}",
                "Registered:".bright_black(),
                p.display().to_string().bright_black(),
                "[OK]".green().bold()
            );
            break;
        }
    }
    if !mcp_found {
        if fix_mode {
            let _ = crate::cli::init::run_init(&[
                "omni".to_string(),
                "init".to_string(),
                "--mcp".to_string(),
            ]);
            println!(
                "   {:<15} {}",
                "Registered:".bright_black(),
                "[FIXED] registered".green().bold()
            );
        } else {
            println!(
                "   {:<15} {}",
                "Registered:".bright_black(),
                "[WARNING] no MCP server found".yellow().bold()
            );
            warnings.push("MCP Server is not configured. Run `omni init`.");
            all_ok = false;
        }
    }

    // 6. Config Filters
    println!("\n {}", "Filters:".bold().bright_white());
    let (built_in, user_filters, local_filters) =
        crate::pipeline::toml_filter::get_filters_by_source();

    println!(
        "   {:<15} {} loaded (embedded)",
        "Built-in:".bright_black(),
        built_in.len().to_string().yellow()
    );

    let user_dir = conf_dir.join("filters");
    if user_dir.exists() {
        println!(
            "   {:<15} ~/.omni/filters/ ({} filters)",
            "User:".bright_black(),
            user_filters.len().to_string().yellow()
        );

        if fix_mode && let Ok(entries) = std::fs::read_dir(&user_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "toml")
                    && crate::pipeline::toml_filter::load_from_file(&path).is_err()
                {
                    let bak_path = path.with_extension("toml.bak");
                    if std::fs::rename(&path, &bak_path).is_ok() {
                        println!(
                            "   {:<15} {} {}",
                            "Cleaned:".bright_black(),
                            path.file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .bright_black(),
                            "[RENAMED TO .bak]".green().bold()
                        );
                    }
                }
            }
        }
    } else {
        println!("   {:<15} none", "User:".bright_black());
    }

    let project_dir = PathBuf::from(".omni/filters");
    if project_dir.exists() {
        if crate::guard::trust::is_trusted(std::env::current_dir().unwrap_or_default().as_path()) {
            println!(
                "   {:<15} .omni/filters/ ({} filters, TRUSTED) {}",
                "Project:".bright_black(),
                local_filters.len().to_string().yellow(),
                "[OK]".green().bold()
            );
        } else {
            if fix_mode {
                let _ = crate::guard::trust::trust_project(
                    std::env::current_dir().unwrap_or_default().as_path(),
                );
                println!(
                    "   {:<15} .omni/filters/ (TRUSTED) {}",
                    "Project:".bright_black(),
                    "[FIXED]".green().bold()
                );
            } else {
                println!(
                    "   {:<15} .omni/filters/ (NOT TRUSTED) {}",
                    "Project:".bright_black(),
                    "[WARNING]".yellow().bold()
                );
                warnings.push("Project filters found but not trusted. Run: `omni trust`.");
                all_ok = false;
            }
        }
    } else {
        println!("   {:<15} none", "Project:".bright_black());
    }

    // 10. Intelligence Consistency
    println!("\n {}", "Intelligence:".bold().bright_white());
    if let Ok(store) = Store::open() {
        if fix_mode {
            match store.reclassify_historical_data() {
                Ok(count) => {
                    if count > 0 {
                        println!(
                            "   {:<15} {} records upgraded to new categories {}",
                            "Upgrade:".bright_black(),
                            count.to_string().yellow().bold(),
                            "[FIXED]".green().bold()
                        );
                    } else {
                        println!(
                            "   {:<15} historical statistics are up to date {}",
                            "Status:".bright_black(),
                            "[OK]".green().bold()
                        );
                    }
                }
                Err(e) => {
                    println!(
                        "   {:<15} upgrade failed: {} {}",
                        "Upgrade:".bright_black(),
                        e,
                        "[ERROR]".red().bold()
                    );
                }
            }
        } else {
            // Diagnostic mode: check how many could be upgraded
            // For now, simpler to just say 'Run with --fix to upgrade'
            println!(
                "   {:<15} Run with {} to upgrade historical stats",
                "Status:".bright_black(),
                "--fix".cyan()
            );
        }
    }

    if let Some(latest) = crate::guard::update::check() {
        crate::guard::update::print_notification(&latest);
    }

    // Status Footer
    println!("\n {}", "Status:".bold().bright_white());
    let status_msg = if all_ok {
        "ALL OK".green().bold()
    } else {
        "ATTENTION NEEDED".yellow().bold()
    };
    let status_icon = if all_ok {
        "✓".green()
    } else {
        "⚠".yellow()
    };
    println!("  {} {}", status_icon, status_msg);

    if !warnings.is_empty() {
        println!("\n {}", "Suggestions:".bold().bright_white());
        for w in warnings {
            println!("  {} {}", "•".yellow(), w);
        }
    }
    println!(
        "\n{}\n",
        "─────────────────────────────────────────"
            .bright_black()
            .bold()
    );

    Ok(())
}
