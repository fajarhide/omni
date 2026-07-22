pub mod agents;
pub mod analytics;
mod cli;
mod distillers;
mod graph;
mod guard;
mod hooks;
mod mcp;
mod paths;
pub mod pipeline;
mod session;
mod store;
mod util;

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
    Cli,
}

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "omni",
    version = env!("CARGO_PKG_VERSION"),
    about = "Less noise. More signal. Right signal.",
    long_about = None,
    disable_help_subcommand = true,
)]
struct OmniArgs {
    #[arg(long, hide = true)]
    hook: bool,
    #[arg(long, hide = true)]
    post_hook: bool,
    #[arg(long, hide = true)]
    mcp: bool,
    #[arg(long = "session-start", hide = true)]
    session_start: bool,
    #[arg(long = "before-agent-start", hide = true)]
    before_agent_start: bool,
    #[arg(long = "pre-compact", hide = true)]
    pre_compact: bool,
    #[arg(long = "pre-hook", hide = true)]
    pre_hook: bool,

    #[command(subcommand)]
    command: Option<OmniCommand>,
}

#[derive(Subcommand, Debug)]
enum OmniCommand {
    /// Setup OMNI Hooks and MCP server
    #[command(trailing_var_arg = true, disable_help_flag = true)]
    Init {
        #[arg(allow_hyphen_values = true, num_args = 0..)]
        extra: Vec<String>,
    },
    /// View token savings analytics
    #[command(trailing_var_arg = true, disable_help_flag = true)]
    Stats {
        #[arg(allow_hyphen_values = true, num_args = 0..)]
        extra: Vec<String>,
    },
    /// Manage session state
    #[command(alias = "sessions", trailing_var_arg = true, disable_help_flag = true)]
    Session {
        #[arg(allow_hyphen_values = true, num_args = 0..)]
        extra: Vec<String>,
    },
    /// Engram
    #[command(alias = "engrams", trailing_var_arg = true, disable_help_flag = true)]
    Engram {
        #[arg(allow_hyphen_values = true, num_args = 0..)]
        extra: Vec<String>,
    },
    /// Auto-generate filters from history
    #[command(trailing_var_arg = true, disable_help_flag = true)]
    Learn {
        #[arg(allow_hyphen_values = true, num_args = 0..)]
        extra: Vec<String>,
    },
    /// Query distillation history (OmniQL)
    #[command(trailing_var_arg = true, disable_help_flag = true)]
    Query {
        #[arg(allow_hyphen_values = true, num_args = 0..)]
        extra: Vec<String>,
    },
    /// View recurring error patterns
    #[command(trailing_var_arg = true, disable_help_flag = true)]
    Patterns {
        #[arg(allow_hyphen_values = true, num_args = 0..)]
        extra: Vec<String>,
    },
    /// Store important knowledge to persistent memory
    #[command(trailing_var_arg = true)]
    Remember {
        #[arg(allow_hyphen_values = true, num_args = 0..)]
        extra: Vec<String>,
    },
    /// Set or view the project goal (North Star context pinning)
    #[command(trailing_var_arg = true, disable_help_flag = true)]
    Goal {
        #[arg(allow_hyphen_values = true, num_args = 0..)]
        extra: Vec<String>,
    },
    /// Execute a command with OMNI distillation
    #[command(trailing_var_arg = true)]
    Exec {
        /// Command and arguments to execute
        #[arg(allow_hyphen_values = true, num_args = 0..)]
        cmd_args: Vec<String>,
    },
    /// Diagnose installation health
    #[command(trailing_var_arg = true, disable_help_flag = true)]
    Doctor {
        #[arg(allow_hyphen_values = true, num_args = 0..)]
        extra: Vec<String>,
    },
    /// Clean uninstall (for backups config)
    Reset,
    /// Compare last original input vs distilled
    #[command(trailing_var_arg = true)]
    Diff {
        #[arg(allow_hyphen_values = true, num_args = 0..)]
        extra: Vec<String>,
    },
    /// Upgrade OMNI to latest
    #[command(trailing_var_arg = true, disable_help_flag = true)]
    Update {
        #[arg(allow_hyphen_values = true, num_args = 0..)]
        extra: Vec<String>,
    },
    /// View version and environment info
    #[command(trailing_var_arg = true)]
    Version {
        #[arg(allow_hyphen_values = true, num_args = 0..)]
        extra: Vec<String>,
    },

    // Fallback for passing unknown args to subcommands
    #[command(external_subcommand)]
    External(Vec<String>),
}

fn detect_pipe_command() -> Option<String> {
    env::var("OMNI_CMD").ok().or_else(|| env::var("CMD").ok())
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
    println!(
        "  {: <12} Query distillation history (OmniQL)",
        "query".cyan()
    );
    println!("  {: <12} View recurring error patterns", "patterns".cyan());

    println!("\n{}", "UTILITIES:".bold().bright_white());
    println!("  {: <12} Diagnose installation health", "doctor".cyan());
    println!(
        "  {: <12} Clean uninstall (for backups config)",
        "reset".cyan()
    );
    println!(
        "  {: <12} Compare last original input vs distilled",
        "diff".cyan()
    );
    println!("  {: <12} Upgrade OMNI to latest", "update".cyan());
    println!(
        "  {: <12} View version and environment info",
        "version".cyan()
    );
    println!("  {: <12} Show this help message", "help, -h".cyan());

    println!("\n{}", "EXAMPLES:".bold().bright_white());
    println!(
        "  omni init             {}",
        "# OMNI setup (interactive)".bright_black()
    );
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

    if let Some(latest) = crate::guard::update::check() {
        crate::guard::update::print_notification(&latest);
    }
}

