use crate::agents::AgentIntegration;
use colored::*;
use serde_json::json;
use std::fs;
use std::path::PathBuf;

pub struct AntigravityIntegration;

impl AgentIntegration for AntigravityIntegration {
    fn id(&self) -> &'static str {
        "antigravity"
    }

    fn name(&self) -> &'static str {
        "Antigravity IDE"
    }

    fn install(&self, exe_path: &str) -> anyhow::Result<()> {
        let extensions_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".antigravity/extensions");
        let plugin_dir = extensions_dir.join("omni-signal-engine");

        fs::create_dir_all(&plugin_dir)?;

        let manifest = json!({
            "name": "omni-signal-engine",
            "version": "0.1.0",
            "description": "OMNI intelligent output distillation for Antigravity IDE",
            "type": "mcp",
            "mcp": {
                "transport": "stdio",
                "command": exe_path,
                "args": ["--mcp"]
            }
        });

        fs::write(
            plugin_dir.join("package.json"),
            serde_json::to_string_pretty(&manifest)?,
        )?;

        println!(
            "  {} Installed MCP Plugin to ~/.antigravity/extensions/omni-signal-engine",
            "✓".green()
        );
        Ok(())
    }

    fn uninstall(&self) -> anyhow::Result<()> {
        let plugin_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".antigravity/extensions/omni-signal-engine");

        if plugin_dir.exists() {
            fs::remove_dir_all(&plugin_dir).ok();
            println!(
                "  {} Removed MCP Plugin from ~/.antigravity/extensions/",
                "✓".yellow()
            );
        }
        Ok(())
    }

    fn doctor_check(&self, _fix_mode: bool, _warnings: &mut Vec<String>) -> bool {
        let antigravity_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".antigravity/extensions");

        println!("\n  {}", "Antigravity IDE:".cyan());
        let mut ag_found = false;
        if antigravity_dir.exists()
            && let Ok(entries) = fs::read_dir(&antigravity_dir)
        {
            for entry in entries.flatten() {
                if entry
                    .file_name()
                    .to_string_lossy()
                    .contains("omni-signal-engine")
                {
                    println!(
                        "   {:<15} {} {}",
                        "Config:".bright_black(),
                        "plugin installed".bright_black(),
                        "[OK]".green().bold()
                    );
                    ag_found = true;
                    break;
                }
            }
        }
        if !ag_found {
            println!(
                "   {:<15} {}",
                "Config:".bright_black(),
                "not configured (check GUI)".bright_black()
            );
        }
        ag_found
    }
}
