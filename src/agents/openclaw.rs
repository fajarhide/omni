use crate::agents::AgentIntegration;
use colored::*;
use std::fs;
use std::path::PathBuf;

pub struct OpenClawIntegration;

/// Returns the OpenClaw plugin install directory.
fn plugin_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".openclaw/plugins/omni-signal-engine")
}

impl AgentIntegration for OpenClawIntegration {
    fn id(&self) -> &'static str {
        "openclaw"
    }

    fn name(&self) -> &'static str {
        "OpenClaw"
    }

    fn install(&self, _exe_path: &str) -> anyhow::Result<()> {
        let dest = plugin_dir();
        fs::create_dir_all(&dest)?;

        println!(
            "  {} Downloading OpenClaw plugin files from GitHub...",
            "↓".cyan()
        );

        // Download key files
        for file in &[
            "openclaw.plugin.json",
            "index.ts",
            "package.json",
            "runtime-api.ts",
            "tsconfig.json",
        ] {
            let url = format!(
                "https://raw.githubusercontent.com/fajarhide/omni/main/integrations/openclaw/{}",
                file
            );
            let to = dest.join(file);

            let response = ureq::get(&url)
                .call()
                .map_err(|e| anyhow::anyhow!("Failed to download {}: {}", file, e))?;
            let mut dest_file = fs::File::create(&to)?;
            std::io::copy(&mut response.into_reader(), &mut dest_file)?;
        }

        // Try downloading package-lock.json, ignore error if missing (e.g., HTTP 404)
        let lock_url = "https://raw.githubusercontent.com/fajarhide/omni/main/integrations/openclaw/package-lock.json";
        if let Ok(response) = ureq::get(lock_url).call() {
            let to = dest.join("package-lock.json");
            if let Ok(mut dest_file) = fs::File::create(&to) {
                let _ = std::io::copy(&mut response.into_reader(), &mut dest_file);
            }
        }

        println!(
            "  {} Installed OpenClaw plugin to ~/.openclaw/plugins/omni-signal-engine/",
            "✓".green()
        );
        println!(
            "  {} Run {} to install dependencies",
            "→".cyan(),
            "cd ~/.openclaw/plugins/omni-signal-engine && npm install".bright_black()
        );
        Ok(())
    }

    fn uninstall(&self) -> anyhow::Result<()> {
        let dest = plugin_dir();
        if dest.exists() {
            fs::remove_dir_all(&dest)?;
            println!(
                "  {} Removed OpenClaw plugin from ~/.openclaw/plugins/",
                "✓".yellow()
            );
        }
        Ok(())
    }

    fn doctor_check(&self, _fix_mode: bool, _warnings: &mut Vec<String>) -> bool {
        let dest = plugin_dir();

        println!("\n  {}", "OpenClaw:".cyan());
        if dest.join("openclaw.plugin.json").exists() {
            println!(
                "   {:<15} {} {}",
                "Plugin:".bright_black(),
                "~/.openclaw/plugins/omni-signal-engine/".bright_black(),
                "[OK]".green().bold()
            );

            // Check if node_modules exists (npm install was run)
            if dest.join("node_modules").exists() {
                println!(
                    "   {:<15} {} {}",
                    "Dependencies:".bright_black(),
                    "installed".bright_black(),
                    "[OK]".green().bold()
                );
            } else {
                println!(
                    "   {:<15} {}",
                    "Dependencies:".bright_black(),
                    "run 'npm install' in plugin dir".yellow()
                );
            }
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
