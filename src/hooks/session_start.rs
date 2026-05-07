use crate::agents::multiagent;
use crate::pipeline::SessionState;
use crate::store::sqlite::Store;
use crate::store::transcript;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Deserialize)]
struct HookInput {
    #[serde(rename = "hookEventName", alias = "hook_event_name")]
    hook_event_name: String,
    #[serde(rename = "sessionId", alias = "session_id")]
    session_id: String,
    #[serde(rename = "workingDirectory", alias = "working_directory")]
    #[serde(alias = "cwd")]
    working_directory: String,
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
    #[serde(rename = "watchPaths", default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub watch_paths: Vec<String>,
}

pub struct SessionConfig {
    pub force_fresh: bool,
    pub force_continue: bool,
    pub ttl_mins: i64,
}

impl SessionConfig {
    pub fn from_env() -> Self {
        Self {
            force_fresh: std::env::var("OMNI_FRESH")
                .map(|v| v == "1")
                .unwrap_or(false),
            force_continue: std::env::var("OMNI_CONTINUE")
                .map(|v| v == "1")
                .unwrap_or(false),
            ttl_mins: std::env::var("OMNI_SESSION_TTL")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(240),
        }
    }
}

pub fn process_payload(input_str: &str, store: Arc<Store>, cfg: SessionConfig) -> Option<String> {
    let parsed: HookInput = match serde_json::from_str(input_str) {
        Ok(p) => p,
        Err(_) => {
            eprintln!("[omni] parse error");
            return None;
        }
    };

    if parsed.hook_event_name != "SessionStart" {
        return None;
    }

    let now = Utc::now().timestamp();
    let mut should_continue = false;
    let mut prev_state: Option<SessionState> = None;

    if !cfg.force_fresh
        && let Some(state) = store.find_latest_session()
    {
        let age_mins = (now - state.last_active) / 60;
        if cfg.force_continue || age_mins < cfg.ttl_mins {
            should_continue = true;
            prev_state = Some(state);
        }
    }

    if should_continue && let Some(state) = prev_state {
        let cwd_for_ctx = if parsed.working_directory.is_empty() {
            std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".to_string())
        } else {
            parsed.working_directory.clone()
        };

        // Auto-sync agent session for multi-agent awareness
        let proj_hash = multiagent::project_hash(&cwd_for_ctx);
        let agent_id = multiagent::detect_agent_id();
        let state_json = serde_json::to_string(&state).unwrap_or_else(|_| "{}".to_string());
        store.sync_agent_session(&agent_id, &state.session_id, &proj_hash, &state_json);

        let summary = build_summary_with_context(&state, now, &store, &cwd_for_ctx);
        let mut summary_truncated = summary.trim().to_string();
        if summary_truncated.len() > 800 {
            summary_truncated.truncate(797);
            summary_truncated.push_str("...");
        }

        store.index_event(
            &state.session_id,
            "SessionStart",
            "Continued previous session",
        );

        let out = HookOutput {
            hook_specific_output: HookSpecificOutput {
                hook_event_name: "SessionStart".to_string(),
                system_prompt_addition: summary_truncated,
                watch_paths: vec![],
            },
        };

        return serde_json::to_string(&out).ok();
    }

    // Fresh session logic
    let mut new_state = SessionState::new();

    // Initialize transcript for new session
    let cwd = if parsed.working_directory.is_empty() {
        std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string())
    } else {
        parsed.working_directory.clone()
    };
    let cwd_path = std::path::Path::new(&cwd);

    if let Some(pm) = crate::session::tracker::detect_js_toolchain(cwd_path) {
        new_state.toolchain_hints.insert("js".to_string(), pm);
    }
    if let Some(pm) = crate::session::tracker::detect_rust_toolchain(cwd_path) {
        new_state.toolchain_hints.insert("rust".to_string(), pm);
    }
    if let Some(pm) = crate::session::tracker::detect_python_toolchain(cwd_path) {
        new_state.toolchain_hints.insert("python".to_string(), pm);
    }

    // Detect watch paths for file monitoring
    let watch_paths = detect_watch_paths(cwd_path, &new_state.toolchain_hints);

    store.upsert_session(&new_state);
    let start_msg = format!("Fresh session started (Client ID: {})", parsed.session_id);
    store.index_event(&new_state.session_id, "SessionStart", &start_msg);

    let t = transcript::Transcript::new(&new_state.session_id, &cwd);
    let _ = t.save();

    // Cleanup old transcripts (7 days)
    transcript::cleanup_old(7);

    // Only check for interrupted sessions when not forcing fresh
    if !cfg.force_fresh
        && let Some(pending) = transcript::find_pending()
        && pending.session_id != new_state.session_id
    {
        let summary = format!(
            "OMNI: Interrupted session detected. {}",
            pending.interrupted_summary()
        );
        let out = HookOutput {
            hook_specific_output: HookSpecificOutput {
                hook_event_name: "SessionStart".to_string(),
                system_prompt_addition: summary,
                watch_paths: watch_paths.clone(),
            },
        };
        return serde_json::to_string(&out).ok();
    }

    // Fresh session: only return output if we have watchPaths to register
    if !watch_paths.is_empty() {
        let out = HookOutput {
            hook_specific_output: HookSpecificOutput {
                hook_event_name: "SessionStart".to_string(),
                system_prompt_addition: String::new(),
                watch_paths,
            },
        };
        return serde_json::to_string(&out).ok();
    }

    None
}

