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

> [!IMPORTANT]
> OMNI is designed for **Deliberate Action**. Core commands like `init`, `session`, and `learn` show help by default if no flags are provided. Always use a flag to perform an action or check status.

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
# Explicit flags are recommended even in pipes for clarity
git diff HEAD~3 | omni --dry-run   # If learning
cargo test 2>&1 | omni             # standard distillation
```

---

## Commands

### `omni init`

Setup OMNI hooks in Claude Code.

```bash
omni init --all        # Recommended: Full Setup (Hooks + MCP)
omni init --hook       # Setup Hooks only
omni init --mcp        # Setup MCP Server only
omni init --status     # Check installation status
omni init --uninstall  # Remove all OMNI components
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
omni session --status     # Show current session details (Hot files, etc.)
omni session --history    # List recent sessions
omni session --clear      # Clear current session
omni session --continue   # Continue a stale session
omni session --resume     # Resume an interrupted session
omni session --transcript # View transcript of recent session
```

---

### `omni learn`

Auto-generate TOML filters from passthrough output.

```bash
omni learn --status     # Discovery: Search for new noise patterns
omni learn --dry-run    # Preview: Show suggested TOML
omni learn --apply      # Action: Commit to learned.toml
omni learn --verify     # Test: Run inline tests on all filters
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
omni doctor             # Run diagnostics only
omni doctor --fix       # Diagnose AND auto-fix all issues
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

**Auto-Fix (`--fix`):**

When `--fix` is passed, OMNI will automatically resolve detected issues:

| Issue | Fix Applied |
|---|---|
| Missing `~/.omni/` directory | Creates the directory |
| Missing Claude Code hooks | Runs `omni init --hook` |
| Missing MCP server registration | Runs `omni init --mcp` |
| Untrusted project filters | Runs `omni trust` on the project |
| Invalid user filter files | Renames broken `.toml` to `.toml.bak` |

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
