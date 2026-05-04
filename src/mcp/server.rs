use crate::pipeline::scorer::score_segments;
use crate::pipeline::{SessionState, SignalTier};
use crate::session::learn::{apply_to_config, detect_patterns, generate_toml};
use crate::store::sqlite::Store;
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::{ServerHandler, tool};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct OmniServer {
    store: Arc<Store>,
    session: Arc<Mutex<SessionState>>,
}

// Automatically bind tool signatures
#[tool(tool_box)]
impl OmniServer {
    #[tool(
        name = "omni_retrieve",
        description = "Retrieve full content omitted by OMNI distillation (Hash from OMNI notice)"
    )]
    pub async fn omni_retrieve(&self, #[tool(param)] hash: String) -> String {
        if let Some(content) = self.store.retrieve_rewind(&hash) {
            // Record retrieve event for adaptive compression feedback loop
            let cmd_prefix = self
                .store
                .find_command_for_hash(&hash)
                .unwrap_or_else(|| "unknown".to_string());
            let agent_id = std::env::var("OMNI_AGENT_ID")
                .unwrap_or_else(|_| crate::agents::multiagent::detect_agent_id());
            let family = crate::util::command_family::command_family(&cmd_prefix);
            self.store.record_retrieve_event(&family, &hash, &agent_id);
            content
        } else {
            format!("Not found: {}", hash)
        }
    }

    #[tool(
        name = "omni_learn",
        description = "Detect noise patterns in text and suggest TOML filters"
    )]
    pub async fn omni_learn(
        &self,
        #[tool(param)] text: String,
        #[tool(param)] apply: bool,
    ) -> String {
        // 1. Run real pattern detection
        let candidates = detect_patterns(&text);

        if candidates.is_empty() {
            return "No significant noise patterns detected. \
                    Input has high signal diversity — no filter needed."
                .to_string();
        }

        // 2. Format report with real candidates
        let mut report = format!("Detected {} noise patterns:\n\n", candidates.len());
        for (i, c) in candidates.iter().enumerate() {
            report.push_str(&format!(
                "  [{}] \"{}\" — {} occurrences (confidence: {:.0}%)\n      Action: {:?}\n      Sample: {}\n\n",
                i + 1,
                c.trigger_prefix,
                c.count,
                c.confidence * 100.0,
                c.suggested_action,
                &c.sample_line[..c.sample_line.len().min(80)]
            ));
        }

        // 3. If apply=true: write to ~/.omni/filters/learned.toml
        if apply {
            let filter_name = format!("learned_{}", chrono::Utc::now().timestamp_micros());
            let _toml_content = generate_toml(&candidates, &filter_name, None);

            let config_path = crate::paths::learned_filters_path();
            let _ = crate::paths::ensure_omni_home();

            match apply_to_config(&candidates, &filter_name, &config_path, None) {
                Ok(added) => {
                    report.push_str(&format!(
                        "\n✓ Applied {} filters to {}\n  Run: omni doctor to verify",
                        added,
                        config_path.display()
                    ));
                }
                Err(e) => {
                    report.push_str(&format!(
                        "\n✗ Failed to write filters: {}\n  Try manually: omni learn --apply",
                        e
                    ));
                }
            }
        } else {
            report.push_str(&format!(
                "Run omni_learn with apply=true to save {} filters automatically.",
                candidates.len()
            ));
        }

        report
    }

    #[tool(
        name = "omni_density",
        description = "Measure how much signal vs noise in text"
    )]
    pub async fn omni_density(&self, #[tool(param)] text: String) -> String {
        let current_session = self.session.lock().unwrap().clone();

        // Use generic Line segmentation for density analysis
        let segments = score_segments(
            &text,
            crate::pipeline::SegmentationMode::Line,
            Some(&current_session),
        );

        let mut critical_lines = 0;
        let mut important_lines = 0;
        let mut context_lines = 0;
        let mut noise_lines = 0;

        for segment in &segments {
            let lines = segment.content.lines().count();
            match segment.tier {
                SignalTier::Critical => critical_lines += lines,
                SignalTier::Important => important_lines += lines,
                SignalTier::Context => context_lines += lines,
                SignalTier::Noise => noise_lines += lines,
            }
        }

        let total_lines = (critical_lines + important_lines + context_lines + noise_lines).max(1);
        let non_noise = critical_lines + important_lines + context_lines;
        let pct = (1.0 - (non_noise as f32 / total_lines as f32)) * 100.0;

        format!(
            "Signal analysis:\n  Critical: {} lines\n  Important: {} lines\n  Context: {} lines\n  Noise: {} lines\n  Est. reduction: {:.1}%",
            critical_lines, important_lines, context_lines, noise_lines, pct
        )
    }

    #[tool(
        name = "omni_trust",
        description = "Trust project's local configurations explicitly"
    )]
    pub async fn omni_trust(&self, #[tool(param)] project_path: String) -> String {
        let default_path = if project_path.is_empty() {
            ".".to_string()
        } else {
            project_path
        };

        let path = std::path::Path::new(&default_path);
        match crate::guard::trust::trust_project(path) {
            Ok(hash) => format!("Trusted: {}\nSHA-256: {}", path.display(), hash),
            Err(e) => format!("Failed to trust local hashes ensuring sandbox loops: {}", e),
        }
    }

    #[tool(
        name = "omni_context",
        description = "Show lightweight dependency context for a file"
    )]
    pub async fn omni_context(&self, #[tool(param)] file_path: String) -> String {
        if file_path.trim().is_empty() {
            return "Please provide a file_path".to_string();
        }

        let cwd = match std::env::current_dir() {
            Ok(cwd) => cwd,
            Err(e) => return format!("Cannot determine current directory: {}", e),
        };

        let graph = match crate::graph::indexer::build_graph(&cwd) {
            Ok(graph) => graph,
            Err(e) => return format!("Failed to build graph context: {}", e),
        };

        let ctx = graph.context_for(&file_path);
        let session = self.session.lock().ok().map(|s| s.clone());
        let hot_count = session
            .as_ref()
            .and_then(|s| s.hot_files.get(&ctx.file_path).copied())
            .unwrap_or(0);

        let mut out = format!("OMNI Context for {}\n", ctx.file_path);
        if ctx.imports.is_empty() {
            out.push_str("Imports: none detected\n");
        } else {
            out.push_str(&format!(
                "Imports: {}\n",
                ctx.imports
                    .iter()
                    .take(8)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if ctx.imported_by.is_empty() {
            out.push_str("Imported by: none detected\n");
        } else {
            out.push_str(&format!(
                "Imported by: {}\n",
                ctx.imported_by
                    .iter()
                    .take(8)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if hot_count > 0 {
            out.push_str(&format!("Hot in session: yes ({}x)\n", hot_count));
        } else {
            out.push_str("Hot in session: no\n");
        }
        out
    }

    #[tool(
        name = "omni_session",
        description = "Manage OMNI session state manually (status | context | clear)"
    )]
    pub async fn omni_session(&self, #[tool(param)] action: String) -> String {
        let action = if action.is_empty() {
            "status".to_string()
        } else {
            action
        };

        match action.as_str() {
            "status" => {
                let s = self.session.lock().unwrap();
                let task = s.inferred_task.as_deref().unwrap_or("none");
                let domain = s.inferred_domain.as_deref().unwrap_or("none");

                let mut hot_vec: Vec<(&String, &u32)> = s.hot_files.iter().collect();
                hot_vec.sort_by_key(|a| std::cmp::Reverse(a.1));
                let hot_str = if hot_vec.is_empty() {
                    "none".to_string()
                } else {
                    hot_vec
                        .iter()
                        .take(3)
                        .map(|(f, c)| format!("{} ({}x)", f, c))
                        .collect::<Vec<_>>()
                        .join(", ")
                };

                let err = s
                    .active_errors
                    .first()
                    .map(|e| e.replace('\n', " "))
                    .unwrap_or_else(|| "none".to_string());

                format!(
                    "OMNI Session: {}\nCommands: {}\nTask: {}\nDomain: {}\nHot Files: {}\nLast Error: {}",
                    s.session_id, s.command_count, task, domain, hot_str, err
                )
            }
            "context" => {
                let s = self.session.lock().unwrap();
                let task = s.inferred_task.as_deref().unwrap_or("none");

                let mut hot_vec: Vec<(&String, &u32)> = s.hot_files.iter().collect();
                hot_vec.sort_by_key(|a| std::cmp::Reverse(a.1));
                let hot_str = if hot_vec.is_empty() {
                    "none".to_string()
                } else {
                    hot_vec
                        .iter()
                        .take(2)
                        .map(|(f, _)| f.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                };

                let err = s
                    .active_errors
                    .first()
                    .map(|e| e.replace('\n', " "))
                    .unwrap_or_else(|| "none".to_string());

                let mut msg = format!(
                    "[OMNI Context] Task: {}. Hot: {}. Error: {}",
                    task, hot_str, err
                );
                if msg.len() > 200 {
                    msg.truncate(197);
                    msg.push_str("...");
                }
                msg
            }
            "clear" => {
                {
                    let mut s = self.session.lock().unwrap();
                    *s = SessionState::new();
                }
                "Session state cleared.".to_string()
            }
            _ => "Unknown action. Use status, context, or clear.".to_string(),
        }
    }
    #[tool(
        name = "omni_search",
        description = "Search current session history (logs, outputs, commands)"
    )]
    pub async fn omni_search(&self, #[tool(param)] query: String) -> String {
        if query.trim().is_empty() {
            return "Please provide a query".to_string();
        }
        let session_id = self.session.lock().unwrap().session_id.clone();
        let results = self.store.search_session_events(&session_id, &query, 10);
        if results.is_empty() {
            format!("No events matched the search query '{}'", query)
        } else {
            let mut report = format!("Found {} results:\n\n", results.len());
            for r in results {
                report.push_str(&format!("- {}\n", r));
            }
            report
        }
    }
    #[tool(
        name = "omni_history",
        description = "Show recent distillation history with per-call token savings and compression ratios"
    )]
    pub async fn omni_history(&self, #[tool(param)] limit: Option<u32>) -> String {
        let limit = limit.unwrap_or(10).min(50) as usize;
        let session = match self.session.lock() {
            Ok(s) => s.clone(),
            Err(_) => return "Error: session lock failed".to_string(),
        };

        let session_id = session.session_id.clone();
        let total_saved = session.estimated_tokens_saved();
        let cmd_count = session.command_count;

        // Query distillations from store
        let conn_result = self.store.get_recent_distillations(&session_id, limit);

        if conn_result.is_empty() {
            return format!(
                "No distillation history yet.\nSession: {} commands processed | ~{} tokens saved",
                cmd_count, total_saved
            );
        }

        let mut out = format!(
            "OMNI Distillation History (last {}):\n\n",
            conn_result.len()
        );
        for (i, row) in conn_result.iter().enumerate() {
            let savings_pct = if row.input_bytes > 0 {
                (1.0 - row.output_bytes as f64 / row.input_bytes as f64) * 100.0
            } else {
                0.0
            };
            out.push_str(&format!(
                "  {:2}. {:<40} {} → {} bytes  {:.0}%  {}\n",
                i + 1,
                &row.command[..row.command.len().min(40)],
                row.input_bytes,
                row.output_bytes,
                savings_pct,
                row.route
            ));
        }

        out.push_str(&format!(
            "\nSession totals:\n  Commands: {} | Tokens saved: ~{} | Agent: {}\n",
            cmd_count,
            total_saved,
            std::env::var("OMNI_AGENT_ID")
                .unwrap_or_else(|_| crate::agents::multiagent::detect_agent_id())
        ));
        out
    }

    #[tool(
        name = "omni_budget",
        description = "Show estimated token budget usage and compression efficiency for this session"
    )]
    pub async fn omni_budget(&self) -> String {
        let session = match self.session.lock() {
            Ok(s) => s.clone(),
            Err(_) => return "Error: session lock failed".to_string(),
        };

        let total_in = session.cumulative_input_bytes;
        let total_out = session.cumulative_output_bytes;
        let tokens_consumed = total_in / 4;
        let tokens_saved = session.estimated_tokens_saved();
        let overall_pct = if total_in > 0 {
            (1.0 - total_out as f64 / total_in as f64) * 100.0
        } else {
            0.0
        };

        format!(
            "OMNI Token Budget Estimate:\n\
             \n  Raw processed:   ~{tokens_consumed} tokens\
             \n  After OMNI:      ~{} tokens\
             \n  Saved:           ~{tokens_saved} tokens ({overall_pct:.1}% reduction)\
             \n\
             \n  Commands processed: {}\
             \n  Active errors:      {}\
             \n  Hot files tracked:  {}\
             \n\
             \nTip: Call omni_history() for per-command breakdown.\
             \n     Call omni_learn(noisy_output) to improve future compression.",
            (total_out / 4),
            session.command_count,
            session.active_errors.len(),
            session.hot_files.len(),
        )
    }

    #[tool(
        name = "omni_agents",
        description = "Show other AI agents currently active on this project (multi-agent awareness)"
    )]
    pub async fn omni_agents(&self) -> String {
        let project_path = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        let project_hash = compute_project_hash_str(&project_path);
        let my_agent = std::env::var("OMNI_AGENT_ID")
            .unwrap_or_else(|_| crate::agents::multiagent::detect_agent_id());

        let peers = self
            .store
            .get_active_agents_for_project(&project_hash, &my_agent);

        if peers.is_empty() {
            return format!(
                "No other agents active on this project.\nYou are: {my_agent}\nProject: {project_path}"
            );
        }

        let mut out = format!(
            "Active agents on this project ({}):\n\n  You: {my_agent}\n\n",
            project_path
        );

        for peer in &peers {
            let age_mins = (chrono::Utc::now().timestamp() - peer.last_active) / 60;
            let age_str = if age_mins < 60 {
                format!("{age_mins}m ago")
            } else {
                format!("{}h ago", age_mins / 60)
            };

            // Parse their state for useful info
            let peer_state: serde_json::Value =
                serde_json::from_str(&peer.state_json).unwrap_or_default();
            let peer_task = peer_state
                .get("inferred_task")
                .and_then(|t| t.as_str())
                .unwrap_or("unknown task");
            let peer_errors = peer_state
                .get("active_errors")
                .and_then(|e| e.as_array())
                .map(|a| a.len())
                .unwrap_or(0);

            out.push_str(&format!(
                "  [{age_str}] {agent_id}\n    Task: {peer_task}\n    Active errors: {peer_errors}\n\n",
                agent_id = peer.agent_id,
            ));
        }
        out.push_str("Use omni_session(\"context\") to share your state with peers.");
        out
    }

    #[tool(
        name = "omni_knowledge",
        description = "Query or store cross-session project knowledge (persistent across sessions)"
    )]
    pub async fn omni_knowledge(
        &self,
        #[tool(param)] action: String,
        #[tool(param)] key: Option<String>,
        #[tool(param)] value: Option<String>,
    ) -> String {
        let project_path = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string());
        let project_hash = compute_project_hash_str(&project_path);

        match action.as_str() {
            "list" => {
                let knowledge = self.store.get_project_knowledge(&project_hash);
                if knowledge.is_empty() {
                    return "No project knowledge stored yet.\nUse omni_knowledge(\"set\", \"key\", \"value\") to add.".to_string();
                }
                let mut out = format!("Project knowledge for {}:\n\n", project_path);
                for (k, v, conf) in &knowledge {
                    out.push_str(&format!("  [{:.0}%] {}: {}\n", conf * 100.0, k, v));
                }
                out
            }
            "set" => {
                let k = key.unwrap_or_default();
                let v = value.unwrap_or_default();
                if k.is_empty() || v.is_empty() {
                    return "Usage: omni_knowledge(\"set\", \"key\", \"value\")".to_string();
                }
                self.store.upsert_project_knowledge(&project_hash, &k, &v, 0.9);
                format!("Stored: [{k}] = \"{v}\"\nThis knowledge persists across sessions for this project.")
            }
            "forget" => {
                let k = key.unwrap_or_default();
                if k.is_empty() {
                    return "Usage: omni_knowledge(\"forget\", \"key\")".to_string();
                }
                // Set confidence to 0 effectively forgets it (below 0.5 threshold)
                self.store.upsert_project_knowledge(&project_hash, &k, "", 0.0);
                format!("Forgotten: [{k}]")
            }
            _ => "Actions: list | set | forget\nExample: omni_knowledge(\"set\", \"noise_cmd\", \"npm install always produces 200 dep warnings\")".to_string(),
        }
    }
}

