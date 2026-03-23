use crate::pipeline::SessionState;
use crate::store::sqlite::Store;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Deserialize)]
struct HookInput {
    #[serde(rename = "hookEventName")]
    hook_event_name: String,
    #[allow(dead_code)]
    #[serde(rename = "sessionId")]
    session_id: String,
    #[allow(dead_code)]
    #[serde(rename = "compactionReason")]
    compaction_reason: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct HookOutput {
    #[serde(rename = "hookSpecificOutput")]
    pub hook_specific_output: HookSpecificOutput,
}

#[derive(Serialize, Deserialize)]
pub struct HookSpecificOutput {
    #[serde(rename = "hookEventName")]
    pub hook_event_name: String,
    #[serde(rename = "systemPromptAddition")]
    pub system_prompt_addition: String,
}

pub fn process_payload(
    input_str: &str,
    store: Arc<Store>,
    session: Arc<Mutex<SessionState>>,
) -> Option<String> {
    let parsed: HookInput = match serde_json::from_str(input_str) {
        Ok(p) => p,
        Err(_) => {
            eprintln!("[omni] parse error");
            return None;
        }
    };

    if parsed.hook_event_name != "PreCompact" {
        return None;
    }

    let mut state = match session.lock() {
        Ok(s) => s,
        Err(_) => return None,
    };

    let summary_str = build_compact_summary(&state, &store);

    // Index checkpoint event to FTS5
    let index_msg = format!("PreCompact: {}", summary_str);
    store.index_event(&state.session_id, "PreCompact", &index_msg);

    // Save updated session state
    state.last_active = Utc::now().timestamp();
    store.upsert_session(&state);

    let out = HookOutput {
        hook_specific_output: HookSpecificOutput {
            hook_event_name: "PreCompact".to_string(),
            system_prompt_addition: summary_str,
        },
    };

    serde_json::to_string(&out).ok()
}

fn build_compact_summary(state: &SessionState, store: &Store) -> String {
    let now = Utc::now().format("%H:%M:%S").to_string();
    let task = state
        .inferred_task
        .as_deref()
        .unwrap_or("general development");
    let domain = state.inferred_domain.as_deref().unwrap_or("unknown");

    let mut out = format!(
        "OMNI Checkpoint [{}]:\nTask: {}\nDomain: {}\n",
        now, task, domain
    );

    let mut hot_vec: Vec<(&String, &u32)> = state.hot_files.iter().collect();
    hot_vec.sort_by(|a, b| b.1.cmp(a.1));
    let top_files: Vec<String> = hot_vec
        .iter()
        .take(5)
        .map(|(path, count)| format!("{} ({}x)", path, count))
        .collect();

    if top_files.is_empty() {
        out.push_str("Hot files: none\n");
    } else {
        out.push_str(&format!("Hot files: {}\n", top_files.join(", ")));
    }

    let errs: Vec<String> = state
        .active_errors
        .iter()
        .take(3)
        .map(|e| e.replace('\n', " ").chars().take(80).collect::<String>())
        .collect();

    if errs.is_empty() {
        out.push_str("Active errors: none\n");
    } else {
        out.push_str(&format!("Active errors: {}\n", errs.join(" | ")));
    }

    let (count, pct) = match store.get_summary(0) {
        Ok(sum) => {
            let savings = if sum.total_input_bytes > 0 {
                (1.0 - sum.total_output_bytes as f64 / sum.total_input_bytes as f64) * 100.0
            } else {
                0.0
            };
            (sum.total_distillations, savings)
        }
        Err(_) => (0, 0.0),
    };

    out.push_str(&format!(
        "Session stats: {} commands distilled, {:.1}% avg savings",
        count, pct
    ));

    if out.len() > 500 {
        out.truncate(497);
        out.push_str("...");
    }

    out
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
    fn test_pre_compact_output_valid_json_format() {
        let (store, _dir) = get_store();
        let session = Arc::new(Mutex::new(SessionState::new()));

        let input = json!({
            "hookEventName": "PreCompact",
            "sessionId": "123",
            "compactionReason": "context_limit_reached"
        });

        let out_str = process_payload(&input.to_string(), store, session).expect("must succeed");
        let parsed: HookOutput = serde_json::from_str(&out_str).expect("must succeed");
        assert_eq!(parsed.hook_specific_output.hook_event_name, "PreCompact");
        assert!(
            parsed
                .hook_specific_output
                .system_prompt_addition
                .contains("OMNI Checkpoint")
        );
    }

    #[test]
    fn test_compact_summary_leq_500_chars() {
        let (store, _dir) = get_store();
        let mut state = SessionState::new();
        state.add_hot_file(&"A".repeat(300));
        state.add_error(&"B".repeat(300));
        let session = Arc::new(Mutex::new(state));

        let input = json!({
            "hookEventName": "PreCompact",
            "sessionId": "123"
        });

        let out_str = process_payload(&input.to_string(), store, session).expect("must succeed");
        let parsed: HookOutput = serde_json::from_str(&out_str).expect("must succeed");
        assert!(parsed.hook_specific_output.system_prompt_addition.len() <= 500);
    }

    #[test]
    fn test_compact_summary_mengandung_hot_files() {
        let (store, _dir) = get_store();
        let mut state = SessionState::new();
        state.add_hot_file("src/main.rs");
        state.add_hot_file("src/lib.rs");
        let session = Arc::new(Mutex::new(state));

        let input = json!({
            "hookEventName": "PreCompact",
            "sessionId": "123"
        });

        let out_str = process_payload(&input.to_string(), store, session).expect("must succeed");
        assert!(out_str.contains("src/main.rs"));
        assert!(out_str.contains("src/lib.rs"));
    }

    #[test]
    fn test_compact_summary_mengandung_active_errors() {
        let (store, _dir) = get_store();
        let mut state = SessionState::new();
        state.add_error("missing semicolon at line 42");
        let session = Arc::new(Mutex::new(state));

        let input = json!({
            "hookEventName": "PreCompact",
            "sessionId": "123"
        });

        let out_str = process_payload(&input.to_string(), store, session).expect("must succeed");
        assert!(out_str.contains("missing semicolon at line 42"));
    }

    #[test]
    fn test_session_state_disave_setelah_compact() {
        let (store, _dir) = get_store();
        let state = SessionState::new();
        let session_id = state.session_id.clone();
        let session = Arc::new(Mutex::new(state));

        let input = json!({
            "hookEventName": "PreCompact",
            "sessionId": &session_id
        });

        // Trigger the hook
        let _ = process_payload(&input.to_string(), store.clone(), session);

        // Verify state is saved in the DB
        let latest = store.find_latest_session().expect("must succeed");
        assert_eq!(latest.session_id, session_id);
    }

    #[test]
    fn test_fts5_indexing_pada_checkpoint() {
        let (store, _dir) = get_store();
        let state = SessionState::new();
        let session_id = state.session_id.clone();
        let session = Arc::new(Mutex::new(state));

        let input = json!({
            "hookEventName": "PreCompact",
            "sessionId": "123"
        });

        let _ = process_payload(&input.to_string(), store.clone(), session);

        let events = store.search_session_events(&session_id, "PreCompact", 10);
        assert_eq!(events.len(), 1);
        assert!(events[0].contains("OMNI Checkpoint"));
    }

    #[test]
    fn test_parse_error_exit_0() {
        let (store, _dir) = get_store();
        let session = Arc::new(Mutex::new(SessionState::new()));
        let out = process_payload("INVALID JSON", store, session);
        assert!(out.is_none());
    }
}
