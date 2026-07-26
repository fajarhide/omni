# OMNI Loop Engineering Guide

> How to use OMNI as the context management layer for autonomous AI agent loops.

## What is Loop Engineering?

Loop Engineering is the practice of running AI agents in iterative loops where each iteration builds on the last until a goal is achieved. OMNI acts as the "Context Operating System" — managing token budgets, distilling outputs, and providing loop-aware intelligence.

## Architecture

```
┌─────────────────────────────────────────────┐
│           Outer Orchestrator                │
│  (shell script, Mastra, custom framework)   │
├─────────────────────────────────────────────┤
│  OMNI_LOOP_ID  │  OMNI_LOOP_GOAL           │
│  OMNI_LOOP_ITERATION  │  OMNI_LOOP_BUDGET  │
├─────────────────────────────────────────────┤
│               OMNI Hooks                    │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  │
│  │PostTool  │  │PreCompact│  │SessionEnd│  │
│  │Distill   │  │Engram    │  │Handoff   │  │
│  └──────────┘  └──────────┘  └──────────┘  │
├─────────────────────────────────────────────┤
│              AI Agent (inner loop)          │
│  Claude Code / Cursor / Codex / Aider       │
└─────────────────────────────────────────────┘
```

## Quick Start

### 1. Initialize OMNI

```bash
cd /path/to/project
omni init
```

### 2. Set Loop Environment

```bash
export OMNI_LOOP_ID=$(uuidgen)
export OMNI_LOOP_GOAL="fix all failing tests in src/"
export OMNI_LOOP_BUDGET=100000
```

### 3. Run Your Loop

See `autonomous-loops/` for ready-to-use templates:
- `shell-loop.sh` — Generic shell loop
- `claude-code-loop.md` — System prompt for Claude Code
- `mastra-integration.ts` — TypeScript/Mastra orchestrator setup

## Environment Variables

| Variable | Description | Default |
|---|---|---|
| `OMNI_LOOP_ID` | Unique loop identifier (alphanumeric + dash, max 64 chars) | None |
| `OMNI_LOOP_GOAL` | Goal string (max 500 chars, no shell metacharacters) | None |
| `OMNI_LOOP_BUDGET` | Token budget per iteration (max 10M) | None |
| `OMNI_LOOP_ITERATION` | Current iteration number | 0 |
| `OMNI_SUBAGENT` | Set to "1" for sub-agent mode | None |
| `OMNI_AGENT_ID` | Agent identity mapping (e.g. maker, checker) | None |

## Token Budget Strategy

OMNI tracks two types of token counts:
1. **Raw tokens** — Total tokens before distillation
2. **Filtered tokens** — Tokens after OMNI's compression pipeline

The budget (`OMNI_LOOP_BUDGET`) refers to the **estimated context window usage** per iteration.

### Recommended Budgets by Task Type

*   **Quick Fix (1–5 iterations)**: `200000` tokens
    *   **Strategy**: High budget, no compactions expected. OMNI role: Passive tracking only.
*   **Feature Development (5–20 iterations)**: `100000` tokens
    *   **Strategy**: Balanced — compact at natural checkpoints. OMNI role: Active distillation, engram generation.
*   **Large Refactor (20–100 iterations)**: `80000` tokens
    *   **Strategy**: Conservative — frequent compactions. OMNI role: Aggressive distillation, predictive warnings.
*   **Marathon Session (100+ iterations)**: `60000` tokens
    *   **Strategy**: Ultra-conservative — maximize signal density. OMNI role: Maximum compression, loop memory persistence.

### Pressure Thresholds & Warnings

OMNI tracks token consumption rate and predicts when the context window will be exhausted. 

| Budget | Warning (65%) | Critical (82%) | Action |
|---|---|---|---|
| 200K | 130K tokens | 164K tokens | Complete current task, prepare to compact / escalate |
| 100K | 65K tokens | 82K tokens | Complete current task, prepare to compact / escalate |
| 80K | 52K tokens | 65.6K tokens | Complete current task, prepare to compact / escalate |
| 60K | 39K tokens | 49.2K tokens | Complete current task, prepare to compact / escalate |

> **Anti-Pattern:** Do not set budgets > 1M, or warnings will never fire before true exhaustion. Do not set budgets < 30K, or the agent will constantly compact and lose critical short-term memory.

### Dynamic Budget Adjustment

OMNI's `GoalScoringModifier` automatically adjusts distillation aggressiveness based on the loop goal:
- **Goal contains "test"** → Preserve test output details (lower compression)
- **Goal contains "debug"** → Keep error context (lower compression)
- **Goal contains "refactor"** → Compress aggressively (higher compression)

## Maker-Checker Verification Pattern

The Maker-Checker pattern splits work between two agents to ensure verified loop execution. OMNI provides the shared context layer between them.

1. **Maker agent** runs tool calls, OMNI tracks outputs
2. **Checker agent** calls `omni_verify` to review maker's work
3. Orchestrator reads structured pass/fail from checker

### Session Isolation
OMNI ensures maker and checker sessions don't contaminate each other by utilizing unique `OMNI_AGENT_ID` values. Distillation traces are tagged with `agent_id`, allowing `omni_verify` to read across sessions while writes remain strictly isolated.

### Example Orchestrator Logic
```bash
#!/bin/bash
GOAL="$1"
LOOP_ID=$(uuidgen)

# Phase 1: Maker
export OMNI_AGENT_ID=maker OMNI_LOOP_ID=$LOOP_ID
claude "Implement: $GOAL"

# Phase 2: Checker
export OMNI_AGENT_ID=checker OMNI_SUBAGENT=1
RESULT=$(claude "Verify the implementation of: $GOAL. Use omni_verify tool.")

# Phase 3: Decision
if echo "$RESULT" | grep -q "PASS"; then
    echo "✅ Maker-Checker verification passed"
else
    echo "❌ Checker found issues, re-running maker..."
fi
```

### Maker-Checker Best Practices
1. **Clear criteria**: Give the checker specific, measurable criteria
2. **Limit scope**: Keep `last_n_calls` reasonable (5–20)
3. **Fail fast**: If checker fails 3x, escalate to human
4. **Audit trail**: OMNI logs all maker/checker interactions in SQLite

## MCP Tools for Loop Control

When OMNI's MCP server is running, these tools are available to agents:

| Tool | Description |
|---|---|
| `omni_loop_status` | Health check with CONTINUE/COMPACT/ESCALATE recommendation |
| `omni_signal_extract` | Extract high-signal content from recent N commands |
| `omni_goal_alignment` | Check if recent work aligns with loop goal |
| `omni_noise_profile` | Analyze noise patterns in recent outputs |
| `omni_iteration_summary` | Summarize what happened in last N iterations |
| `omni_loop_memory` | Read/write persistent loop memory |
| `omni_budget` | Token budget status and projections |
| `omni_verify` | Read another agent's session for Maker-Checker verification |

## Monitoring & Analytics

```bash
# Real-time stats
omni stats --today

# Full breakdown
omni stats --detail

# Machine-readable for Integrations
omni stats --json

# Health check
omni doctor

# Check current session budget status — MCP only.
# `omni handoff` was removed as a CLI subcommand in #164; the `omni_handoff`
# MCP tool is unchanged, so this is reachable from an MCP client, not a shell.
```
