use anyhow::Result;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use crate::hooks::pipe::{run_inner, stream_filter_for};
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

    let (mut child, cmd_name) = if needs_shell {
        #[cfg(target_family = "windows")]
        let mut c = Command::new("cmd");
        #[cfg(target_family = "windows")]
        c.arg("/C");

        #[cfg(not(target_family = "windows"))]
        let mut c = Command::new("sh");
        #[cfg(not(target_family = "windows"))]
        c.arg("-c");

        let c = c
            .env_clear()
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

    let mut child_stdout = child.stdout.take().expect("Failed to open stdout");

    let status = if stream_filter_for(&cmd_name).is_some() {
        // Stream-mode command: distilled output is emitted line-by-line as it
        // arrives, before the exit code is known, so the exit-code gate below
        // cannot apply. Keep the live-streaming behavior.
        let stdout = std::io::stdout().lock();
        let stderr = std::io::stderr().lock();
        run_inner(
            child_stdout,
            stdout,
            stderr,
            store.clone(),
            session.clone(),
            Some(&cmd_name),
        )?;
        child.wait()?
    } else {
        // Buffered path: drain stdout first (draining before wait() avoids the
        // classic full-pipe deadlock), then gate on the real exit code. #122: a
        // command that exited non-zero passes its stdout through verbatim and is
        // never distilled — distillation must not turn a failure into output that
        // reads as success.
        let mut buf = Vec::new();
        child_stdout.read_to_end(&mut buf)?;
        let status = child.wait()?;
        if status.success() {
            let stdout = std::io::stdout().lock();
            let stderr = std::io::stderr().lock();
            run_inner(
                std::io::Cursor::new(&buf),
                stdout,
                stderr,
                store.clone(),
                session.clone(),
                Some(&cmd_name),
            )?;
        } else {
            let mut stdout = std::io::stdout().lock();
            stdout.write_all(&buf)?;
            stdout.flush()?;
        }
        status
    };

    if !status.success() {
        if let (Some(sess), Some(st)) = (&session, &store) {
            let tracker = crate::session::tracker::SessionTracker::new(sess.clone(), st.clone());
            tracker.track_error(""); // stderr is inherited, so we just track the failure
        }
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}
