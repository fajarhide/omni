use crate::agents::AgentIntegration;
use colored::*;
use serde_json::json;
use std::fs;
use std::path::PathBuf;

pub struct CursorIntegration;

impl AgentIntegration for CursorIntegration {
    fn id(&self) -> &'static str {
        "cursor"
    }

    fn name(&self) -> &'static str {
        "Cursor AI"
    }

    fn install(&self, exe_path: &str) -> anyhow::Result<()> {
        let mcp_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".cursor/mcp.json");

        if let Some(parent) = mcp_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut val = if mcp_path.exists() {
            let content = fs::read_to_string(&mcp_path)?;
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

        fs::write(&mcp_path, serde_json::to_string_pretty(&val)?)?;
        println!(
            "  {} Configured MCP Server in ~/.cursor/mcp.json",
            "✓".green()
        );
        Ok(())
    }

    fn uninstall(&self) -> anyhow::Result<()> {
        let mcp_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".cursor/mcp.json");

        if !mcp_path.exists() {
            return Ok(());
        }

        let content = fs::read_to_string(&mcp_path)?;
        let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&content) else {
            return Ok(());
        };

        if let Some(obj) = val.as_object_mut()
            && let Some(servers) = obj.get_mut("mcpServers").and_then(|v| v.as_object_mut())
        {
            servers.remove("omni");
        }

        fs::write(&mcp_path, serde_json::to_string_pretty(&val)?)?;
        println!(
            "  {} Removed MCP Server from ~/.cursor/mcp.json",
            "✓".yellow()
        );
        Ok(())
    }

    fn doctor_check(&self, _fix_mode: bool, _warnings: &mut Vec<String>) -> bool {
        let mcp_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".cursor/mcp.json");

        println!("\n  {}", "Cursor AI:".cyan());
        if mcp_path.exists()
            && fs::read_to_string(&mcp_path)
                .unwrap_or_default()
                .contains("\"omni\":")
        {
            println!(
                "   {:<15} {} {}",
                "Config:".bright_black(),
                "~/.cursor/mcp.json".bright_black(),
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