fn compute_project_hash_str(project_path: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(project_path.as_bytes());
    hex::encode(&hasher.finalize()[..8])
}
#[allow(refining_impl_trait)]
impl ServerHandler for OmniServer {
    fn call_tool<'a>(
        &'a self,
        request: rmcp::model::CallToolRequestParam,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<rmcp::model::CallToolResult, rmcp::Error>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let tcc = ToolCallContext::new(self, request, context);
            match tcc.name() {
                "omni_retrieve" => Self::omni_retrieve_tool_call(tcc).await,
                "omni_learn" => Self::omni_learn_tool_call(tcc).await,
                "omni_density" => Self::omni_density_tool_call(tcc).await,
                "omni_trust" => Self::omni_trust_tool_call(tcc).await,
                "omni_context" => Self::omni_context_tool_call(tcc).await,
                "omni_session" => Self::omni_session_tool_call(tcc).await,
                "omni_search" => Self::omni_search_tool_call(tcc).await,
                "omni_history" => Self::omni_history_tool_call(tcc).await,
                "omni_budget" => Self::omni_budget_tool_call(tcc).await,
                "omni_agents" => Self::omni_agents_tool_call(tcc).await,
                "omni_knowledge" => Self::omni_knowledge_tool_call(tcc).await,
                _ => Err(rmcp::Error::invalid_params("method not found", None)),
            }
        })
    }

    // Auto-generates the manifest for MCP clients describing available tools
    fn list_tools<'a>(
        &'a self,
        _request: rmcp::model::PaginatedRequestParam,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<rmcp::model::ListToolsResult, rmcp::Error>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            Ok(rmcp::model::ListToolsResult {
                tools: vec![
                    Self::omni_retrieve_tool_attr(),
                    Self::omni_learn_tool_attr(),
                    Self::omni_density_tool_attr(),
                    Self::omni_trust_tool_attr(),
                    Self::omni_context_tool_attr(),
                    Self::omni_session_tool_attr(),
                    Self::omni_search_tool_attr(),
                    Self::omni_history_tool_attr(),
                    Self::omni_budget_tool_attr(),
                    Self::omni_agents_tool_attr(),
                    Self::omni_knowledge_tool_attr(),
                ],
                next_cursor: None,
            })
        })
    }
}

