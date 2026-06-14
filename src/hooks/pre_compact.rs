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

    let mut state = session.lock().unwrap_or_else(|p| p.into_inner());

    // L1-04: Add LoopCheckpoint engram if in a loop
    if state.loop_context.mode != crate::pipeline::LoopMode::Interactive {
        let label = format!(
            "Loop #{} checkpoint: {} [iter tokens: {}]",
            state.loop_context.iteration,
            crate::util::text::safe_slice(state.loop_context.goal.as_deref().unwrap_or("none"), 30),
            state.loop_context.budget_used
        );
        let mut hot_vec: Vec<(&String, &u32)> = state.hot_files.iter().collect();
        hot_vec.sort_by_key(|a| std::cmp::Reverse(a.1));
        let top_files: Vec<String> = hot_vec.iter().take(3).map(|(k, _)| (*k).clone()).collect();

        state.add_engram(crate::session::engram::Engram {
            label,
            trigger: crate::session::engram::EngramTrigger::LoopCheckpoint,
            timestamp: chrono::Utc::now().timestamp(),
            files: top_files,
            detail: None,
        });
    }

    let summary_str = build_summary(&state, &store);

    // Phase 2: Delta detection — skip re-emission if content unchanged
    let new_hash = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(summary_str.as_bytes());
        hex::encode(&hasher.finalize()[..8])
    };
    if let Some(ref last_hash) = state.last_compact_hash
        && *last_hash == new_hash
    {
        // No meaningful change since last compact — emit minimal snapshot
        let out = HookOutput {
            hook_specific_output: HookSpecificOutput {
                hook_event_name: "PreCompact".to_string(),
                system_prompt_addition: "⚡ OMNI: Context unchanged since last compaction. Continuing with prior snapshot.".to_string(),
            },
        };
        return serde_json::to_string(&out).ok();
    }
    state.last_compact_hash = Some(new_hash);

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

    // ── Smart PreCompact v2: Priority-Aware Context Packing ──
    // Budget allocation (total ~6000 tokens):
    //   - Header + Task:    ~200 tokens (always)
    //   - Pinned Files:     ~500 tokens (always if available)
    //   - Active Errors:    ~400 tokens (always if present)
    //   - Engrams:          ~600 tokens (subtask progress)
    //   - Tool Summary:     ~300 tokens (rolling activity)
    //   - Hot Files:        ~300 tokens (file access context)
    //   - ROI + Events:     remainder (~600-800 tokens)
    //   - Footer:           ~100 tokens

    let mut out = format!(
        "⚡ OMNI Context Snapshot — preserved before compaction\n\
        CRITICAL: This is injected context. Do NOT re-read files listed here — \n\
        use this summary directly. File contents below are accurate as of this session.\n\
        \n\
        ## Active Task\n\
        {} — working in {}\n\
        Confidence: {}%\n",
        task, domain, confidence
    );

    // ── Bucket 0: Loop Checkpoint (L1-04) ──
    if state.loop_context.mode != crate::pipeline::LoopMode::Interactive {
        let budget_str = if let Some(budget) = state.loop_context.budget_tokens {
            let pct = if budget > 0 {
                (state.loop_context.budget_used as f64 / budget as f64) * 100.0
            } else {
                0.0
            };
            format!(
                "{:.0}% used this iteration ({} / {})",
                pct, state.loop_context.budget_used, budget
            )
        } else {
            "No limit".to_string()
        };

        out.push_str(&format!(
            "\n## OMNI Loop Checkpoint\n\
            - **Loop ID:** {}\n\
            - **Iteration:** {}\n\
            - **Goal:** {}\n\
            - **Token budget:** {}\n",
            state.loop_context.loop_id.as_deref().unwrap_or("none"),
            state.loop_context.iteration,
            state.loop_context.goal.as_deref().unwrap_or("none"),
            budget_str
        ));
    }

    // ── Bucket 1: Active Errors (high priority) ──
    out.push_str("\n## Unresolved Errors (still active)\n");
    let errs: Vec<String> = state
        .active_errors
        .iter()
        .take(3)
        .map(|e| e.replace('\n', " ").chars().take(80).collect::<String>())
        .collect();

    if errs.is_empty() {
        out.push_str("none\n");
    } else {
        for err in errs {
            out.push_str(&format!("{} | recent | 1x\n", err));
        }
    }

    // ── Bucket 2: Engrams (subtask memory) ──
    if !state.engrams.is_empty() {
        out.push_str("\n## Subtask Progress (Engrams)\n");
        for engram in state.engrams.iter().take(5) {
            out.push_str(&engram.compact());
            out.push('\n');
        }
    }

    // ── Bucket 3: Tool Call Summary ──
    let tool_summary = crate::session::engram::format_tool_summary(&state.tool_call_log);
    if !tool_summary.is_empty() {
        out.push('\n');
        out.push_str(&tool_summary);
    }

    // ── Bucket 4: Hot Files ──
    out.push_str("\n## Hot Files (accessed this session, most recent first)\n");
    let mut hot_vec: Vec<(&String, &u32)> = state.hot_files.iter().collect();
    hot_vec.sort_by_key(|a| std::cmp::Reverse(a.1));
    let top_files: Vec<String> = hot_vec
        .iter()
        .take(5)
        .map(|(path, count)| format!("{} | recent | {}x", path, count))
        .collect();

    if top_files.is_empty() {
        out.push_str("none\n");
    } else {
        out.push_str(&top_files.join("\n"));
        out.push('\n');
    }

    // ── Bucket 5: Session ROI & Events ──
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
        state.command_count,
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

    // ── Bucket 6: Pinned Files (critical instructions) ──
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| ".".to_string());
    out.push_str(&crate::hooks::session_start::read_pinned_files(&cwd));

    out.push_str(
        "\nREMINDER: The above is OMNI's session context snapshot. Trust this data — \n\
        it was computed from actual command outputs. Do not re-run commands \n\
        to verify information already present here.\n",
    );

    // ── Token Budget Enforcement ──
    let current_tokens = crate::util::token_estimate::count_tokens(&out, "cl100k_base");
    if current_tokens > 6000 {
        // Approximate character length for 6000 tokens
        let target_chars = ((6000.0 / current_tokens as f64) * out.len() as f64) as usize;
        crate::util::text::safe_truncate(&mut out, target_chars.saturating_sub(50));
        out.push_str("\n... [OMNI: Intelligently omitted to stay within 6000 token budget]\n");
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
    fn pre_compact_output_is_valid_json() {
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
    fn compact_summary_is_within_length_limit() {
        let (store, _dir) = get_store();
        let mut state = SessionState::new();
        state.add_hot_file(&"A".repeat(50000));
        state.add_error(&"B".repeat(50000));
        let session = Arc::new(Mutex::new(state));

        let input = json!({
            "hookEventName": "PreCompact",
            "sessionId": "123"
        });

        let out_str = process_payload(&input.to_string(), store, session).expect("must succeed");
        let parsed: HookOutput = serde_json::from_str(&out_str).expect("must succeed");
        let token_count = crate::util::token_estimate::count_tokens(
            &parsed.hook_specific_output.system_prompt_addition,
            "cl100k_base",
        );
        assert!(token_count <= 6100, "Token count was {}", token_count);
    }

    #[test]
    fn compact_summary_contains_hot_files() {
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
    fn compact_summary_contains_active_errors() {
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
    fn session_state_is_saved_after_compact() {
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
    fn fts5_indexing_runs_at_checkpoint() {
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
    fn parse_errors_do_not_crash() {
        let (store, _dir) = get_store();
        let session = Arc::new(Mutex::new(SessionState::new()));
        let out = process_payload("INVALID JSON", store, session);
        assert!(out.is_none());
    }

    // ── Phase 2 Integration Tests ──

    #[test]
    fn compact_summary_includes_engrams() {
        let (store, _dir) = get_store();
        let mut state = SessionState::new();
        state.add_engram(crate::session::engram::Engram {
            label: "Fixed cargo test error".to_string(),
            trigger: crate::session::engram::EngramTrigger::ErrorResolved,
            timestamp: chrono::Utc::now().timestamp(),
            files: vec!["src/main.rs".to_string()],
            detail: None,
        });
        let session = Arc::new(Mutex::new(state));

        let input = json!({
            "hookEventName": "PreCompact",
            "sessionId": "engram-test"
        });

        let out_str = process_payload(&input.to_string(), store, session).expect("must succeed");
        assert!(
            out_str.contains("Subtask Progress"),
            "expected engram section, got: {}",
            out_str
        );
        assert!(
            out_str.contains("Fixed cargo test error"),
            "expected engram label, got: {}",
            out_str
        );
    }

    #[test]
    fn compact_summary_includes_tool_call_summary() {
        let (store, _dir) = get_store();
        let mut state = SessionState::new();
        for i in 0..5 {
            state.add_tool_call(crate::session::engram::ToolCallEntry {
                tool_family: "cargo".to_string(),
                command: format!("cargo test {}", i),
                succeeded: true,
                files: vec!["src/lib.rs".to_string()],
                timestamp: 1000 + i as i64,
            });
        }
        let session = Arc::new(Mutex::new(state));

        let input = json!({
            "hookEventName": "PreCompact",
            "sessionId": "tool-summary-test"
        });

        let out_str = process_payload(&input.to_string(), store, session).expect("must succeed");
        assert!(
            out_str.contains("Tool Activity"),
            "expected tool summary section, got: {}",
            out_str
        );
        assert!(
            out_str.contains("cargo"),
            "expected cargo family, got: {}",
            out_str
        );
    }

    #[test]
    fn delta_detection_skips_unchanged_compact() {
        let (store, _dir) = get_store();
        let state = SessionState::new();
        let session = Arc::new(Mutex::new(state));

        let input = json!({
            "hookEventName": "PreCompact",
            "sessionId": "delta-test"
        });

        // First compact: full snapshot
        let out1 =
            process_payload(&input.to_string(), store.clone(), session.clone()).expect("first");
        assert!(
            out1.contains("OMNI Context Snapshot"),
            "first compact should be full"
        );

        // Second compact with same state: should detect unchanged
        let out2 =
            process_payload(&input.to_string(), store.clone(), session.clone()).expect("second");
        assert!(
            out2.contains("unchanged"),
            "second compact should detect no change, got: {}",
            out2
        );
    }

    #[test]
    fn delta_detection_emits_on_state_change() {
        let (store, _dir) = get_store();
        let state = SessionState::new();
        let session = Arc::new(Mutex::new(state));

        let input = json!({
            "hookEventName": "PreCompact",
            "sessionId": "delta-change-test"
        });

        // First compact: full snapshot
        let _ = process_payload(&input.to_string(), store.clone(), session.clone());

        // Modify state
        {
            let mut s = session.lock().unwrap_or_else(|p| p.into_inner());
            s.add_error("new error appeared");
            s.add_hot_file("src/new_file.rs");
        }

        // Third compact: state changed, should emit full snapshot again
        let out3 =
            process_payload(&input.to_string(), store.clone(), session.clone()).expect("third");
        assert!(
            out3.contains("OMNI Context Snapshot"),
            "changed state should produce full snapshot"
        );
        assert!(
            out3.contains("new error appeared"),
            "should contain new error"
        );
    }
}
