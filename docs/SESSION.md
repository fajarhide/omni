# Session Continuity

OMNI tracks your working context across tool invocations, compaction events, and even across sessions — giving your AI agent persistent memory about what you're working on.

## How It Works

OMNI maintains a `SessionState` in SQLite that tracks:

| State | What's Tracked | How It's Used |
|---|---|---|
| **Hot Files** | Files appearing in tool output (max 10) | Boost relevance scores for related output |
| **Last Commands** | Recent commands executed (max 5) | Infer task domain |
| **Active Errors** | Unresolved errors from builds/tests (max 3) | Boost error-related output |
| **Domain Hint** | Inferred working directory (e.g., `src/auth/`) | Context-aware scoring |
| **Task Hint** | Inferred task (e.g., "fixing rust tests") | Session summary injection |

## Session Lifecycle

```
omni --hook (SessionStart)
  │
  ├─ Load latest session from SQLite
  ├─ If session < TTL (4 hours): continue
  ├─ If session > TTL: start fresh
  ├─ Inject session context into Claude's system prompt
  │
  ▼
omni --hook (PostToolUse) — repeated
  │
  ├─ Distill tool output
  ├─ Track files, commands, errors in background thread
  ├─ Update SessionState in SQLite
  │
  ▼
omni --hook (PreCompact) — when Claude compacts
  │
  ├─ Save full session state
  ├─ Index checkpoint in FTS5
  ├─ Inject summary into compaction metadata
  │
  ▼
Session persists across compactions
```

## Configuration

### Session TTL

By default, sessions expire after **4 hours** of inactivity. Override with:

```bash
export OMNI_SESSION_TTL=480  # 8 hours (in minutes)
```

### Force Fresh Session

```bash
export OMNI_FRESH=1  # Start with a clean session
```

### Continue Existing Session

```bash
export OMNI_CONTINUE=1  # Always continue the last session
```

## Inspecting Sessions

```bash
# View current session
omni session

# View session with context boost details
omni session --inject

# List recent sessions
omni session --history

# Clear current session
omni session --clear
```

### Example Output

```
─────────────────────────────────────
 OMNI Session — active
─────────────────────────────────────
 ID:            a1b2c3d4-e5f6
 Started:       2 hours ago
 Last active:   10 minutes ago
 Task:          fixing auth module
 Domain:        src/auth/

 Context Boost:
   Hot files:   src/auth/mod.rs (12x), src/api/routes.rs (8x)
   Last cmds:   cargo test, git diff
   Errors:      error[E0432]: unresolved import

 Distillations: 47 total, 82% avg reduction
─────────────────────────────────────
```

## How Context Boost Works

When scoring output lines, OMNI applies a **context boost** (max +0.4) based on session state:

1. **Hot file match** (+0.2): If a line mentions a file you've been editing
2. **Active error match** (+0.3): If a line matches an unresolved error pattern
3. **Domain match** (+0.1): If a line relates to your inferred working domain

This means errors in files you're actively working on get **maximum priority** in the distilled output.

## Debugging Sessions

```bash
# Check if session is active
omni doctor

# View raw session state
omni session

# Check if hooks are installed
omni init --status

# Force session reset
omni session --clear
```

### Common Issues

**Session not persisting**: Check that `~/.omni/omni.db` exists and is writable. Run `omni doctor`.

**Context boost not working**: Session needs at least 2-3 tool invocations to build context. Check `omni session` for hot files.

**Session expired**: Default TTL is 4 hours. Set `OMNI_SESSION_TTL` for longer sessions.
