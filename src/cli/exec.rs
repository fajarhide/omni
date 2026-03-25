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

    // Detect if we need to run via shell
    let needs_shell = cmd_args.iter().any(|arg| {
        arg.contains('|')
            || arg.contains('>')
            || arg.contains('<')
            || arg.contains('&')
            || arg.contains(';')
    }) || cmd.contains('|')
        || cmd.contains('>')
        || cmd.contains('<')
        || cmd.contains('&')
        || cmd.contains(';');

    let full_cmd = if cmd_args.is_empty() {
        cmd.to_string()
    } else {
        format!("{} {}", cmd, cmd_args.join(" "))
    };

    let (child, cmd_name) = if needs_shell {
        let c = Command::new("sh")
            .arg("-c")
            .arg(&full_cmd)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| {
                anyhow::anyhow!(
                    "omni exec: failed to execute shell command '{}': {}",
                    full_cmd,
                    e
                )
            })?;
        (c, full_cmd)
    } else {
        let c = Command::new(cmd)
            .args(cmd_args)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| anyhow::anyhow!("omni exec: failed to execute '{}': {}", cmd, e))?;
        (c, full_cmd)
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
        Some(&cmd_name),
    )?;

    if !output.status.success() {
        std::process::exit(output.status.code().unwrap_or(1));
    }

    Ok(())
}