/// Auto-detect critical project files to watch based on toolchain
fn detect_watch_paths(
    cwd: &std::path::Path,
    toolchain: &std::collections::HashMap<String, String>,
) -> Vec<String> {
    let mut paths: Vec<String> = vec![];

    if toolchain.contains_key("rust") {
        paths.push("Cargo.toml".to_string());
        paths.push("Cargo.lock".to_string());
    }
    if toolchain.contains_key("js") {
        paths.push("package.json".to_string());
        paths.push("tsconfig.json".to_string());
        paths.push("package-lock.json".to_string());
    }
    if toolchain.contains_key("python") {
        paths.push("pyproject.toml".to_string());
        paths.push("requirements.txt".to_string());
    }
    if cwd.join("go.mod").exists() {
        paths.push("go.mod".to_string());
    }
    if cwd.join("CLAUDE.md").exists() {
        paths.push("CLAUDE.md".to_string());
    }
    if cwd.join(".omni").join("filters").exists() {
        paths.push(".omni/filters/".to_string());
    }
    if cwd.join("Makefile").exists() {
        paths.push("Makefile".to_string());
    }

    paths.truncate(10);
    paths
}

fn build_summary(state: &SessionState, now: i64) -> String {
    let age_mins = (now - state.last_active) / 60;
    let time_str = if age_mins < 60 {
        format!("{}m ago", age_mins)
    } else {
        format!("{}h ago", age_mins / 60)
    };

    let mut out = format!("OMNI: Session continued ({}). ", time_str);

    if let Some(task) = &state.inferred_task {
        out.push_str(&format!("Last: {}. ", task));
    } else if let Some(domain) = &state.inferred_domain {
        out.push_str(&format!("Last: working on {}. ", domain));
    } else if let Some(last_cmd) = state.last_commands.first() {
        out.push_str(&format!("Last: ran `{}`. ", last_cmd));
    }

    let mut hot_vec: Vec<(&String, &u32)> = state.hot_files.iter().collect();
    hot_vec.sort_by_key(|a| std::cmp::Reverse(a.1));
    let top_files: Vec<String> = hot_vec
        .iter()
        .take(3)
        .map(|(path, count)| format!("{} ({}x)", path, count))
        .collect();
    if !top_files.is_empty() {
        out.push_str(&format!("Hot: {}. ", top_files.join(", ")));
    }

    if let Some(err) = state.active_errors.first() {
        let clean_err = err.replace('\n', " ").chars().take(80).collect::<String>();
        out.push_str(&format!("Last error: {}. ", clean_err));
    }

    if state.estimated_tokens_saved() > 0 {
        out.push_str(&format!(
            "OMNI saved ~{}tok last session. ",
            state.estimated_tokens_saved()
        ));
    }

    out
}

