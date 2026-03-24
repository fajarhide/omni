mod cli;
mod distillers;
mod guard;
mod hooks;
mod mcp;
pub mod pipeline;
mod session;
mod store;

use colored::*;
use std::env;
use std::io::{self, IsTerminal};
use std::sync::{Arc, Mutex};

use crate::pipeline::SessionState;
use crate::store::sqlite::Store;

// ─── Mode Detection ─────────────────────────────────────

#[derive(Debug, PartialEq)]
enum Mode {
    PostHook,
    Mcp,
    SessionStart,
    PreCompact,
    PreHook,
    Pipe,
    Cli,
}

fn detect_mode(args: &[String]) -> Mode {
    if args.len() > 1 {
        match args[1].as_str() {
            "--hook" | "--post-hook" => return Mode::PostHook,
            "--mcp" => return Mode::Mcp,
            "--session-start" => return Mode::SessionStart,
            "--pre-compact" => return Mode::PreCompact,
            "--pre-hook" => return Mode::PreHook,
            _ => {}
        }
    }
    if args.len() == 1 && !io::stdin().is_terminal() {
        return Mode::Pipe;
    }
    Mode::Cli
}

// ─── Engine / Globals ───────────────────────────────────

fn init_globals() -> (Option<Arc<Store>>, Option<Arc<Mutex<SessionState>>>) {
    match Store::open() {
        Ok(store) => {
            let session = store
                .find_latest_session()
                .unwrap_or_else(SessionState::new);
            let store_arc = Arc::new(store);
            let session_arc = Arc::new(Mutex::new(session));
            (Some(store_arc), Some(session_arc))
        }
        Err(_) => (None, None),
    }
}

// ─── Help Text ──────────────────────────────────────────

fn print_help() {
    let version = env!("CARGO_PKG_VERSION");

    println!(
        "\n{} {} — Less noise. More signal. Right signal.",
        "omni".bold().cyan(),
        version.bright_black()
    );

    println!("\n{}", "USAGE:".bold().bright_white());
    println!("  omni {} {}", "[COMMAND]".cyan(), "[FLAGS]".bright_black());
    println!(
        "  {} | omni       {}",
        "cmd / cli".bright_black(),
        "# Distill command output".bright_black()
    );

    println!("\n{}", "COMMANDS:".bold().bright_white());
    println!("  {: <12} Setup OMNI Hooks and MCP server", "init".cyan());
    println!("  {: <12} View token savings analytics", "stats".cyan());
    println!("  {: <12} Manage session state", "session".cyan());
    println!(
        "  {: <12} Auto-generate filters from history",
        "learn".cyan()
    );

    println!("\n{}", "UTILITIES:".bold().bright_white());
    println!("  {: <12} Diagnose installation health", "doctor".cyan());
    println!(
        "  {: <12} Clean uninstall (for backups config)",
        "reset".cyan()
    );
    println!("  {: <12} Print version info", "version, -v".cyan());
    println!("  {: <12} Show this help message", "help, -h".cyan());

    println!("\n{}", "EXAMPLES:".bold().bright_white());
    println!(
        "  omni doctor           {}",
        "# Diagnose installation health".bright_black()
    );
    println!(
        "  omni stats            {}",
        "# View your savings".bright_black()
    );
    println!(
        "  ls -R | omni          {}",
        "# Distill long output".bright_black()
    );
    println!();
}

// ─── Main ───────────────────────────────────────────────

