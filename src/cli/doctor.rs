use crate::store::sqlite::Store;
use colored::*;
use std::fs;
use std::path::PathBuf;

/// Read by both `print_help` and `super::check_flags` (#151).
const FLAGS: super::Flags = &[
    (
        "--fix",
        "Automatically fix configuration and integration issues",
    ),
    (
        "--test-filter <name>",
        "Run inline tests for a specific filter",
    ),
    (
        "--benchmark",
        "Run filter tests and report slow filters (> 5ms)",
    ),
    (
        "--coverage",
        "Analyze filter coverage against past commands",
    ),
    (
        "--validate <file.toml>",
        "Validate a TOML filter file (syntax and tests)",
    ),
    ("--json", "Machine-readable JSON output"),
    (
        "--detail",
        "Print every integration row, not just the ones needing attention",
    ),
];

fn print_help() {
    println!(
        "\n{} {}: Installation diagnostics",
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
    super::print_flags(FLAGS);
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
    /// What the per-agent reports said that a check name cannot carry.
    ///
    /// These were collected and dropped. A Codex install whose hooks are
    /// installed but awaiting review (#367) pushes its warning here and nowhere
    /// else, so `--json` reported `hooks: installed`, `healthy: true`, and gave a
    /// program no way to learn the hooks were being skipped.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

#[derive(serde::Serialize)]
pub struct DoctorCheck {
    pub name: String,
    pub ok: bool,
    pub message: String,
}

/// Changelog entries this build carries that no release contains (#137).
///
/// Counted by `build.rs` from the `## [Unreleased]` section of the tree the
/// binary was compiled from, so a properly cut release reports 0 and says
/// nothing. Parsing cannot fail into a false alarm: an unreadable or malformed
/// value means "nothing to report", never "something is wrong".
fn unreleased_entries() -> usize {
    option_env!("OMNI_UNRELEASED_ENTRIES")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
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
    let conf_dir = crate::paths::config_home();
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
        // The report is human-readable and this path owes stdout a single JSON
        // document, so it is captured and dropped rather than printed (#353).
        // The report is dropped; its findings travel in `warnings`, which is
        // serialised below.
        let (ok, _report) =
            crate::agents::output::capture(|| agent.doctor_check(fix_mode, &mut warnings));
        if ok {
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
        warnings,
    };

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

/// One host's report, trimmed to what a diagnostic is read for.
///
/// #426: doctor answered one question in a hundred lines, 34 of which said
/// `[OK]`. A row confirming that something is fine is not what a diagnostic is
/// read for. A host with nothing wrong collapses to a single line naming its
/// distill tier and how many checks passed; a host with anything to say keeps
/// its heading, every row that is not `[OK]`, and the same count. `--detail`
/// returns the whole report, and a row that is not `[OK]` is never hidden in
/// either mode.
fn condense_agent_report(report: &str, tier: &str, detail: bool) -> String {
    let heading = report
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .to_string();
    let tier_row = format!("   {:<15} {}\n", "Distill tier:", tier);
    if detail {
        return format!("{report}{tier_row}");
    }

    let body: Vec<&str> = report
        .lines()
        .skip_while(|l| l.trim().is_empty())
        .skip(1)
        .collect();
    let hidden = body.iter().filter(|l| l.contains("[OK]")).count();
    let kept: Vec<&&str> = body
        .iter()
        .filter(|l| !l.contains("[OK]") && !l.trim().is_empty())
        .collect();

    if kept.is_empty() {
        let checks = if hidden == 1 { "check" } else { "checks" };
        return format!("{heading} {tier}, {hidden} {checks} [OK]\n");
    }

    // The tier goes on the heading here too. The `kept.is_empty()` branch above
    // already writes it inline, so appending a separate `Distill tier:` row only
    // in this branch gave the list two shapes: a one-liner for a clean host and
    // a block with a trailing row for any host with a note (#463). One shape.
    let mut out = format!("{heading} {tier}\n");
    for line in kept {
        out.push_str(line);
        out.push('\n');
    }
    if hidden > 0 {
        let checks = if hidden == 1 { "check" } else { "checks" };
        out.push_str(&format!(
            "   {:<15} {hidden} more {checks} [OK], omni doctor --detail to see them\n",
            ""
        ));
    }
    out
}

pub fn run(args: &[String]) -> anyhow::Result<()> {
    if args
        .iter()
        .any(|a| a == "--help" || a == "-h" || a == "help")
    {
        print_help();
        return Ok(());
    }
    super::check_flags("doctor", args, FLAGS)?;

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
    let detail = args.iter().any(|a| a == "--detail");

    let mut all_ok = true;
    let mut warnings: Vec<String> = Vec::new();
    println!();
    super::print_rule();
    println!(" {}: Installation Diagnostics", "OMNI Doctor".bold().cyan());
    super::print_rule();

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

    // #137: `[LATEST]` above answers "is there a newer release than mine". It
    // cannot see fixes that were never released, because then the newest
    // release *is* the running version, exactly the state #127 filed, where
    // six correctness fixes sat merged and unshipped while doctor said
    // `[LATEST]`. This is the other question, answered from the tree the binary
    // was built from.
    if unreleased_entries() > 0 {
        println!(
            "  {} {}",
            format!("[{} UNRELEASED]", unreleased_entries())
                .yellow()
                .bold(),
            "built into this binary, in no release. Cut a tag".bright_black()
        );
    }

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
            // Name both numbers. "{sessions} records" read as a row count, so a
            // 112 MB database holding 5,730 distillations reported 17 (#118).
            println!(
                "  {:<15} ~/.omni/omni.db ({} distillations, {} sessions) {}",
                "Database:".bright_black(),
                store.distillation_count().to_string().yellow(),
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

            // Dead pages. Deleting rows does not shrink the file: SQLite keeps
            // the pages on a freelist for reuse, and with `auto_vacuum = 0` they
            // are never handed back. One prune left 142 MB of a 196 MB file
            // holding 54 MB of data (#393). Reported always, reclaimed only when
            // asked, because `VACUUM` rewrites the whole file.
            if let Some((pages, free)) = store.page_stats() {
                let share = 100 * free / pages.max(1);
                if share < 40 {
                    println!(
                        "  {:<15} {share}% of {pages} pages free {}",
                        "DB Space:".bright_black(),
                        "[OK]".green().bold()
                    );
                } else if fix_mode {
                    match store.vacuum() {
                        Ok(()) => {
                            let reclaimed = store.page_stats().map_or(free, |(_, f)| free - f);
                            println!(
                                "  {:<15} reclaimed {reclaimed} of {pages} pages {}",
                                "DB Space:".bright_black(),
                                "[FIXED]".green().bold()
                            );
                        }
                        Err(e) => {
                            println!(
                                "  {:<15} vacuum failed: {e} {}",
                                "DB Space:".bright_black(),
                                "[WARNING]".yellow().bold()
                            );
                            warnings.push(format!("Could not reclaim database space: {e}"));
                        }
                    }
                } else {
                    println!(
                        "  {:<15} {share}% of {pages} pages free {}",
                        "DB Space:".bright_black(),
                        "[WARNING]".yellow().bold()
                    );
                    warnings.push(
                        "Most of the database file is dead space. Run `omni doctor --fix` to reclaim it."
                            .to_string(),
                    );
                }
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

            let (_s_ts, d_ts) = store.latest_activity_timestamps().unwrap_or_default();

            // Last distillation check: warn if no distillation in last 10 min
            if let Some(rt) = d_ts {
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
        // #426. doctor answered one question in a hundred lines, 34 of which
        // said [OK]. A row that says a thing is fine is not what a diagnostic is
        // read for, so by default each host keeps only the rows that need
        // attention plus a count of the rest. `--detail` prints all of them, and
        // anything that is not [OK] always prints, in either mode.
        let (ok, report) =
            crate::agents::output::capture(|| agent.doctor_check(fix_mode, &mut warnings));
        if ok {
            any_agent_ok = true;
        }
        // The tier is printed for every host, configured or not. "Hooks
        // installed" and "the model reads less" are different claims, and three
        // integrations shipped the first while delivering none of the second
        // (#351). It survives the condensing for that reason.
        print!(
            "{}",
            condense_agent_report(&report, agent.tier().name(), detail)
        );
        // Note: integrations are optional; "not configured" should not fail doctor
    }
    // The three tier sentences, once. They used to arrive in full on every host
    // line, which on this machine meant the MCP-only explanation ten times in a
    // block of a hundred lines (#426). The per-host line above still names the
    // tier, so nothing #351 asked for is lost.
    // One line per tier. As a single sentence it ran to 206 columns from an
    // empty 15-wide label, so it wrapped mid-word in any normal terminal and
    // sat under a rule a third of its length (#463). The `·` separators were
    // already doing the splitting; this just believes them.
    for tier in [
        "Full = model-facing distill active",
        "Handoff-first = built-in tool output not rewritten, omni_run distils",
        "                what you route through it",
        "MCP-only = memory and session state, no shell distill",
    ] {
        println!("   {}", tier.bright_black());
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

    let built_in = crate::pipeline::toml_filter::get_filters_by_source();

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

    // --- Elegant Warning Display ---
    let mut all_filter_warnings = Vec::new();
    // Line patterns compile on first use now, so loading no longer reports a
    // malformed one (#283). `doctor` is where that check moved: it is run by a
    // human asking whether the config is sound, not on the hook's path, so it
    // can afford to compile every pattern.
    for report in [&built_in] {
        for filter in &report.filters {
            all_filter_warnings.extend(filter.validate_line_patterns());
        }
    }
    all_filter_warnings.extend(built_in.warnings);

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
    println!();
    super::print_rule();
    println!();

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

#[cfg(test)]
mod tests {
    use super::condense_agent_report;

    const REPORT: &str = "  Claude Code:\n   PreToolUse      [OK] installed\n   MCP Server:     ~/.claude.json [OK]\n";

    /// #426: 34 of doctor's hundred lines said [OK]. A host with nothing to
    /// report is one line now, and the tier #351 asked for is still on it.
    #[test]
    fn collapses_a_healthy_host_to_one_line() {
        let out = condense_agent_report(REPORT, "Full", false);

        assert_eq!(out, "  Claude Code: Full, 2 checks [OK]\n");
    }

    /// The rows that matter are never the ones hidden.
    #[test]
    fn keeps_every_row_that_is_not_ok() {
        let report = format!("{REPORT}   Plugin:         not installed\n");

        let out = condense_agent_report(&report, "MCP-only", false);

        assert!(out.contains("Plugin:         not installed"), "{out}");
        assert!(out.contains("2 more checks [OK]"), "{out}");
        // The tier rides the heading, the same shape the clean-host one-liner
        // uses. A trailing `Distill tier:` row here was the second shape (#463).
        assert!(out.lines().next().unwrap().ends_with("MCP-only"), "{out}");
        assert!(!out.contains("Distill tier:"), "{out}");
    }

    #[test]
    fn detail_returns_the_whole_report() {
        let out = condense_agent_report(REPORT, "Full", true);

        assert!(out.contains("PreToolUse"), "{out}");
        assert!(out.contains("MCP Server:"), "{out}");
        assert!(out.contains("Distill tier:"), "{out}");
    }
}