fn build_summary_with_context(state: &SessionState, now: i64, store: &Store, cwd: &str) -> String {
    let mut out = build_summary(state, now);

    // Inject peer agent context (multi-agent awareness)
    if let Some(peer_ctx) = multiagent::build_peer_context(store, cwd) {
        out.push_str(&peer_ctx);
    }

    // Inject cross-session project knowledge
    if let Some(knowledge_ctx) = multiagent::build_knowledge_context(store, cwd) {
        out.push_str(&knowledge_ctx);
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
        // Set transcript dir to clean temp dir so find_pending() doesn't interfere
        let transcript_dir = dir.path().join("transcripts");
        crate::store::transcript::MOCK_TRANSCRIPT_DIR.with(|d| {
            *d.borrow_mut() = Some(transcript_dir);
        });
        (
            Arc::new(Store::open_path(&db_path).expect("must succeed")),
            dir,
        )
    }

    fn default_config() -> SessionConfig {
        SessionConfig {
            force_fresh: false,
            force_continue: false,
            ttl_mins: 240,
        }
    }

    #[test]
    fn fresh_session_exits_without_output() {
        let (store, _dir) = get_store();
        let input = json!({
            "hookEventName": "SessionStart",
            "sessionId": "123",
            "workingDirectory": "/tmp"
        });

        let out = process_payload(&input.to_string(), store, default_config());
        assert!(out.is_none());
    }

    #[test]
    fn continue_session_injects_summary() {
        let (store, _dir) = get_store();

        let mut state = SessionState::new();
        state.add_command("cargo test");
        state.add_error("missing semicolon");
        state.add_hot_file("src/main.rs");
        store.upsert_session(&state);

        let input = json!({
            "hookEventName": "SessionStart",
            "sessionId": "456",
            "workingDirectory": "/tmp"
        });

        let mut cfg = default_config();
        cfg.force_continue = true;

        let out = process_payload(&input.to_string(), store.clone(), cfg);
        assert!(out.is_some());
        let res = out.expect("must succeed");
        assert!(res.contains("systemPromptAddition"));
        assert!(res.contains("missing semicolon"));
        assert!(res.contains("src/main.rs (1x)"));
    }

    #[test]
    fn session_summary_is_within_length_limit() {
        let (store, _dir) = get_store();

        let mut state = SessionState::new();
        state.add_hot_file(&"A".repeat(400));
        state.add_error(&"B".repeat(400));
        store.upsert_session(&state);

        let input = json!({
            "hookEventName": "SessionStart",
            "sessionId": "789",
            "workingDirectory": "/tmp"
        });

        let mut cfg = default_config();
        cfg.force_continue = true;

        let out = process_payload(&input.to_string(), store.clone(), cfg);
        assert!(out.is_some());

        let parsed: HookOutput =
            serde_json::from_str(&out.expect("must succeed")).expect("must succeed");
        let summary_len = parsed.hook_specific_output.system_prompt_addition.len();
        assert!(summary_len <= 800, "Length was {}", summary_len);
    }

    #[test]
    fn force_fresh_overrides_continue() {
        let (store, _dir) = get_store();
        let state = SessionState::new();
        store.upsert_session(&state);

        let input = json!({
            "hookEventName": "SessionStart",
            "sessionId": "AAA",
            "workingDirectory": "/tmp"
        });

        let mut cfg = default_config();
        cfg.force_fresh = true;
        cfg.force_continue = true; // force_fresh should override

        let out = process_payload(&input.to_string(), store.clone(), cfg);
        assert!(out.is_none());
    }

    #[test]
    fn expired_sessions_are_treated_as_fresh() {
        let (store, _dir) = get_store();
        let mut state = SessionState::new();
        state.last_active = Utc::now().timestamp() - (500 * 60); // 500 minutes ago
        store.upsert_session(&state);

        let input = json!({
            "hookEventName": "SessionStart",
            "sessionId": "BBB",
            "workingDirectory": "/tmp"
        });

        let mut cfg = default_config();
        cfg.ttl_mins = 240;

        let out = process_payload(&input.to_string(), store.clone(), cfg);
        // Should drop and treat as fresh
        assert!(out.is_none());
    }

    #[test]
    fn parse_errors_do_not_crash() {
        let (store, _dir) = get_store();
        let out = process_payload("NOT JSON", store, default_config());
        assert!(out.is_none());
    }

    #[test]
    fn accepts_claude_code_cwd_alias() {
        // Claude Code sends "cwd" not "workingDirectory" — this must not produce a parse error
        let (store, _dir) = get_store();
        // Claude Code sends "cwd" and snake_case field names (from actual hook transcripts)
        let input = json!({
            "hook_event_name": "SessionStart",
            "session_id": "4ba52c00-c43f-46ed-9e0e-9069d5294302",
            "transcript_path": "/home/user/.claude/projects/test/session.jsonl",
            "cwd": "/home/user/project",
            "source": "startup",
            "model": "claude-sonnet-4-6"
        });

        let out = process_payload(&input.to_string(), store.clone(), default_config());
        // Fresh session with no toolchain → no watch_paths → None output is correct
        // The important thing is: no "[omni] parse error", session IS written to DB
        assert!(out.is_none());
        // Verify the session was actually persisted
        assert!(
            store.find_latest_session().is_some(),
            "session must be written to DB when cwd alias is used"
        );
    }

    #[test]
    fn session_summary_preserves_context() {
        let (store, _dir) = get_store();
        let mut state = SessionState::new();
        state.add_hot_file("secret.txt");
        state.add_command("cat secret.txt");
        store.upsert_session(&state);

        let input = json!({
            "hookEventName": "SessionStart",
            "sessionId": "CCC",
            "workingDirectory": "/tmp"
        });

        let mut cfg = default_config();
        cfg.force_continue = true;
        let out = process_payload(&input.to_string(), store.clone(), cfg);
        assert!(out.is_some());
        // Since we explicitly added hot_file = secret.txt, it naturally tracks it.
        // No magic regex scrubbing mandated yet, so let it assert the correct structure is appended.
        let output = out.expect("must succeed");
        assert!(output.contains("Last: ran `cat secret.txt`"));
    }
}
