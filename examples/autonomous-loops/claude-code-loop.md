# Claude Code `/loop` Integration with OMNI

> Setup OMNI as middleware for Claude Code agentic loops.

## Prerequisites

```bash
# Install OMNI
brew install fajarhide/tap/omni

# Initialize in your project
cd /path/to/project
omni init
```

## CLAUDE.md Configuration

Add to your project's `CLAUDE.md`:

```markdown
## OMNI Integration

This project uses OMNI for context-aware token management.

### Key Behaviors
- ALWAYS check `omni_budget` before starting expensive operations
- Call `omni_loop_status` at the start of each iteration
- When context pressure is WARNING: complete current subtask then compact
- When context pressure is CRITICAL: immediately summarize and compact

### MCP Tools Available
- `omni_budget` — Check remaining token budget
- `omni_session` — View current session state
- `omni_loop_status` — Get loop health + recommendation (CONTINUE/COMPACT/ESCALATE)
- `omni_signal_extract` — Extract high-signal content from recent outputs
- `omni_handoff` — Export session state for handoff to next agent
```

## Hook Setup

OMNI hooks are auto-configured via `omni init`. The hooks process:
- `PostToolUse` — Distills tool output, tracks token usage
- `PreCompact` — Generates loop checkpoint engrams
- `SessionStart` / `SessionEnd` — Session lifecycle management

## Loop-Aware Mode

Set environment variables before launching the loop:

```bash
export OMNI_LOOP_ID=$(uuidgen)
export OMNI_LOOP_GOAL="fix authentication tests"
export OMNI_LOOP_BUDGET=100000  # 100K tokens per iteration
```

## Budget Management Strategies

### Strategy 1: Conservative (recommended for long tasks)
```bash
export OMNI_LOOP_BUDGET=80000
# OMNI warns at 65%, compacts at 82%
```

### Strategy 2: Aggressive (for short, focused tasks)
```bash
export OMNI_LOOP_BUDGET=150000
# More headroom, fewer compactions
```

### Strategy 3: Unlimited (for debugging)
```bash
# Don't set OMNI_LOOP_BUDGET — OMNI still tracks but won't warn
```

## MCP Tool Usage Patterns

### At iteration start:
```
Call omni_loop_status to check if CONTINUE, COMPACT, or ESCALATE
```

### Before expensive operations:
```
Call omni_budget to check remaining token budget
If budget < 20%, defer non-critical file reads
```

### After completing a subtask:
```
Call omni_signal_extract to review what was accomplished
Call omni_handoff --json to checkpoint progress
```

## Example: Full Loop Script

```bash
#!/bin/bash
export OMNI_LOOP_ID=$(uuidgen)
export OMNI_LOOP_GOAL="$1"

for i in $(seq 1 ${MAX_ITERATIONS:-20}); do
    export OMNI_LOOP_ITERATION=$i

    STATUS=$(omni handoff --json | jq -r '.recommendation.action')
    case "$STATUS" in
        DONE) echo "✅ Loop completed"; break ;;
        ESCALATE) echo "⚠️ Human review needed"; exit 2 ;;
    esac

    claude --dangerously-skip-permissions "Continue: $OMNI_LOOP_GOAL"
done
```
