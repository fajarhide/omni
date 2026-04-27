use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};
use std::sync::Mutex;

use crate::paths;
use crate::pipeline::DistillResult;
use crate::pipeline::SessionState;

pub struct Store {
    conn: Mutex<Connection>,
}

impl Store {
    /// Creates dir ~/.omni/ if none exists, Open/create DB, Run schema migrations
    pub fn open() -> Result<Self> {
        let db_path = if let Ok(custom_path) = std::env::var("OMNI_DB_PATH") {
            std::path::PathBuf::from(custom_path)
        } else {
            paths::database_path()
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
    pub fn filter_breakdown(&self, since: i64) -> Result<Vec<(String, u64, u64, u64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT CASE WHEN command != '' THEN command ELSE filter_name END as grp_name, COUNT(*), 
                    COALESCE(SUM(input_bytes), 0), COALESCE(SUM(output_bytes), 0)
             FROM distillations WHERE ts >= ?1 GROUP BY grp_name ORDER BY COUNT(*) DESC"
        )?;
        let rows = stmt
            .query_map(params![since], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, u64>(1)?,
                    row.get::<_, u64>(2)?,
                    row.get::<_, u64>(3)?,
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

    pub fn list_recent_rewinds(
        &self,
        limit: usize,
    ) -> Result<Vec<crate::cli::rewind::RewindEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT hash, ts, original_len, retrieved FROM rewind_store ORDER BY ts DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(crate::cli::rewind::RewindEntry {
                hash: row.get(0)?,
                ts: row.get(1)?,
                original_len: row.get(2)?,
                retrieved: row.get(3)?,
            })
        })?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
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

    /// Collapse aggregate: (event_count, total_original_lines, total_collapsed_lines)
    pub fn collapse_aggregate(&self, since: i64) -> Result<(u64, u64, u64)> {
        let conn = self.conn.lock().unwrap();
        let r = conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(collapse_original),0), COALESCE(SUM(collapse_to),0) FROM distillations WHERE ts >= ?1 AND collapse_original > 0",
            params![since],
            |row| Ok((row.get::<_,u64>(0)?, row.get::<_,u64>(1)?, row.get::<_,u64>(2)?)),
        ).unwrap_or((0, 0, 0));
        Ok(r)
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
                input_bytes  INTEGER NOT NULL,
                output_bytes INTEGER NOT NULL,
                route        TEXT NOT NULL,
                score        REAL NOT NULL DEFAULT 0.0,
                context_score REAL NOT NULL DEFAULT 0.0,
                latency_ms   INTEGER NOT NULL,
                rewind_hash  TEXT DEFAULT '',
                command      TEXT DEFAULT '',
                project_path TEXT DEFAULT '',
                agent_id     TEXT DEFAULT 'claude_code'
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

