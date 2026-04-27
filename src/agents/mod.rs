pub mod antigravity;
pub mod claude;
pub mod cline;
pub mod codex;
pub mod copilot;
pub mod cursor;
pub mod gemini;
pub mod multiagent;
pub mod openclaw;
pub mod opencode;
pub mod roo_code;
pub mod zed;

pub use antigravity::AntigravityIntegration;
pub use claude::ClaudeIntegration;
pub use cline::ClineIntegration;
pub use codex::CodexIntegration;
pub use copilot::CopilotIntegration;
pub use cursor::CursorIntegration;
pub use gemini::GeminiIntegration;
pub use openclaw::OpenClawIntegration;
pub use opencode::OpenCodeIntegration;
pub use roo_code::RooCodeIntegration;
pub use zed::ZedIntegration;

pub trait AgentIntegration {
    /// CLI identifier used in `--[id]` (e.g. "vscode", "codex", "claude").
    fn id(&self) -> &'static str;

    /// Human-readable name for logging (e.g. "Claude Code").
    fn name(&self) -> &'static str;

    /// Runs the actual setup script.
    /// For Claude, it modifies `settings.json`. For Antigravity, it downloads the zip, etc.
    fn install(&self, exe_path: &str) -> anyhow::Result<()>;

    /// Uninstalls and removes configuration injected into the agent.
    fn uninstall(&self) -> anyhow::Result<()>;

    /// Runs a diagnostic check to see if the configuration is intact.
    /// `fix_mode` determines whether the doctor should attempt auto-repair.
    /// Returns `true` if healthy or successfully repaired.
    fn doctor_check(&self, fix_mode: bool, warnings: &mut Vec<String>) -> bool;
}

pub fn all_integrations() -> Vec<Box<dyn AgentIntegration>> {
    vec![
        Box::new(claude::ClaudeIntegration),
        Box::new(cursor::CursorIntegration),
        Box::new(zed::ZedIntegration),
        Box::new(cline::ClineIntegration),
        Box::new(roo_code::RooCodeIntegration),
        Box::new(copilot::CopilotIntegration),
        Box::new(gemini::GeminiIntegration),
        Box::new(opencode::OpenCodeIntegration),
        Box::new(codex::CodexIntegration),
        Box::new(openclaw::OpenClawIntegration),
        Box::new(antigravity::AntigravityIntegration),
    ]
}
