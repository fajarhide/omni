//! Which of OMNI's MCP tools a host is told about.
//!
//! Measured 2026-08-15 across 228 Claude Code transcripts: 25 tools are
//! advertised in the prefix of every request, 16 of them have never been
//! called, and those 16 are 4,940 bytes. That is the same size as the 4,942
//! bytes OMNI removes from tool output in a median session, and it sits at
//! position 0 where it is re-read on every request rather than from the middle
//! of the session. Advertising them costs about twice what the distillers earn.
//!
//! The set is chosen by `Tier`, which already exists and is what `doctor`
//! prints, so this module adds no second opinion about what a host can do.

use crate::agents::Tier;

/// Called at least once in the corpus, so a host that can use them is told
/// about them. Ordered as the probe reports them, most-called first.
pub const FULL: &[&str] = &[
    "omni_retrieve",
    "omni_explain_savings",
    "omni_remember",
    "omni_recall",
    "omni_run",
    "omni_find_noise",
    "omni_context_breakdown",
    "omni_history",
    "omni_context",
];

/// Same list today, kept separate so a future edit to `FULL` cannot silently
/// drop `omni_run` from the one tier that has nothing else. The test is what
/// makes that guarantee real.
pub const HANDOFF: &[&str] = FULL;

/// No shell distillation exists on this tier, so the tools that report on it
/// would describe something that never happens.
pub const MEMORY: &[&str] = &[
    "omni_remember",
    "omni_recall",
    "omni_retrieve",
    "omni_knowledge",
];

/// Every tool the binary can serve. Kept here so `doctor` does not have to
/// build a router to count them, and asserted against the router in
/// `server.rs` so the two cannot drift.
pub const ALL_TOOL_COUNT: usize = 25;

/// `detect_agent_id` and `AgentIntegration::id` disagree for exactly the two
/// Full-tier hosts, which is the whole reason this function is not a one-liner.
fn integration_id(detected: &str) -> &str {
    match detected {
        "claude_code" => "claude",
        "codex_cli" => "codex",
        other => other,
    }
}

/// The tier the host's own integration declares. `Tier` stays the single source
/// of truth; this only translates the name.
pub fn tier_for(agent_id: &str) -> Tier {
    let want = integration_id(agent_id);
    crate::agents::all_integrations()
        .iter()
        .find(|a| a.id() == want)
        .map_or(Tier::McpOnly, |a| a.tier())
}

/// The tools this host is told about.
pub fn active_tools(agent_id: &str) -> &'static [&'static str] {
    match tier_for(agent_id) {
        Tier::Full => FULL,
        Tier::HandoffFirst => HANDOFF,
        Tier::McpOnly => MEMORY,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::Tier;

    /// The trap this module exists for. `detect_agent_id` returns `claude_code`
    /// and `codex_cli`; the integrations that carry the tier are registered as
    /// `claude` and `codex`. A lookup without the alias silently drops both
    /// Full-tier hosts to `McpOnly`, which is the one outcome that would cut
    /// tools the main host actually calls.
    #[test]
    fn the_detected_ids_map_to_the_tier_their_integration_declares() {
        assert_eq!(tier_for("claude_code"), Tier::Full);
        assert_eq!(tier_for("codex_cli"), Tier::Full);
        assert_eq!(tier_for("cursor"), Tier::HandoffFirst);
        assert_eq!(tier_for("gemini"), Tier::Full);
        // Anything unrecognised claims nothing, the same default the
        // `AgentIntegration` trait uses.
        assert_eq!(tier_for("something-new"), Tier::McpOnly);
    }

    /// `strategy.md` section 5 makes this product law: on a Handoff-first host
    /// the built-in tool output is never rewritten, so `omni_run` is the only
    /// path by which the model reads less. Cutting it there would remove the
    /// host's entire reason to have OMNI installed.
    #[test]
    fn a_handoff_first_host_keeps_omni_run() {
        assert!(active_tools("cursor").contains(&"omni_run"));
    }

    /// An MCP-only host has no shell distillation to report on, so the read
    /// tools over it are noise there.
    #[test]
    fn an_mcp_only_host_gets_the_memory_set_and_nothing_else() {
        let tools = active_tools("windsurf");
        assert!(tools.contains(&"omni_remember"));
        assert!(tools.contains(&"omni_retrieve"));
        assert!(!tools.contains(&"omni_explain_savings"));
    }

    /// Measured 2026-08-15: nine tools were called across 228 sessions and the
    /// other sixteen never were. If this list grows without the probe being
    /// re-run, the design's own justification has stopped being true.
    #[test]
    fn the_full_set_is_the_nine_that_were_actually_called() {
        let mut got = active_tools("claude_code").to_vec();
        got.sort_unstable();
        let mut want = vec![
            "omni_context",
            "omni_context_breakdown",
            "omni_explain_savings",
            "omni_find_noise",
            "omni_history",
            "omni_recall",
            "omni_remember",
            "omni_retrieve",
            "omni_run",
        ];
        want.sort_unstable();
        assert_eq!(got, want);
    }
}
