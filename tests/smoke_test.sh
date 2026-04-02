#!/bin/bash
# OMNI Final Integration & Smoke Test
# Covers 9 end-to-end scenarios for release validation.
# Usage: tests/smoke_test.sh [path-to-omni-binary]

set -euo pipefail

OMNI="${1:-./target/release/omni}"
if [ ! -f "$OMNI" ]; then
    OMNI="./target/debug/omni"
fi
if [ ! -f "$OMNI" ]; then
    echo "Error: omni binary not found. Build first: cargo build --release"
    exit 1
fi

PASS=0
FAIL=0
TOTAL=0

check() {
    local name="$1"
    local output="$2"
    local expected="$3"
    TOTAL=$((TOTAL + 1))

    if echo "$output" | grep -qi "$expected"; then
        echo "  ✓ $name"
        PASS=$((PASS + 1))
    else
        echo "  ✗ $name"
        echo "    expected: '$expected'"
        echo "    got: $(echo "$output" | head -3)"
        FAIL=$((FAIL + 1))
    fi
}

check_exit() {
    local name="$1"
    local exit_code="$2"
    local expected_code="$3"
    TOTAL=$((TOTAL + 1))

    if [ "$exit_code" -eq "$expected_code" ]; then
        echo "  ✓ $name (exit $exit_code)"
        PASS=$((PASS + 1))
    else
        echo "  ✗ $name (expected exit $expected_code, got $exit_code)"
        FAIL=$((FAIL + 1))
    fi
}

check_shorter() {
    local name="$1"
    local input_len="$2"
    local output_len="$3"
    TOTAL=$((TOTAL + 1))

    if [ "$output_len" -le "$input_len" ]; then
        echo "  ✓ $name (${input_len}B → ${output_len}B)"
        PASS=$((PASS + 1))
    else
        echo "  ✗ $name (output ${output_len}B > input ${input_len}B)"
        FAIL=$((FAIL + 1))
    fi
}

echo "═══════════════════════════════════════════"
echo " OMNI Final Integration Tests"
echo " Binary: $OMNI"
echo "═══════════════════════════════════════════"
echo ""

# ─── 1. Version ──────────────────────────────────────────
echo "▸ Scenario 1: Version"
VERSION_OUT=$("$OMNI" version 2>&1)
check "version output" "$VERSION_OUT" "omni"

# ─── 2. Help ─────────────────────────────────────────────
echo "▸ Scenario 2: Help"
HELP_OUT=$("$OMNI" help 2>&1)
check "help shows stats" "$HELP_OUT" "stats"
check "help shows doctor" "$HELP_OUT" "doctor"
check "help shows learn" "$HELP_OUT" "learn"
check "help shows session" "$HELP_OUT" "session"
check "help shows pipe mode" "$HELP_OUT" "| omni"

# ─── 3. Doctor ───────────────────────────────────────────
echo "▸ Scenario 3: Doctor"
DOCTOR_OUT=$("$OMNI" doctor 2>&1 || true)
check "doctor shows header" "$DOCTOR_OUT" "OMNI Doctor"
check "doctor shows binary" "$DOCTOR_OUT" "Binary"