            -- 6. Execution traces
            CREATE TABLE IF NOT EXISTS execution_traces (
                id INTEGER PRIMARY KEY,
                session_id TEXT NOT NULL,
                ts INTEGER NOT NULL,
                command TEXT NOT NULL,
                agent_id TEXT NOT NULL,
                project_path TEXT NOT NULL,
                raw_input TEXT NOT NULL,
                distilled_output TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_traces_ts ON execution_traces(ts);

            -- 7. Session summaries 
            CREATE TABLE IF NOT EXISTS session_summaries (
                session_id   TEXT PRIMARY KEY,
                started_at   INTEGER NOT NULL,
                ended_at     INTEGER NOT NULL,
                agent_id     TEXT DEFAULT 'unknown',
                total_commands INTEGER DEFAULT 0,
                tokens_saved INTEGER DEFAULT 0,
                top_filter   TEXT DEFAULT '',
                exit_reason  TEXT DEFAULT 'unknown',
                project_path TEXT DEFAULT ''
            );

            -- 8. Retrieve events for adaptive threshold 
            CREATE TABLE IF NOT EXISTS retrieve_events (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                command_prefix TEXT NOT NULL,
                hash         TEXT DEFAULT '',
                ts           INTEGER NOT NULL,
                agent_id     TEXT DEFAULT 'unknown'
            );
            CREATE INDEX IF NOT EXISTS idx_retrieve_cmd ON retrieve_events(command_prefix);
            CREATE INDEX IF NOT EXISTS idx_retrieve_ts  ON retrieve_events(ts);

            -- 9. Project knowledge — cross-session semantic memory 
            CREATE TABLE IF NOT EXISTS project_knowledge (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                project_hash TEXT NOT NULL,
                key          TEXT NOT NULL,
                value        TEXT NOT NULL,
                confidence   REAL DEFAULT 0.7,
                hit_count    INTEGER DEFAULT 1,
                last_updated INTEGER NOT NULL,
                UNIQUE(project_hash, key)
            );
            CREATE INDEX IF NOT EXISTS idx_pk_project ON project_knowledge(project_hash);

            -- 10. Multi-agent sessions — shared state across agents 
            CREATE TABLE IF NOT EXISTS agent_sessions (
                agent_id     TEXT NOT NULL,
                session_id   TEXT NOT NULL,
                project_hash TEXT NOT NULL,
                last_active  INTEGER NOT NULL,
                state_json   TEXT DEFAULT '{}',
                PRIMARY KEY (agent_id, project_hash)
            );
            CREATE INDEX IF NOT EXISTS idx_as_project ON agent_sessions(project_hash);
            "#,
        )?;

        // Safe migration: check for legacy content_type (v0.5.6 migration)
        let has_content_type = {
            let mut stmt = conn.prepare("PRAGMA table_info(distillations)")?;
            let mut rows = stmt.query([])?;
            let mut found = false;
            while let Some(row) = rows.next()? {
                let name: String = row.get(1)?;
                if name == "content_type" {
                    found = true;
                    break;
                }
            }
            found
        };

        if has_content_type {
            // Recreate table to remove legacy NOT NULL content_type column
            conn.execute_batch(
                r#"
                ALTER TABLE distillations RENAME TO distillations_old;
                CREATE TABLE distillations (
                    id           INTEGER PRIMARY KEY AUTOINCREMENT,
                    session_id   TEXT NOT NULL,
                    ts           INTEGER NOT NULL,
                    filter_name  TEXT NOT NULL,
                    input_bytes  INTEGER NOT NULL,
                    output_bytes INTEGER NOT NULL,
                    route        TEXT NOT NULL,
                    score        REAL NOT NULL DEFAULT 0.0,
                    context_score REAL NOT NULL DEFAULT 0.0,
                    latency_ms   INTEGER NOT NULL,
                    rewind_hash  TEXT DEFAULT '',
                    command      TEXT DEFAULT '',
                    project_path TEXT DEFAULT '',
                    agent_id     TEXT DEFAULT 'claude_code'
                );
                INSERT INTO distillations 
                (id, session_id, ts, filter_name, input_bytes, output_bytes, route, score, context_score, latency_ms, rewind_hash, command, project_path, agent_id)
                SELECT id, session_id, ts, filter_name, input_bytes, output_bytes, route, score, context_score, latency_ms, rewind_hash, command, '', 'claude_code' 
                FROM distillations_old;
                DROP TABLE distillations_old;
                CREATE INDEX idx_dist_ts ON distillations(ts);
                CREATE INDEX idx_dist_session ON distillations(session_id);
                CREATE INDEX idx_dist_filter ON distillations(filter_name);
                "#,
            )?;
        }

        // Safe migration: add collapse columns if not present
        let _ = conn.execute(
            "ALTER TABLE distillations ADD COLUMN collapse_original INTEGER DEFAULT 0",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE distillations ADD COLUMN collapse_to INTEGER DEFAULT 0",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE distillations ADD COLUMN project_path TEXT DEFAULT ''",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE distillations ADD COLUMN agent_id TEXT DEFAULT 'claude_code'",
            [],
        );

