use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};
use std::sync::Mutex;

use crate::pipeline::{DistillResult, SessionState};

pub struct FilterStats {
    pub filter_name: String,
    pub total_input_bytes: u64,
    pub total_output_bytes: u64,
    pub count: u32,
}

pub struct RouteStats {
    pub route: String,
    pub count: u32,
}

pub struct StoreSummary {
    pub total_sessions: u32,
    pub total_distillations: u32,
    pub total_input_bytes: u64,
    pub total_output_bytes: u64,
    pub by_filter: Vec<FilterStats>,
    pub by_route: Vec<RouteStats>,
    pub passthrough_commands: Vec<(String, u32)>, // (command, count)
}

pub struct Store {
    conn: Mutex<Connection>,
}

impl Store {
    /// Creates dir ~/.omni/ if none exists, Open/create DB, Run schema migrations
    pub fn open() -> Result<Self> {
        let db_path = if let Ok(custom_path) = std::env::var("OMNI_DB_PATH") {
            std::path::PathBuf::from(custom_path)
        } else {
            dirs::home_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join(".omni")
                .join("omni.db")
        };

        Self::open_path(&db_path)
    }

    /// Open a Store at a specific path (used by open() and tests)
    pub fn open_path(path: &std::path::Path) -> Result<Self> {
        let parent = path.parent().unwrap_or_else(|| std::path::Path::new(""));
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).context("Failed to create .omni directory")?;
        }

        let conn = Connection::open(path).context("Failed to open SQLite database")?;

        // Optimizations
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;",
        )?;

        let store = Self {
            conn: Mutex::new(conn),
        };

        store.init_schema()?;
        Ok(store)
    }

    pub fn stats(&self) -> Result<(usize, usize)> {
        let conn = self.conn.lock().unwrap();
        let sessions: usize = conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
            .unwrap_or(0);
        let rewinds: usize = conn
            .query_row("SELECT COUNT(*) FROM rewind_store", [], |row| row.get(0))
            .unwrap_or(0);
        Ok((sessions, rewinds))
    }

    pub fn latest_activity_timestamps(&self) -> Result<(Option<u64>, Option<u64>)> {
        let conn = self.conn.lock().unwrap();
        let s_ts: Option<i64> = conn
            .query_row(
                "SELECT last_active FROM sessions ORDER BY last_active DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .ok()
            .flatten();
        let r_ts: Option<i64> = conn
            .query_row(
                "SELECT ts FROM rewind_store ORDER BY ts DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .ok()
            .flatten();
        Ok((s_ts.map(|v| v as u64), r_ts.map(|v| v as u64)))
    }

    pub fn check_fts5(&self) -> bool {
        let conn = self.conn.lock().unwrap();
        let query =
            "SELECT 1 FROM pragma_compile_options WHERE compile_options LIKE 'ENABLE_FTS5%'";
        conn.query_row(query, [], |row| row.get::<_, i64>(0))
            .is_ok()
    }

    /// Aggregate distillation stats since a given timestamp
    pub fn aggregate_stats(&self, since: i64) -> Result<(u64, u64, u64, u64, i64)> {
        let conn = self.conn.lock().unwrap();
        // returns (count, total_input, total_output, sum_latency, max_latency)
        let r = conn.query_row(
            "SELECT COALESCE(COUNT(*),0), COALESCE(SUM(input_bytes),0), COALESCE(SUM(output_bytes),0), COALESCE(SUM(latency_ms),0), COALESCE(MAX(latency_ms),0) FROM distillations WHERE ts >= ?1",
            params![since],
            |row| Ok((row.get::<_,u64>(0)?, row.get::<_,u64>(1)?, row.get::<_,u64>(2)?, row.get::<_,u64>(3)?, row.get::<_,i64>(4)?)),
        ).unwrap_or((0, 0, 0, 0, 0));
        Ok(r)
    }

    /// Per-filter breakdown: (filter_name, count, avg_reduction_pct)
    pub fn filter_breakdown(&self, since: i64) -> Result<Vec<(String, u64, f64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT filter_name, COUNT(*), 
                    CASE WHEN SUM(input_bytes)=0 THEN 0.0 
                         ELSE ROUND(100.0*(1.0 - CAST(SUM(output_bytes) AS REAL)/SUM(input_bytes)),1) END
             FROM distillations WHERE ts >= ?1 GROUP BY filter_name ORDER BY COUNT(*) DESC"
        )?;
        let rows = stmt
            .query_map(params![since], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, u64>(1)?,
                    row.get::<_, f64>(2)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    /// Route distribution: (route, count)
    pub fn route_distribution(&self, since: i64) -> Result<Vec<(String, u64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT route, COUNT(*) FROM distillations WHERE ts >= ?1 GROUP BY route ORDER BY COUNT(*) DESC"
        )?;
        let rows = stmt
            .query_map(params![since], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?))
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    /// RewindStore metrics: (total_stored, total_retrieved)
    pub fn rewind_metrics(&self) -> Result<(u64, u64)> {
        let conn = self.conn.lock().unwrap();
        let total: u64 = conn
            .query_row("SELECT COUNT(*) FROM rewind_store", [], |row| row.get(0))
            .unwrap_or(0);
        let retrieved: u64 = conn
            .query_row(
                "SELECT COUNT(*) FROM rewind_store WHERE retrieved > 0",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        Ok((total, retrieved))
    }

    /// Passthrough candidates: commands that went through without filter
    pub fn passthrough_candidates(&self, since: i64) -> Result<Vec<(String, u64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT command, COUNT(*) as cnt FROM distillations WHERE ts >= ?1 AND route = 'Passthrough' AND command != '' GROUP BY command ORDER BY cnt DESC LIMIT 10"
        )?;
        let rows = stmt
            .query_map(params![since], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?))
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    /// Hot files for session insight
    pub fn hot_files_global(&self, since: i64) -> Result<Vec<(String, u64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT file_path, SUM(access_count) as cnt FROM file_access WHERE last_access >= ?1 GROUP BY file_path ORDER BY cnt DESC LIMIT 5"
        )?;
        let rows = stmt
            .query_map(params![since], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?))
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            r#"
            -- 1. Sessions
            CREATE TABLE IF NOT EXISTS sessions (
                id           TEXT PRIMARY KEY,
                started_at   INTEGER NOT NULL,
                last_active  INTEGER NOT NULL,
                task_hint    TEXT DEFAULT '',
                domain_hint  TEXT DEFAULT '',
                state_json   TEXT DEFAULT '{}'
            );

            -- 2. Distillation tracking
            CREATE TABLE IF NOT EXISTS distillations (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id   TEXT NOT NULL,
                ts           INTEGER NOT NULL,
                filter_name  TEXT NOT NULL,
                content_type TEXT NOT NULL,
                input_bytes  INTEGER NOT NULL,
                output_bytes INTEGER NOT NULL,
                route        TEXT NOT NULL,
                score        REAL NOT NULL DEFAULT 0.0,
                context_score REAL NOT NULL DEFAULT 0.0,
                latency_ms   INTEGER NOT NULL,
                rewind_hash  TEXT DEFAULT '',
                command      TEXT DEFAULT ''
            );
            CREATE INDEX IF NOT EXISTS idx_dist_ts ON distillations(ts);
            CREATE INDEX IF NOT EXISTS idx_dist_session ON distillations(session_id);
            CREATE INDEX IF NOT EXISTS idx_dist_filter ON distillations(filter_name);

            -- 3. File access
            CREATE TABLE IF NOT EXISTS file_access (
                session_id   TEXT NOT NULL,
                file_path    TEXT NOT NULL,
                access_count INTEGER DEFAULT 1,
                last_access  INTEGER NOT NULL,
                PRIMARY KEY (session_id, file_path)
            );

            -- 4. RewindStore
            CREATE TABLE IF NOT EXISTS rewind_store (
                hash         TEXT PRIMARY KEY,
                content      TEXT NOT NULL,
                ts           INTEGER NOT NULL,
                original_len INTEGER NOT NULL,
                retrieved    INTEGER DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_rewind_ts ON rewind_store(ts);

            -- 5. FTS5 for session events
            CREATE VIRTUAL TABLE IF NOT EXISTS session_events USING fts5(
                session_id UNINDEXED,
                event_type UNINDEXED,
                content,
                ts UNINDEXED,
                tokenize = 'porter ascii'
            );
            "#,
        )?;
        Ok(())
    }

    pub fn record_distillation(&self, session_id: &str, result: &DistillResult, command: &str) {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return,
        };

        let ts = chrono::Utc::now().timestamp();
        let _ = conn.execute(
            "INSERT INTO distillations 
             (session_id, ts, filter_name, content_type, input_bytes, output_bytes, route, score, context_score, latency_ms, rewind_hash, command)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                session_id,
                ts,
                result.filter_name,
                result.content_type.to_string(),
                result.input_bytes,
                result.output_bytes,
                result.route.to_string(),
                result.score,
                result.context_score,
                result.latency_ms,
                result.rewind_hash.as_deref().unwrap_or(""),
                command,
            ],
        );
    }

    pub fn store_rewind(&self, content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let hash_result = hasher.finalize();
        let full_hash = hex::encode(hash_result);
        let short_hash = full_hash[..8].to_string();

        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return short_hash,
        };

        let ts = chrono::Utc::now().timestamp();
        let original_len = content.len() as i64;

        let _ = conn.execute(
            "INSERT OR IGNORE INTO rewind_store (hash, content, ts, original_len, retrieved)
             VALUES (?1, ?2, ?3, ?4, 0)",
            params![short_hash, content, ts, original_len],
        );

        short_hash
    }

    pub fn retrieve_rewind(&self, hash: &str) -> Option<String> {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return None,
        };

        let content: Option<String> = conn
            .query_row(
                "SELECT content FROM rewind_store WHERE hash = ?1",
                params![hash],
                |row| row.get(0),
            )
            .optional()
            .unwrap_or(None);

        if content.is_some() {
            let _ = conn.execute(
                "UPDATE rewind_store SET retrieved = retrieved + 1 WHERE hash = ?1",
                params![hash],
            );
        }

        content
    }

    pub fn delete_session(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM sessions WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn list_recent_sessions(&self, limit: usize) -> Result<Vec<SessionState>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT state_json FROM sessions ORDER BY last_active DESC LIMIT ?1")?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            let json_str: String = row.get(0)?;
            Ok(json_str)
        })?;

        let mut out = Vec::new();
        for r in rows {
            if let Ok(j) = r
                && let Ok(s) = serde_json::from_str::<SessionState>(&j)
            {
                out.push(s);
            }
        }
        Ok(out)
    }

    pub fn upsert_session(&self, state: &SessionState) {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return,
        };

        let state_json = serde_json::to_string(state).unwrap_or_else(|_| "{}".to_string());
        let _ = conn.execute(
            "INSERT OR REPLACE INTO sessions (id, started_at, last_active, task_hint, domain_hint, state_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                state.session_id,
                state.started_at,
                state.last_active,
                state.inferred_task.as_deref().unwrap_or(""),
                state.inferred_domain.as_deref().unwrap_or(""),
                state_json,
            ],
        );
    }

    pub fn load_session(&self, session_id: &str) -> Option<SessionState> {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return None,
        };

        let state_json: Option<String> = conn
            .query_row(
                "SELECT state_json FROM sessions WHERE id = ?1",
                params![session_id],
                |row| row.get(0),
            )
            .optional()
            .unwrap_or(None);

        if let Some(json) = state_json {
            serde_json::from_str(&json).ok()
        } else {
            None
        }
    }

    pub fn find_latest_session(&self) -> Option<SessionState> {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return None,
        };

        let state_json: Option<String> = conn
            .query_row(
                "SELECT state_json FROM sessions ORDER BY last_active DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()
            .unwrap_or(None);

        if let Some(json) = state_json {
            serde_json::from_str(&json).ok()
        } else {
            None
        }
    }

    pub fn index_event(&self, session_id: &str, event_type: &str, content: &str) {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return,
        };

        let ts = chrono::Utc::now().timestamp();
        let _ = conn.execute(
            "INSERT INTO session_events (session_id, event_type, content, ts) VALUES (?1, ?2, ?3, ?4)",
            params![session_id, event_type, content, ts],
        );
    }

    pub fn search_session_events(
        &self,
        session_id: &str,
        query: &str,
        limit: usize,
    ) -> Vec<String> {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return vec![],
        };

        let mut stmt = match conn.prepare(
            "SELECT content FROM session_events 
             WHERE session_id = ?1 AND session_events MATCH ?2 
             ORDER BY rank LIMIT ?3",
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let event_iter = match stmt.query_map(params![session_id, query, limit], |row| row.get(0)) {
            Ok(iter) => iter,
            Err(_) => return vec![],
        };

        let mut results = Vec::new();
        for content in event_iter.flatten() {
            results.push(content);
        }
        results
    }

    pub fn get_summary(&self, since_secs: i64) -> Result<StoreSummary> {
        let conn = self.conn.lock().unwrap();

        let ts_threshold = chrono::Utc::now().timestamp() - since_secs;

        let total_sessions: u32 = conn.query_row(
            "SELECT COUNT(*) FROM sessions WHERE started_at >= ?1",
            params![ts_threshold],
            |row| row.get(0),
        )?;

        let (total_distillations, total_input_bytes, total_output_bytes): (u32, i64, i64) = conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(input_bytes), 0), COALESCE(SUM(output_bytes), 0) FROM distillations WHERE ts >= ?1",
            params![ts_threshold],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;

        let mut by_filter = Vec::new();
        let mut stmt = conn.prepare(
            "SELECT filter_name, COUNT(*), COALESCE(SUM(input_bytes), 0), COALESCE(SUM(output_bytes), 0) 
             FROM distillations WHERE ts >= ?1 GROUP BY filter_name"
        )?;
        let rows = stmt.query_map(params![ts_threshold], |row| {
            Ok(FilterStats {
                filter_name: row.get(0)?,
                count: row.get(1)?,
                total_input_bytes: row.get(2)?,
                total_output_bytes: row.get(3)?,
            })
        })?;
        for stats in rows.flatten() {
            by_filter.push(stats);
        }

        let mut by_route = Vec::new();
        let mut stmt = conn
            .prepare("SELECT route, COUNT(*) FROM distillations WHERE ts >= ?1 GROUP BY route")?;
        let rows = stmt.query_map(params![ts_threshold], |row| {
            Ok(RouteStats {
                route: row.get(0)?,
                count: row.get(1)?,
            })
        })?;
        for stats in rows.flatten() {
            by_route.push(stats);
        }

        let mut passthrough_commands = Vec::new();
        let mut stmt = conn.prepare(
            "SELECT command, COUNT(*) as c FROM distillations 
             WHERE ts >= ?1 AND route = 'Passthrough' AND command != '' 
             GROUP BY command ORDER BY c DESC LIMIT 10",
        )?;
        let rows = stmt.query_map(params![ts_threshold], |row| Ok((row.get(0)?, row.get(1)?)))?;
        for stats in rows.flatten() {
            passthrough_commands.push(stats);
        }

        Ok(StoreSummary {
            total_sessions,
            total_distillations,
            total_input_bytes: total_input_bytes as u64,
            total_output_bytes: total_output_bytes as u64,
            by_filter,
            by_route,
            passthrough_commands,
        })
    }

    pub fn cleanup_old(&self, days: u32) {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return,
        };

        let ts_threshold = chrono::Utc::now().timestamp() - (days as i64 * 86400);

        let _ = conn.execute(
            "DELETE FROM sessions WHERE started_at < ?1",
            params![ts_threshold],
        );
        let _ = conn.execute(
            "DELETE FROM distillations WHERE ts < ?1",
            params![ts_threshold],
        );
        let _ = conn.execute(
            "DELETE FROM file_access WHERE last_access < ?1",
            params![ts_threshold],
        );
        let _ = conn.execute(
            "DELETE FROM rewind_store WHERE ts < ?1",
            params![ts_threshold],
        );
        let _ = conn.execute(
            "DELETE FROM session_events WHERE ts < ?1",
            params![ts_threshold],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::ContentType;
    use tempfile::tempdir;

    fn get_temp_store() -> (Store, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("omni.db");
        (Store::open_path(&db_path).unwrap(), dir)
    }

    #[test]
    fn test_open_creates_database_and_schema() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("omni.db");
        let store = Store::open_path(&db_path).unwrap();

        let conn = store.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table'")
            .unwrap();
        let tables: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        assert!(tables.contains(&"sessions".to_string()));
        assert!(tables.contains(&"distillations".to_string()));
        assert!(tables.contains(&"file_access".to_string()));
        assert!(tables.contains(&"rewind_store".to_string()));
        assert!(tables.contains(&"session_events".to_string())); // Because of fts5, session_events and its shadow tables exist
    }

    #[test]
    fn test_record_distillation_fire_and_forget_tidak_panic() {
        let (store, _dir) = get_temp_store();
        let res = DistillResult {
            output: "hello".to_string(),
            route: crate::pipeline::Route::Keep,
            filter_name: "test_filter".to_string(),
            content_type: ContentType::TestOutput,
            score: 0.8,
            context_score: 0.1,
            input_bytes: 100,
            output_bytes: 10,
            latency_ms: 12,
            rewind_hash: None,
            segments_kept: 1,
            segments_dropped: 0,
        };
        // Should not panic
        store.record_distillation("sess_123", &res, "npm start");
    }

    #[test]
    fn test_store_rewind_and_retrieve_rewind_roundtrip() {
        let (store, _dir) = get_temp_store();
        let content = "this is some compressed content";
        let hash = store.store_rewind(content);

        assert_eq!(hash.len(), 8);

        let retrieved = store.retrieve_rewind(&hash);
        assert_eq!(retrieved, Some(content.to_string()));

        // Retrieved counts updated
        let conn = store.conn.lock().unwrap();
        let count: i32 = conn
            .query_row(
                "SELECT retrieved FROM rewind_store WHERE hash = ?1",
                params![hash],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_duplicate_rewind_hash_tidak_error() {
        let (store, _dir) = get_temp_store();
        let content = "duplicate me";
        let hash1 = store.store_rewind(content);
        let hash2 = store.store_rewind(content); // duplicate

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_index_event_and_search_session_events_fts5() {
        let (store, _dir) = get_temp_store();
        store.index_event("sess_1", "command", "git status is running fast");
        store.index_event("sess_1", "command", "npm install");
        store.index_event("sess_2", "command", "git status is running"); // diff session

        let res = store.search_session_events("sess_1", "running", 10);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0], "git status is running fast");
    }

    #[test]
    fn test_fts5_porter_stemming_running_matches_run() {
        let (store, _dir) = get_temp_store();
        store.index_event("sess_2", "log", "The server is running now");

        // Porter stemming makes 'run' match 'running'
        let res = store.search_session_events("sess_2", "run", 10);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0], "The server is running now");
    }

    #[test]
    fn test_cleanup_old_menghapus_entries_lama() {
        let (store, _dir) = get_temp_store();
        let old_ts = chrono::Utc::now().timestamp() - (5 * 86400); // 5 days ago

        let conn = store.conn.lock().unwrap();
        conn.execute("INSERT INTO distillations (session_id, ts, filter_name, content_type, input_bytes, output_bytes, route, latency_ms) VALUES ('sess_1', ?1, 'f', 't', 1, 1, 'K', 1)", [old_ts]).unwrap();
        drop(conn);

        store.cleanup_old(2); // keep last 2 days

        let conn = store.conn.lock().unwrap();
        let count: i32 = conn
            .query_row("SELECT COUNT(*) FROM distillations", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_upsert_session_and_load_session_roundtrip() {
        let (store, _dir) = get_temp_store();
        let mut state = SessionState::new();
        state.inferred_task = Some("fixing bugs".to_string());

        store.upsert_session(&state);

        let loaded = store.load_session(&state.session_id).unwrap();
        assert_eq!(loaded.inferred_task, Some("fixing bugs".to_string()));
    }

    #[test]
    fn test_get_summary_kalkulasi_benar() {
        let (store, _dir) = get_temp_store();

        let mut state = SessionState::new();
        state.session_id = "sess_1".to_string();
        store.upsert_session(&state);

        let res1 = DistillResult {
            output: "".to_string(),
            route: crate::pipeline::Route::Keep,
            filter_name: "f1".to_string(),
            content_type: ContentType::Unknown,
            score: 0.9,
            context_score: 0.1,
            input_bytes: 100,
            output_bytes: 50,
            latency_ms: 10,
            rewind_hash: None,
            segments_kept: 1,
            segments_dropped: 0,
        };
        store.record_distillation("sess_1", &res1, "cmd1");

        let res2 = DistillResult {
            output: "".to_string(),
            route: crate::pipeline::Route::Keep,
            filter_name: "f1".to_string(),
            content_type: ContentType::Unknown,
            score: 0.9,
            context_score: 0.1,
            input_bytes: 200,
            output_bytes: 50,
            latency_ms: 10,
            rewind_hash: None,
            segments_kept: 1,
            segments_dropped: 0,
        };
        store.record_distillation("sess_1", &res2, "cmd2");

        let summary = store.get_summary(3600).unwrap();
        assert_eq!(summary.total_sessions, 1);
        assert_eq!(summary.total_distillations, 2);
        assert_eq!(summary.total_input_bytes, 300);
        assert_eq!(summary.total_output_bytes, 100);

        assert_eq!(summary.by_filter.len(), 1);
        assert_eq!(summary.by_filter[0].filter_name, "f1");
        assert_eq!(summary.by_filter[0].count, 2);
        assert_eq!(summary.by_filter[0].total_input_bytes, 300);

        assert_eq!(summary.by_route.len(), 1);
        assert_eq!(summary.by_route[0].route, "Keep");
        assert_eq!(summary.by_route[0].count, 2);
    }
}