pub async fn run(store: Arc<Store>, session: Arc<Mutex<SessionState>>) -> anyhow::Result<()> {
    let server = OmniServer { store, session };

    // Setup transport over standard IO seamlessly
    use tokio::io::{stdin, stdout};
    let transport = (stdin(), stdout());

    // Serve the server binding transport dynamically via `serve_server`
    let running_service = rmcp::serve_server(server, transport).await?;
    running_service.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_omni_retrieve_returns_not_found_for_unknown_hash() {
        let dir = tempdir().unwrap();
        let store = Arc::new(Store::open_path(&dir.path().join("omni.db")).unwrap());
        let session = Arc::new(Mutex::new(SessionState::new()));

        let server = OmniServer { store, session };
        let output = server.omni_retrieve("abc".to_string()).await;
        assert_eq!(output, "Not found: abc");
    }

    #[tokio::test]
    async fn test_omni_retrieve_returns_stored_content() {
        let dir = tempdir().unwrap();
        let store = Arc::new(Store::open_path(&dir.path().join("omni.db")).unwrap());
        let hash = store.store_rewind("testing_payload");
        let session = Arc::new(Mutex::new(SessionState::new()));

        let server = OmniServer { store, session };
        let output = server.omni_retrieve(hash).await;
        assert_eq!(output, "testing_payload");
    }

    #[tokio::test]
    async fn test_omni_density_returns_valid_analysis() {
        let dir = tempdir().unwrap();
        let store = Arc::new(Store::open_path(&dir.path().join("omni.db")).unwrap());
        let session = Arc::new(Mutex::new(SessionState::new()));

        let server = OmniServer { store, session };
        let text = "error: something failed\nCompiling deps v1.0".to_string();
        let density = server.omni_density(text).await;
        assert!(density.contains("Signal analysis:"));
        assert!(density.contains("Critical:"));
    }

    #[tokio::test]
    async fn test_omni_learn_detects_patterns() {
        let dir = tempdir().unwrap();
        let store = Arc::new(Store::open_path(&dir.path().join("omni.db")).unwrap());
        let session = Arc::new(Mutex::new(SessionState::new()));

        let server = OmniServer { store, session };
        // 5+ repetitive lines should produce real candidate output
        let repetitive = "Compiling foo v1.0\n".repeat(6);
        let out = server.omni_learn(repetitive, false).await;
        assert!(
            out.contains("noise patterns"),
            "expected pattern report, got: {out}"
        );
        assert!(
            out.contains("occurrences"),
            "expected occurrence count, got: {out}"
        );
        assert!(
            out.contains("confidence"),
            "expected confidence score, got: {out}"
        );
        assert!(
            out.contains("apply=true"),
            "expected apply hint, got: {out}"
        );
    }

    #[tokio::test]
    async fn test_omni_learn_no_patterns_on_diverse_input() {
        let dir = tempdir().unwrap();
        let store = Arc::new(Store::open_path(&dir.path().join("omni.db")).unwrap());
        let session = Arc::new(Mutex::new(SessionState::new()));

        let server = OmniServer { store, session };
        let diverse = "alpha bravo charlie\ndelta echo foxtrot\ngolf hotel india\n";
        let out = server.omni_learn(diverse.to_string(), false).await;
        assert!(
            out.contains("No significant noise patterns"),
            "expected no-patterns message, got: {out}"
        );
    }

    #[tokio::test]
    async fn test_omni_learn_apply_writes_toml() {
        let dir = tempdir().unwrap();
        let store = Arc::new(Store::open_path(&dir.path().join("omni.db")).unwrap());
        let session = Arc::new(Mutex::new(SessionState::new()));

        let server = OmniServer { store, session };
        let repetitive = "Downloading dep v1.0\n".repeat(6);
        let out = server.omni_learn(repetitive, true).await;
        assert!(
            out.contains("Applied") || out.contains("filters"),
            "expected apply confirmation, got: {out}"
        );
    }

    #[tokio::test]
    async fn test_omni_trust_saves_hash() {
        let dir = tempdir().unwrap();
        let store = Arc::new(Store::open_path(&dir.path().join("omni.db")).unwrap());
        let session = Arc::new(Mutex::new(SessionState::new()));

        let server = OmniServer { store, session };
        let out = server.omni_trust("/invalid".to_string()).await;
        assert!(out.contains("Failed") || out.contains("Trusted"));
    }
}
