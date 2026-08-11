use colored::Colorize;

pub mod checker;
pub mod claude;
pub mod cline;
pub mod codex;
pub mod cursor;
pub mod gemini;
pub mod hermes;
pub mod mcp_host;
pub mod multiagent;
pub mod openclaw;
pub mod output;
pub mod pi;

pub use claude::ClaudeIntegration;
pub use cline::ClineIntegration;
pub use codex::CodexIntegration;
pub use cursor::CursorIntegration;
pub use gemini::GeminiIntegration;
pub use hermes::HermesIntegration;
pub use openclaw::OpenClawIntegration;
pub use pi::PiIntegration;

/// Reports the outcome of a `doctor --fix` repair attempt, honestly.
///
/// Every call site did `let _ = self.install(...)` and then printed `[FIXED]`
/// unconditionally. Against an unwritable home, `omni doctor --json --fix`
/// answered `healthy: true`, `hooks installed`, and **zero warnings**, while
/// nothing had been written: in fix mode the `[FIXED]` line replaced the warning
/// that would otherwise have been pushed, so the failure had nowhere to surface
/// (#386). Nineteen sites across eleven integrations shared the pattern and not
/// one checked the result.
pub fn report_fix(
    field: &str,
    what: &str,
    outcome: anyhow::Result<()>,
    warnings: &mut Vec<String>,
) -> bool {
    match outcome {
        Ok(()) => {
            crate::agent_report!(
                "   {:<15} {}",
                field.bright_black(),
                format!("[FIXED] {what}").green().bold()
            );
            true
        }
        Err(e) => {
            crate::agent_report!(
                "   {:<15} {}",
                field.bright_black(),
                format!("[FAILED] {what}: {e}").red().bold()
            );
            warnings.push(format!("Could not repair {field} {what}: {e}"));
            false
        }
    }
}

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
    /// The host applies OMNI's rewrite, so the model reads distilled output for
    /// its own built-in tools.
    Full,
    /// The host cannot rewrite built-in tool output, but OMNI can still be the
    /// tool that runs the command: `omni_run` returns distilled bytes, and the
    /// installed rule is what makes the agent reach for it. Distinct from
    /// `McpOnly` because the model demonstrably reads less here, which is the
    /// unit #352 measures by.
    HandoffFirst,
    /// Memory, recall and session state only. No path by which OMNI changes what
    /// the model reads for a shell command, and no claim that it does.
    McpOnly,
}

impl Tier {
    /// One line for `doctor`, phrased as the thing a user actually wants to know.
    /// The tier alone, for the per-host line. `label()` carries the tier plus its
    /// explanation, and printing that in full once per host meant ten identical
    /// sentences on a machine with ten MCP-only hosts (#426). The line itself has
    /// to stay per host: "hooks installed" and "the model reads less" are
    /// different claims, and three integrations shipped the first while
    /// delivering none of the second (#351).
    pub fn name(self) -> &'static str {
        match self {
            Tier::Full => "Full",
            Tier::HandoffFirst => "Handoff-first",
            Tier::McpOnly => "MCP-only",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Tier::Full => "Full: model-facing distill active",
            Tier::HandoffFirst => {
                "Handoff-first: built-in tool output not rewritten; omni_run distils what you route through it"
            }
            Tier::McpOnly => "MCP-only: memory and session state, no shell distill",
        }
    }
}

pub fn all_integrations() -> Vec<Box<dyn AgentIntegration>> {
    // The hosts that do more than write one JSON entry, then the six that do
    // exactly that, from `mcp_host::HOSTS` (#443). Order is what `omni doctor`
    // prints, so the ones with hooks come first.
    let mut all: Vec<Box<dyn AgentIntegration>> = vec![
        Box::new(claude::ClaudeIntegration),
        Box::new(cursor::CursorIntegration),
        Box::new(cline::ClineIntegration),
        Box::new(gemini::GeminiIntegration),
        Box::new(codex::CodexIntegration),
        Box::new(openclaw::OpenClawIntegration),
        Box::new(hermes::HermesIntegration),
        Box::new(pi::PiIntegration),
    ];
    all.extend(
        mcp_host::HOSTS
            .iter()
            .map(|h| Box::new(h) as Box<dyn AgentIntegration>)
            .collect::<Vec<_>>(),
    );
    all
}

#[cfg(test)]
mod fix_reporting_tests {
    use super::report_fix;

    /// #386: every site did `let _ = self.install(...)` and printed `[FIXED]`
    /// unconditionally. Against an unwritable home, `doctor --json --fix`
    /// answered `healthy: true`, `hooks installed` and zero warnings while
    /// nothing had been written, because in fix mode the `[FIXED]` line replaced
    /// the warning that would otherwise have been pushed.
    #[test]
    fn a_failed_repair_is_reported_and_warned_about() {
        let mut warnings = Vec::new();

        let (ok, printed) = crate::agents::output::capture(|| {
            report_fix(
                "Hooks:",
                "missing hooks installed",
                Err(anyhow::anyhow!("Permission denied (os error 13)")),
                &mut warnings,
            )
        });

        assert!(!ok, "a failed repair must not report success");
        assert!(printed.contains("[FAILED]"), "{printed}");
        assert!(printed.contains("Permission denied"), "{printed}");
        assert_eq!(
            warnings.len(),
            1,
            "the failure must reach --json: {warnings:?}"
        );
    }

    /// A repair that worked still says so, or the fix has swallowed the signal
    /// it exists to give.
    #[test]
    fn a_successful_repair_still_reports_fixed() {
        let mut warnings = Vec::new();

        let (ok, printed) = crate::agents::output::capture(|| {
            report_fix("Config:", "registered", Ok(()), &mut warnings)
        });

        assert!(ok);
        assert!(printed.contains("[FIXED]"), "{printed}");
        assert!(warnings.is_empty(), "{warnings:?}");
    }
}
