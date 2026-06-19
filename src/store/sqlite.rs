// Safety: String slicing uses ASCII delimiter positions or boundary-checked safe utilities.
#![allow(clippy::string_slice)]

use anyhow::{Context, Result};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};

use crate::paths;
use crate::pipeline::DistillResult;
use crate::pipeline::SessionState;

/// Customizes each pooled connection with required PRAGMA settings.
/// WAL mode is database-level and persists, but synchronous and foreign_keys are per-connection.
#[derive(Debug)]
struct PragmaCustomizer;

impl r2d2::CustomizeConnection<Connection, rusqlite::Error> for PragmaCustomizer {
    fn on_acquire(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;",
        )?;
        Ok(())
    }
}

pub struct SqliteBackend {
    pub(crate) pool: Pool<SqliteConnectionManager>,
}

pub struct Store {
    pub backend: SqliteBackend,
}

impl std::ops::Deref for Store {
    type Target = SqliteBackend;

    fn deref(&self) -> &Self::Target {
        &self.backend
    }
}

impl Store {
    pub fn open() -> Result<Self> {
        Ok(Self {
            backend: SqliteBackend::open()?,
        })
    }

    pub fn open_path(path: &std::path::Path) -> Result<Self> {
        Ok(Self {
            backend: SqliteBackend::open_path(path)?,
        })
    }
}

impl SqliteBackend {
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

        let manager = SqliteConnectionManager::file(path);
        let pool = Pool::builder()
            .max_size(4)
            .connection_customizer(Box::new(PragmaCustomizer))
            .build(manager)
            .context("Failed to create SQLite connection pool")?;

