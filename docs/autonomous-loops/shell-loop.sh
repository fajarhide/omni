#!/bin/bash
# ═══════════════════════════════════════════════════════════
# OMNI Generic Shell Loop Template
# ═══════════════════════════════════════════════════════════
#
# Usage:
#   ./shell-loop.sh "fix authentication tests" [max_iterations]
#
# Prerequisites:
#   - omni installed and initialized (omni init)
#   - jq installed
#   - Your agent CLI (claude, codex, etc.) available in PATH
#
# ═══════════════════════════════════════════════════════════

set -euo pipefail

GOAL="${1:?Usage: $0 <goal> [max_iterations]}"
MAX_ITERATIONS="${2:-20}"
AGENT_CMD="${OMNI_AGENT_CMD:-claude --dangerously-skip-permissions}"

# ── Initialize Loop Context ──────────────────────────────
export OMNI_LOOP_ID=$(uuidgen | tr '[:upper:]' '[:lower:]')
export OMNI_LOOP_GOAL="$GOAL"
export OMNI_LOOP_BUDGET="${OMNI_LOOP_BUDGET:-100000}"

echo "═══════════════════════════════════════════════"
echo " OMNI Loop Engine"
echo " Goal: $GOAL"
echo " Budget: $OMNI_LOOP_BUDGET tokens/iteration"
echo " Max iterations: $MAX_ITERATIONS"
echo " Loop ID: $OMNI_LOOP_ID"
echo "═══════════════════════════════════════════════"

# ── Main Loop ────────────────────────────────────────────
for i in $(seq 1 "$MAX_ITERATIONS"); do
    export OMNI_LOOP_ITERATION=$i
    echo ""
    echo "▸ Iteration $i/$MAX_ITERATIONS"

    # No shell-reachable checkpoint any more (#180).
    #
    # This used to be `omni handoff --json | jq -r '.recommendation.action'`.
    # #164 removed `handoff` as a CLI subcommand; the `omni_handoff` MCP tool is
    # unchanged but is reachable only from an MCP client, not from a shell.
    #
    # The old line is not kept-but-guarded on purpose: it ended in
    # `|| echo "CONTINUE"`, so once the command was gone the loop kept running
    # against a default it never computed — a confident value standing in for a
    # missing one, which is the failure OMNI exists to stop. Whether `handoff`
    # returns as a CLI subcommand for exactly this use is the open question on
    # #180. Until it is answered, this loop runs to MAX_ITERATIONS.
    STATUS="CONTINUE"

    case "$STATUS" in
        DONE)
            echo "  Loop completed successfully"
            break
            ;;
        ESCALATE)
            echo "  Loop escalated — human review needed"
            exit 2
            ;;
        COMPACT_OR_ESCALATE)
            echo "  ⚠️ Context pressure critical — compacting..."
            # Agent should auto-compact on next iteration
            ;;
    esac

    # Run agent
    echo "  Running agent..."
    $AGENT_CMD "Continue working on: $GOAL" || true

    # Optional: run verification
    if [ -n "${OMNI_VERIFY_CMD:-}" ]; then
        echo "  Running verification: $OMNI_VERIFY_CMD"
        if eval "$OMNI_VERIFY_CMD" 2>/dev/null; then
            echo "  Verification passed"
            break
        else
            echo "  Verification failed, continuing..."
        fi
    fi
done

echo ""
echo "═══════════════════════════════════════════════"
echo " Loop completed after $i iterations"
if command -v omni &>/dev/null; then
    omni stats --today 2>/dev/null || true
fi
echo "═══════════════════════════════════════════════"
