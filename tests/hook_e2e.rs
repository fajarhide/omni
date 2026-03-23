use std::io::Write;
/// Hook E2E tests — spawn the omni binary as a child process.
use std::process::{Command, Stdio};

fn omni_binary() -> String {
    // Cargo sets this during `cargo test`
    env!("CARGO_BIN_EXE_omni").to_string()
}

#[test]
fn test_hook_e2e_git_diff() {
    let fixture =
        std::fs::read_to_string("tests/fixtures/git_diff_multi_file.txt").expect("fixture missing");

    let mock_input = serde_json::json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": "git diff HEAD~1"},
        "tool_response": {
            "content": fixture
        }
    });

    let mut child = Command::new(omni_binary())
        .arg("--hook")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn omni");

    child
        .stdin
        .take()
        .unwrap()
        .write_all(mock_input.to_string().as_bytes())
        .expect("Failed to write stdin");

    let output = child.wait_with_output().expect("Failed to wait");

    // Hook should exit cleanly (0)
    assert!(
        output.status.success(),
        "Hook exited with non-zero: {:?}",
        output.status
    );

    // Output should be valid JSON
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.trim().is_empty() {
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(stdout.trim());
        assert!(parsed.is_ok(), "Hook output is not valid JSON: {}", stdout);
    }
}

#[test]
fn test_hook_non_bash_exit_clean() {
    let mock_input = serde_json::json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Read",
        "tool_input": {"file_path": "/tmp/foo.txt"},
        "tool_response": {
            "content": "file contents here"
        }
    });

    let mut child = Command::new(omni_binary())
        .arg("--hook")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn omni");

    child
        .stdin
        .take()
        .unwrap()
        .write_all(mock_input.to_string().as_bytes())
        .unwrap();

    let output = child.wait_with_output().unwrap();
    // Non-Bash tools should still exit cleanly
    assert!(
        output.status.success(),
        "Non-Bash hook should exit 0, got {:?}",
        output.status
    );
}

#[test]
fn test_hook_invalid_json_exit_clean() {
    let mut child = Command::new(omni_binary())
        .arg("--hook")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn omni");

    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"this is not json at all {{{")
        .unwrap();

    let output = child.wait_with_output().unwrap();
    // Invalid JSON should exit cleanly (graceful degradation)
    assert!(
        output.status.success(),
        "Invalid JSON should exit 0, got {:?}",
        output.status
    );
}

#[test]
fn test_hook_short_content_exit_clean() {
    let mock_input = serde_json::json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": "pwd"},
        "tool_response": {
            "content": "/Users/test"
        }
    });

    let mut child = Command::new(omni_binary())
        .arg("--hook")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn omni");

    child
        .stdin
        .take()
        .unwrap()
        .write_all(mock_input.to_string().as_bytes())
        .unwrap();

    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "Short content hook should exit 0");

    // Short content should either pass through or produce minimal output
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.trim().is_empty() {
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(stdout.trim());
        assert!(
            parsed.is_ok(),
            "Output must be valid JSON if present: {}",
            stdout
        );
    }
}

#[test]
fn test_pipe_mode_via_binary() {
    let input = "diff --git a/foo.rs b/foo.rs\n@@ -1,2 +1,2 @@\n-old line\n+new line\n";

    let mut child = Command::new(omni_binary())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn omni");

    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();

    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "Pipe mode should exit 0");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.is_empty(), "Pipe mode should produce output");
}

#[test]
fn test_cli_version() {
    let output = Command::new(omni_binary())
        .arg("version")
        .output()
        .expect("Failed to run");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("omni"),
        "Version output should contain 'omni'"
    );
}

#[test]
fn test_cli_help() {
    let output = Command::new(omni_binary())
        .arg("help")
        .output()
        .expect("Failed to run");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("stats"), "Help should list stats command");
    assert!(stdout.contains("doctor"), "Help should list doctor command");
    assert!(stdout.contains("learn"), "Help should list learn command");
    assert!(
        stdout.contains("session"),
        "Help should list session command"
    );
}

#[test]
fn test_cli_unknown_command() {
    let output = Command::new(omni_binary())
        .arg("nonexistent-cmd-xyz")
        .output()
        .expect("Failed to run");

    // Unknown command should exit non-zero
    assert!(
        !output.status.success(),
        "Unknown command should exit non-zero"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unknown command"),
        "Should show unknown command error"
    );
}

#[test]
fn test_cli_doctor_runs() {
    let output = Command::new(omni_binary())
        .arg("doctor")
        .output()
        .expect("Failed to run");

    assert!(output.status.success(), "Doctor should exit 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("OMNI Doctor"), "Doctor should show header");
}

#[test]
fn test_cli_stats_no_crash() {
    let temp_db = std::env::temp_dir().join("omni_test_stats.db");
    if temp_db.exists() {
        let _ = std::fs::remove_file(&temp_db);
    }

    let output = Command::new(omni_binary())
        .arg("stats")
        .env("OMNI_DB_PATH", &temp_db)
        .output()
        .expect("Failed to run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "Stats should exit 0. Stderr: {}",
        stderr
    );
    assert!(
        stdout.contains("Signal Report") || stdout.contains("OMNI"),
        "Stats should show report header. Stdout: {}\nStderr: {}",
        stdout,
        stderr
    );

    if temp_db.exists() {
        let _ = std::fs::remove_file(&temp_db);
    }
}