        let store = Self { pool };
        store.init_schema()?;
        Ok(store)
    }

    pub fn stats(&self) -> Result<(usize, usize)> {
        let conn = self.pool.get().context("DB pool exhausted")?;
        let sessions: usize = conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
            .unwrap_or(0);
        let rewinds: usize = conn
            .query_row("SELECT COUNT(*) FROM rewind_store", [], |row| row.get(0))
            .unwrap_or(0);
        Ok((sessions, rewinds))
    }

    pub fn latest_activity_timestamps(&self) -> Result<(Option<u64>, Option<u64>)> {
        let conn = self.pool.get().context("DB pool exhausted")?;
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
        let conn = match self.pool.get() {
            Ok(c) => c,
            Err(_) => return false,
        };
        let query =
            "SELECT 1 FROM pragma_compile_options WHERE compile_options LIKE 'ENABLE_FTS5%'";
        conn.query_row(query, [], |row| row.get::<_, i64>(0))
            .is_ok()
    }

    /// Aggregate distillation stats since a given timestamp
    pub fn aggregate_stats(&self, since: i64) -> Result<(u64, u64, u64, u64, i64, u64, u64)> {
        let conn = self.pool.get().context("DB pool exhausted")?;
        // returns (count, total_input, total_output, sum_latency, max_latency, raw_tokens, filtered_tokens)
        let r = conn.query_row(
            "SELECT COALESCE(COUNT(*),0), COALESCE(SUM(input_bytes),0), COALESCE(SUM(output_bytes),0), COALESCE(SUM(latency_ms),0), COALESCE(MAX(latency_ms),0), COALESCE(SUM(raw_tokens),0), COALESCE(SUM(filtered_tokens),0) FROM distillations WHERE ts >= ?1",
            params![since],
            |row| Ok((
                row.get::<_,u64>(0)?,
                row.get::<_,u64>(1)?,
                row.get::<_,u64>(2)?,
                row.get::<_,u64>(3)?,
                row.get::<_,i64>(4)?,
                row.get::<_,u64>(5)?,
                row.get::<_,u64>(6)?
            )),
        ).unwrap_or((0, 0, 0, 0, 0, 0, 0));
        Ok(r)
    }

    /// Per-filter breakdown: (filter_name, count, avg_reduction_pct)
    #[allow(clippy::type_complexity)]
    pub fn filter_breakdown(&self, since: i64) -> Result<Vec<(String, u64, u64, u64, u64, u64)>> {
        let conn = self.pool.get().context("DB pool exhausted")?;
        let mut stmt = conn.prepare(
            "SELECT CASE WHEN command != '' THEN command ELSE '[unknown command]' END as grp_name, COUNT(*),
                    COALESCE(SUM(input_bytes), 0), COALESCE(SUM(output_bytes), 0),
                    COALESCE(SUM(raw_tokens), 0), COALESCE(SUM(filtered_tokens), 0)
             FROM distillations WHERE ts >= ?1 GROUP BY grp_name ORDER BY COUNT(*) DESC"
        )?;
        let rows = stmt
            .query_map(params![since], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, u64>(1)?,
                    row.get::<_, u64>(2)?,
                    row.get::<_, u64>(3)?,
                    row.get::<_, u64>(4)?,
                    row.get::<_, u64>(5)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    /// Route distribution: (route, count)
    pub fn route_distribution(&self, since: i64) -> Result<Vec<(String, u64)>> {
        let conn = self.pool.get().context("DB pool exhausted")?;
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
        let conn = self.pool.get().context("DB pool exhausted")?;
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
        let conn = self.pool.get().context("DB pool exhausted")?;
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
        let conn = self.pool.get().context("DB pool exhausted")?;
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
        let conn = self.pool.get().context("DB pool exhausted")?;
        let r = conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(collapse_original),0), COALESCE(SUM(collapse_to),0) FROM distillations WHERE ts >= ?1 AND collapse_original > 0",
            params![since],
            |row| Ok((row.get::<_,u64>(0)?, row.get::<_,u64>(1)?, row.get::<_,u64>(2)?)),
        ).unwrap_or((0, 0, 0));
        Ok(r)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.pool.get().context("DB pool exhausted")?;
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

            -- 1b. Passthrough events telemetry
            CREATE TABLE IF NOT EXISTS passthrough_events (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                command      TEXT NOT NULL,
                bytes        INTEGER NOT NULL,
                ts           INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_pt_ts ON passthrough_events(ts);

            -- 1c. Unhandled tools telemetry
            CREATE TABLE IF NOT EXISTS unhandled_tools (
                tool_name    TEXT PRIMARY KEY,
                count        INTEGER DEFAULT 1,
                last_seen    INTEGER NOT NULL
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
                agent_id     TEXT DEFAULT 'unknown'
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

            -- 11b. Pattern memory — cross-session error pattern tracking
            CREATE TABLE IF NOT EXISTS pattern_memory (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                pattern_hash    TEXT NOT NULL UNIQUE,
                pattern_text    TEXT NOT NULL,
                tool_family     TEXT DEFAULT '',
                first_seen      INTEGER NOT NULL,
                last_seen       INTEGER NOT NULL,
                occurrence_count INTEGER DEFAULT 1,
                was_resolved    INTEGER DEFAULT 0,
                resolution_hint TEXT DEFAULT ''
            );
            CREATE INDEX IF NOT EXISTS idx_pm_tool ON pattern_memory(tool_family);
            CREATE INDEX IF NOT EXISTS idx_pm_last ON pattern_memory(last_seen);

            -- 12. Context Turns
            CREATE TABLE IF NOT EXISTS context_turns (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                turn_number INTEGER NOT NULL,
                ts INTEGER NOT NULL,
                estimated_total_tokens INTEGER DEFAULT 0,
                file_read_tokens INTEGER DEFAULT 0,
                tool_output_tokens INTEGER DEFAULT 0,
                conversation_tokens INTEGER DEFAULT 0,
                system_prompt_tokens INTEGER DEFAULT 0,
                has_duplicate_reads INTEGER DEFAULT 0,
                duplicate_files TEXT DEFAULT '[]',
                largest_read_file TEXT DEFAULT '',
                largest_read_tokens INTEGER DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_ctx_session ON context_turns(session_id);

            -- 11. One-time data migrations tracker
            CREATE TABLE IF NOT EXISTS schema_migrations (
                id           TEXT PRIMARY KEY,
                applied_at   INTEGER NOT NULL
            );

            -- 13. Maker-Checker Verification Results (L2-01)
            CREATE TABLE IF NOT EXISTS verification_results (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                checker_agent TEXT NOT NULL,
                maker_session TEXT NOT NULL,
                criteria TEXT NOT NULL,
                passed INTEGER NOT NULL,
                issues TEXT,
                timestamp INTEGER NOT NULL
            );

            -- 14. Loop Memory — cross-session persistent knowledge (L2-02)
            CREATE TABLE IF NOT EXISTS loop_memory (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                loop_goal_hash TEXT NOT NULL,
                key TEXT NOT NULL,
                value TEXT NOT NULL,
                confidence REAL DEFAULT 0.5,
                times_confirmed INTEGER DEFAULT 1,
                first_seen INTEGER NOT NULL,
                last_seen INTEGER NOT NULL,
                ttl_days INTEGER DEFAULT 30,
                UNIQUE(loop_goal_hash, key)
            );
            CREATE INDEX IF NOT EXISTS idx_lm_goal ON loop_memory(loop_goal_hash);
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
                    agent_id     TEXT DEFAULT 'unknown'
                );
                INSERT INTO distillations 
                (id, session_id, ts, filter_name, input_bytes, output_bytes, route, score, context_score, latency_ms, rewind_hash, command, project_path, agent_id)
                SELECT id, session_id, ts, filter_name, input_bytes, output_bytes, route, score, context_score, latency_ms, rewind_hash, command, '', 'unknown' 
                FROM distillations_old;
                DROP TABLE distillations_old;
                CREATE INDEX idx_dist_ts ON distillations(ts);
                CREATE INDEX idx_dist_session ON distillations(session_id);
                CREATE INDEX idx_dist_filter ON distillations(filter_name);
                "#,
            )?;
        }

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
            "ALTER TABLE distillations ADD COLUMN agent_id TEXT DEFAULT 'unknown'",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE distillations ADD COLUMN raw_tokens INTEGER DEFAULT 0",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE distillations ADD COLUMN filtered_tokens INTEGER DEFAULT 0",
            [],
        );

        // One-time data migration:
        // Older builds could attribute Cursor sessions as "vscode" because TERM_PROGRAM
        // detection happened before explicit OMNI_AGENT_ID routing.
        // Keep this migration idempotent and run only once.
        let migration_id = "2026_05_cursor_agent_id_backfill_vscode_to_cursor";
        let already_applied: Option<i64> = conn
            .query_row(
                "SELECT 1 FROM schema_migrations WHERE id = ?1 LIMIT 1",
                params![migration_id],
                |row| row.get(0),
            )
            .optional()?;
        if already_applied.is_none() {
            let tx = conn.unchecked_transaction()?;
            tx.execute(
                "UPDATE distillations SET agent_id = 'cursor' WHERE agent_id = 'vscode'",
                [],
            )?;
            tx.execute(
                "UPDATE execution_traces SET agent_id = 'cursor' WHERE agent_id = 'vscode'",
                [],
            )?;
            tx.execute(
                "UPDATE session_summaries SET agent_id = 'cursor' WHERE agent_id = 'vscode'",
                [],
            )?;
            tx.execute(
                "INSERT INTO schema_migrations (id, applied_at) VALUES (?1, ?2)",
                params![migration_id, chrono::Utc::now().timestamp()],
            )?;
            tx.commit()?;
        }

        let migration_id2 = "2026_05_token_backfill";
        let already_applied2: Option<i64> = conn
            .query_row(
                "SELECT 1 FROM schema_migrations WHERE id = ?1 LIMIT 1",
                params![migration_id2],
                |row| row.get(0),
            )
            .optional()?;
        if already_applied2.is_none() {
            let tx = conn.unchecked_transaction()?;
            // Use 3.8 chars/token (~4 bytes/token heuristic) for backfill
            tx.execute(
                "UPDATE distillations SET raw_tokens = input_bytes / 4, filtered_tokens = output_bytes / 4 WHERE raw_tokens = 0",
                [],
            )?;
            tx.execute(
                "INSERT INTO schema_migrations (id, applied_at) VALUES (?1, ?2)",
                params![migration_id2, chrono::Utc::now().timestamp()],
            )?;
            tx.commit()?;
        }

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
        let conn = match self.pool.get() {
            Ok(c) => c,
            Err(_) => return,
        };

        let ts = chrono::Utc::now().timestamp();
        let (col_orig, col_to) = result.collapse_savings.unwrap_or((0, 0));
        let res = conn.execute(
            "INSERT INTO distillations 
             (session_id, ts, filter_name, input_bytes, output_bytes, route, score, context_score, latency_ms, rewind_hash, command, collapse_original, collapse_to, project_path, agent_id, raw_tokens, filtered_tokens)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
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
                result.raw_tokens as i64,
                result.filtered_tokens as i64,
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
        let conn = match self.pool.get() {
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

    pub fn record_unhandled_tool(&self, tool_name: &str) {
        let conn = match self.pool.get() {
            Ok(c) => c,
            Err(_) => return,
        };
        let now = chrono::Utc::now().timestamp();
        let _ = conn.execute(
            "INSERT INTO unhandled_tools (tool_name, count, last_seen) VALUES (?1, 1, ?2)
             ON CONFLICT(tool_name) DO UPDATE SET count = count + 1, last_seen = excluded.last_seen",
            params![tool_name, now],
        );
    }

    pub fn record_passthrough(&self, command: &str, bytes: usize) {
        let conn = match self.pool.get() {
            Ok(c) => c,
            Err(_) => return,
        };
        let now = chrono::Utc::now().timestamp();
        let _ = conn.execute(
            "INSERT INTO passthrough_events (command, bytes, ts) VALUES (?1, ?2, ?3)",
            params![command, bytes as i64, now],
        );
    }

    pub fn store_rewind(&self, content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let hash_result = hasher.finalize();
        let full_hash = hex::encode(hash_result);
        let short_hash = full_hash[..16].to_string();
        let ts_ns = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let rewind_key = format!("{}_{}", ts_ns, short_hash);

        let conn = match self.pool.get() {
            Ok(c) => c,
            Err(_) => return rewind_key,
        };

        let ts = chrono::Utc::now().timestamp();
        let original_len = content.len() as i64;

        let _ = conn.execute(
            "INSERT OR IGNORE INTO rewind_store (hash, content, ts, original_len, retrieved)
             VALUES (?1, ?2, ?3, ?4, 0)",
            params![rewind_key, content, ts, original_len],
        );

        rewind_key
    }

    pub fn retrieve_rewind(&self, hash: &str) -> Option<String> {
        let conn = match self.pool.get() {
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
        let conn = match self.pool.get() {
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
        let conn = self.pool.get().context("DB pool exhausted")?;
        conn.execute("DELETE FROM sessions WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn list_recent_sessions(&self, limit: usize) -> Result<Vec<SessionState>> {
        let conn = self.pool.get().context("DB pool exhausted")?;
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
        let conn = match self.pool.get() {
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
        let conn = match self.pool.get() {
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

    pub fn record_context_turn(&self, turn: &crate::analytics::context_composition::ContextTurn) {
        let conn = match self.pool.get() {
            Ok(c) => c,
            Err(_) => return,
        };

        let duplicate_files_json =
            serde_json::to_string(&turn.duplicate_files).unwrap_or_else(|_| "[]".to_string());

        let res = conn.execute(
            "INSERT INTO context_turns 
             (session_id, turn_number, ts, estimated_total_tokens, file_read_tokens, tool_output_tokens, conversation_tokens, system_prompt_tokens, has_duplicate_reads, duplicate_files, largest_read_file, largest_read_tokens)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                turn.session_id,
                turn.turn_number,
                turn.timestamp,
                turn.estimated_total_tokens as i64,
                turn.file_read_tokens as i64,
                turn.tool_output_tokens as i64,
                turn.conversation_tokens as i64,
                turn.system_prompt_tokens as i64,
                turn.has_duplicate_file_reads as i32,
                duplicate_files_json,
                turn.largest_single_read.0,
                turn.largest_single_read.1 as i64,
            ],
        );

        if let Err(e) = res {
            eprintln!("[omni:error] failed to record context turn: {}", e);
        }
    }

    pub fn index_event(&self, session_id: &str, event_type: &str, content: &str) {
        let conn = match self.pool.get() {
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
        let conn = match self.pool.get() {
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

    #[allow(clippy::type_complexity)]
    pub fn get_per_command_stats(
        &self,
        since: i64,
        limit: usize,
    ) -> Result<Vec<(String, u64, u64, u64, u64, u64)>> {
        let conn = self.pool.get().context("DB pool exhausted")?;
        let mut stmt = conn.prepare(
            "SELECT
                command,
                COUNT(*) as calls,
                COALESCE(SUM(input_bytes), 0) as total_input,
                COALESCE(SUM(output_bytes), 0) as total_output,
                COALESCE(SUM(raw_tokens), 0) as raw_tok,
                COALESCE(SUM(filtered_tokens), 0) as filt_tok
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
                    row.get::<_, u64>(4)?,
                    row.get::<_, u64>(5)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(rows)
    }

    /// Agent breakdown: (agent_id, count, input_bytes, output_bytes)
    pub fn get_agent_breakdown(&self, since: i64) -> Result<Vec<(String, u64, u64, u64)>> {
        let conn = self.pool.get().context("DB pool exhausted")?;
        let mut stmt = conn.prepare(
            "SELECT
                COALESCE(agent_id, 'unknown') as agent,
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
        let conn = self.pool.get().context("DB pool exhausted")?;
        let mut stmt = conn.prepare(
            "SELECT
                CASE WHEN command != '' THEN command ELSE '[unknown command]' END as command_name,
                COALESCE(agent_id, 'unknown') as agent,
                COUNT(*) as calls,
                COALESCE(SUM(input_bytes), 0) as total_input,
                COALESCE(SUM(output_bytes), 0) as total_output
            FROM distillations
            WHERE ts >= ?1 AND command != '[pipe]'
            GROUP BY command_name, agent
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

    /// Multi-period stats: Vec of (period_label, count, input_bytes, output_bytes, raw_tokens, filtered_tokens)
    #[allow(clippy::type_complexity)]
    pub fn multi_period_stats(&self) -> Result<Vec<(String, u64, u64, u64, u64, u64)>> {
        let now = chrono::Utc::now().timestamp();
        let midnight = now - (now % 86400);
        let week_ago = now - 7 * 86400;

        let mut periods = Vec::new();
        for (label, since) in [
            ("Today", midnight),
            ("This Week", week_ago),
            ("All Time", 0i64),
        ] {
            let (count, input, output, _, _, raw_tok, filt_tok) = self.aggregate_stats(since)?;
            periods.push((label.to_string(), count, input, output, raw_tok, filt_tok));
        }
        Ok(periods)
    }

    pub fn get_project_stats(&self, since: i64) -> Result<Vec<(String, u64, f64)>> {
        let conn = self.pool.get().context("DB pool exhausted")?;
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

    // ─── Loop Memory (L2-02) ────────────────────────────────

    /// Set or update a loop memory entry. On conflict, bumps times_confirmed and updates value.
    pub fn loop_memory_set(&self, goal_hash: &str, key: &str, value: &str, confidence: f64) {
        let conn = match self.pool.get() {
            Ok(c) => c,
            Err(_) => return,
        };
        let now = chrono::Utc::now().timestamp();
        let _ = conn.execute(
            "INSERT INTO loop_memory (loop_goal_hash, key, value, confidence, times_confirmed, first_seen, last_seen)
             VALUES (?1, ?2, ?3, ?4, 1, ?5, ?5)
             ON CONFLICT(loop_goal_hash, key) DO UPDATE SET
                value = excluded.value,
                confidence = excluded.confidence,
                times_confirmed = times_confirmed + 1,
                last_seen = excluded.last_seen",
            params![goal_hash, key, value, confidence, now],
        );
    }

    /// Get a single loop memory entry.
    pub fn loop_memory_get(&self, goal_hash: &str, key: &str) -> Option<(String, f64, i64)> {
        let conn = match self.pool.get() {
            Ok(c) => c,
            Err(_) => return None,
        };
        conn.query_row(
            "SELECT value, confidence, times_confirmed FROM loop_memory WHERE loop_goal_hash = ?1 AND key = ?2",
            params![goal_hash, key],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?, row.get::<_, i64>(2)?)),
        ).optional().unwrap_or(None)
    }

    /// List all loop memory entries for a given goal hash.
    pub fn loop_memory_list(&self, goal_hash: &str) -> Vec<(String, String, f64, i64)> {
        let conn = match self.pool.get() {
            Ok(c) => c,
            Err(_) => return vec![],
        };
        let mut stmt = match conn.prepare(
            "SELECT key, value, confidence, times_confirmed FROM loop_memory WHERE loop_goal_hash = ?1 ORDER BY last_seen DESC",
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        match stmt.query_map(params![goal_hash], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        }) {
            Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
            Err(_) => vec![],
        }
    }

    /// Delete a loop memory entry.
    pub fn loop_memory_forget(&self, goal_hash: &str, key: &str) {
        let conn = match self.pool.get() {
            Ok(c) => c,
            Err(_) => return,
        };
        let _ = conn.execute(
            "DELETE FROM loop_memory WHERE loop_goal_hash = ?1 AND key = ?2",
            params![goal_hash, key],
        );
    }

    pub fn cleanup_old(&self, days: u32) {
        let conn = match self.pool.get() {
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
        let conn = self.pool.get().context("DB pool exhausted")?;
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
        let conn = match self.pool.get() {
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

    // ──  Session Summaries ─────────────────────────────────────

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
        let conn = match self.pool.get() {
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
        let conn = match self.pool.get() {
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

    // ──  Retrieve Events (adaptive threshold) ──────────────────

    pub fn record_retrieve_event(&self, command_prefix: &str, hash: &str, agent_id: &str) {
        let conn = match self.pool.get() {
            Ok(c) => c,
            Err(_) => return,
        };
        let now = chrono::Utc::now().timestamp();
        let prefix = crate::util::text::safe_slice(command_prefix, 40);
        let _ = conn.execute(
            "INSERT INTO retrieve_events (command_prefix, hash, ts, agent_id)
             VALUES (?1, ?2, ?3, ?4)",
            params![prefix, hash, now, agent_id],
        );
    }

    /// Returns retrieve_rate for a command prefix (0.0 – 1.0)
    /// High rate = OMNI too aggressive for this command type
    pub fn get_retrieve_rate(&self, command_prefix: &str, window_days: i64) -> f64 {
        let conn = match self.pool.get() {
            Ok(c) => c,
            Err(_) => return 0.0,
        };
        let cutoff = chrono::Utc::now().timestamp() - window_days * 86400;
        let prefix = crate::util::text::safe_slice(command_prefix, 40);
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
        let conn = self.pool.get().ok()?;
        conn.query_row(
            "SELECT command FROM distillations WHERE rewind_hash = ? LIMIT 1",
            params![hash],
            |r| r.get(0),
        )
        .ok()
    }

    // ──  Project Knowledge (cross-session semantic memory) ─────

    pub fn upsert_project_knowledge(
        &self,
        project_hash: &str,
        key: &str,
        value: &str,
        confidence: f32,
    ) {
        let conn = match self.pool.get() {
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
        let conn = match self.pool.get() {
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

    // ──  Multi-Agent Sessions ─────────────────────────────────

    /// Sync agent session for cross-agent state sharing
    pub fn sync_agent_session(
        &self,
        agent_id: &str,
        session_id: &str,
        project_hash: &str,
        state_json: &str,
    ) {
        let conn = match self.pool.get() {
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
        let conn = match self.pool.get() {
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

    // ── Pattern Memory (AI-5) ────────────────────────────────────────

    /// Record or update a recurring error pattern
    pub fn upsert_pattern(&self, pattern_text: &str, tool_family: &str) {
        let hash = Self::pattern_hash(pattern_text);
        let now = chrono::Utc::now().timestamp();
        let conn = match self.pool.get() {
            Ok(c) => c,
            Err(_) => return,
        };
        let _ = conn.execute(
            "INSERT INTO pattern_memory (pattern_hash, pattern_text, tool_family, first_seen, last_seen, occurrence_count)
             VALUES (?1, ?2, ?3, ?4, ?4, 1)
             ON CONFLICT(pattern_hash) DO UPDATE SET
               last_seen = ?4,
               occurrence_count = occurrence_count + 1,
               tool_family = CASE WHEN tool_family = '' THEN ?3 ELSE tool_family END",
            params![hash, &pattern_text[..pattern_text.len().min(500)], tool_family, now],
        );
    }

    /// Mark a pattern as resolved (command succeeded after failure)
    pub fn resolve_pattern(&self, tool_family: &str, resolution_hint: &str) {
        let now = chrono::Utc::now().timestamp();
        let conn = match self.pool.get() {
            Ok(c) => c,
            Err(_) => return,
        };
        // Resolve patterns for this tool family that were seen in the last hour
        let _ = conn.execute(
            "UPDATE pattern_memory SET was_resolved = 1, resolution_hint = ?1
             WHERE tool_family = ?2 AND was_resolved = 0 AND last_seen > ?3",
            params![
                &resolution_hint[..resolution_hint.len().min(500)],
                tool_family,
                now - 3600
            ],
        );
    }

    /// Get recurring patterns for a tool family (sorted by occurrence count)
    pub fn get_patterns(&self, tool_family: Option<&str>, limit: usize) -> Vec<PatternMemoryRow> {
        let conn = match self.pool.get() {
            Ok(c) => c,
            Err(_) => return vec![],
        };
        let (query, tool_val) = if let Some(tool) = tool_family {
            (
                "SELECT pattern_hash, pattern_text, tool_family, first_seen, last_seen, \
                 occurrence_count, was_resolved, resolution_hint \
                 FROM pattern_memory WHERE tool_family = ?1 \
                 ORDER BY occurrence_count DESC LIMIT ?2",
                tool.to_string(),
            )
        } else {
            (
                "SELECT pattern_hash, pattern_text, tool_family, first_seen, last_seen, \
                 occurrence_count, was_resolved, resolution_hint \
                 FROM pattern_memory WHERE 1=1 \
                 ORDER BY occurrence_count DESC LIMIT ?2",
                String::new(),
            )
        };

        let mut stmt = match conn.prepare(query) {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        if tool_family.is_some() {
            match stmt.query_map(params![tool_val, limit as i64], |row| {
                Ok(PatternMemoryRow {
                    pattern_hash: row.get(0)?,
                    pattern_text: row.get(1)?,
                    tool_family: row.get(2)?,
                    first_seen: row.get(3)?,
                    last_seen: row.get(4)?,
                    occurrence_count: row.get(5)?,
                    was_resolved: row.get::<_, i64>(6)? != 0,
                    resolution_hint: row.get(7)?,
                })
            }) {
                Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
                Err(_) => vec![],
            }
        } else {
            match stmt.query_map(params!["", limit as i64], |row| {
                Ok(PatternMemoryRow {
                    pattern_hash: row.get(0)?,
                    pattern_text: row.get(1)?,
                    tool_family: row.get(2)?,
                    first_seen: row.get(3)?,
                    last_seen: row.get(4)?,
                    occurrence_count: row.get(5)?,
                    was_resolved: row.get::<_, i64>(6)? != 0,
                    resolution_hint: row.get(7)?,
                })
            }) {
                Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
                Err(_) => vec![],
            }
        }
    }

    /// Get top recurring issues across all tools
    pub fn get_top_insights(&self, limit: usize) -> Vec<PatternMemoryRow> {
        let conn = match self.pool.get() {
            Ok(c) => c,
            Err(_) => return vec![],
        };
        let mut stmt = match conn.prepare(
            "SELECT pattern_hash, pattern_text, tool_family, first_seen, last_seen, \
             occurrence_count, was_resolved, resolution_hint \
             FROM pattern_memory \
             WHERE occurrence_count >= 2 \
             ORDER BY occurrence_count DESC, last_seen DESC \
             LIMIT ?1",
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        match stmt.query_map(params![limit as i64], |row| {
            Ok(PatternMemoryRow {
                pattern_hash: row.get(0)?,
                pattern_text: row.get(1)?,
                tool_family: row.get(2)?,
                first_seen: row.get(3)?,
                last_seen: row.get(4)?,
                occurrence_count: row.get(5)?,
                was_resolved: row.get::<_, i64>(6)? != 0,
                resolution_hint: row.get(7)?,
            })
        }) {
            Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
            Err(_) => vec![],
        }
    }

    fn pattern_hash(text: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let normalized = text.trim().to_lowercase();
        let mut hasher = DefaultHasher::new();
        normalized.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }
}

// ── v0.5.7 Row Types ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PatternMemoryRow {
    pub pattern_hash: String,
    pub pattern_text: String,
    pub tool_family: String,
    pub first_seen: i64,
    pub last_seen: i64,
    pub occurrence_count: i64,
    pub was_resolved: bool,
    pub resolution_hint: String,
}

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
    fn open_creates_database_and_schema() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("omni.db");

        let store = Store::open_path(&db_path).unwrap();

        let conn = store.pool.get().unwrap();
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
    fn record_distillation_does_not_panic() {
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
            raw_tokens: 20,
            filtered_tokens: 5,
        };
        // Should not panic
        store.record_distillation("sess_123", &res, "npm start", "", "claude_code");
    }

    #[test]
    fn rewinds_and_retrieves_content() {
        let (store, _dir) = get_temp_store();
        let content = "this is some compressed content";
        let hash = store.store_rewind(content);

        assert!(hash.contains('_'));

        let retrieved = store.retrieve_rewind(&hash);
        assert_eq!(retrieved, Some(content.to_string()));

        // Retrieved counts updated
        let conn = store.pool.get().unwrap();
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
    fn duplicate_rewind_hashes_are_unique() {
        let (store, _dir) = get_temp_store();
        let content = "duplicate me";
        let hash1 = store.store_rewind(content);
        let hash2 = store.store_rewind(content); // duplicate

        assert_ne!(hash1, hash2);
        assert_eq!(store.retrieve_rewind(&hash1), Some(content.to_string()));
        assert_eq!(store.retrieve_rewind(&hash2), Some(content.to_string()));
    }

    #[test]
    fn indexes_and_searches_session_events() {
        let (store, _dir) = get_temp_store();
        store.index_event("sess_1", "command", "git status is running fast");
        store.index_event("sess_1", "command", "npm install");
        store.index_event("sess_2", "command", "git status is running"); // diff session

        let res = store.search_session_events("sess_1", "running", 10);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0], "git status is running fast");
    }

    #[test]
    fn fts5_stems_search_terms() {
        let (store, _dir) = get_temp_store();
        store.index_event("sess_2", "log", "The server is running now");

        // Porter stemming makes 'run' match 'running'
        let res = store.search_session_events("sess_2", "run", 10);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0], "The server is running now");
    }

    #[test]
    fn cleanup_removes_stale_entries() {
        let (store, _dir) = get_temp_store();
        let old_ts = chrono::Utc::now().timestamp() - (5 * 86400); // 5 days ago

        let conn = store.pool.get().unwrap();
        conn.execute("INSERT INTO distillations (session_id, ts, filter_name, input_bytes, output_bytes, route, latency_ms) VALUES ('sess_1', ?1, 'f', 1, 1, 'K', 1)", [old_ts]).unwrap();
        drop(conn);

        store.cleanup_old(2); // keep last 2 days

        let conn = store.pool.get().unwrap();
        let count: i32 = conn
            .query_row("SELECT COUNT(*) FROM distillations", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }
}
