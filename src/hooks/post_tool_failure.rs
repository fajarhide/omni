use crate::pipeline::SessionState;
use crate::store::sqlite::Store;
use serde::Deserialize;
use std::sync::{Arc, Mutex};

#[derive(Deserialize)]
struct HookInput {
    #[serde(rename = "hookEventName")]
    hook_event_name: String,
    #[serde(rename = "tool_name", default)]
    tool_name: String,
    #[serde(rename = "tool_input")]
    tool_input: Option<ToolInput>,
    #[serde(rename = "tool_response")]
    tool_response: Option<ToolResponse>,
}

#[derive(Deserialize)]
struct ToolInput {
    command: Option<String>,
}

#[derive(Deserialize)]
struct ToolResponse {
    stderr: Option<String>,
    error: Option<String>,
    content: Option<String>,
}

pub fn process_payload(
    input_str: &str,
    store: Arc<Store>,
    session: Arc<Mutex<SessionState>>,
) -> Option<String> {
    let parsed: HookInput = serde_json::from_str(input_str).ok()?;

    if parsed.hook_event_name != "PostToolUseFailure" {
        return None;
    }

    let command = parsed
        .tool_input
        .as_ref()
        .and_then(|i| i.command.as_deref())
        .unwrap_or(&parsed.tool_name);

    // Extract error message
    let error_msg = parsed
        .tool_response
        .as_ref()
        .and_then(|r| {
            r.stderr
                .as_deref()
                .filter(|s| !s.is_empty())
                .or(r.error.as_deref())
                .or(r.content.as_deref())
        })
        .unwrap_or("unknown error");

    let short_error = error_msg
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or(error_msg);
    let short_error = &short_error[..short_error.len().min(200)];

    // Update session state with error
    if let Ok(mut state) = session.lock() {
        state.add_error(short_error);
        state.add_command(command);
        store.upsert_session(&state);
    }

    // Index failure to FTS5 for searchability
    let index_msg = format!(
        "ToolFailure [{}]: {}",
        &command[..command.len().min(50)],
        short_error
    );
    if let Ok(state) = session.lock() {
        store.index_event(&state.session_id, "PostToolUseFailure", &index_msg);
    }

    // PostToolUseFailure never needs to return a response — just side effects
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    fn get_store() -> (Arc<Store>, tempfile::TempDir) {
        let dir = tempdir().expect("tempdir");
        let db = dir.path().join("omni.db");
        (Arc::new(Store::open_path(&db).expect("store")), dir)
    }

    #[test]
    fn test_failure_adds_error_to_session() {
        let (store, _dir) = get_store();
        let session = Arc::new(Mutex::new(SessionState::new()));

        let input = json!({
            "hookEventName": "PostToolUseFailure",
            "tool_name": "Bash",
            "tool_input": { "command": "cargo build" },
            "tool_response": { "stderr": "error[E0308]: mismatched types" }
        });

        let out = process_payload(&input.to_string(), store, session.clone());
        assert!(out.is_none());

        let state = session.lock().unwrap();
        assert!(!state.active_errors.is_empty());
        assert!(state.active_errors[0].contains("E0308"));
    }

    #[test]
    fn test_failure_ignores_wrong_event() {
        let (store, _dir) = get_store();
        let session = Arc::new(Mutex::new(SessionState::new()));
        let input = json!({ "hookEventName": "PostToolUse" });
        let out = process_payload(&input.to_string(), store, session);
        assert!(out.is_none());
    }
}
