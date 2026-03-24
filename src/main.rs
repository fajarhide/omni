#![allow(dead_code, unused_variables, unused_imports)]

mod cli;
mod distillers;
mod guard;
mod hooks;
mod mcp;
pub mod pipeline;
mod session;
mod store;

use std::env;
use std::io::{self, IsTerminal};
use std::sync::{Arc, Mutex};

use crate::pipeline::SessionState;
use crate::store::sqlite::Store;

// ─── Mode Detection ─────────────────────────────────────

#[derive(Debug, PartialEq)]
enum Mode {
    Hook,
    Mcp,
    SessionStart,
    PreCompact,
    Pipe,
    Cli,
}

fn detect_mode(args: &[String]) -> Mode {
    if args.len() > 1 {
        match args[1].as_str() {
            "--hook" => return Mode::Hook,
            "--mcp" => return Mode::Mcp,
            "--session-start" => return Mode::SessionStart,
            "--pre-compact" => return Mode::PreCompact,
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
    println!(
        r#"omni {} — Less noise. More signal. Right signal.

USAGE:
  omni [MODE] [COMMAND] [FLAGS]

MODES (automatic, no user interaction needed):
  --hook          PostToolUse/SessionStart/PreCompact hook
  --mcp           MCP server mode

COMMANDS:
  init            Setup OMNI hooks and MCP
  stats           Token savings analytics
  session         Session state management
  learn           Auto-generate filters from passthrough
  reset           Backup ~/.omni config for a clean uninstall
  doctor          Diagnose installation
  version         Print version
  help            Print this help

PIPE MODE (automatic):
  command | omni  Distil command output

Quick start:
  omni init --hook   # Setup Claude Code hooks
  omni init --mcp    # Setup MCP for Claude Desktop
  omni stats         # View savings after first session"#,
        env!("CARGO_PKG_VERSION")
    );
}

// ─── Main ───────────────────────────────────────────────

fn main() {
    let args: Vec<String> = env::args().collect();
    let mode = detect_mode(&args);

    match mode {
        Mode::Hook => {
            let (store, session) = init_globals();
            if let (Some(s), Some(ss)) = (store, session) {
                let _ = hooks::dispatcher::run(s, ss);
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
            if let Err(e) = hooks::pipe::run(store_arc, session_arc) {
                eprintln!("[omni] Pipe engine error: {}", e);
                std::process::exit(1);
            }
        }

        Mode::Cli => {
            let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("help");

            match cmd {
                "version" => {
                    println!("omni {}", env!("CARGO_PKG_VERSION"));
                }

                "help" | "--help" | "-h" => {
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

                "reset" => {
                    if let Err(e) = cli::reset::run() {
                        eprintln!("[omni] Reset error: {}", e);
                        std::process::exit(1);
                    }
                }

                "doctor" => {
                    if let Err(e) = cli::doctor::run() {
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
