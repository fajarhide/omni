use crate::agents::AgentIntegration;
use colored::*;
use std::fs;
use std::path::{Path, PathBuf};

pub struct HermesIntegration;

fn plugin_dir() -> PathBuf {
    hermes_home_dir().join("plugins").join("omni-signal-engine")
}

fn omni_home_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".omni")
}

fn omni_config_path() -> PathBuf {
    omni_home_dir().join("config.toml")
}

fn hermes_home_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".hermes")
}

fn hermes_config_path() -> PathBuf {
    hermes_home_dir().join("config.yaml")
}

fn config_mentions_omni_plugin(config: &str) -> Option<&'static str> {
    if config.contains("hermes-omni-plugin") {
        Some("hermes-omni-plugin")
    } else if config.contains("omni-signal-engine") {
        Some("omni-signal-engine")
    } else {
        None
    }
}

fn config_mentions_omni_mcp(config: &str) -> bool {
    let has_mcp_section = config.contains("mcp_servers:") || config.contains("mcp:");
    let has_omni_server = config.contains("omni:");
    let has_omni_command = config.contains("--mcp") || config.contains("OMNI_AGENT_ID");
    has_mcp_section && has_omni_server && has_omni_command
}

fn configured_omni_plugin(config_path: &Path) -> Option<&'static str> {
    fs::read_to_string(config_path)
        .ok()
        .and_then(|config| config_mentions_omni_plugin(&config))
}

fn configured_omni_mcp(config_path: &Path) -> bool {
    fs::read_to_string(config_path)
        .map(|config| config_mentions_omni_mcp(&config))
        .unwrap_or(false)
}

fn configured_compression(config: &str) -> bool {
    let has_compression = config.contains("compression:");
    let has_enabled = config.contains("enabled: true") || config.contains("enabled:true");
    has_compression && has_enabled
}

fn configured_compression_in_config(config_path: &Path) -> bool {
    fs::read_to_string(config_path)
        .map(|config| configured_compression(&config))
        .unwrap_or(false)
}

