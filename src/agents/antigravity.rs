use crate::agents::AgentIntegration;
use colored::*;
use serde_json::json;
use std::fs;
use std::path::PathBuf;

pub struct AntigravityIntegration;

impl AntigravityIntegration {
    /// Returns the path to the Antigravity MCP config file.
    /// macOS/Linux: ~/.gemini/antigravity/mcp_config.json
    /// Windows:     %USERPROFILE%\.gemini\antigravity\mcp_config.json
    fn config_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".gemini/antigravity/mcp_config.json")
    }
}

impl AgentIntegration for AntigravityIntegration {
    fn id(&self) -> &'static str {
        "antigravity"
    }

    fn name(&self) -> &'static str {
        "Antigravity IDE"
    }

    fn install(&self, exe_path: &str) -> anyhow::Result<()> {
        let config_path = Self::config_path();

        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut val = if config_path.exists() {
            let content = fs::read_to_string(&config_path)?;
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
                        "command": exe_path,
                        "args": ["serve"]
                    }),
                );
            }
        }

        fs::write(&config_path, serde_json::to_string_pretty(&val)?)?;
        println!(
            "  {} Configured MCP Server in ~/.gemini/antigravity/mcp_config.json",
            "✓".green()
        );
        Ok(())
    }

    fn uninstall(&self) -> anyhow::Result<()> {
        let config_path = Self::config_path();

        if !config_path.exists() {
            return Ok(());
        }

        let content = fs::read_to_string(&config_path)?;
        let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&content) else {
            return Ok(());
        };

        if let Some(obj) = val.as_object_mut()
            && let Some(servers) = obj.get_mut("mcpServers").and_then(|v| v.as_object_mut())
        {
            servers.remove("omni");
        }

        fs::write(&config_path, serde_json::to_string_pretty(&val)?)?;
        println!(
            "  {} Removed MCP Server from ~/.gemini/antigravity/mcp_config.json",
            "✓".yellow()
        );
        Ok(())
    }

    fn doctor_check(&self, _fix_mode: bool, _warnings: &mut Vec<String>) -> bool {
        let config_path = Self::config_path();

        println!("\n  {}", "Antigravity IDE:".cyan());
        if config_path.exists()
            && fs::read_to_string(&config_path)
                .unwrap_or_default()
                .contains("\"omni\":")
        {
            println!(
                "   {:<15} {} {}",
                "Config:".bright_black(),
                "~/.gemini/antigravity/mcp_config.json".bright_black(),
                "[OK]".green().bold()
            );
            true
        } else {
            println!(
                "   {:<15} {}",
                "Config:".bright_black(),
                "not configured".bright_black()
            );
            false
        }
    }
}