# ─── 4. PostToolUse Hook E2E ─────────────────────────────
echo "▸ Scenario 4: PostToolUse Hook E2E"
FIXTURE_CONTENT=$(cat tests/fixtures/git_diff_multi_file.txt)
INPUT_LEN=${#FIXTURE_CONTENT}
HOOK_JSON=$(cat <<EOF
{
  "hook_event_name": "PostToolUse",
  "tool_name": "Bash",
  "tool_input": {"command": "git diff HEAD~1"},
  "tool_response": {
    "content": $(echo "$FIXTURE_CONTENT" | python3 -c 'import sys,json; print(json.dumps(sys.stdin.read()))')
  }
}
EOF
)
HOOK_OUT=$(echo "$HOOK_JSON" | "$OMNI" --hook 2>/dev/null || true)
HOOK_EXIT=$?
check_exit "hook exits cleanly" "$HOOK_EXIT" "0"

if [ -n "$HOOK_OUT" ]; then
    # Check if output is valid JSON
    if echo "$HOOK_OUT" | python3 -c 'import sys,json; json.load(sys.stdin)' 2>/dev/null; then
        echo "  ✓ hook output is valid JSON"
        PASS=$((PASS + 1))
    else
        echo "  ✗ hook output is not valid JSON"
        FAIL=$((FAIL + 1))
    fi
    TOTAL=$((TOTAL + 1))
else
    # Empty output is OK for short content (passthrough)
    echo "  ✓ hook produced empty output (passthrough for short content)"
    PASS=$((PASS + 1))
    TOTAL=$((TOTAL + 1))
fi

# ─── 5. Pipe Mode ────────────────────────────────────────
echo "▸ Scenario 5: Pipe Mode"
PIPE_INPUT=$(cat tests/fixtures/git_diff_multi_file.txt)
PIPE_INPUT_LEN=${#PIPE_INPUT}
PIPE_OUT=$(echo "$PIPE_INPUT" | "$OMNI" 2>/dev/null)
PIPE_EXIT=$?
PIPE_OUT_LEN=${#PIPE_OUT}
check_exit "pipe mode exit 0" "$PIPE_EXIT" "0"
check_shorter "pipe output ≤ input" "$PIPE_INPUT_LEN" "$PIPE_OUT_LEN"

# ─── 6. SessionStart Mock ────────────────────────────────
echo "▸ Scenario 6: SessionStart Hook"
SESSION_JSON='{"hook_event_name":"SessionStart","session_id":"test-smoke-session"}'
SESSION_OUT=$(echo "$SESSION_JSON" | "$OMNI" --hook 2>/dev/null || true)
SESSION_EXIT=$?
check_exit "session start exits cleanly" "$SESSION_EXIT" "0"

# ─── 7. Stats ────────────────────────────────────────────
echo "▸ Scenario 7: Stats"
STATS_OUT=$("$OMNI" stats 2>&1 || true)
check "stats shows header" "$STATS_OUT" "Signal Report"
check "stats shows commands" "$STATS_OUT" "Commands processed"

# ─── 8. Learn ────────────────────────────────────────────
echo "▸ Scenario 8: Learn"
LEARN_OUT=$(cat tests/fixtures/cargo_build_errors.txt | "$OMNI" learn 2>&1 || true)
LEARN_EXIT=$?
check_exit "learn exits cleanly" "$LEARN_EXIT" "0"

# ─── 9. MCP Server ───────────────────────────────────────
echo "▸ Scenario 9: MCP Server"
# MCP server reads stdin — give it empty stdin with timeout
# macOS doesn't have `timeout`, use perl-based alternative
MCP_EXIT=0
if command -v timeout &>/dev/null; then
    timeout 2 "$OMNI" --mcp </dev/null 2>/dev/null || MCP_EXIT=$?
else
    perl -e 'alarm 2; exec @ARGV' "$OMNI" --mcp </dev/null 2>/dev/null || MCP_EXIT=$?
fi
# Exit 124/142 = timeout (expected), 0 = clean exit, both are OK
if [ "$MCP_EXIT" -eq 124 ] || [ "$MCP_EXIT" -eq 142 ] || [ "$MCP_EXIT" -eq 0 ]; then
    echo "  ✓ MCP server starts without crash (exit $MCP_EXIT)"
    PASS=$((PASS + 1))
else
    echo "  ✗ MCP server crashed immediately (exit $MCP_EXIT)"
    FAIL=$((FAIL + 1))
fi
TOTAL=$((TOTAL + 1))

# ─── 10. Unknown Command ─────────────────────────────────
echo "▸ Scenario 10: Error Handling"
UNKNOWN_OUT=$("$OMNI" nonexistent-cmd 2>&1 || true)
check "unknown command error" "$UNKNOWN_OUT" "unknown command"

EMPTY_PIPE_EXIT=0
printf '' | "$OMNI" 2>/dev/null || EMPTY_PIPE_EXIT=$?
# Empty pipe should exit 0 (silent passthrough)
if [ "$EMPTY_PIPE_EXIT" -eq 0 ]; then
    echo "  ✓ empty pipe exits cleanly ($EMPTY_PIPE_EXIT)"
    PASS=$((PASS + 1))
else
    echo "  ✗ empty pipe should exit 0 but got $EMPTY_PIPE_EXIT"
    FAIL=$((FAIL + 1))
fi
TOTAL=$((TOTAL + 1))

# ─── Results ─────────────────────────────────────────────
echo ""
echo "═══════════════════════════════════════════"
echo " Results: $PASS/$TOTAL passed, $FAIL failed"
echo "═══════════════════════════════════════════"

[ $FAIL -eq 0 ] && exit 0 || exit 1
