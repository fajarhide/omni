use crate::cli::init::get_settings_path;
use crate::store::sqlite::Store;
use std::fs;
use std::path::PathBuf;

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

pub fn run() -> anyhow::Result<()> {
    let mut all_ok = true;
    let mut warnings = Vec::new();

    println!("─────────────────────────────────────────");
    println!(" OMNI Doctor — Installation Diagnostics");
    println!("─────────────────────────────────────────");

    // 1. Binary Version
    println!(" Binary:         omni v{} [OK]", env!("CARGO_PKG_VERSION"));

    // 2. Config Dir
    let conf_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".omni");
    if conf_dir.exists()
        && fs::metadata(&conf_dir)
            .map(|m| !m.permissions().readonly())
            .unwrap_or(false)
    {
        println!(" Config dir:     ~/.omni/ [OK]");
    } else {
        println!(" Config dir:     ~/.omni/ [ERROR]");
        warnings.push("Config directory ~/.omni/ is missing or not writable. Run `omni init`.");
        all_ok = false;
    }

    // 3. Database
    match Store::open() {
        Ok(store) => {
            let (sessions, rewinds) = store.stats().unwrap_or_default();
            println!(
                " Database:       ~/.omni/omni.db ({} records) [OK]",
                sessions
            );

            if store.check_fts5() {
                println!(" FTS5:           available [OK]");
            } else {
                println!(" FTS5:           missing [WARNING]");
                warnings.push(
                    "SQLite FTS5 extension is not enabled. Search capabilities will be degraded.",
                );
                all_ok = false;
            }

            // 9. RewindStore
            println!(" \n RewindStore:   {} items tracked\n", rewinds);

            let (s_ts, r_ts) = store.latest_activity_timestamps().unwrap_or_default();
            println!(" Recent activity:");
            if let Some(s) = s_ts {
                println!("   Last session: {}", format_time_ago(s));
            } else {
                println!("   Last session: none");
            }
            if let Some(r) = r_ts {
                println!("   Last distill: {}", format_time_ago(r));
            } else {
                println!("   Last distill: none");
            }
            println!();
        }
        Err(_) => {
            println!(" Database:       ~/.omni/omni.db (missing) [ERROR]");
            println!(" FTS5:           unknown [ERROR]\n");
            warnings.push("Database is totally inaccessible.");
            all_ok = false;
        }
    }

    // 4. Hook entries in ~/.claude/settings.json
    println!(" Hooks (Claude Code):");
    #[allow(clippy::match_single_binding)]
    match get_settings_path() {
        path => {
            if path.exists() {
                if let Ok(content) = fs::read_to_string(&path) {
                    if content.contains("--hook") {
                        if content.contains("PostToolUse") {
                            println!("   PostToolUse:  [OK] installed");
                        } else {
                            println!("   PostToolUse:  [WARNING] missing");
                            warnings
                                .push("PostToolUse hook is not installed. Run `omni init --hook`.");
                            all_ok = false;
                        }

                        if content.contains("SessionStart") {
                            println!("   SessionStart: [OK] installed");
                        } else {
                            println!("   SessionStart: [WARNING] missing");
                            warnings.push("SessionStart hook is not installed.");
                            all_ok = false;
                        }

                        if content.contains("PreCompact") {
                            println!("   PreCompact:   [OK] installed");
                        } else {
                            println!("   PreCompact:   [WARNING] missing");
                            warnings.push("PreCompact hook is not installed.");
                            all_ok = false;
                        }
                    } else {
                        println!(
                            "   Hooks:        [WARNING] omni --hook not found in settings.json"
                        );
                        warnings.push("OMNI hooks are not configured. Run `omni init --hook` to install them.");
                        all_ok = false;
                    }
                }
            } else {
                println!("   Hooks:        [ERROR] settings.json not found.");
                warnings.push("Claude settings not found. Have you installed Claude Code?");
                all_ok = false;
            }
        }
    }
    println!();

    // 5. MCP Server registration
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
            println!(" MCP Server:    {} (registered) [OK]\n", p.display());
            break;
        }
    }
    if !mcp_found {
        println!(" MCP Server:    [WARNING] not found in config\n");
        warnings.push(
            "MCP Server is not configured. Run `omni init --mcp` to install it for Claude Desktop.",
        );
        all_ok = false;
    }

    // 6. Config Filters
    println!(" Filters:");
    let (built_in, user_filters, local_filters) =
        crate::pipeline::toml_filter::get_filters_by_source();

    println!(
        "   Built-in:    {} filters loaded (embedded)",
        built_in.len()
    );

    let user_dir = conf_dir.join("filters");
    if user_dir.exists() {
        println!(
            "   User:        ~/.omni/filters/ ({} filters)",
            user_filters.len()
        );
    } else {
        println!("   User:        ~/.omni/filters/ (none)");
    }

    let project_dir = PathBuf::from(".omni/filters");
    if project_dir.exists() {
        if crate::guard::trust::is_trusted(std::env::current_dir().unwrap_or_default().as_path()) {
            println!(
                "   Project:     .omni/filters/ ({} filters, TRUSTED) [OK]",
                local_filters.len()
            );
        } else {
            println!(
                "   Project:     .omni/filters/ ({} filters, NOT TRUSTED — run: omni trust) [WARNING]",
                local_filters.len()
            );
            warnings.push("Project filters found but not trusted.");
            all_ok = false;
        }
    } else {
        println!("   Project:     .omni/filters/ (none)");
    }

    // Status Footer
    println!(
        "\n Status: {}  {}",
        if all_ok { "ALL OK" } else { "ATTENTION NEEDED" },
        if all_ok { "✓" } else { "⚠" }
    );
    if !warnings.is_empty() {
        println!("─────────────────────────────────────────");
        println!(" Suggestions:");
        for w in warnings {
            println!(" - {}", w);
        }
    }
    println!("─────────────────────────────────────────");

    Ok(())
}