        Ok(())
    }

    pub fn record_distillation(
        &self,
        session_id: &str,
        result: &DistillResult,
        command: &str,
        project_path: &str,
        agent_id: &str,
    ) {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return,
        };

        let ts = chrono::Utc::now().timestamp();
        let (col_orig, col_to) = result.collapse_savings.unwrap_or((0, 0));
        let res = conn.execute(
            "INSERT INTO distillations 
             (session_id, ts, filter_name, input_bytes, output_bytes, route, score, context_score, latency_ms, rewind_hash, command, collapse_original, collapse_to, project_path, agent_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                session_id,
                ts,
                result.filter_name,
                result.input_bytes,
                result.output_bytes,
                result.route.to_string(),
                result.score,
                result.context_score,
                result.latency_ms,
                result.rewind_hash.as_deref().unwrap_or(""),
                command,
                col_orig as i64,
                col_to as i64,
                project_path,
                agent_id,
            ],
        );

        if let Err(e) = res {
            // Log to stderr for visibility during development/debugging
            eprintln!("[omni:error] failed to record distillation: {}", e);
            if e.to_string().contains("NOT NULL constraint failed") {
                eprintln!(
                    "[omni:error] hint: legacy 'content_type' column may be blocking inserts. OMNI will attempt auto-migration on next startup."
                );
            }
        }
    }

    pub fn record_trace(
        &self,
        session_id: &str,
        command: &str,
        agent_id: &str,
        project_path: &str,
        raw_input: &str,
        distilled_output: &str,
    ) {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return,
        };

        let ts = chrono::Utc::now().timestamp();
        let res = conn.execute(
            "INSERT INTO execution_traces 
             (session_id, ts, command, agent_id, project_path, raw_input, distilled_output)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                session_id,
                ts,
                command,
                agent_id,
                project_path,
                raw_input,
                distilled_output,
            ],
        );

        if let Err(e) = res {
            eprintln!("[omni:error] failed to record trace: {}", e);
        }
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

    /// Get recent distillation rows for omni_history MCP tool
    pub fn get_recent_distillations(&self, session_id: &str, limit: usize) -> Vec<DistillationRow> {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return vec![],
        };
        let mut stmt = match conn.prepare(
            "SELECT command, input_bytes, output_bytes, route, filter_name
             FROM distillations WHERE session_id = ?1
             ORDER BY ts DESC LIMIT ?2",
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        match stmt.query_map(params![session_id, limit as i64], |row| {
            Ok(DistillationRow {
                command: row.get(0)?,
                input_bytes: row.get::<_, i64>(1)? as usize,
                output_bytes: row.get::<_, i64>(2)? as usize,
                route: row.get(3)?,
                filter_name: row.get(4)?,
            })
        }) {
            Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
            Err(_) => vec![],
        }
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

    pub fn get_per_command_stats(
        &self,
        since: i64,
        limit: usize,
    ) -> Result<Vec<(String, u64, u64, u64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT
                command,
                COUNT(*) as calls,
                COALESCE(SUM(input_bytes), 0) as total_input,
                COALESCE(SUM(output_bytes), 0) as total_output
            FROM distillations
            WHERE ts >= ?1 AND command != '' AND command != '[pipe]'
            GROUP BY command
            ORDER BY calls DESC
            LIMIT ?2",
        )?;

        let rows = stmt
            .query_map(params![since, limit as i64], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, u64>(1)?,
                    row.get::<_, u64>(2)?,
                    row.get::<_, u64>(3)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(rows)
    }

    /// Agent breakdown: (agent_id, count, input_bytes, output_bytes)
    pub fn get_agent_breakdown(&self, since: i64) -> Result<Vec<(String, u64, u64, u64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT
                COALESCE(agent_id, 'claude_code') as agent,
                COUNT(*) as calls,
                COALESCE(SUM(input_bytes), 0) as total_input,
                COALESCE(SUM(output_bytes), 0) as total_output
            FROM distillations
            WHERE ts >= ?1
            GROUP BY agent
            ORDER BY calls DESC",
        )?;

        let rows = stmt
            .query_map(params![since], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, u64>(1)?,
                    row.get::<_, u64>(2)?,
                    row.get::<_, u64>(3)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(rows)
    }

    /// Per-command stats with agent_id: (command, agent_id, count, input_bytes, output_bytes)
    #[allow(clippy::type_complexity)]
    pub fn get_per_command_with_agent(
        &self,
        since: i64,
        limit: usize,
    ) -> Result<Vec<(String, String, u64, u64, u64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT
                command,
                COALESCE(agent_id, 'claude_code') as agent,
                COUNT(*) as calls,
                COALESCE(SUM(input_bytes), 0) as total_input,
                COALESCE(SUM(output_bytes), 0) as total_output
            FROM distillations
            WHERE ts >= ?1 AND command != '' AND command != '[pipe]'
            GROUP BY command, agent
            ORDER BY calls DESC
            LIMIT ?2",
        )?;

        let rows = stmt
            .query_map(params![since, limit as i64], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, u64>(2)?,
                    row.get::<_, u64>(3)?,
                    row.get::<_, u64>(4)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(rows)
    }

    /// Multi-period stats: Vec of (period_label, count, input_bytes, output_bytes)
    pub fn multi_period_stats(&self) -> Result<Vec<(String, u64, u64, u64)>> {
        let now = chrono::Utc::now().timestamp();
        let midnight = now - (now % 86400);
        let week_ago = now - 7 * 86400;

        let mut periods = Vec::new();
        for (label, since) in [
            ("Today", midnight),
            ("This Week", week_ago),
            ("All Time", 0i64),
        ] {
            let (count, input, output, _, _) = self.aggregate_stats(since)?;
            periods.push((label.to_string(), count, input, output));
        }
        Ok(periods)
    }

    pub fn get_project_stats(&self, since: i64) -> Result<Vec<(String, u64, f64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT 
                project_path, 
                COUNT(*) as count,
                CASE 
                    WHEN SUM(input_bytes) = 0 THEN 0.0 
                    ELSE ROUND(100.0 * (1.0 - CAST(SUM(output_bytes) AS REAL) / SUM(input_bytes)), 1)
                END as savings
             FROM distillations 
             WHERE ts >= ?1 AND project_path != ''
             GROUP BY project_path 
             ORDER BY count DESC"
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

    pub fn get_recent_traces(&self, limit: usize) -> Result<Vec<(String, String, String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT session_id, command, raw_input, distilled_output FROM execution_traces ORDER BY ts DESC LIMIT ?1",
        )?;
        let rows = stmt
            .query_map(params![limit as i64], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    /// Test that the database is actually writable (catches sandbox restrictions)
    pub fn test_write(&self) -> bool {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return false,
        };
        match conn.execute("CREATE TABLE IF NOT EXISTS _write_test (id INTEGER)", []) {
            Ok(_) => {
                let _ = conn.execute("DROP TABLE IF EXISTS _write_test", []);
                true
            }
            Err(_) => false,
        }
    }

    // ── v0.5.7: Session Summaries ─────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    pub fn save_session_summary(
        &self,
        session_id: &str,
        started_at: i64,
        agent_id: &str,
        total_commands: u32,
        tokens_saved: u64,
        top_filter: &str,
        exit_reason: &str,
        project_path: &str,
    ) {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return,
        };
        let now = chrono::Utc::now().timestamp();
        let _ = conn.execute(
            "INSERT OR REPLACE INTO session_summaries
             (session_id, started_at, ended_at, agent_id, total_commands,
              tokens_saved, top_filter, exit_reason, project_path)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                session_id,
                started_at,
                now,
                agent_id,
                total_commands as i64,
                tokens_saved as i64,
                top_filter,
                exit_reason,
                project_path
            ],
        );
    }

    pub fn get_recent_session_summaries(&self, limit: usize) -> Vec<SessionSummaryRow> {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return vec![],
        };
        let mut stmt = match conn.prepare(
            "SELECT session_id, started_at, ended_at, agent_id, total_commands,
                    tokens_saved, top_filter, exit_reason, project_path
             FROM session_summaries ORDER BY ended_at DESC LIMIT ?1",
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        match stmt.query_map(params![limit as i64], |row| {
            Ok(SessionSummaryRow {
                session_id: row.get(0)?,
                started_at: row.get(1)?,
                ended_at: row.get(2)?,
                agent_id: row.get(3)?,
                total_commands: row.get::<_, i64>(4)? as u32,
                tokens_saved: row.get::<_, i64>(5)? as u64,
                top_filter: row.get(6)?,
                exit_reason: row.get(7)?,
                project_path: row.get(8)?,
            })
        }) {
            Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
            Err(_) => vec![],
        }
    }

    // ── v0.5.7: Retrieve Events (adaptive threshold) ──────────────────

    pub fn record_retrieve_event(&self, command_prefix: &str, hash: &str, agent_id: &str) {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return,
        };
        let now = chrono::Utc::now().timestamp();
        let prefix = &command_prefix[..command_prefix.len().min(40)];
        let _ = conn.execute(
            "INSERT INTO retrieve_events (command_prefix, hash, ts, agent_id)
             VALUES (?1, ?2, ?3, ?4)",
            params![prefix, hash, now, agent_id],
        );
    }

    /// Returns retrieve_rate for a command prefix (0.0 – 1.0)
    /// High rate = OMNI too aggressive for this command type
    pub fn get_retrieve_rate(&self, command_prefix: &str, window_days: i64) -> f64 {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return 0.0,
        };
        let cutoff = chrono::Utc::now().timestamp() - window_days * 86400;
        let prefix = &command_prefix[..command_prefix.len().min(40)];
        let retrieves: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM retrieve_events WHERE command_prefix = ?1 AND ts > ?2",
                params![prefix, cutoff],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let distillations: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM distillations WHERE command LIKE ?1 AND ts > ?2",
                params![format!("{}%", prefix), cutoff],
                |r| r.get(0),
            )
            .unwrap_or(1)
            .max(1);
        (retrieves as f64 / distillations as f64).min(1.0)
    }

    pub fn find_command_for_hash(&self, hash: &str) -> Option<String> {
        let conn = self.conn.lock().ok()?;
        conn.query_row(
            "SELECT command FROM distillations WHERE rewind_hash = ? LIMIT 1",
            params![hash],
            |r| r.get(0),
        )
        .ok()
    }

    // ── v0.5.7: Project Knowledge (cross-session semantic memory) ─────

    pub fn upsert_project_knowledge(
        &self,
        project_hash: &str,
        key: &str,
        value: &str,
        confidence: f32,
    ) {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return,
        };
        let now = chrono::Utc::now().timestamp();
        let _ = conn.execute(
            "INSERT INTO project_knowledge (project_hash, key, value, confidence, hit_count, last_updated)
             VALUES (?1,?2,?3,?4,1,?5)
             ON CONFLICT(project_hash, key) DO UPDATE SET
               value      = excluded.value,
               confidence = excluded.confidence,
               hit_count  = hit_count + 1,
               last_updated = excluded.last_updated",
            params![project_hash, key, value, confidence as f64, now],
        );
    }

    pub fn get_project_knowledge(&self, project_hash: &str) -> Vec<(String, String, f32)> {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return vec![],
        };
        let mut stmt = match conn.prepare(
            "SELECT key, value, confidence FROM project_knowledge
             WHERE project_hash = ?1 AND confidence >= 0.5
             ORDER BY confidence DESC, hit_count DESC LIMIT 50",
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        match stmt.query_map(params![project_hash], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f64>(2)? as f32,
            ))
        }) {
            Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
            Err(_) => vec![],
        }
    }

    // ── v0.5.7: Multi-Agent Sessions ─────────────────────────────────

    /// Sync agent session for cross-agent state sharing
    pub fn sync_agent_session(
        &self,
        agent_id: &str,
        session_id: &str,
        project_hash: &str,
        state_json: &str,
    ) {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return,
        };
        let now = chrono::Utc::now().timestamp();
        let _ = conn.execute(
            "INSERT INTO agent_sessions (agent_id, session_id, project_hash, last_active, state_json)
             VALUES (?1,?2,?3,?4,?5)
             ON CONFLICT(agent_id, project_hash) DO UPDATE SET
               session_id  = excluded.session_id,
               last_active = excluded.last_active,
               state_json  = excluded.state_json",
            params![agent_id, session_id, project_hash, now, state_json],
        );
    }

    /// Get all active agents working on the same project (within last 8h)
    pub fn get_active_agents_for_project(
        &self,
        project_hash: &str,
        exclude_agent: &str,
    ) -> Vec<AgentSessionRow> {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return vec![],
        };
        let cutoff = chrono::Utc::now().timestamp() - 8 * 3600;
        let mut stmt = match conn.prepare(
            "SELECT agent_id, session_id, last_active, state_json
             FROM agent_sessions
             WHERE project_hash = ?1 AND agent_id != ?2 AND last_active > ?3
             ORDER BY last_active DESC",
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        match stmt.query_map(params![project_hash, exclude_agent, cutoff], |row| {
            Ok(AgentSessionRow {
                agent_id: row.get(0)?,
                session_id: row.get(1)?,
                last_active: row.get(2)?,
                state_json: row.get(3)?,
            })
        }) {
            Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
            Err(_) => vec![],
        }
    }
}

