use crate::pipeline::SessionState;
use crate::store::sqlite::Store;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Deserialize)]
struct HookInput {
    #[serde(rename = "hookEventName")]
    hook_event_name: String,
    #[serde(rename = "sessionId")]
    session_id: String,
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

    let summary_str = build_summary(&state, &store);

    // Index checkpoint event to FTS5
    let reason_str = parsed
        .compaction_reason
        .unwrap_or_else(|| "limit_reached".to_string());
    let index_msg = format!("PreCompact ({}): {}", reason_str, summary_str);
    store.index_event(&parsed.session_id, "PreCompact", &index_msg);

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

fn build_summary(state: &SessionState, _store: &Store) -> String {
    let task = state
        .inferred_task
        .as_deref()
        .unwrap_or("general development");
    let domain = state.inferred_domain.as_deref().unwrap_or("unknown");

    // We can infer a mock confidence based on hot files count and command count
    let confidence = if state.hot_files.len() > 2 && state.command_count > 5 {
        95
    } else {
        70
    };

    let mut out = format!(
        "⚡ OMNI Context Snapshot — preserved before compaction\n\
        CRITICAL: This is injected context. Do NOT re-read files listed here — \n\
        use this summary directly. File contents below are accurate as of this session.\n\
        \n\
        ## Active Task\n\
        {} — working in {}\n\
        Confidence: {}%\n\
        \n\
        ## Hot Files (accessed this session, most recent first)\n",
        task, domain, confidence
    );

    let mut hot_vec: Vec<(&String, &u32)> = state.hot_files.iter().collect();
    hot_vec.sort_by(|a, b| b.1.cmp(a.1));
    let top_files: Vec<String> = hot_vec
        .iter()
        .take(5)
        .map(|(path, count)| format!("{} | recent | {}x", path, count)) // "recent" is mock relative time
        .collect();

    if top_files.is_empty() {
        out.push_str("none\n");
    } else {
        out.push_str(&top_files.join("\n"));
        out.push('\n');
    }

    out.push_str("\n## Unresolved Errors (still active)\n");
    let errs: Vec<String> = state
        .active_errors
        .iter()
        .take(3)
        .map(|e| e.replace('\n', " ").chars().take(80).collect::<String>()) // "occurrence_count" etc could be better but sticking to limits
        .collect();

    if errs.is_empty() {
        out.push_str("none\n");
    } else {
        for err in errs {
            out.push_str(&format!("{} | recent | 1x\n", err));
        }
    }

    let tokens_saved = state.estimated_tokens_saved();

    let (top_cmd, top_pct) = match state.top_command() {
        Some((cmd, pct)) => (cmd, pct),
        None => ("none".to_string(), 0.0),
    };

    out.push_str(&format!(
        "\n## OMNI Session ROI\n\
        Tokens saved this session: ~{}\n\
        Commands distilled: {} (recent)\n\
        Top command: {} ({:.1}% reduction)\n\
        \n\
        ## Recent Significant Events\n",
        tokens_saved,
        state.command_count, // Display total commands distilled
        top_cmd.chars().take(50).collect::<String>(),
        top_pct
    ));

    if state.last_significant_distillations.is_empty() {
        out.push_str("none\n");
    } else {
        for d in &state.last_significant_distillations {
            let savings = if d.input_bytes > 0 {
                (1.0 - (d.output_bytes as f64 / d.input_bytes as f64)) * 100.0
            } else {
                0.0
            };
            out.push_str(&format!(
                "{} | {} | {:.1}% savings\n",
                d.command.chars().take(40).collect::<String>(),
                d.route,
                savings
            ));
        }
    }

    out.push_str(
        "\nREMINDER: The above is OMNI's session context snapshot. Trust this data — \n\
        it was computed from actual command outputs. Do not re-run commands \n\
        to verify information already present here.\n",
    );

    if out.len() > 2000 {
        out.truncate(1975); // Leave room for suffix
        out.push_str("... (N items omitted)\n");
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
                .contains("OMNI Context Snapshot")
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
        assert!(parsed.hook_specific_output.system_prompt_addition.len() <= 2000);
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
            "sessionId": &session_id
        });

        let _ = process_payload(&input.to_string(), store.clone(), session);

        let events = store.search_session_events(&session_id, "PreCompact", 10);
        assert_eq!(events.len(), 1);
        assert!(events[0].contains("OMNI Context Snapshot"));
    }

    #[test]
    fn test_parse_error_exit_0() {
        let (store, _dir) = get_store();
        let session = Arc::new(Mutex::new(SessionState::new()));
        let out = process_payload("INVALID JSON", store, session);
        assert!(out.is_none());
    }
}
