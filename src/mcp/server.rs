use crate::pipeline::scorer::score_segments;
use crate::pipeline::{SessionState, SignalTier};
use crate::session::learn::{apply_to_config, detect_patterns, generate_toml};
use crate::store::sqlite::Store;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct OmniServer {
    store: Arc<Store>,
    session: Arc<Mutex<SessionState>>,
}

#[derive(Deserialize, JsonSchema)]
pub struct OmniRetrieveParams {
    pub hash: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct OmniLearnParams {
    pub text: String,
    pub apply: bool,
}

#[derive(Deserialize, JsonSchema)]
pub struct OmniDensityParams {
    pub text: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct OmniQueryParams {
    pub query: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct OmniRecallParams {
    pub tool: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct OmniTrustParams {
    pub project_path: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct OmniContextParams {
    pub file_path: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct OmniSessionParams {
    pub action: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct OmniSearchParams {
    pub query: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct OmniHistoryParams {
    pub limit: Option<u32>,
}

#[derive(Deserialize, JsonSchema)]
pub struct OmniExplainSavingsParams {
    pub limit: Option<u32>,
}

#[derive(Deserialize, JsonSchema)]
pub struct OmniFindNoiseParams {
    pub limit: Option<u32>,
}

#[derive(Deserialize, JsonSchema)]
pub struct OmniKnowledgeParams {
    pub action: String,
    pub key: Option<String>,
    pub value: Option<String>,
}

// Automatically bind tool signatures
#[tool_router(server_handler)]
impl OmniServer {
    #[tool(
        name = "omni_retrieve",
        description = "Retrieve full content omitted by OMNI distillation (Hash from OMNI notice)"
    )]
    pub async fn omni_retrieve(&self, params: Parameters<OmniRetrieveParams>) -> String {
        let hash = params.0.hash;
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
    pub async fn omni_learn(&self, params: Parameters<OmniLearnParams>) -> String {
        let text = params.0.text;
        let apply = params.0.apply;
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

        // 3. If apply=true: write to ~/.omni/signals/learned.toml
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
    pub async fn omni_density(&self, params: Parameters<OmniDensityParams>) -> String {
        let text = params.0.text;
        let current_session = self.session.lock().unwrap().clone();

        // Use generic Line segmentation for density analysis
        let segments = score_segments(
            &text,
            crate::pipeline::SegmentationMode::Line,
            Some(&current_session),
            "omni_density",
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
        name = "omni_query",
        description = "Query distillation history using OmniQL. Supported queries: 'errors in last N commands', 'warnings from <tool>', 'context for <file_path>', 'timeline today'"
    )]
    pub async fn omni_query(&self, params: Parameters<OmniQueryParams>) -> String {
        let query = params.0.query;
        match self.store.execute_omni_query(&query) {
            Ok(result) => serde_json::to_string_pretty(&result)
                .unwrap_or_else(|e| format!("Serialization error: {}", e)),
            Err(e) => format!("OmniQL error: {}", e),
        }
    }

    #[tool(
        name = "omni_recall",
        description = "Recall cross-session error patterns for a specific tool (e.g., cargo, npm) and what fixed them"
    )]
    pub async fn omni_recall(&self, params: Parameters<OmniRecallParams>) -> String {
        let tool = params.0.tool;
        let patterns = self.store.get_patterns(Some(&tool), 5);
        if patterns.is_empty() {
            return format!("No recurring patterns found for tool: {}", tool);
        }

        let mut report = format!(
            "Found {} recurring patterns for {}:\n\n",
            patterns.len(),
            tool
        );
        for (i, p) in patterns.iter().enumerate() {
            report.push_str(&format!(
                "[{}] Seen {}x | Status: {}\n",
                i + 1,
                p.occurrence_count,
                if p.was_resolved { "RESOLVED" } else { "ACTIVE" }
            ));

            let lines: Vec<&str> = p.pattern_text.lines().collect();
            for line in lines.iter().take(3) {
                report.push_str(&format!("  {}\n", line));
            }
            if lines.len() > 3 {
                report.push_str("  ...\n");
            }
            if p.was_resolved && !p.resolution_hint.is_empty() {
                report.push_str(&format!("  Fix hint: {}\n", p.resolution_hint));
            }
            report.push('\n');
        }
        report
    }

    #[tool(
        name = "omni_insight",
        description = "Show the top recurring issues and error patterns across the entire project"
    )]
    pub async fn omni_insight(&self) -> String {
        let patterns = self.store.get_top_insights(5);
        if patterns.is_empty() {
            return "No recurring issues detected yet.".to_string();
        }

        let mut report = format!("Top {} recurring issues:\n\n", patterns.len());
        for (i, p) in patterns.iter().enumerate() {
            report.push_str(&format!(
                "[{}] Tool: {} | Seen {}x | Status: {}\n",
                i + 1,
                p.tool_family,
                p.occurrence_count,
                if p.was_resolved { "RESOLVED" } else { "ACTIVE" }
            ));
            let mut pattern_preview = p.pattern_text.replace('\n', " ");
            if pattern_preview.len() > 100 {
                pattern_preview.truncate(97);
                pattern_preview.push_str("...");
            }
            report.push_str(&format!("  Pattern: {}\n\n", pattern_preview));
        }
        report
    }

    #[tool(
        name = "omni_trust",
        description = "Trust project's local configurations explicitly"
    )]
    pub async fn omni_trust(&self, params: Parameters<OmniTrustParams>) -> String {
        let project_path = params.0.project_path;
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
    pub async fn omni_context(&self, params: Parameters<OmniContextParams>) -> String {
        let file_path = params.0.file_path;
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
    pub async fn omni_session(&self, params: Parameters<OmniSessionParams>) -> String {
        let action = params.0.action;
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
            "summary" => {
                let s = self.session.lock().unwrap();
                let tool_summary = crate::session::engram::format_tool_summary(&s.tool_call_log);
                if tool_summary.is_empty() {
                    "No tool calls recorded yet.".to_string()
                } else {
                    tool_summary
                }
            }
            _ => "Unknown action. Use status, context, summary, or clear.".to_string(),
        }
    }
    #[tool(
        name = "omni_search",
        description = "Search current session history (logs, outputs, commands)"
    )]
    pub async fn omni_search(&self, params: Parameters<OmniSearchParams>) -> String {
        let query = params.0.query;
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
    pub async fn omni_history(&self, params: Parameters<OmniHistoryParams>) -> String {
        let limit = params.0.limit;
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
        name = "omni_explain_savings",
        description = "Explain why recent commands were compressed: shows route, filter, input/output bytes, and savings %"
    )]
    pub async fn omni_explain_savings(
        &self,
        params: Parameters<OmniExplainSavingsParams>,
    ) -> String {
        let limit = params.0.limit;
        let limit = limit.unwrap_or(10).min(50) as usize;
        let session_id = self
            .session
            .lock()
            .ok()
            .map(|s| s.session_id.clone())
            .unwrap_or_default();
        let rows = self.store.get_recent_distillations(&session_id, limit);
        if rows.is_empty() {
            return "No recent distillations found in current session.".to_string();
        }
        let mut out = format!(
            "OMNI Savings Explanation (last {} commands):\n\n",
            rows.len()
        );
        for d in &rows {
            let pct = if d.input_bytes > 0 {
                100.0 - (d.output_bytes as f64 / d.input_bytes as f64) * 100.0
            } else {
                0.0
            };
            let filter_display = if !d.filter_name.is_empty() {
                format!(" [filter: {}]", d.filter_name)
            } else {
                String::new()
            };
            out.push_str(&format!(
                "- {}: {} → {} bytes ({:.0}% saved)\n  Route: {}{}\n",
                d.command, d.input_bytes, d.output_bytes, pct, d.route, filter_display
            ));
        }
        out
    }

