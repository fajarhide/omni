use crate::agents::AgentIntegration;
use colored::*;
use serde_json::json;
use std::fs;
use std::path::PathBuf;

pub struct OpenCodeIntegration;

impl AgentIntegration for OpenCodeIntegration {
    fn id(&self) -> &'static str {
        "opencode"
    }

    fn name(&self) -> &'static str {
        "OpenCode"
    }

    fn install(&self, exe_path: &str) -> anyhow::Result<()> {
        let opencode_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config/opencode");
        let config_path = opencode_dir.join("opencode.json");

        let mut val = if config_path.exists() {
            let content = fs::read_to_string(&config_path)?;
            serde_json::from_str(&content).unwrap_or_else(|_| json!({}))
        } else {
            fs::create_dir_all(&opencode_dir)?;
            json!({})
        };

        if let Some(obj) = val.as_object_mut() {
            let mcp_servers = obj.entry("mcpServers").or_insert_with(|| json!({}));
            if let Some(servers_obj) = mcp_servers.as_object_mut() {
                servers_obj.insert(
                    "omni".to_string(),
                    json!({
                        "command": exe_path,
                        "args": ["--mcp"]
                    }),
                );
            }
        }

        fs::write(&config_path, serde_json::to_string_pretty(&val)?)?;
        println!(
            "  {} Configured MCP Server in ~/.config/opencode/opencode.json",
            "✓".green()
        );
        Ok(())
    }

    fn uninstall(&self) -> anyhow::Result<()> {
        let config_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config/opencode/opencode.json");

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
            "  {} Removed MCP Server from ~/.config/opencode/opencode.json",
            "✓".yellow()
        );
        Ok(())
    }

    fn doctor_check(&self, _fix_mode: bool, _warnings: &mut Vec<String>) -> bool {
        let opencode_config = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config/opencode/opencode.json");

        println!("\n  {}", "OpenCode:".cyan());
        if opencode_config.exists()
            && fs::read_to_string(&opencode_config)
                .unwrap_or_default()
                .contains("omni")
        {
            println!(
                "   {:<15} {} {}",
                "Config:".bright_black(),
                "~/.config/opencode/opencode.json".bright_black(),
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
