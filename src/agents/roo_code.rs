use crate::agents::AgentIntegration;
use colored::*;
use serde_json::json;
use std::fs;
use std::path::PathBuf;

pub struct RooCodeIntegration;

impl AgentIntegration for RooCodeIntegration {
    fn id(&self) -> &'static str {
        "roo-code"
    }

    fn name(&self) -> &'static str {
        "Roo Code"
    }

    fn install(&self, exe_path: &str) -> anyhow::Result<()> {
        let settings_path = get_roo_path();

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
                        "command": exe_path,
                        "args": ["--mcp"],
                        "disabled": false,
                        "env": {
                            "OMNI_AGENT_ID": "roo_code"
                        }
                    }),
                );
            }
        }

        fs::write(&settings_path, serde_json::to_string_pretty(&val)?)?;
        println!(
            "  {} Configured MCP Server in Roo Code settings",
            "✓".green()
        );
        Ok(())
    }

    fn uninstall(&self) -> anyhow::Result<()> {
        let settings_path = get_roo_path();

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
        println!(
            "  {} Removed MCP Server from Roo Code settings",
            "✓".yellow()
        );
        Ok(())
    }

    fn doctor_check(&self, _fix_mode: bool, _warnings: &mut Vec<String>) -> bool {
        let settings_path = get_roo_path();

        println!("\n  {}", "Roo Code:".cyan());
        if settings_path.exists()
            && fs::read_to_string(&settings_path)
                .unwrap_or_default()
                .contains("\"omni\"")
        {
            println!(
                "   {:<15} {} {}",
                "Config:".bright_black(),
                settings_path.display(),
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

fn get_roo_path() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join("Library/Application Support/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/cline_mcp_settings.json")
    }
    #[cfg(target_os = "windows")]
    {
        dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join(
            "Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/cline_mcp_settings.json",
        )
    }
    #[cfg(target_os = "linux")]
    {
        dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/cline_mcp_settings.json")
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        PathBuf::from("roo_mcp_settings.json")
    }
}
