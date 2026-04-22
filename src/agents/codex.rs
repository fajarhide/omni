use crate::agents::AgentIntegration;
use colored::*;
use std::fs;
use std::path::PathBuf;

pub struct CodexIntegration;

impl AgentIntegration for CodexIntegration {
    fn id(&self) -> &'static str {
        "codex"
    }

    fn name(&self) -> &'static str {
        "Codex CLI"
    }

    fn install(&self, exe_path: &str) -> anyhow::Result<()> {
        let codex_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".codex");
        let config_path = codex_dir.join("config.toml");

        fs::create_dir_all(&codex_dir)?;
        let mut content = if config_path.exists() {
            fs::read_to_string(&config_path)?
        } else {
            String::new()
        };

        if !content.contains("[mcp_servers.omni]") {
            content.push_str(&format!(
                "\n[mcp_servers.omni]\ncommand = \"{}\"\nargs = [\"--mcp\"]\n",
                exe_path
            ));
            fs::write(&config_path, content)?;
        }

        println!(
            "  {} Configured MCP Server in ~/.codex/config.toml",
            "✓".green()
        );
        Ok(())
    }

    fn uninstall(&self) -> anyhow::Result<()> {
        let codex_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".codex");
        let config_path = codex_dir.join("config.toml");

        if config_path.exists() {
            let content = fs::read_to_string(&config_path)?;
            if content.contains("[mcp_servers.omni]") {
                let mut new_content = String::new();
                let mut skip = false;
                for line in content.lines() {
                    if line.starts_with("[mcp_servers.omni]") {
                        skip = true;
                    } else if skip && line.starts_with('[') {
                        skip = false;
                    }
                    if !skip {
                        new_content.push_str(line);
                        new_content.push('\n');
                    }
                }
                fs::write(&config_path, new_content.trim_end().to_string() + "\n")?;
                println!(
                    "  {} Removed MCP Server from ~/.codex/config.toml",
                    "✓".yellow()
                );
            }
        }
        Ok(())
    }

    fn doctor_check(&self, _fix_mode: bool, _warnings: &mut Vec<String>) -> bool {
        let codex_config = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".codex/config.toml");

        println!("\n  {}", "Codex CLI:".cyan());
        if codex_config.exists()
            && fs::read_to_string(&codex_config)
                .unwrap_or_default()
                .contains("omni")
        {
            println!(
                "   {:<15} {} {}",
                "Config:".bright_black(),
                "~/.codex/config.toml".bright_black(),
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
