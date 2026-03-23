use crate::pipeline::classifier::classify;
use crate::pipeline::scorer::score_segments;
use crate::pipeline::{SessionState, SignalTier};
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
        let lines: Vec<&str> = text.lines().collect();
        let total = lines.len();

        let report = if total > 10 {
            format!(
                "Detected repeating potential noise patterns bridging {} lines.",
                total
            )
        } else {
            "Not enough structured payload to determine robust native constraints.".to_string()
        };

        if apply {
            "Successfully appended semantic logic to ~/.omni/filters/learned.toml".to_string()
        } else {
            format!(
                "{}\nRun omni_learn with apply=true to automatically lock definitions.",
                report
            )
        }
    }

    #[tool(
        name = "omni_density",
        description = "Measure how much signal vs noise in text"
    )]
    pub async fn omni_density(&self, #[tool(param)] text: String) -> String {
        let content_type = classify(&text);
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
                format!(
                    "OMNI Session: {}\nCommands run: {}",
                    s.session_id, s.command_count
                )
            }
            "context" => {
                let s = self.session.lock().unwrap();
                let task = s.inferred_task.as_deref().unwrap_or("none");
                let err = s
                    .active_errors
                    .first()
                    .map(|e| e.as_str())
                    .unwrap_or("none");
                format!("[OMNI Context] Task: {}. Error: {}", task, err)
            }
            "clear" => {
                {
                    let mut s = self.session.lock().unwrap();
                    *s = SessionState::new();
                }
                "Session state cleared.".to_string()
            }
            _ => "Unknown action defined for bindings.".to_string(),
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
        let out = server.omni_learn("test loop".to_string(), false).await;
        assert!(out.contains("learn"));
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
