# OMNI Migration Guide: 0.4.x → 0.5.0

## What's New in 0.5.0

OMNI 0.5.0 is a **full rewrite in Rust** — replacing the previous Node.js + Zig hybrid architecture with a single static binary. This brings:

- **Zero dependencies** — no Node.js, no npm, no Zig runtime
- **50% smaller binary** — single ~4MB static binary
- **2x faster distillation** — native Rust performance
- **Built-in MCP server** — no separate MCP process needed
- **New CLI commands** — `stats`, `doctor`, `learn`

## Breaking Changes

### ⚠️ Hooks Must Be Reinstalled

The hook format has changed. Run:

```bash
omni init --hook
```

This will update `~/.claude/settings.json` with the new hook entries.
If you had custom hooks, they will be backed up to `settings.json.bak`.

### ⚠️ `omni monitor` → `omni stats`

The `monitor` command has been renamed to `stats` with improved output:

```bash
# Old (0.4.x)
omni monitor

# New (0.5.0)
omni stats           # Last 30 days (default)
omni stats --today   # Today only
omni stats --week    # Last 7 days
```

### ⚠️ MCP Server Registration

The MCP server is now built-in. Update your Claude config:

```json
{
  "mcpServers": {
    "omni": {
      "command": "omni",
      "args": ["--mcp"]
    }
  }
}
```

Or run `omni init --hook` which handles this automatically.

## What's Compatible (No Action Needed)

| Feature | Status |
|---|---|
| `omni_config.json` format | ✅ Fully compatible |
| `~/.omni/` directory | ✅ Same location |
| `omni.db` SQLite schema | ✅ Compatible (new tables added) |
| TOML filter files | ✅ Same format |
| `~/.omni/trusted.json` | ✅ Compatible |
| Pipe mode (`cmd \| omni`) | ✅ Works the same |

## Upgrade Steps

### Via Homebrew (Recommended)

```bash
brew update
brew upgrade omni
omni init --hook    # Reinstall hooks
omni doctor         # Verify everything
```

### Via Install Script

```bash
curl -fsSL https://raw.githubusercontent.com/fajarhide/omni/main/scripts/install.sh | sh
omni init --hook
omni doctor
```

### Manual

```bash
# Download for your platform
curl -LO https://github.com/fajarhide/omni/releases/download/v0.5.0/omni-v0.5.0-aarch64-apple-darwin.tar.gz
tar xzf omni-v0.5.0-aarch64-apple-darwin.tar.gz
mv omni ~/.local/bin/
omni init --hook
omni doctor
```

## Removing Old Installation

If you previously installed OMNI via npm:

```bash
npm uninstall -g omni    # Remove old npm package
rm -f ~/.omni/omni.wasm  # Remove old Zig binary
```

The `~/.omni/` directory and `omni.db` are safe to keep — they're compatible with 0.5.0.

## New Commands

| Command | Description |
|---|---|
| `omni doctor` | Diagnose installation (hooks, DB, filters) |
| `omni stats` | Token savings analytics dashboard |
| `omni learn` | Auto-generate TOML filters from passthrough output |
| `omni init --hook` | Setup/update Claude Code hooks |
| `omni --mcp` | Built-in MCP server mode |

## Troubleshooting

**"omni: command not found"** — Ensure `~/.local/bin` is in your `PATH`:
```bash
export PATH="$HOME/.local/bin:$PATH"
```

**Hooks not firing** — Run `omni doctor` to check hook installation status.

**Old data not showing** — Session data from 0.4.x is in the same DB. Run `omni stats` to verify.

**Need help?** — Run `omni doctor` for a full diagnostic report.