impl AgentIntegration for HermesIntegration {
    fn id(&self) -> &'static str {
        "hermes"
    }

    fn name(&self) -> &'static str {
        "Hermes Agent"
    }

    fn install(&self, exe_path: &str) -> anyhow::Result<()> {
        let mut actions = Vec::new();
        let mut warnings = Vec::new();

        let dest = plugin_dir();
        fs::create_dir_all(&dest)?;

        let plugin_yaml_content = r#"name: omni-signal-engine
version: "1.0"
description: OMNI Signal Engine integration for Hermes Agent hooks
"#;

        let init_py_content = format!(
            r#""""""OMNI integration for Hermes Agent""""""
import subprocess
import os

def register(ctx):
    def on_post_tool_call(tool_name, params, result):
        env = os.environ.copy()
        env["OMNI_AGENT_ID"] = "hermes"
        try:
            subprocess.run(["{}", "--post-hook"], env=env, capture_output=True)
        except Exception:
            pass

    def on_pre_tool_call(tool_name, params):
        env = os.environ.copy()
        env["OMNI_AGENT_ID"] = "hermes"
        try:
            subprocess.run(["{}", "--pre-hook"], env=env, capture_output=True)
        except Exception:
            pass

    def on_session_start():
        env = os.environ.copy()
        env["OMNI_AGENT_ID"] = "hermes"
        try:
            subprocess.run(["{}", "--session-start"], env=env, capture_output=True)
        except Exception:
            pass

    ctx.register_hook("post_tool_call", on_post_tool_call)
    ctx.register_hook("pre_tool_call", on_pre_tool_call)
    ctx.register_hook("on_session_start", on_session_start)
"#,
            exe_path, exe_path, exe_path
        );

        fs::write(dest.join("plugin.yaml"), plugin_yaml_content)?;
        fs::write(dest.join("__init__.py"), init_py_content)?;
        actions.push(format!(
            "{} Installed Hermes plugin to ~/.hermes/plugins/omni-signal-engine/",
            "✓".green()
        ));

        let config_path = hermes_config_path();
        let requires_manual_plugin_step = !fs::metadata(&config_path)
            .ok()
            .map(|meta| meta.is_file())
            .unwrap_or(false);

        if requires_manual_plugin_step {
            actions.push(format!(
                "{} Run {} to enable the OMNI plugin",
                "→".cyan(),
                "hermes plugins enable omni-signal-engine".bright_black()
            ));
            warnings.push(
                "Hermes config not found; enable the OMNI plugin once Hermes is initialized.".to_string(),
            );
        }

        if let Ok(config) = fs::read_to_string(&config_path) {
            if configured_omni_mcp(&config_path) {
                actions.push(
                    format!(
                        "{} OMNI MCP server is already registered in ~/.hermes/config.yaml",
                        "✓".green()
                    )
                    .to_string(),
                );
            } else {
                let mcp_block = "\nmcp_servers:\n  omni:\n    command: \"{}\"\n    args: [\"--mcp\"]\n    env:\n      OMNI_AGENT_ID: \"hermes\"\n\n";
                let mcp_block = format!("{}", mcp_block.replace("{}", exe_path));

                let mut updated = String::new();
                let mut inserted = false;
                for line in config.lines() {
                    updated.push_str(line);
                    updated.push('\n');
                    if !inserted && line.trim_start().starts_with("plugins:") {
                        updated.push_str(&mcp_block);
                        inserted = true;
                        actions.push(
                            format!(
                                "{} Registered OMNI MCP server inside ~/.hermes/config.yaml",
                                "✓".green()
                            )
                            .to_string(),
                        );
                    }
                }

                if !inserted {
                    updated.push_str(&mcp_block);
                    actions.push(
                        format!(
                            "{} Registered OMNI MCP server at the end of ~/.hermes/config.yaml",
                            "✓".green()
                        )
                        .to_string(),
                    );
                }

                fs::write(&config_path, updated)?;

                if configured_compression_in_config(&config_path) {
                    actions.push(
                        format!(
                            "{} Hermes compression is already enabled in ~/.hermes/config.yaml",
                            "✓".green()
                        )
                        .to_string(),
                    );
                } else if !requires_manual_plugin_step {
                    if let Ok(current) = fs::read_to_string(&config_path) {
                        let compression_block = "\ncompression:\n  enabled: true\n  threshold: 0.50\n  target_ratio: 0.20\n\n";

                        let mut updated = current;
                        if !updated.contains("compression:") {
                            updated.push_str(&compression_block);
                            actions.push(
                                format!(
                                    "{} Enabled Hermes compression in ~/.hermes/config.yaml",
                                    "✓".bright_green()
                                )
                                .to_string(),
                            );
                            fs::write(&config_path, updated)?;
                        }
                    }
                }
            }
        } else {
            warnings
                .push("Could not read ~/.hermes/config.yaml to register the OMNI MCP server.".to_string());
        }

        let omni_config_path = omni_config_path();
        fs::create_dir_all(omni_home_dir())?;
        let default_config = crate::agents::hermes::hermes_default_config();
        let file_already_exists = fs::metadata(&omni_config_path)
            .ok()
            .map(|meta| meta.is_file())
            .unwrap_or(false);

        if !file_already_exists {
            let mut config_lines = Vec::new();
            config_lines.push("[global]".to_string());
            config_lines
                .push(format!("mode = \"{}\"", format!("{:?}", default_config.mode.unwrap_or_default()).to_lowercase()));
            if let Some(readfile) = default_config.enable_readfile_distillation {
                config_lines.push(format!("enable_readfile_distillation = {}", readfile));
            }
            if let Some(grep) = default_config.enable_grep_distillation {
                config_lines.push(format!("enable_grep_distillation = {}", grep));
            }
            if let Some(webfetch) = default_config.enable_webfetch_distillation {
                config_lines.push(format!("enable_webfetch_distillation = {}", webfetch));
            }
            if let Some(pinned) = &default_config.pinned_files {
                if !pinned.is_empty() {
                    config_lines.push("pinned_files = [".to_string());
                    for path in pinned {
                        config_lines.push(format!("  \"{}\",", path));
                    }
                    config_lines.push("]".to_string());
                }
            }

            fs::write(&omni_config_path, format!("{}\n", config_lines.join("\n")))?;
            actions.push(
                format!(
                    "{} Wrote Hermes OMNI defaults to {}",
                    "✓".green(),
                    omni_config_path.display().to_string().bright_black()
                )
                .to_string(),
            );
        } else {
            actions.push(
                format!(
                    "{} Existing OMNI config preserved at {}",
                    "✓".green(),
                    omni_config_path.display().to_string().bright_black()
                )
                .to_string(),
            );
        }

        for message in &actions {
            println!("  {}", message);
        }

        if !warnings.is_empty() {
            println!("\n  {}", "Warnings:".yellow());
            for warning in &warnings {
                println!("   - {}", warning);
            }
        }

        Ok(())
    }

    fn uninstall(&self) -> anyhow::Result<()> {
        let dest = plugin_dir();
        if dest.exists() {
            fs::remove_dir_all(&dest)?;
            println!(
                "  {} Removed Hermes plugin from ~/.hermes/plugins/",
                "✓".yellow()
            );
        }
        Ok(())
    }

    fn doctor_check(&self, _fix_mode: bool, _warnings: &mut Vec<String>) -> bool {
        let dest = plugin_dir();
        let config_path = hermes_config_path();
        let directory_plugin_installed = dest.join("plugin.yaml").exists();
        let configured_plugin = configured_omni_plugin(&config_path);
        let mcp_configured = configured_omni_mcp(&config_path);
        let installed = directory_plugin_installed || configured_plugin.is_some();

        println!("\n  {}", "Hermes Agent:".cyan());
        if directory_plugin_installed {
            println!(
                "   {:<15} {} {}",
                "Plugin:".bright_black(),
                "~/.hermes/plugins/omni-signal-engine/".bright_black(),
                "[OK]".green().bold()
            );
        } else if let Some(plugin_name) = configured_plugin {
            println!(
                "   {:<15} {} {}",
                "Plugin:".bright_black(),
                format!("{} in ~/.hermes/config.yaml", plugin_name).bright_black(),
                "[OK]".green().bold()
            );
        } else {
            println!(
                "   {:<15} {}",
                "Plugin:".bright_black(),
                "not installed".bright_black()
            );
        }

        println!(
            "   {:<15} {}",
            "MCP Server:".bright_black(),
            if mcp_configured {
                "configured in ~/.hermes/config.yaml [OK]".green().bold()
            } else {
                "not configured".bright_black()
            }
        );

        if installed && !mcp_configured {
            println!(
                "   {:<15} {}",
                "Note:".bright_black(),
                "MCP is optional; native Hermes plugin detection passed.".bright_black()
            );
        }

        installed
    }
}