// ── v0.5.7 Row Types ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct DistillationRow {
    pub command: String,
    pub input_bytes: usize,
    pub output_bytes: usize,
    pub route: String,
    pub filter_name: String,
}

#[derive(Debug, Clone)]
pub struct SessionSummaryRow {
    pub session_id: String,
    pub started_at: i64,
    pub ended_at: i64,
    pub agent_id: String,
    pub total_commands: u32,
    pub tokens_saved: u64,
    pub top_filter: String,
    pub exit_reason: String,
    pub project_path: String,
}

#[derive(Debug, Clone)]
pub struct AgentSessionRow {
    pub agent_id: String,
    pub session_id: String,
    pub last_active: i64,
    pub state_json: String,
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert!(tables.contains(&"execution_traces".to_string()));
    }

    #[test]
    fn test_record_distillation_fire_and_forget_not_panic() {
        let (store, _dir) = get_temp_store();
        let res = DistillResult {
            output: "hello".to_string(),
            route: crate::pipeline::Route::Keep,
            filter_name: "test_filter".to_string(),
            score: 0.8,
            context_score: 0.1,
            input_bytes: 100,
            output_bytes: 10,
            latency_ms: 12,
            rewind_hash: None,
            segments_kept: 1,
            segments_dropped: 0,
            collapse_savings: None,
        };
        // Should not panic
        store.record_distillation("sess_123", &res, "npm start", "", "claude_code");
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
    fn test_duplicate_rewind_hash_not_error() {
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
        conn.execute("INSERT INTO distillations (session_id, ts, filter_name, input_bytes, output_bytes, route, latency_ms) VALUES ('sess_1', ?1, 'f', 1, 1, 'K', 1)", [old_ts]).unwrap();
        drop(conn);

        store.cleanup_old(2); // keep last 2 days

        let conn = store.conn.lock().unwrap();
        let count: i32 = conn
            .query_row("SELECT COUNT(*) FROM distillations", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }
}
