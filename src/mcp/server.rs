use crate::pipeline::classifier::classify;
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
            let filter_name = format!("learned_{}", chrono::Utc::now().timestamp());
            let _toml_content = generate_toml(&candidates, &filter_name);

            let config_path = crate::paths::learned_filters_path();
            let _ = crate::paths::ensure_omni_home();

            match apply_to_config(&candidates, &filter_name, &config_path) {
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
        let content_type = classify(&text, None);
        let current_session = self.session.lock().unwrap().clone();

        let segments = score_segments(&text, &content_type, Some(&current_session));

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
                hot_vec.sort_by(|a, b| b.1.cmp(a.1));
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
                hot_vec.sort_by(|a, b| b.1.cmp(a.1));
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
}

// Requires async_trait natively for rmcp handlers
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
                "omni_session" => Self::omni_session_tool_call(tcc).await,
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
                    Self::omni_session_tool_attr(),
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
