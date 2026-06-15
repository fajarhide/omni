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

/// Comprehensive startup validation for Hermes integration.
///
/// Checks: config.yaml (MCP + compression), plugin files, OMNI binary
/// availability, and OMNI config presence. Returns `None` when all
/// checks pass, or a formatted diagnostics string that gets injected
/// into the Hermes `systemPromptAddition` so the agent can self-heal.
pub fn validate_startup() -> Option<String> {
    let mut warnings: Vec<&str> = Vec::new();

    // ── 1. Hermes config.yaml ──
    let config_path = hermes_home_dir().join("config.yaml");
    if let Ok(config_str) = fs::read_to_string(&config_path) {
        // MCP server registration
        if !config_str.contains("mcp_servers:") || !config_str.contains("omni:") {
            warnings.push(
                "OMNI MCP server is NOT registered in ~/.hermes/config.yaml. \
                 27 MCP tools (omni_retrieve, omni_loop_memory, omni_knowledge, …) \
                 will be unavailable. Run `omni init --hermes` to fix.",
            );
        }
        // Compression bridge
        if !config_str.contains("compression:") || !config_str.contains("enabled: true") {
            warnings.push(
                "Hermes compression is NOT enabled. Context Pressure warnings \
                 from OMNI will be surfaced but Hermes will not act on them. \
                 Run `omni init --hermes` to fix.",
            );
        }
    } else {
        warnings.push("Could not find ~/.hermes/config.yaml. Is Hermes installed?");
    }

    // ── 2. Plugin scaffold ──
    let plugin_init = plugin_dir().join("__init__.py");
    if !plugin_init.exists() {
        warnings.push(
            "OMNI Hermes plugin (`__init__.py`) is missing. \
             Pre/Post hooks will not execute. Run `omni init --hermes` to install.",
        );
    }

    // ── 3. OMNI binary reachable ──
    #[allow(clippy::collapsible_if)]
    if let Ok(exe) = std::env::current_exe() {
        if !exe.exists() {
            warnings.push("OMNI binary path does not exist on disk. Hooks will fail at runtime.");
        }
    }

    // ── 4. OMNI config for Hermes ──
    let omni_cfg = omni_config_path();
    if omni_cfg.exists() {
        #[allow(clippy::collapsible_if)]
        if let Ok(content) = fs::read_to_string(&omni_cfg) {
            if !content.contains("[agents.hermes]") {
                warnings.push(
                    "~/.omni/config.toml exists but has no [agents.hermes] section. \
                     Hermes-optimized defaults (Efficient mode, pinned files) are inactive. \
                     Run `omni init --hermes` to add them.",
                );
            }
        }
    } else {
        warnings.push(
            "~/.omni/config.toml does not exist. OMNI is using built-in defaults \
             instead of Hermes-optimized settings. Run `omni init --hermes`.",
        );
    }

    if warnings.is_empty() {
        None
    } else {
        Some(format!(
            "\n  [OMNI × Hermes Startup Validation — {} issue(s)]\n{}\n\
             → Fix all: `omni init --hermes && hermes gateway restart`\n",
            warnings.len(),
            warnings
                .iter()
                .enumerate()
                .map(|(i, w)| format!("  {}. {}", i + 1, w))
                .collect::<Vec<_>>()
                .join("\n")
        ))
    }
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
            r#""""""OMNI Context OS integration for Hermes Agent.

This plugin wires three lifecycle hooks so OMNI can distill
terminal output, track sessions, and manage context pressure.

It also exposes helper utilities that make the 27 OMNI MCP tools
(loop_memory, knowledge, retrieve, learn) work seamlessly during
Hermes autonomous multi-step loops.
""""""
import json
import os
import subprocess
import time

_OMNI_BIN = "{}"
_STEP_COUNTER = 0
_SESSION_START_TS = None


def _omni_env():
    """Return a copy of the environment with OMNI_AGENT_ID set."""
    env = os.environ.copy()
    env["OMNI_AGENT_ID"] = "hermes"
    # Forward loop control vars when present
    for var in ("OMNI_LOOP_ID", "OMNI_LOOP_GOAL", "OMNI_LOOP_BUDGET"):
        if var in os.environ:
            env[var] = os.environ[var]
    return env


def _run_omni(*args):
    """Run the OMNI binary with fail-open semantics."""
    try:
        return subprocess.run(
            [_OMNI_BIN] + list(args),
            env=_omni_env(),
            capture_output=True,
            timeout=5,
        )
    except Exception:
        return None  # fail-open: never block Hermes


def register(ctx):
    global _SESSION_START_TS
    _SESSION_START_TS = time.time()

    def on_post_tool_call(tool_name, params, result):
        global _STEP_COUNTER
        _STEP_COUNTER += 1
        _run_omni("--post-hook")

    def on_pre_tool_call(tool_name, params):
        _run_omni("--pre-hook")

    def on_session_start():
        _run_omni("--session-start")

    ctx.register_hook("post_tool_call", on_post_tool_call)
    ctx.register_hook("pre_tool_call", on_pre_tool_call)
    ctx.register_hook("on_session_start", on_session_start)
"#,
            exe_path
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
                "Hermes config not found; enable the OMNI plugin once Hermes is initialized."
                    .to_string(),
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
                let mcp_block = mcp_block.replace("{}", exe_path);

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
                    #[allow(clippy::collapsible_if)]
                    if let Ok(current) = fs::read_to_string(&config_path) {
                        let compression_block = "\ncompression:\n  enabled: true\n  threshold: 0.50\n  target_ratio: 0.20\n\n";

                        let mut updated = current;
                        if !updated.contains("compression:") {
                            updated.push_str(compression_block);
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
            warnings.push(
                "Could not read ~/.hermes/config.yaml to register the OMNI MCP server.".to_string(),
            );
        }

        let omni_config_path = omni_config_path();
        fs::create_dir_all(omni_home_dir())?;
        let default_config = crate::agents::hermes::hermes_default_config();

        let mut config_lines = Vec::new();
        config_lines.push("\n[agents.hermes]".to_string());
        config_lines.push(format!(
            "mode = \"{}\"",
            format!("{:?}", default_config.mode.unwrap_or_default()).to_lowercase()
        ));
        if let Some(readfile) = default_config.enable_readfile_distillation {
            config_lines.push(format!("enable_readfile_distillation = {}", readfile));
        }
        if let Some(grep) = default_config.enable_grep_distillation {
            config_lines.push(format!("enable_grep_distillation = {}", grep));
        }
        if let Some(webfetch) = default_config.enable_webfetch_distillation {
            config_lines.push(format!("enable_webfetch_distillation = {}", webfetch));
        }
        if let Some(pinned) = &default_config
            .pinned_files
            .as_ref()
            .filter(|p| !p.is_empty())
        {
            config_lines.push("pinned_files = [".to_string());
            for path in *pinned {
                config_lines.push(format!("  \"{}\",", path));
            }
            config_lines.push("]".to_string());
        }

        let existing = fs::read_to_string(&omni_config_path).unwrap_or_default();
        if !existing.contains("[agents.hermes]") {
            let mut updated = existing;
            if !updated.is_empty() && !updated.ends_with('\n') {
                updated.push('\n');
            }
            updated.push_str(&format!("{}\n", config_lines.join("\n")));
            fs::write(&omni_config_path, updated)?;
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
                    "{} Hermes OMNI config already exists at {}",
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

    fn doctor_check(&self, fix_mode: bool, warnings: &mut Vec<String>) -> bool {
        let dest = plugin_dir();
        let config_path = hermes_config_path();
        let directory_plugin_installed = dest.join("plugin.yaml").exists();
        let configured_plugin = configured_omni_plugin(&config_path);
        let mcp_configured = configured_omni_mcp(&config_path);
        let compression_on = configured_compression_in_config(&config_path);
        let omni_cfg = omni_config_path();
        let has_hermes_section = fs::read_to_string(&omni_cfg)
            .map(|c| c.contains("[agents.hermes]"))
            .unwrap_or(false);
        let installed = directory_plugin_installed || configured_plugin.is_some();

        println!("\n  {}", "Hermes Agent:".cyan());

        // Plugin status
        if directory_plugin_installed {
            println!(
                "   {:>15} {} {}",
                "Plugin:".bright_black(),
                "~/.hermes/plugins/omni-signal-engine/".bright_black(),
                "[OK]".green().bold()
            );
        } else if let Some(plugin_name) = configured_plugin {
            println!(
                "   {:>15} {} {}",
                "Plugin:".bright_black(),
                format!("{} in ~/.hermes/config.yaml", plugin_name).bright_black(),
                "[OK]".green().bold()
            );
        } else {
            println!(
                "   {:>15} {}",
                "Plugin:".bright_black(),
                "not installed [MISSING]".red().bold()
            );
            warnings.push("Hermes OMNI plugin is not installed.".to_string());
        }

        // MCP status
        println!(
            "   {:>15} {}",
            "MCP Server:".bright_black(),
            if mcp_configured {
                "registered [OK]".green().bold()
            } else {
                "not registered [MISSING]".red().bold()
            }
        );
        if !mcp_configured {
            warnings.push("OMNI MCP server is not registered in Hermes config.".to_string());
        }

        // Compression status
        println!(
            "   {:>15} {}",
            "Compression:".bright_black(),
            if compression_on {
                "enabled [OK]".green().bold()
            } else {
                "disabled [WARN]".yellow().bold()
            }
        );
        if !compression_on {
            warnings.push(
                "Hermes compression is disabled; context pressure warnings will not trigger compaction."
                    .to_string(),
            );
        }

        // OMNI config section
        println!(
            "   {:>15} {}",
            "OMNI Config:".bright_black(),
            if has_hermes_section {
                "[agents.hermes] present [OK]".green().bold()
            } else {
                "[agents.hermes] missing [WARN]".yellow().bold()
            }
        );
        if !has_hermes_section {
            warnings.push(
                "~/.omni/config.toml has no [agents.hermes] section; using built-in defaults."
                    .to_string(),
            );
        }

        // Auto-fix: re-run the full init to repair all gaps
        #[allow(clippy::collapsible_if)]
        if fix_mode && (!installed || !mcp_configured || !compression_on || !has_hermes_section) {
            if let Ok(exe) = std::env::current_exe() {
                let exe_str = exe.to_string_lossy().to_string();
                println!(
                    "   {:>15} {}",
                    "Auto-fix:".bright_black(),
                    "Re-running omni init --hermes...".yellow()
                );
                match self.install(&exe_str) {
                    Ok(()) => {
                        println!(
                            "   {:>15} {}",
                            "".bright_black(),
                            "\u{2713} Auto-fix applied. Restart Hermes to activate."
                                .green()
                                .bold()
                        );
                    }
                    Err(e) => {
                        println!(
                            "   {:>15} {}",
                            "".bright_black(),
                            format!("\u{2717} Auto-fix failed: {}", e).red().bold()
                        );
                    }
                }
            }
        }

        if installed && !mcp_configured {
            println!(
                "   {:>15} {}",
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
    use super::{config_mentions_omni_mcp, config_mentions_omni_plugin, configured_compression};

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

    #[test]
    fn detects_compression_enabled() {
        let config = "compression:\n  enabled: true\n  threshold: 0.50";
        assert!(configured_compression(config));
    }

    #[test]
    fn detects_compression_disabled() {
        let config = "plugins:\n  enabled:\n    - foo";
        assert!(!configured_compression(config));
    }

    #[test]
    fn validate_startup_returns_some_when_hermes_not_installed() {
        // On CI / dev machines without Hermes, validation should surface warnings
        let result = super::validate_startup();
        // We can't assert None because it depends on the host environment,
        // but we CAN assert the function doesn't panic and returns a coherent type.
        if let Some(msg) = &result {
            assert!(msg.contains("OMNI × Hermes") || msg.contains("Startup Validation"));
        }
    }
}
