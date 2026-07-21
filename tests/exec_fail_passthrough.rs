//! #122: `omni exec` must pass a failed command's stdout through verbatim and
//! never distill it — the same invariant #120 enforced on the hook path, applied
//! where the exit code comes from the wrapped child rather than the agent JSON.
//!
//! Unix-only: the reproduction drives a POSIX `sh` loop. On Windows `omni exec`
//! wraps commands in `cmd /C`, which does not speak this syntax; the gate itself
//! is OS-agnostic (`cli::exec`), only the test's shell script is not.
#![cfg(unix)]

use std::process::Command;

fn omni() -> String {
    env!("CARGO_BIN_EXE_omni").to_string()
}

/// Run `omni exec <script>` with an isolated DB. The script is passed as a single
/// argument on purpose: `omni exec` re-wraps a shell command in `sh -c`, so
/// `omni exec sh -c '…'` would double-wrap and mangle the quotes.
fn run_exec(script: &str) -> (String, i32) {
    let db = tempfile::NamedTempFile::new().expect("temp db");
    let out = Command::new(omni())
        .arg("exec")
        .arg(script)
        .env("OMNI_DB_PATH", db.path())
        .env("OMNI_QUIET", "1")
        .output()
        .expect("spawn omni exec");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        out.status.code().unwrap_or(-1),
    )
}

// 60 identical noisy lines: `collapse` folds them to one marker on a *successful*
// run, so their survival is a direct signal that distillation was skipped.
const NOISE_LOOP: &str = "i=0; while [ $i -lt 60 ]; do echo noise noise noise; i=$((i+1)); done;";

#[test]
fn failed_command_passes_through_verbatim() {
    let (stdout, code) = run_exec(&format!("{NOISE_LOOP} exit 1"));

    assert_eq!(code, 1, "the child's non-zero exit code must propagate");
    assert_eq!(
        stdout.lines().filter(|l| l.contains("noise")).count(),
        60,
        "a failed command must pass through verbatim, not be collapsed"
    );
    assert!(
        !stdout.contains("collapsed"),
        "a failed command must carry no distillation marker"
    );
}

#[test]
fn successful_command_is_still_distilled() {
    // Guards the guard: the identical output with a zero exit is still distilled,
    // so the fix did not simply disable `omni exec` distillation.
    let (stdout, code) = run_exec(&format!("{NOISE_LOOP} exit 0"));

    assert_eq!(code, 0);
    assert!(
        stdout.contains("collapsed"),
        "a successful command should still be distilled"
    );
}
