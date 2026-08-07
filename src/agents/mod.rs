pub mod antigravity;
pub mod checker;
pub mod claude;
pub mod cline;
pub mod codex;
pub mod copilot;
pub mod cursor;
pub mod gemini;
pub mod hermes;
pub mod multiagent;
pub mod openclaw;
pub mod opencode;
pub mod pi;
pub mod roo_code;
pub mod vscode;
pub mod zed;

pub use antigravity::AntigravityIntegration;
pub use claude::ClaudeIntegration;
pub use cline::ClineIntegration;
pub use codex::CodexIntegration;
pub use copilot::CopilotIntegration;
pub use cursor::CursorIntegration;
pub use gemini::GeminiIntegration;
pub use hermes::HermesIntegration;
pub use openclaw::OpenClawIntegration;
pub use opencode::OpenCodeIntegration;
pub use pi::PiIntegration;
pub use roo_code::RooCodeIntegration;
pub use vscode::VscodeIntegration;
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

    /// What this host actually lets OMNI do to the bytes the model reads.
    ///
    /// Defaults to `McpOnly` so a new integration has to *claim* distillation
    /// rather than inherit the claim. Installing hooks is not the same as the
    /// host calling them, and three integrations shipped that gap at once:
    /// Cursor registered `afterFileEdit`, Cline registered Claude's
    /// `PreToolUse`, and Gemini matched on `"Bash"`. All three printed
    /// `[OK] installed` and recorded nothing (#351).
    fn tier(&self) -> Tier {
        Tier::McpOnly
    }
}

/// How much of OMNI's job a host permits, stated once so `doctor` and the README
/// cannot drift apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// The host applies OMNI's rewrite, so the model reads distilled output.
    Full,
    /// The host exposes no way to change what the model reads for built-in
    /// tools. MCP, session memory and command rewriting may still work, and
    /// `omni_run` distils anything routed through it, but the built-in shell
    /// tool cannot be touched.
    McpOnly,
}

impl Tier {
    /// One line for `doctor`, phrased as the thing a user actually wants to know.
    pub fn label(self) -> &'static str {
        match self {
            Tier::Full => "model-facing distill active",
            Tier::McpOnly => "hooks/MCP only, built-in tool output not rewritten",
        }
    }
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
        Box::new(hermes::HermesIntegration),
        Box::new(pi::PiIntegration),
        Box::new(vscode::VscodeIntegration),
    ]
}
