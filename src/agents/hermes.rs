use crate::agents::AgentIntegration;
use colored::*;
use std::fs;
use std::path::PathBuf;

pub struct HermesIntegration;

/// Returns the Hermes plugin directory.
fn plugin_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".hermes/plugins/omni-signal-engine")
}

impl AgentIntegration for HermesIntegration {
    fn id(&self) -> &'static str {
        "hermes"
    }

    fn name(&self) -> &'static str {
        "Hermes Agent"
    }

    fn install(&self, exe_path: &str) -> anyhow::Result<()> {
        let dest = plugin_dir();
        fs::create_dir_all(&dest)?;

        println!("  {} Generating Hermes plugin files...", "↓".cyan());

        let plugin_yaml_content = r#"name: omni-signal-engine
version: "1.0"
description: OMNI Signal Engine integration for Hermes Agent hooks
"#;

        let init_py_content = format!(
            r#"\"\"\"OMNI integration for Hermes Agent\"\"\"
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

        println!(
            "  {} Installed Hermes plugin to ~/.hermes/plugins/omni-signal-engine/",
            "✓".green()
        );

        println!(
            "  {} Run {} to enable the plugin",
            "→".cyan(),
            "hermes plugins enable omni-signal-engine".bright_black()
        );

        println!(
            "\n  {} To add the OMNI MCP Server, append this to your ~/.hermes/config.yaml:",
            "ℹ".blue()
        );
        println!("{}", "  mcp_servers:".bright_black());
        println!("{}", "    omni:".bright_black());
        println!("      command: \"{}\"", exe_path.bright_black());
        println!("{}", "      args: [\"--mcp\"]".bright_black());
        println!("{}", "      env:".bright_black());
        println!("{}", "        OMNI_AGENT_ID: \"hermes\"".bright_black());

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

        println!("\n  {}", "Hermes Agent:".cyan());
        if dest.join("plugin.yaml").exists() {
            println!(
                "   {:<15} {} {}",
                "Plugin:".bright_black(),
                "~/.hermes/plugins/omni-signal-engine/".bright_black(),
                "[OK]".green().bold()
            );
            println!(
                "   {:<15} {}",
                "Note:".bright_black(),
                "Verify OMNI is under mcp_servers in ~/.hermes/config.yaml".bright_black()
            );
            true
        } else {
            println!(
                "   {:<15} {}",
                "Plugin:".bright_black(),
                "not installed".bright_black()
            );
            false
        }
    }
}
