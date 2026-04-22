use crate::agents::AgentIntegration;
use colored::*;
use serde_json::json;
use std::fs;
use std::path::PathBuf;

pub struct ZedIntegration;

impl AgentIntegration for ZedIntegration {
    fn id(&self) -> &'static str {
        "zed"
    }

    fn name(&self) -> &'static str {
        "Zed Editor"
    }

    fn install(&self, exe_path: &str) -> anyhow::Result<()> {
        let settings_path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("zed/settings.json");

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
            let context_servers = obj.entry("context_servers").or_insert_with(|| json!({}));
            if let Some(servers) = context_servers.as_object_mut() {
                servers.insert(
                    "omni".to_string(),
                    json!({
                        "command": exe_path,
                        "args": ["serve"]
                    }),
                );
            }
        }

        fs::write(&settings_path, serde_json::to_string_pretty(&val)?)?;
        println!(
            "  {} Configured MCP Server in zed/settings.json",
            "✓".green()
        );
        Ok(())
    }

    fn uninstall(&self) -> anyhow::Result<()> {
        let settings_path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("zed/settings.json");

        if !settings_path.exists() {
            return Ok(());
        }

        let content = fs::read_to_string(&settings_path)?;
        let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&content) else {
            return Ok(());
        };

        if let Some(obj) = val.as_object_mut()
            && let Some(servers) = obj
                .get_mut("context_servers")
                .and_then(|v| v.as_object_mut())
        {
            servers.remove("omni");
        }

        fs::write(&settings_path, serde_json::to_string_pretty(&val)?)?;
        println!(
            "  {} Removed MCP Server from zed/settings.json",
            "✓".yellow()
        );
        Ok(())
    }

    fn doctor_check(&self, _fix_mode: bool, _warnings: &mut Vec<String>) -> bool {
        let settings_path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("zed/settings.json");

        println!("\n  {}", "Zed Editor:".cyan());
        if settings_path.exists()
            && fs::read_to_string(&settings_path)
                .unwrap_or_default()
                .contains("\"omni\":")
        {
            println!(
                "   {:<15} {} {}",
                "Config:".bright_black(),
                "zed/settings.json".bright_black(),
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