fn main() {
    let args: Vec<String> = env::args().collect();
    let mode = detect_mode(&args);

    match mode {
        Mode::PostHook => {
            let (store, session) = init_globals();
            if let (Some(s), Some(ss)) = (store, session) {
                let _ = hooks::dispatcher::run(s, ss);
            }
        }

        Mode::PreHook => {
            if let Err(e) = hooks::pre_tool::run() {
                eprintln!("[omni] Pre-Hook error: {}", e);
                std::process::exit(1);
            }
        }

        Mode::SessionStart => {
            // Legacy flag — route through dispatcher
            let (store, session) = init_globals();
            if let (Some(s), Some(ss)) = (store, session) {
                let _ = hooks::dispatcher::run(s, ss);
            }
        }

        Mode::PreCompact => {
            // Legacy flag — route through dispatcher
            let (store, session) = init_globals();
            if let (Some(s), Some(ss)) = (store, session) {
                let _ = hooks::dispatcher::run(s, ss);
            }
        }

        Mode::Mcp => {
            let (store, session) = init_globals();
            if let (Some(s), Some(ss)) = (store, session) {
                let rt = tokio::runtime::Runtime::new().unwrap();
                if let Err(e) = rt.block_on(async { mcp::server::run(s, ss).await }) {
                    eprintln!("[omni] MCP Server error: {}", e);
                }
            } else {
                eprintln!("[omni] Failed to open SQLite store for MCP.");
            }
        }

        Mode::Pipe => {
            let store_arc = Store::open().map(Arc::new).ok();
            let session_arc = store_arc.as_ref().map(|s| {
                let session = s.find_latest_session().unwrap_or_else(SessionState::new);
                Arc::new(Mutex::new(session))
            });
            if let Err(e) = hooks::pipe::run(store_arc, session_arc, None) {
                eprintln!("[omni] Pipe engine error: {}", e);
                std::process::exit(1);
            }
        }

        Mode::Cli => {
            let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("help");

            match cmd {
                "version" | "-v" | "--version" => {
                    println!("omni {}", env!("CARGO_PKG_VERSION"));
                }

                "help" | "-h" | "--help" => {
                    print_help();
                }

                "init" => {
                    let _ = cli::init::run_init(&args);
                }

                "stats" => match Store::open() {
                    Ok(store) => {
                        if let Err(e) = cli::stats::run(&args, &store) {
                            eprintln!("[omni] Stats error: {}", e);
                            std::process::exit(1);
                        }
                    }
                    Err(e) => {
                        eprintln!("[omni] Cannot open database for stats: {}", e);
                        std::process::exit(1);
                    }
                },

                "session" => match Store::open() {
                    Ok(store) => {
                        let store_arc = Arc::new(store);
                        if let Err(e) = cli::session::run_session(&args, store_arc) {
                            eprintln!("[omni] Session error: {}", e);
                            std::process::exit(1);
                        }
                    }
                    Err(e) => {
                        eprintln!("[omni] Cannot open database for session: {}", e);
                        std::process::exit(1);
                    }
                },

                "learn" => {
                    if let Err(e) = cli::learn::run_learn(&args) {
                        eprintln!("[omni] Auto-Learn error: {}", e);
                        std::process::exit(1);
                    }
                }

                "exec" => {
                    let store_arc = Store::open().map(Arc::new).ok();
                    let session_arc = store_arc.as_ref().map(|s| {
                        let session = s.find_latest_session().unwrap_or_else(SessionState::new);
                        Arc::new(Mutex::new(session))
                    });
                    if let Err(e) = cli::exec::run_exec(&args, store_arc, session_arc) {
                        eprintln!("[omni] Exec error: {}", e);
                        std::process::exit(1);
                    }
                }

                "rewrite" => {
                    if let Err(_e) = cli::rewrite::run_rewrite(&args) {
                        std::process::exit(1); // Standard silent fail for rewrite hook
                    }
                }

                "reset" => {
                    if let Err(e) = cli::reset::run(&args) {
                        eprintln!("[omni] Reset error: {}", e);
                        std::process::exit(1);
                    }
                }

                "doctor" => {
                    if let Err(e) = cli::doctor::run(&args) {
                        eprintln!("[omni] Doctor error: {}", e);
                        std::process::exit(1);
                    }
                }

                unknown => {
                    eprintln!(
                        "omni: unknown command '{}'\nRun 'omni help' for usage.",
                        unknown
                    );
                    std::process::exit(1);
                }
            }
        }
    }
}