    #[tool(
        name = "omni_find_noise",
        description = "Analyze recent raw terminal traces to identify repetitive noisy patterns and suggest TOML filters"
    )]
    pub async fn omni_find_noise(&self, params: Parameters<OmniFindNoiseParams>) -> String {
        let limit = params.0.limit;
        let limit = limit.unwrap_or(50).min(200) as usize;
        let traces = match self.store.get_recent_traces(limit) {
            Ok(t) => t,
            Err(_) => return "Failed to retrieve recent traces.".to_string(),
        };
        if traces.is_empty() {
            return "No recent traces found.".to_string();
        }
        let mut concatenated_raw = String::new();
        for (_, _, raw, _) in &traces {
            concatenated_raw.push_str(raw);
            concatenated_raw.push('\n');
        }
        let patterns = crate::session::learn::detect_patterns(&concatenated_raw);
        if patterns.is_empty() {
            return "No dominant noisy patterns detected in recent traces.".to_string();
        }
        let toml_snippet = crate::session::learn::generate_toml(&patterns, "omni_auto_noise", None);
        let mut out = format!(
            "OMNI Noise Analysis (from {} recent traces):\n\n",
            traces.len()
        );
        out.push_str("Identified repetitive patterns:\n");
        for (i, p) in patterns.iter().take(5).enumerate() {
            out.push_str(&format!(
                "{}. Prefix: '{}' (count: {}, conf: {:.2})\n",
                i + 1,
                p.trigger_prefix,
                p.count,
                p.confidence
            ));
        }
        out.push_str("\nSuggested TOML Signal (add to ~/.omni/signals/user.toml):\n\n```toml\n");
        out.push_str(&toml_snippet);
        out.push_str("\n```");
        out
    }

    #[tool(
        name = "omni_budget",
        description = "Show token budget usage and compression efficiency for this session"
    )]
    pub async fn omni_budget(&self) -> String {
        let session = match self.session.lock() {
            Ok(s) => s.clone(),
            Err(_) => return "Error: session lock failed".to_string(),
        };

        let raw_tokens = session.cumulative_raw_tokens;
        let filtered_tokens = session.cumulative_filtered_tokens;
        let tokens_saved = session.actual_tokens_saved();

        let overall_pct = if raw_tokens > 0 {
            (1.0 - filtered_tokens as f64 / raw_tokens as f64) * 100.0
        } else if session.cumulative_input_bytes > 0 {
            (1.0 - session.cumulative_output_bytes as f64 / session.cumulative_input_bytes as f64)
                * 100.0
        } else {
            0.0
        };

        let is_actual = raw_tokens > 0;
        let method = if is_actual { "actual" } else { "estimated" };

        // Fallback for legacy
        let display_raw = if is_actual {
            raw_tokens
        } else {
            session.cumulative_input_bytes / 4
        };
        let display_filtered = if is_actual {
            filtered_tokens
        } else {
            session.cumulative_output_bytes / 4
        };
        let display_saved = if is_actual {
            tokens_saved
        } else {
            session.estimated_tokens_saved()
        };

        let tilde = if is_actual { "" } else { "~" };

        format!(
            "OMNI Token Budget Report:\n\
             \n  Measurement Method: {}\n\
             \n  Raw processed:   {}{display_raw} tokens\
             \n  After OMNI:      {}{display_filtered} tokens\
             \n  Saved:           {}{display_saved} tokens ({overall_pct:.1}% reduction)\
             \n\
             \n  Commands processed: {}\
             \n  Active errors:      {}\
             \n  Hot files tracked:  {}\
             \n\
             \nTip: Call omni_history() for per-command breakdown.\
             \n     Call omni_learn(noisy_output) to improve future compression.",
            method,
            tilde,
            tilde,
            tilde,
            session.command_count,
            session.active_errors.len(),
            session.hot_files.len(),
        )
    }

    #[tool(
        name = "omni_context_breakdown",
        description = "Show token breakdown by source for the current context turn"
    )]
    pub async fn omni_context_breakdown(&self) -> String {
        let session = match self.session.lock() {
            Ok(s) => s.clone(),
            Err(_) => return "Error: session lock failed".to_string(),
        };

        let turn = &session.current_turn;
        serde_json::to_string_pretty(turn).unwrap_or_else(|_| "Serialization error".to_string())
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
    pub async fn omni_knowledge(&self, params: Parameters<OmniKnowledgeParams>) -> String {
        let action = params.0.action;
        let key = params.0.key;
        let value = params.0.value;
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

    #[tool(
        name = "omni_handoff",
        description = "Export current session state as portable markdown for pasting into a new session or terminal (no network needed)"
    )]
    pub async fn omni_handoff(&self) -> String {
        let session = match self.session.lock() {
            Ok(s) => s.clone(),
            Err(_) => return "Error: session lock failed".to_string(),
        };

        let task = session
            .inferred_task
            .as_deref()
            .unwrap_or("general development");
        let domain = session.inferred_domain.as_deref().unwrap_or("unknown");
        let agent_id = std::env::var("OMNI_AGENT_ID")
            .unwrap_or_else(|_| crate::agents::multiagent::detect_agent_id());

        let mut out = String::from("# OMNI Session Handoff\n\n");
        out.push_str(&format!("**Session:** {}\n", session.session_id));
        out.push_str(&format!("**Agent:** {}\n", agent_id));
        out.push_str(&format!("**Task:** {}\n", task));
        out.push_str(&format!("**Domain:** {}\n", domain));
        out.push_str(&format!("**Commands:** {}\n", session.command_count));
        out.push_str(&format!(
            "**Tokens Saved:** ~{}\n\n",
            session.estimated_tokens_saved()
        ));

        // Active Errors
        out.push_str("## Active Errors\n");
        if session.active_errors.is_empty() {
            out.push_str("None\n");
        } else {
            for err in &session.active_errors {
                let clean = err.replace('\n', " ");
                out.push_str(&format!("- {}\n", &clean[..clean.len().min(120)]));
            }
        }

        // Engrams
        out.push_str("\n## Subtask Progress\n");
        if session.engrams.is_empty() {
            out.push_str("No engrams recorded.\n");
        } else {
            for engram in &session.engrams {
                out.push_str(&engram.compact());
                out.push('\n');
            }
        }

        // Hot Files
        out.push_str("\n## Hot Files\n");
        let mut hot_vec: Vec<(&String, &u32)> = session.hot_files.iter().collect();
        hot_vec.sort_by_key(|a| std::cmp::Reverse(a.1));
        if hot_vec.is_empty() {
            out.push_str("None\n");
        } else {
            for (path, count) in hot_vec.iter().take(10) {
                out.push_str(&format!("- {} ({}x)\n", path, count));
            }
        }

        // Tool Call Summary
        let tool_summary = crate::session::engram::format_tool_summary(&session.tool_call_log);
        if !tool_summary.is_empty() {
            out.push('\n');
            out.push_str(&tool_summary);
        }

        // Recent Commands
        out.push_str("\n## Recent Commands\n");
        for cmd in session.last_commands.iter().take(10) {
            out.push_str(&format!("- `{}`\n", &cmd[..cmd.len().min(80)]));
        }

        // Context Pressure
        out.push_str(&format!(
            "\n## Context Pressure: {}\n",
            session.context_pressure
        ));

        out.push_str("\n---\n*Paste this into a new session to continue where you left off.*\n");
        out
    }
}

