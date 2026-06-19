use anyhow::Result;
use serde_json::Value;

use crate::pipeline::SessionState;

#[allow(dead_code)]
pub trait StorageBackend: Send + Sync {
    /// Save session state
    fn insert_session(&self, state: &SessionState) -> Result<()>;

    /// Fetch latest active session
    fn find_latest_session(&self) -> Option<SessionState>;

    /// Find session by ID
    fn find_session_by_id(&self, session_id: &str) -> Option<SessionState>;

    /// Find active session for specific agent
    fn find_active_agent_session(&self, agent_id: &str) -> Option<SessionState>;

    /// Record distillation metrics and store hook contents
    fn record_distillation(
        &self,
        input: &str,
        output: &str,
        command: &str,
        exit_code: i32,
        duration_ms: u64,
        session_id: &str,
    ) -> Result<String>;

    /// Get overall database stats (sessions, rewinds)
    fn stats(&self) -> Result<(usize, usize)>;

    /// Check if Full Text Search is enabled
    fn check_fts5(&self) -> bool;

    /// Rebuild search indices
    fn rebuild_fts(&self) -> Result<()>;

    /// Query raw history returning JSON rows
    fn query_history(&self, sql: &str, params: &[&dyn rusqlite::ToSql]) -> Result<Vec<Value>>;
}
