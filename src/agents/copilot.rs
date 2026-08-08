use crate::agents::AgentIntegration;
use colored::*;
use serde_json::json;
use std::fs;
use std::path::PathBuf;

pub struct CopilotIntegration;

impl CopilotIntegration {
    fn config_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".copilot/mcp-config.json")
    }
}

impl AgentIntegration for CopilotIntegration {
    fn id(&self) -> &'static str {
        "copilot"
    }

    fn name(&self) -> &'static str {
        "GitHub Copilot CLI"
    }

    fn install(&self, exe_path: &str) -> anyhow::Result<()> {
        let settings_path = Self::config_path();

        if let Some(parent) = settings_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut val = if settings_path.exists() {
            let content = fs::read_to_string(&settings_path)?;
            serde_json::from_str(&content).unwrap_or_else(|_| json!({}))
        } else {
            json!({})
        };

        if let Some(obj) = val.as_object_mut() {
            let mcp_servers = obj.entry("mcpServers").or_insert_with(|| json!({}));
            if let Some(servers) = mcp_servers.as_object_mut() {
                servers.insert(
                    "omni".to_string(),
                    json!({
                        "type": "stdio",
                        "command": exe_path,
                        "args": ["--mcp"],
                        "env": {
                            "OMNI_AGENT_ID": "copilot"
                        }
                    }),
                );
            }
        }

        fs::write(&settings_path, serde_json::to_string_pretty(&val)?)?;
        crate::agent_report!(
            "  {} Configured MCP Server in ~/.copilot/mcp-config.json",
            "✓".green()
        );
        Ok(())
    }

    fn uninstall(&self) -> anyhow::Result<()> {
        let settings_path = Self::config_path();

        if !settings_path.exists() {
            return Ok(());
        }

        let content = fs::read_to_string(&settings_path)?;
        let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&content) else {
            return Ok(());
        };

        if let Some(obj) = val.as_object_mut()
            && let Some(servers) = obj.get_mut("mcpServers").and_then(|v| v.as_object_mut())
        {
            servers.remove("omni");
        }

        fs::write(&settings_path, serde_json::to_string_pretty(&val)?)?;
        crate::agent_report!(
            "  {} Removed MCP Server from ~/.copilot/mcp-config.json",
            "✓".yellow()
        );
        Ok(())
    }

    fn doctor_check(&self, fix_mode: bool, warnings: &mut Vec<String>) -> bool {
        let settings_path = Self::config_path();

        crate::agent_report!("\n  {}", "Copilot CLI:".cyan());
        if settings_path.exists()
            && fs::read_to_string(&settings_path)
                .unwrap_or_default()
                .contains("\"omni\"")
        {
            crate::agent_report!(
                "   {:<15} {} {}",
                "Config:".bright_black(),
                "~/.copilot/mcp-config.json".bright_black(),
                "[OK]".green().bold()
            );
            true
        } else if fix_mode {
            if let Ok(exe_path) = std::env::current_exe() {
                crate::agents::report_fix(
                    "Config:",
                    "registered",
                    self.install(&exe_path.to_string_lossy()),
                    warnings,
                );
            }
            true
        } else {
            crate::agent_report!(
                "   {:<15} {}",
                "Config:".bright_black(),
                "not configured".bright_black()
            );
            warnings.push(
                "Copilot CLI MCP server not configured. Run `omni init --copilot`.".to_string(),
            );
            false
        }
    }
}