fn compute_project_hash_str(project_path: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(project_path.as_bytes());
    hex::encode(&hasher.finalize()[..8])
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
        let output = server
            .omni_retrieve(Parameters(OmniRetrieveParams {
                hash: "abc".to_string(),
            }))
            .await;
        assert_eq!(output, "Not found: abc");
    }

    #[tokio::test]
    async fn test_omni_retrieve_returns_stored_content() {
        let dir = tempdir().unwrap();
        let store = Arc::new(Store::open_path(&dir.path().join("omni.db")).unwrap());
        let hash = store.store_rewind("testing_payload");
        let session = Arc::new(Mutex::new(SessionState::new()));

        let server = OmniServer { store, session };
        let output = server
            .omni_retrieve(Parameters(OmniRetrieveParams { hash }))
            .await;
        assert_eq!(output, "testing_payload");
    }

    #[tokio::test]
    async fn test_omni_density_returns_valid_analysis() {
        let dir = tempdir().unwrap();
        let store = Arc::new(Store::open_path(&dir.path().join("omni.db")).unwrap());
        let session = Arc::new(Mutex::new(SessionState::new()));

        let server = OmniServer { store, session };
        let text = "error: something failed\nCompiling deps v1.0".to_string();
        let density = server
            .omni_density(Parameters(OmniDensityParams { text }))
            .await;
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
        let out = server
            .omni_learn(Parameters(OmniLearnParams {
                text: repetitive,
                apply: false,
            }))
            .await;
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
        let out = server
            .omni_learn(Parameters(OmniLearnParams {
                text: diverse.to_string(),
                apply: false,
            }))
            .await;
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
        let out = server
            .omni_learn(Parameters(OmniLearnParams {
                text: repetitive,
                apply: true,
            }))
            .await;
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
        let out = server
            .omni_trust(Parameters(OmniTrustParams {
                project_path: "/invalid".to_string(),
            }))
            .await;
        assert!(out.contains("Failed") || out.contains("Trusted"));
    }

    // ── Phase 2 MCP Tests ──

    #[tokio::test]
    async fn test_omni_handoff_exports_markdown() {
        let dir = tempdir().unwrap();
        let store = Arc::new(Store::open_path(&dir.path().join("omni.db")).unwrap());
        let mut state = SessionState::new();
        state.inferred_task = Some("fix auth bug".to_string());
        state.add_hot_file("src/auth.rs");
        state.add_command("cargo test");
        let session = Arc::new(Mutex::new(state));

        let server = OmniServer { store, session };
        let out = server.omni_handoff().await;
        assert!(
            out.contains("OMNI Session Handoff"),
            "should be markdown format"
        );
        assert!(out.contains("fix auth bug"), "should contain task");
        assert!(out.contains("src/auth.rs"), "should contain hot files");
        assert!(out.contains("cargo test"), "should contain recent commands");
        assert!(
            out.contains("Paste this into a new session"),
            "should have handoff instructions"
        );
    }

    #[tokio::test]
    async fn test_omni_handoff_includes_engrams() {
        let dir = tempdir().unwrap();
        let store = Arc::new(Store::open_path(&dir.path().join("omni.db")).unwrap());
        let mut state = SessionState::new();
        state.add_engram(crate::session::engram::Engram {
            label: "Fixed clippy warning".to_string(),
            trigger: crate::session::engram::EngramTrigger::ErrorResolved,
            timestamp: chrono::Utc::now().timestamp(),
            files: vec!["src/main.rs".to_string()],
            detail: None,
        });
        let session = Arc::new(Mutex::new(state));

        let server = OmniServer { store, session };
        let out = server.omni_handoff().await;
        assert!(
            out.contains("Fixed clippy warning"),
            "should contain engram label"
        );
        assert!(
            out.contains("Subtask Progress"),
            "should have engram section"
        );
    }

    #[tokio::test]
    async fn test_omni_session_summary_action() {
        let dir = tempdir().unwrap();
        let store = Arc::new(Store::open_path(&dir.path().join("omni.db")).unwrap());
        let mut state = SessionState::new();
        state.add_tool_call(crate::session::engram::ToolCallEntry {
            tool_family: "git".to_string(),
            command: "git status".to_string(),
            succeeded: true,
            files: vec![],
            timestamp: 1000,
        });
        let session = Arc::new(Mutex::new(state));

        let server = OmniServer { store, session };
        let out = server
            .omni_session(Parameters(OmniSessionParams {
                action: "summary".to_string(),
            }))
            .await;
        assert!(out.contains("git"), "should contain tool family");
        assert!(out.contains("Tool Activity"), "should have tool header");
    }

    #[tokio::test]
    async fn test_omni_session_summary_empty() {
        let dir = tempdir().unwrap();
        let store = Arc::new(Store::open_path(&dir.path().join("omni.db")).unwrap());
        let session = Arc::new(Mutex::new(SessionState::new()));

        let server = OmniServer { store, session };
        let out = server
            .omni_session(Parameters(OmniSessionParams {
                action: "summary".to_string(),
            }))
            .await;
        assert!(
            out.contains("No tool calls recorded"),
            "expected empty message, got: {out}"
        );
    }
}
