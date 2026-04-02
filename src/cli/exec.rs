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
        #[cfg(target_family = "windows")]
        let mut c = Command::new("cmd");
        #[cfg(target_family = "windows")]
        c.arg("/C");

        #[cfg(not(target_family = "windows"))]
        let mut c = Command::new("sh");
        #[cfg(not(target_family = "windows"))]
        c.arg("-c");

        let c = c.env_clear()
            .envs(crate::guard::env::sanitize_env())
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
            .env_clear()
            .envs(crate::guard::env::sanitize_env())
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
        store.clone(),
        session.clone(),
        Some(&cmd_name),
    )?;

    if !output.status.success() {
        if let (Some(sess), Some(st)) = (&session, &store) {
            let tracker = crate::session::tracker::SessionTracker::new(sess.clone(), st.clone());
            tracker.track_error(&String::from_utf8_lossy(&output.stderr));
        }
        std::process::exit(output.status.code().unwrap_or(1));
    }

    Ok(())
}