/// Hermes-optimized agent config defaults.
///
/// Hermes agent uses pipe mode with `OMNI_CMD` env var — no PreToolUse hook.
/// Sessions tend to be longer and Hermes benefits from more aggressive
/// compression since it manages its own context summarization.
pub fn hermes_default_config() -> crate::guard::config::AgentConfig {
    crate::guard::config::AgentConfig {
        mode: Some(crate::guard::config::DistillationMode::Efficient),
        enable_readfile_distillation: Some(true),
        enable_grep_distillation: Some(true),
        enable_webfetch_distillation: Some(true),
        pinned_files: Some(vec![
            "AGENTS.md".to_string(),
            ".omni/CONTEXT.md".to_string(),
        ]),
    }
}

/// Command patterns commonly issued by Hermes agent tool calls
pub fn hermes_command_patterns() -> Vec<&'static str> {
    vec![
        "terminal", "hermes", "shell", "python", "node", "npm", "pip",
    ]
}

/// Check if a given agent_id looks like Hermes
pub fn is_hermes_agent(agent_id: &str) -> bool {
    let id = agent_id.to_lowercase();
    id == "hermes" || id.starts_with("hermes-") || id.contains("hermes")
}

#[cfg(test)]
mod tests {
    use super::{config_mentions_omni_mcp, config_mentions_omni_plugin};

    #[test]
    fn detects_packaged_hermes_omni_plugin_in_config() {
        let config = r#""
plugins:
  enabled:
    - disk-cleanup
    - hermes-omni-plugin
"#;

        assert_eq!(
            config_mentions_omni_plugin(config),
            Some("hermes-omni-plugin")
        );
    }

    #[test]
    fn detects_legacy_omni_signal_engine_plugin_in_config() {
        let config = r#"
plugins:
  enabled:
    - omni-signal-engine
"#;

        assert_eq!(
            config_mentions_omni_plugin(config),
            Some("omni-signal-engine")
        );
    }

    #[test]
    fn detects_hermes_omni_mcp_config() {
        let config = r#"
mcp_servers:
  omni:
    command: "omni"
    args: ["--mcp"]
    env:
      OMNI_AGENT_ID: "hermes"
"#;

        assert!(config_mentions_omni_mcp(config));
    }

    #[test]
    fn missing_plugin_and_mcp_config_are_not_detected() {
        let config = r#"
plugins:
  enabled:
    - unrelated-plugin
"#;

        assert_eq!(config_mentions_omni_plugin(config), None);
        assert!(!config_mentions_omni_mcp(config));
    }

    #[test]
    fn detects_hermes_agent_id() {
        assert!(super::is_hermes_agent("hermes"));
        assert!(super::is_hermes_agent("HERMES"));
        assert!(super::is_hermes_agent("hermes-cli"));
        assert!(super::is_hermes_agent("my-hermes-agent"));
        assert!(!super::is_hermes_agent("claude"));
        assert!(!super::is_hermes_agent("cursor"));
    }

    #[test]
    fn hermes_default_config_enables_efficient_mode_and_pins_files() {
        let config = super::hermes_default_config();
        assert_eq!(
            config.mode,
            Some(crate::guard::config::DistillationMode::Efficient)
        );
        assert_eq!(config.enable_readfile_distillation, Some(true));
        assert_eq!(config.enable_grep_distillation, Some(true));
        assert_eq!(config.enable_webfetch_distillation, Some(true));
        let pinned = config.pinned_files.unwrap_or_default();
        assert!(pinned.contains(&"AGENTS.md".to_string()));
        assert!(pinned.contains(&".omni/CONTEXT.md".to_string()));
    }

    #[test]
    fn hermes_command_patterns_includes_common_hermes_tools() {
        let pats = super::hermes_command_patterns();
        assert!(pats.contains(&"terminal"));
        assert!(pats.contains(&"python"));
        assert!(pats.contains(&"npm"));
        assert!(pats.contains(&"hermes"));
    }
}
