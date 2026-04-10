use crate::hooks::{post_tool, pre_compact, session_start};
use crate::pipeline::SessionState;
use crate::store::sqlite::Store;
use crate::store::transcript::{Transcript, TranscriptEntry};
use serde::Deserialize;
use std::io::{self, Read};
use std::sync::{Arc, Mutex};

#[derive(Deserialize)]
struct HookPeeker {
    #[serde(rename = "hookEventName", alias = "hook_event_name")]
    hook_event_name: Option<String>,
}

pub fn run(store: Arc<Store>, session: Arc<Mutex<SessionState>>) -> anyhow::Result<()> {
    let session_clone = session.clone();
    match std::panic::catch_unwind(|| {
        let stdin = io::stdin();
        let mut input_str = String::new();
        if stdin
            .take(16 * 1024 * 1024)
            .read_to_string(&mut input_str)
            .is_err()
        {
            return Ok(());
        }

        if input_str.trim().is_empty() {
            return Ok(());
        }

        let out = process_payload(&input_str, store, session);

        if let Some(res) = out {
            println!("{}", res);
        }

        Ok(())
    }) {
        Ok(res) => res,
        Err(_) => {
            // Transcript: mark failed on panic so crash is recorded
            if let Ok(guard) = session_clone.lock()
                && let Some(mut transcript) = Transcript::load(&guard.session_id)
            {
                let _ = transcript.mark_last_failed("process panicked during hook dispatch");
            }
            Ok(())
        }
    }
}

pub fn process_payload(
    input_str: &str,
    store: Arc<Store>,
    session: Arc<Mutex<SessionState>>,
) -> Option<String> {
    let peeker: HookPeeker = match serde_json::from_str(input_str) {
        Ok(p) => p,
        Err(_) => return None,
    };

    let event_name = peeker.hook_event_name.as_deref().unwrap_or("PostToolUse");

    // Transcript: persist hook payload BEFORE dispatching
    if let Ok(guard) = session.lock() {
        let cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string());
        let mut transcript = Transcript::load_or_new(&guard.session_id, &cwd);
        let entry = TranscriptEntry::new_hook(event_name, input_str);
        let _ = transcript.append_entry(entry);
    }

    let result = match event_name {
        "SessionStart" => {
            let cfg = session_start::SessionConfig::from_env();
            session_start::process_payload(input_str, store, cfg)
        }
        "PreCompact" => pre_compact::process_payload(input_str, store, session.clone()),
        _ => post_tool::process_payload(input_str, Some(store), Some(session.clone())),
    };

    // Transcript: mark completed + snapshot state after dispatch
    if let Ok(guard) = session.lock()
        && let Some(mut transcript) = Transcript::load(&guard.session_id)
    {
        let output_str = result.as_deref().unwrap_or("(no output)");
        let _ = transcript.mark_last_completed(output_str);
        let _ = transcript.snapshot_state(&guard);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    fn get_store() -> (Arc<Store>, tempfile::TempDir) {
        let dir = tempdir().expect("must succeed");
        let db_path = dir.path().join("omni.db");
        (
            Arc::new(Store::open_path(&db_path).expect("must succeed")),
            dir,
        )
    }

    #[test]
    fn test_dispatcher_routes_post_tool_use_ke_correct_handler() {
        let (store, _dir) = get_store();
        let session = Arc::new(Mutex::new(SessionState::new()));

        // Buat input PostToolUse valid
        let diff_str = "diff --git a/test.txt b/test.txt\n--- a/test.txt\n+++ b/test.txt\n@@ -1,1 +1,2 @@\n-old\n+new line 1\n".to_string();
        let mut big_diff = diff_str.clone();
        for _ in 0..50 {
            big_diff.push_str(" \n");
        }

        let input = json!({
            "tool_name": "Bash",
            "tool_input": { "command": "git diff" },
            "tool_response": { "content": big_diff }
        });

        let out = process_payload(&input.to_string(), store, session);
        assert!(out.is_some());
        assert!(out.expect("must succeed").contains("PostToolUse"));
    }

    #[test]
    fn test_dispatcher_routes_session_start_ke_correct_handler() {
        let (store, _dir) = get_store();
        let mut state = SessionState::new();
        state.add_command("cargo build");
        store.upsert_session(&state);

        let session = Arc::new(Mutex::new(SessionState::new())); // dipatcher state doesn't matter much for SessionStart

        let input = json!({
            "hookEventName": "SessionStart",
            "sessionId": "456",
            "workingDirectory": "/tmp"
        });

        let out = process_payload(&input.to_string(), store, session);

        assert!(out.is_some());
        assert!(
            out.expect("must succeed").contains("SessionStart"),
            "Dispatched output must be SessionStart"
        );
    }

    #[test]
    fn test_dispatcher_routes_pre_compact_ke_correct_handler() {
        let (store, _dir) = get_store();
        let session = Arc::new(Mutex::new(SessionState::new()));

        let input = json!({
            "hookEventName": "PreCompact",
            "sessionId": "123",
            "compactionReason": "context_limit_reached"
        });

        let out = process_payload(&input.to_string(), store, session);
        assert!(out.is_some());
        assert!(out.expect("must succeed").contains("PreCompact"));
    }
}
