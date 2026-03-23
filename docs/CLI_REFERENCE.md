# CLI Reference

Complete reference for all OMNI commands and flags.

## Global Usage

```
omni [MODE] [COMMAND] [FLAGS]
```

## Modes (Automatic)

These are used by Claude Code hooks and MCP — you typically don't call them manually.

### `omni --hook`

Universal hook mode. Reads JSON from stdin with `hook_event_name` and dispatches to the appropriate handler.

```bash
# Called automatically by Claude Code
echo '{"hook_event_name": "PostToolUse", ...}' | omni --hook
```

### `omni --mcp`

Start the MCP server. Provides 5 tools:

| Tool | Description |
|---|---|
| `omni_retrieve(hash)` | Retrieve content from RewindStore |
| `omni_learn(text, apply?)` | Detect noise patterns, suggest filters |
| `omni_density(text)` | Analyze token reduction ratio |
| `omni_trust(projectPath?)` | Trust a project's local config |
| `omni_compress(text)` | Compress text through the pipeline |

### Pipe Mode

```bash
# Automatically detected when stdin is not a TTY
git diff HEAD~3 | omni
cargo test 2>&1 | omni
kubectl get pods | omni
```

---

## Commands

### `omni init`

Setup OMNI hooks in Claude Code.

```bash
omni init --hook       # Install PostToolUse/SessionStart/PreCompact hooks
omni init --status     # Check installation status
omni init --uninstall  # Remove all OMNI hooks
```

**What it does:**
- Creates/updates `~/.claude/settings.json`
- Backs up existing settings to `settings.json.bak`
- Registers hook commands pointing to your `omni` binary

---

### `omni stats`

Token savings analytics dashboard.

```bash
omni stats              # Last 30 days (default)
omni stats --today      # Today only
omni stats --week       # Last 7 days
omni stats --month      # Last 30 days (explicit)
omni stats --passthrough  # Show commands without filter coverage
omni stats --session    # Session-level breakdown
```

**Output includes:**
- Commands processed, input/output bytes, signal ratio
- Estimated cost savings (@$3/1M tokens)
- Per-filter breakdown with ASCII bar charts
- Route distribution (Keep/Soft/Passthrough/Rewind)
- Session insights (hot files, accuracy signals)

---

### `omni session`

Inspect and manage session state.

```bash
omni session            # Show current session
omni session --inject   # Output the injection string (for use in other agents)
omni session --history  # List recent sessions
omni session --clear    # Clear current session
```

---

### `omni learn`

Auto-generate TOML filters from passthrough output.

```bash
omni learn < output.log           # Analyze from stdin
omni learn --from-queue           # Analyze from learn queue
omni learn --dry-run              # Show candidates without applying
omni learn --apply                # Write to ~/.omni/filters/learned.toml
omni learn --verify               # Run inline tests on all loaded filters
```

**How it works:**
1. Reads output text (stdin or learn queue)
2. Detects repetitive patterns (≥3 occurrences)
3. Generates TOML filter candidates
4. Optionally applies them to `learned.toml`

---

### `omni doctor`

Diagnose installation health.

```bash
omni doctor
```

**Checks:**
- Binary version
- Config directory (`~/.omni/`)
- SQLite database accessibility
- FTS5 support
- Claude Code hook installation
- MCP server registration
- Filter loading (built-in, user, project)
- RewindStore status
- Recent activity timestamps

---

### `omni version`

```bash
omni version    # Prints: omni 0.5.0
```

---

### `omni help`

```bash
omni help       # Show usage information
omni --help     # Same as above
omni -h         # Same as above
```

---

## Environment Variables

| Variable | Default | Description |
|---|---|---|
| `OMNI_SESSION_TTL` | `240` | Session timeout in minutes |
| `OMNI_FRESH` | unset | Set to `1` to force a fresh session |
| `OMNI_CONTINUE` | unset | Set to `1` to always continue last session |
| `OMNI_DB_PATH` | `~/.omni/omni.db` | Custom database path |

## Exit Codes

| Code | Meaning |
|---|---|
| `0` | Success |
| `1` | Error (unknown command, pipe mode empty stdin, etc.) |

Hooks **always** exit 0 — they never crash the host agent.