// ─── Main ───────────────────────────────────────────────

fn main() {
    // Initialize observability
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr) // Write to stderr to avoid polluting stdout
        .try_init()
        .ok(); // Ignore if already initialized (e.g. in tests)

    let args: Vec<String> = env::args().collect();

    // Fast-path pipe mode
    if args.len() == 1 && !io::stdin().is_terminal() {
        let store_arc = Store::open().map(Arc::new).ok();
        let session_arc = store_arc.as_ref().map(|s| {
            let session = s.find_latest_session().unwrap_or_else(SessionState::new);
            Arc::new(Mutex::new(session))
        });
        let cmd_name = detect_pipe_command();
        if let Err(e) = hooks::pipe::run(store_arc, session_arc, cmd_name.as_deref()) {
            eprintln!("[omni] Pipe engine error: {}", e);
            std::process::exit(1);
        }
        return;
    }

    // Parse CLI arguments with clap
    let parsed = match OmniArgs::try_parse() {
        Ok(p) => p,
        Err(e) => {
            // Because we use allow_external_subcommands, this error only happens
            // for invalid global flags.
            e.print().expect("failed to print error");
            std::process::exit(1);
        }
    };

    let mode = if parsed.hook || parsed.post_hook {
        Mode::PostHook
    } else if parsed.mcp {
        Mode::Mcp
    } else if parsed.session_start || parsed.before_agent_start {
        Mode::SessionStart
    } else if parsed.pre_compact {
        Mode::PreCompact
    } else if parsed.pre_hook {
        Mode::PreHook
    } else {
        Mode::Cli
    };

    match mode {
        Mode::PostHook => {
            let (store, session) = init_globals();
            if let (Some(s), Some(ss)) = (store, session) {
                let _ = hooks::dispatcher::run(s, ss);
            }
        }

        Mode::PreHook => {
            let (store, session) = init_globals();
            if let Err(e) = hooks::pre_tool::run(store, session) {
                eprintln!("[omni] Pre-Hook error: {}", e);
                std::process::exit(1);
            }
        }

        Mode::SessionStart => {
            // Legacy flag — route through dispatcher
            let (store, session) = init_globals();
            if let (Some(s), Some(ss)) = (store, session) {
                // Background cleanup to prevent DB bloating
                let s_clone = Arc::clone(&s);
                std::thread::spawn(move || {
                    /// Number of days to retain session history in the database
                    const SESSION_RETENTION_DAYS: u32 = 30;
                    s_clone.cleanup_old(SESSION_RETENTION_DAYS);
                });
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

        Mode::Cli => {
            let cmd = parsed.command;

            match cmd {
                Some(OmniCommand::Version { .. }) => {
                    cli::version::run_version(&args);
                }
                None => {
                    print_help();
                }
                Some(OmniCommand::Diff { .. }) => {
                    if let Err(e) = cli::diff::run_diff(&args) {
                        eprintln!("[omni] Diff error: {}", e);
                        std::process::exit(1);
                    }
                }
                Some(OmniCommand::Init { .. }) => {
                    // Not `let _ =`: a rejected flag has to reach the user, or
                    // `omni init --curser` installs nothing and exits 0 (#151).
                    if let Err(e) = cli::init::run_init(&args) {
                        eprintln!("[omni] Init error: {}", e);
                        std::process::exit(1);
                    }
                }
                Some(OmniCommand::Reset) => {
                    if let Err(e) = cli::reset::handle_reset() {
                        eprintln!("[omni] Reset error: {}", e);
                        std::process::exit(1);
                    }
                }
                Some(OmniCommand::Stats { .. }) => match Store::open() {
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
                Some(OmniCommand::Session { .. }) => match Store::open() {
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
                Some(OmniCommand::Engram { .. }) => match Store::open() {
                    Ok(store) => {
                        let store_arc = Arc::new(store);
                        if let Err(e) = cli::engram::run_engram(&args, store_arc) {
                            eprintln!("[omni] Engram error: {}", e);
                            std::process::exit(1);
                        }
                    }
                    Err(e) => {
                        eprintln!("[omni] Cannot open database for engrams: {}", e);
                        std::process::exit(1);
                    }
                },
                Some(OmniCommand::Remember { extra }) => match Store::open() {
                    Ok(store) => {
                        let store_arc = Arc::new(store);
                        if let Err(e) = cli::remember::run(&extra, store_arc) {
                            eprintln!("[omni] Remember error: {}", e);
                            std::process::exit(1);
                        }
                    }
                    Err(e) => {
                        eprintln!("[omni] Cannot open database for remember: {}", e);
                        std::process::exit(1);
                    }
                },
                Some(OmniCommand::Goal { extra }) => match Store::open() {
                    Ok(store) => {
                        if let Err(e) = cli::goal::run(&extra, &store) {
                            eprintln!("[omni] Goal error: {}", e);
                            std::process::exit(1);
                        }
                    }
                    Err(e) => {
                        eprintln!("[omni] Cannot open database for goal: {}", e);
                        std::process::exit(1);
                    }
                },
                Some(OmniCommand::Learn { .. }) => {
                    if let Err(e) = cli::learn::run_learn(&args) {
                        eprintln!("[omni] Auto-Learn error: {}", e);
                        std::process::exit(1);
                    }
                }
                Some(OmniCommand::Query { .. }) => match Store::open() {
                    Ok(store) => {
                        if let Err(e) = cli::query::run_query(&args, &store) {
                            eprintln!("[omni] Query error: {}", e);
                            std::process::exit(1);
                        }
                    }
                    Err(e) => {
                        eprintln!("[omni] Cannot open database for query: {}", e);
                        std::process::exit(1);
                    }
                },
                Some(OmniCommand::Patterns { .. }) => match Store::open() {
                    Ok(store) => {
                        if let Err(e) = cli::patterns::run_patterns(&args, &store) {
                            eprintln!("[omni] Patterns error: {}", e);
                            std::process::exit(1);
                        }
                    }
                    Err(e) => {
                        eprintln!("[omni] Cannot open database for patterns: {}", e);
                        std::process::exit(1);
                    }
                },
                Some(OmniCommand::Exec { .. }) => {
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
                Some(OmniCommand::Doctor { .. }) => {
                    if let Err(e) = cli::doctor::run(&args) {
                        eprintln!("[omni] Doctor error: {}", e);
                        std::process::exit(1);
                    }
                }
                Some(OmniCommand::Update { .. }) => {
                    if let Err(e) = cli::update::run(&args) {
                        eprintln!("[omni] Update error: {}", e);
                        std::process::exit(1);
                    }
                }
                Some(OmniCommand::External(_ext_args)) => {
                    let cmd_name = args.get(1).map(|s| s.as_str()).unwrap_or("help");
                    match cmd_name {
                        "version" | "-v" | "--version" => cli::version::run_version(&args),
                        "help" | "-h" | "--help" => print_help(),
                        "diff" => {
                            if let Err(e) = cli::diff::run_diff(&args) {
                                eprintln!("[omni] Diff error: {}", e);
                                std::process::exit(1);
                            }
                        }
                        "init" => {
                            let _ = cli::init::run_init(&args);
                        }
                        "reset" => {
                            if let Err(e) = cli::reset::handle_reset() {
                                eprintln!("[omni] Reset error: {}", e);
                                std::process::exit(1);
                            }
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
                        "session" | "sessions" => match Store::open() {
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
                        "engram" | "engrams" => match Store::open() {
                            Ok(store) => {
                                let store_arc = Arc::new(store);
                                if let Err(e) = cli::engram::run_engram(&args, store_arc) {
                                    eprintln!("[omni] Engram error: {}", e);
                                    std::process::exit(1);
                                }
                            }
                            Err(e) => {
                                eprintln!("[omni] Cannot open database for engrams: {}", e);
                                std::process::exit(1);
                            }
                        },
                        "learn" => {
                            if let Err(e) = cli::learn::run_learn(&args) {
                                eprintln!("[omni] Auto-Learn error: {}", e);
                                std::process::exit(1);
                            }
                        }
                        "query" => match Store::open() {
                            Ok(store) => {
                                if let Err(e) = cli::query::run_query(&args, &store) {
                                    eprintln!("[omni] Query error: {}", e);
                                    std::process::exit(1);
                                }
                            }
                            Err(e) => {
                                eprintln!("[omni] Cannot open database for query: {}", e);
                                std::process::exit(1);
                            }
                        },
                        "patterns" => match Store::open() {
                            Ok(store) => {
                                if let Err(e) = cli::patterns::run_patterns(&args, &store) {
                                    eprintln!("[omni] Patterns error: {}", e);
                                    std::process::exit(1);
                                }
                            }
                            Err(e) => {
                                eprintln!("[omni] Cannot open database for patterns: {}", e);
                                std::process::exit(1);
                            }
                        },
                        "exec" => {
                            let store_arc = Store::open().map(Arc::new).ok();
                            let session_arc = store_arc.as_ref().map(|s| {
                                let session =
                                    s.find_latest_session().unwrap_or_else(SessionState::new);
                                Arc::new(Mutex::new(session))
                            });
                            if let Err(e) = cli::exec::run_exec(&args, store_arc, session_arc) {
                                eprintln!("[omni] Exec error: {}", e);
                                std::process::exit(1);
                            }
                        }
                        "doctor" => {
                            if let Err(e) = cli::doctor::run(&args) {
                                eprintln!("[omni] Doctor error: {}", e);
                                std::process::exit(1);
                            }
                        }
                        "update" => {
                            if let Err(e) = cli::update::run(&args) {
                                eprintln!("[omni] Update error: {}", e);
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
    }
}
