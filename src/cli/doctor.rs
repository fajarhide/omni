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

/// One line for the MCP section, phrased so a missing tool is a setting rather
/// than a mystery.
///
/// It names the id it resolved because the caller passes whatever
/// `detect_agent_id` returned, which in a plain terminal is `terminal` and not
/// the host the reader is thinking of. `keep_everything` comes from
/// `policy::override_is_on`, the same read the served router makes, so the line
/// cannot offer the override as a remedy while the override is already on.
pub(crate) fn mcp_tool_line(agent_id: &str, keep_everything: bool) -> String {
    let total = crate::mcp::policy::ALL_TOOL_COUNT;
    let (active, note) = if keep_everything {
        (total, "OMNI_MCP_TOOLS=all is set")
    } else {
        (
            crate::mcp::policy::active_tools(agent_id).len(),
            "OMNI_MCP_TOOLS=all restores the rest",
        )
    };
    format!("  MCP tools:      {active} of {total} advertised to {agent_id} ({note})")
}

fn run_json(args: &[String]) -> anyhow::Result<()> {
    let fix_mode = super::has_flag(args, "--fix");
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
    if super::wants_help(args) {
        print_help();
        return Ok(());
    }
    super::check_flags("doctor", args, FLAGS)?;

    if super::has_flag(args, "--json") {
        return run_json(args);
    }

    let fix_mode = super::has_flag(args, "--fix");
    let detail = super::has_flag(args, "--detail");

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

    // The tools this host is told about, not the host's own agent list above:
    // #577 cut the advertised surface from 25 to 9 on the evidence of one
    // machine, so the line has to name the setting that undoes it.
    println!(
        "{}",
        mcp_tool_line(
            &crate::agents::multiagent::detect_agent_id(),
            crate::mcp::policy::override_is_on(),
        )
    );

    if !any_agent_ok {
        warnings.push(
            "No agent integrations are configured. Run `omni init` to set up hooks + MCP for your agent."
                .to_string(),
        );
        all_ok = false;
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

    /// The cut has to be visible and reversible from the same line, because the
    /// evidence behind it is n=1 and the failure mode is a user finding a tool
    /// missing with nothing telling them why.
    #[test]
    fn doctor_says_how_many_tools_are_advertised_and_how_to_restore_them() {
        let line = super::mcp_tool_line("claude_code", false);
        assert_eq!(
            line,
            "  MCP tools:      2 of 25 advertised to claude_code \
             (OMNI_MCP_TOOLS=all restores the rest)"
        );
    }

    /// With the override in force nothing is cut, so naming it as a remedy is a
    /// false claim about the process the reader is looking at.
    #[test]
    fn doctor_reports_the_override_instead_of_offering_it() {
        let line = super::mcp_tool_line("claude_code", true);
        assert_eq!(
            line,
            "  MCP tools:      25 of 25 advertised to claude_code \
             (OMNI_MCP_TOOLS=all is set)"
        );
    }
}
