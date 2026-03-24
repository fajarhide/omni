use anyhow::Result;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use crate::hooks::pipe::run_inner;
use crate::pipeline::SessionState;
use crate::store::sqlite::Store;

pub fn run_exec(
    args: &[String],
    store: Option<Arc<Store>>,
    session: Option<Arc<Mutex<SessionState>>>,
) -> Result<()> {
    if args.len() < 3 {
        eprintln!("Usage: omni exec <command> [args...]");
        std::process::exit(1);
    }

    let cmd = &args[2];
    let cmd_args = &args[3..];

    let child_res = Command::new(cmd)
        .args(cmd_args)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit()) // Let stderr bypass OMNI directly to the terminal
        .spawn();

    let child = match child_res {
        Ok(c) => c,
        Err(e) => {
            eprintln!("omni exec: failed to execute '{}': {}", cmd, e);
            std::process::exit(1);
        }
    };

    let output = child.wait_with_output()?;

    let stdout = std::io::stdout().lock();
    let stderr = std::io::stderr().lock();

    // Pipe the stdout of the child process through OMNI's pipeline
    run_inner(
        &output.stdout[..],
        stdout,
        stderr,
        store,
        session,
        Some(cmd),
    )?;

    if !output.status.success() {
        std::process::exit(output.status.code().unwrap_or(1));
    }

    Ok(())
}
