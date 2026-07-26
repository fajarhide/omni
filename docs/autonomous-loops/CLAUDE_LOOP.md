# Claude Agent — Loop Mode Configuration

## OMNI Integration

This agent is running inside a loop managed by an external orchestrator.
OMNI is the context management middleware providing:
- Token budget tracking and predictive pressure warnings
- Output distillation (reducing noise, preserving errors)
- Loop memory persistence across iterations
- Maker-checker verification support

## Key Behaviors

### Context Management
- **ALWAYS** check `omni_budget` before starting expensive operations (large file reads, recursive searches)
- Call `omni_loop_status` at the start of each iteration to get CONTINUE/COMPACT/ESCALATE recommendation
- When context pressure is **WARNING**: complete current subtask, then compact
- When context pressure is **CRITICAL**: immediately summarize progress and request compaction

### Loop Awareness
- Your work is part of a larger loop. Each iteration should make measurable progress.
- Use `omni_loop_memory` to persist key insights across compactions:
  - Files you've already verified
  - Patterns you've identified
  - Decisions you've made and why

### Verification Protocol
After completing significant work:
1. Run verification command (e.g., `cargo test`, `npm test`)
2. OMNI will automatically create an engram on test success
3. Call `omni_handoff` (MCP tool) to checkpoint progress for the orchestrator

## Available MCP Tools

| Tool | When to Use |
|---|---|
| `omni_budget` | Before expensive operations |
| `omni_session` | Check session state |
| `omni_loop_status` | Start of each iteration |
| `omni_signal_extract` | Review recent high-signal outputs |
| `omni_goal_alignment` | Check if work aligns with goal |
| `omni_loop_memory` | Persist/retrieve cross-iteration memory |
| `omni_noise_profile` | Identify noisy tool outputs |
| `omni_handoff` | Export session state |

## Error Handling

- If a tool fails, OMNI tracks it automatically via `PostToolUseFailure` hook
- Don't retry the same failing command more than 3 times
- If stuck, call `omni_loop_status` — it may recommend ESCALATE

## Do NOT

- ❌ Read the same file more than once per iteration (OMNI detects duplicates)
- ❌ Ignore context pressure warnings
- ❌ Make changes without running verification
- ❌ Attempt to bypass OMNI hooks or environment variables
