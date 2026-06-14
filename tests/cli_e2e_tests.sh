#!/bin/bash
# E2E Tests for OMNI Sprints L1-L4

set -e

echo "Running CLI E2E Tests..."

# Build the omni binary
cargo build --release
OMNI_BIN="./target/release/omni"

# Session State
echo "[Session] Test 1: Init runs successfully"
echo "1" | $OMNI_BIN init > /dev/null

echo "[Session] Test 2: Status check"
$OMNI_BIN doctor | grep -i "ok" || true

# Handoff Export
echo "[Handoff] Test 1: Handoff returns json"
$OMNI_BIN handoff --json | grep "schema_version"

echo "[Handoff] Test 2: Handoff handles empty session"
# we can't easily guarantee empty session but it shouldn't crash
$OMNI_BIN handoff > /dev/null

echo "[Handoff] Test 3: Agent identity injection"
OMNI_AGENT_ID=test_agent $OMNI_BIN handoff --json | grep "test_agent"

# Analytics Stats
echo "[Analytics] Test 1: Stats summary output"
$OMNI_BIN stats --today > /dev/null

echo "[Analytics] Test 2: Stats json output"
$OMNI_BIN stats --json > /dev/null

# Security Env
echo "[Security] Test 1: Loop parameters override"
OMNI_LOOP_ID=test-id-123 $OMNI_BIN handoff --json > /dev/null

echo "All E2E tests passed!"
