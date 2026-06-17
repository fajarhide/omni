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
        "  {: <32} Automatically fix configuration and integration issues",
        "--fix".cyan()
    );
    println!(
        "  {: <32} Run inline tests for a specific filter",
        "--test-filter <name>".cyan()
    );
    println!(
        "  {: <32} Run filter tests and report slow filters (> 5ms)",
        "--benchmark".cyan()
    );
    println!(
        "  {: <32} Analyze filter coverage against past commands",
        "--coverage".cyan()
    );
    println!(
        "  {: <32} Validate a TOML filter file (syntax and tests)",
        "--validate <file.toml>".cyan()
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

#[derive(serde::Serialize)]
pub struct DoctorJson {
    pub version: String,
    pub healthy: bool,
    pub checks: Vec<DoctorCheck>,
    pub fix_available: bool,
}

#[derive(serde::Serialize)]
pub struct DoctorCheck {
    pub name: String,
    pub ok: bool,
    pub message: String,
}

fn run_json(args: &[String]) -> anyhow::Result<()> {
    let fix_mode = args.iter().any(|a| a == "--fix");
    let mut checks = Vec::new();
    let mut all_ok = true;
    let mut fix_available = false;

    // 1. Binary
    checks.push(DoctorCheck {
        name: "binary".to_string(),
        ok: true,
        message: format!("omni v{}", env!("CARGO_PKG_VERSION")),
    });

    // 2. Config Dir
    let conf_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".omni");
    if conf_dir.exists() {
        let test_file = conf_dir.join(".write_test");
        if fs::write(&test_file, "ok").is_ok() {
            let _ = fs::remove_file(&test_file);
            checks.push(DoctorCheck {
                name: "config".to_string(),
                ok: true,
                message: "config valid".to_string(),
            });
        } else {
            checks.push(DoctorCheck {
                name: "config".to_string(),
                ok: false,
                message: "Cannot write to ~/.omni/. Sandbox issue?".to_string(),
            });
            all_ok = false;
        }
    } else {
        if fix_mode && fs::create_dir_all(&conf_dir).is_ok() {
            checks.push(DoctorCheck {
                name: "config".to_string(),
                ok: true,
                message: "config directory created".to_string(),
            });
        } else {
            checks.push(DoctorCheck {
                name: "config".to_string(),
                ok: false,
                message: "missing ~/.omni/".to_string(),
            });
            all_ok = false;
            fix_available = true;
        }
    }

    // 3. Database
    match Store::open() {
        Ok(store) => {
            let (sessions, _) = store.stats().unwrap_or_default();
            checks.push(DoctorCheck {
                name: "sqlite".to_string(),
                ok: true,
                message: format!("database healthy, {} events", sessions),
            });

            if !store.check_fts5() {
                checks.push(DoctorCheck {
                    name: "sqlite_fts5".to_string(),
                    ok: false,
                    message: "FTS5 missing".to_string(),
                });
                all_ok = false;
            }
            if !store.test_write() {
                checks.push(DoctorCheck {
                    name: "sqlite_write".to_string(),
                    ok: false,
                    message: "database read-only".to_string(),
                });
                all_ok = false;
            }
        }
        Err(_) => {
            checks.push(DoctorCheck {
                name: "sqlite".to_string(),
                ok: false,
                message: "database inaccessible".to_string(),
            });
            all_ok = false;
        }
    }

    // 4. Agents
    let integrations = crate::agents::all_integrations();
    let mut any_agent_ok = false;
    let mut warnings = Vec::new();
    for agent in integrations {
        if agent.doctor_check(fix_mode, &mut warnings) {
            any_agent_ok = true;
        }
    }
    if any_agent_ok {
        checks.push(DoctorCheck {
            name: "hooks".to_string(),
            ok: true,
            message: "hooks installed".to_string(),
        });
    } else {
        checks.push(DoctorCheck {
            name: "hooks".to_string(),
            ok: false,
            message: "no agents configured".to_string(),
        });
        all_ok = false;
        fix_available = true;
    }

    let output = DoctorJson {
        version: "1".to_string(),
        healthy: all_ok,
        checks,
        fix_available,
    };

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

pub fn run(args: &[String]) -> anyhow::Result<()> {
    if args
        .iter()
        .any(|a| a == "--help" || a == "-h" || a == "help")
    {
        print_help();
        return Ok(());
    }

    if args.iter().any(|a| a == "--json") {
        return run_json(args);
    }

    let mut i = 1; // Assuming args[0] is "doctor"
    if !args.is_empty() && args[0] != "doctor" {
        i = 0;
    } // Adjust if args doesn't contain the command itself
    while i < args.len() {
        match args[i].as_str() {
            "--test-filter" if i + 1 < args.len() => {
                return run_test_filter(&args[i + 1]);
            }
            "--test-filter" => {} // Handle edge case
            "--benchmark" => return run_benchmark(),
            "--coverage" => return run_coverage(),
            "--validate" if i + 1 < args.len() => {
                return run_validate(&args[i + 1]);
            }
            "--validate" => {} // Handle edge case
            "doctor" => {}
            _ => {}
        }
        i += 1;
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

    // 6. Config Signals
    println!("\n {}", "Signals:".bold().bright_white());

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

    let user_dir = {
        let signals_path = conf_dir.join("signals");
        if signals_path.exists() {
            signals_path
        } else {
            conf_dir.join("filters") // backward compat
        }
    };
    if user_dir.exists() {
        let dir_name = if user_dir.ends_with("signals") {
            "signals"
        } else {
            "filters"
        };
        println!(
            "   {:<15} ~/.omni/{dir_name}/ ({} signals)",
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
        let local_signals_dir = {
            let s = cwd.join(".omni").join("signals");
            if s.exists() {
                s
            } else {
                cwd.join(".omni").join("filters")
            }
        };
        if local_signals_dir.exists() {
            let dir_label = if local_signals_dir.ends_with("signals") {
                "signals"
            } else {
                "filters"
            };
            if crate::guard::trust::is_trusted(&cwd.join("omni_config.json")) {
                println!(
                    "   {:<15} .omni/{dir_label}/ ({} signals, TRUSTED) {}",
                    "Project:".bright_black(),
                    local_report.filters.len().to_string().yellow(),
                    "[OK]".green().bold()
                );
            } else if fix_mode {
                let _ = crate::guard::trust::trust_project(&cwd);
                println!(
                    "   {:<15} .omni/{dir_label}/ (TRUSTED) {}",
                    "Project:".bright_black(),
                    "[FIXED]".green().bold()
                );
            } else {
                println!(
                    "   {:<15} .omni/{dir_label}/ ({} signals, NOT TRUSTED) {}",
                    "Project:".bright_black(),
                    local_report.filters.len().to_string().yellow(),
                    "[WARNING]".yellow().bold()
                );
                warnings.push(
                    "Project signals found but not trusted. Run: `omni doctor --fix`.".to_string(),
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

fn run_test_filter(filter_name: &str) -> anyhow::Result<()> {
    println!(
        "\n {} Testing filter: {}\n",
        "🔬".cyan(),
        filter_name.bold()
    );
    let filters = crate::pipeline::toml_filter::load_all_filters();
    let target = filters.into_iter().find(|f| f.name == filter_name);

    match target {
        Some(filter) => {
            if filter.inline_tests.is_empty() {
                println!(
                    "  {} No inline tests defined for this filter.",
                    "⚠".yellow()
                );
                return Ok(());
            }

            let mut passed = 0;
            let total = filter.inline_tests.len();
            for test in &filter.inline_tests {
                let actual = filter.apply(&test.input);
                if actual.trim() == test.expected.trim() {
                    passed += 1;
                    println!("  {} {} {}", "✓".green(), "PASS".green().bold(), test.name);
                } else {
                    println!("\n  {} {} {}", "✗".red(), "FAIL".red().bold(), test.name);
                    println!("    {}", "Expected:".bright_black());
                    for line in test.expected.lines() {
                        println!("      {}", line.green());
                    }
                    println!("    {}", "Got:".bright_black());
                    for line in actual.lines() {
                        println!("      {}", line.red());
                    }
                    println!();
                }
            }

            println!("\n  {} / {} tests passed.", passed, total);
            if passed != total {
                std::process::exit(1);
            }
        }
        None => {
            println!("  {} Filter '{}' not found.", "✗".red(), filter_name);
            std::process::exit(1);
        }
    }
    Ok(())
}

fn run_benchmark() -> anyhow::Result<()> {
    println!("\n {} Benchmarking filters...\n", "⏱ ".cyan());
    let filters = crate::pipeline::toml_filter::load_all_filters();

    let mut slow_count = 0;
    for filter in filters {
        if filter.inline_tests.is_empty() {
            continue;
        }

        let start = std::time::Instant::now();
        for test in &filter.inline_tests {
            let _ = filter.apply(&test.input);
        }
        let elapsed = start.elapsed();
        let avg = elapsed.as_secs_f64() * 1000.0 / (filter.inline_tests.len() as f64);

        if avg > 5.0 {
            println!(
                "  {} {} ({:.2}ms avg)",
                "⚠".yellow(),
                filter.name.yellow(),
                avg
            );
            slow_count += 1;
        } else {
            println!(
                "  {} {} ({:.2}ms avg)",
                "✓".green(),
                filter.name.green(),
                avg
            );
        }
    }

    if slow_count > 0 {
        println!("\n  Found {} slow filters (> 5ms).", slow_count);
    } else {
        println!("\n  All tested filters are fast!");
    }

    Ok(())
}

fn run_coverage() -> anyhow::Result<()> {
    println!("\n {} Filter Coverage Analysis\n", "📊".cyan());

    let store = Store::open()?;
    let conn = store.pool.get()?;

    // Find most frequent commands that are passing through unfiltered
    let mut stmt = conn.prepare(
        "SELECT command, COUNT(*) as count 
         FROM distillations 
         WHERE route = 'passthrough' 
         GROUP BY command 
         ORDER BY count DESC 
         LIMIT 10",
    )?;

    let iter = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;

    let mut found = false;
    for row in iter.flatten() {
        if !found {
            println!("  Top unfiltered commands (candidates for new filters):\n");
            found = true;
        }
        println!(
            "  {:<5} {}",
            row.1.to_string().yellow(),
            row.0.bright_black()
        );
    }

    if !found {
        println!(
            "  {} Excellent! No highly-repeated unfiltered commands found.",
            "✓".green()
        );
    }

    Ok(())
}

fn run_validate(path_str: &str) -> anyhow::Result<()> {
    println!(
        "\n {} Validating TOML filter: {}\n",
        "🔍".cyan(),
        path_str.bold()
    );
    let path = std::path::Path::new(path_str);

    if !path.exists() {
        println!("  {} File not found.", "✗".red());
        std::process::exit(1);
    }

    let report = crate::pipeline::toml_filter::load_from_file(path)?;

    let mut ok = true;
    for warning in report.warnings {
        println!("  {} {}", "⚠".yellow(), warning);
        ok = false;
    }

    for filter in report.filters {
        println!("  {} Parsed filter '{}'", "✓".green(), filter.name);

        let test_report =
            crate::pipeline::toml_filter::run_inline_tests(std::slice::from_ref(&filter));
        if !test_report.failures.is_empty() {
            println!("    {} Inline tests failed:", "✗".red());
            for f in test_report.failures {
                println!("      {}", f.bright_black());
            }
            ok = false;
        } else if filter.inline_tests.is_empty() {
            println!("    {} No inline tests found.", "⚠".yellow());
        } else {
            println!(
                "    {} All {} inline tests passed.",
                "✓".green(),
                test_report.passes
            );
        }
    }

    if !ok {
        println!("\n  {} Validation failed.", "✗".red());
        std::process::exit(1);
    } else {
        println!("\n  {} File is valid and ready.", "✓".green().bold());
    }

    Ok(())
}
