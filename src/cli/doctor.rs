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
    let mut warnings: Vec<String> = Vec::new();
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
                    "Cannot write to ~/.omni/. If using Claude Code, add ~/.omni to sandbox.filesystem.allowWrite in ~/.claude/settings.json".to_string(),
                );
                all_ok = false;
            }
        }
    } else if fix_mode && fs::create_dir_all(&conf_dir).is_ok() {
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
        warnings.push(
            "Config directory ~/.omni/ is missing or not writable. Run `omni init`.".to_string(),
        );
        all_ok = false;
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
                    "Database is read-only. Claude Code sandbox may be blocking writes to ~/.omni/omni.db. Add ~/.omni to sandbox.filesystem.allowWrite in ~/.claude/settings.json".to_string(),
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
                    "SQLite FTS5 extension is not enabled. Search capabilities will be degraded."
                        .to_string(),
                );
                all_ok = false;
            }

            // 9. RewindStore
            println!(
                "  {:<15} {} items tracked",
                "RewindStore:".bright_black(),
                rewinds.to_string().magenta()
            );

            let (_s_ts, r_ts) = store.latest_activity_timestamps().unwrap_or_default();

            // Last distillation check: warn if no distillation in last 10 min
            if let Some(rt) = r_ts {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                if now.saturating_sub(rt) > 600 {
                    println!(
                        "  {:<15} {} {} (run a noisy command to verify)",
                        "Last distill:".bright_black(),
                        format_time_ago(rt).bright_black(),
                        "[IDLE]".yellow()
                    );
                } else {
                    println!(
                        "  {:<15} {} {}",
                        "Last distill:".bright_black(),
                        format_time_ago(rt).bright_black(),
                        "[ACTIVE]".green().bold()
                    );
                }
            } else {
                println!(
                    "  {:<15} {} {}",
                    "Last distill:".bright_black(),
                    "never".bright_black(),
                    "[IDLE]".yellow()
                );
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
            warnings.push("Database is totally inaccessible.".to_string());
            all_ok = false;
        }
    }

    // 4. Agent Integrations
    println!("\n {}", "Agent Integrations:".bold().bright_white());
    let integrations = crate::agents::all_integrations();
    let mut any_agent_ok = false;
    for agent in integrations {
        if agent.doctor_check(fix_mode, &mut warnings) {
            any_agent_ok = true;
        }
        // Note: integrations are optional; "not configured" should not fail doctor
    }
    if !any_agent_ok {
        warnings.push(
            "No agent integrations are configured. Run `omni init` to set up hooks + MCP for your agent."
                .to_string(),
        );
        all_ok = false;
    }

    // 6. Config Filters
    println!("\n {}", "Filters:".bold().bright_white());

    // In --fix mode, repair legacy learned.toml *before* loading reports so warnings reflect fixes.
    if fix_mode {
        let learned_path = crate::paths::learned_filters_path();
        if learned_path.exists() {
            let _ = crate::pipeline::toml_filter::try_repair_file(&learned_path);
        }
    }

    let (built_in, user_report, local_report) =
        crate::pipeline::toml_filter::get_filters_by_source();

    println!(
        "   {:<15} {} loaded (embedded)",
        "Built-in:".bright_black(),
        built_in.filters.len().to_string().yellow()
    );

    let built_in_tests = crate::pipeline::toml_filter::run_inline_tests(&built_in.filters);
    if built_in_tests.failures.is_empty() {
        println!(
            "   {:<15} {} inline tests {}",
            "Filter tests:".bright_black(),
            built_in_tests.passes.to_string().yellow(),
            "[OK]".green().bold()
        );
    } else {
        println!(
            "   {:<15} {} failures {}",
            "Filter tests:".bright_black(),
            built_in_tests.failures.len().to_string().red(),
            "[ERROR]".red().bold()
        );
        for failure in built_in_tests.failures.iter().take(3) {
            println!(
                "   {:<15} {}",
                "Failure:".red().bold(),
                failure.bright_black()
            );
        }
        warnings.push("Built-in TOML filter inline tests failed.".to_string());
        all_ok = false;
    }

    let user_dir = conf_dir.join("filters");
    if user_dir.exists() {
        println!(
            "   {:<15} ~/.omni/filters/ ({} filters)",
            "User:".bright_black(),
            user_report.filters.len().to_string().yellow()
        );

        if fix_mode && let Ok(entries) = std::fs::read_dir(&user_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "toml")
                    && crate::pipeline::toml_filter::load_from_file(&path).is_err()
                {
                    // Try to repair before renaming to .bak
                    match crate::pipeline::toml_filter::try_repair_file(&path) {
                        Ok(true) => {
                            println!(
                                "   {:<15} {} {}",
                                "Cleaned:".bright_black(),
                                path.file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .bright_black(),
                                "[REPAIRED]".green().bold()
                            );
                        }
                        _ => {
                            let bak_path = path.with_extension("toml.bak");
                            if std::fs::rename(&path, &bak_path).is_ok() {
                                println!(
                                    "   {:<15} {} {}",
                                    "Cleaned:".bright_black(),
                                    path.file_name()
                                        .unwrap_or_default()
                                        .to_string_lossy()
                                        .bright_black(),
                                    "[RENAMED TO .bak]".yellow().bold()
                                );
                            }
                        }
                    }
                }
            }
        }
    } else {
        println!("   {:<15} none", "User:".bright_black());
    }

    if let Ok(cwd) = std::env::current_dir() {
        let local_filters_dir = cwd.join(".omni").join("filters");
        if local_filters_dir.exists() {
            if crate::guard::trust::is_trusted(&cwd.join("omni_config.json")) {
                println!(
                    "   {:<15} .omni/filters/ ({} filters, TRUSTED) {}",
                    "Project:".bright_black(),
                    local_report.filters.len().to_string().yellow(),
                    "[OK]".green().bold()
                );
            } else if fix_mode {
                let _ = crate::guard::trust::trust_project(&cwd);
                println!(
                    "   {:<15} .omni/filters/ (TRUSTED) {}",
                    "Project:".bright_black(),
                    "[FIXED]".green().bold()
                );
            } else {
                println!(
                    "   {:<15} .omni/filters/ ({} filters, NOT TRUSTED) {}",
                    "Project:".bright_black(),
                    local_report.filters.len().to_string().yellow(),
                    "[WARNING]".yellow().bold()
                );
                warnings.push(
                    "Project filters found but not trusted. Run: `omni doctor --fix`.".to_string(),
                );
                all_ok = false;
            }
        } else {
            println!(
                "   {:<15} {}",
                "Project:".bright_black(),
                "none".bright_black()
            );
        }
    }

    // --- Elegant Warning Display ---
    let mut all_filter_warnings = Vec::new();
    all_filter_warnings.extend(built_in.warnings);
    all_filter_warnings.extend(user_report.warnings);
    all_filter_warnings.extend(local_report.warnings);

    if !all_filter_warnings.is_empty() {
        for warning in all_filter_warnings.iter().take(5) {
            println!(
                "   {:<15} {}",
                "Warning:".yellow().bold(),
                warning.bright_black()
            );
        }
        if all_filter_warnings.len() > 5 {
            println!(
                "   {:<15} ... and {} more",
                "".repeat(15),
                (all_filter_warnings.len() - 5).to_string().bright_black()
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
