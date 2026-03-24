# Installing OMNI

## Quick Install (Recommended)

### Via Homebrew

```bash
brew install fajarhide/tap/omni
```

### Via Install Script

```bash
curl -fsSL https://omni.weekndlabs.com/install | sh
```

### From Source

```bash
git clone https://github.com/fajarhide/omni.git
cd omni
cargo build --release
cp target/release/omni ~/.local/bin/
```

## Setup

After installing, setup Claude Code hooks:

```bash
omni init --hook    # Install PostToolUse/SessionStart/PreCompact hooks
omni doctor         # Verify everything is working
```

## System Requirements

- **macOS** (arm64, x86_64) or **Linux** (arm64, x86_64)
- No runtime dependencies — OMNI is a single static binary
- ~4MB disk space

## Verify Installation

```bash
omni version    # Should print: omni 0.5.0
omni doctor     # Full diagnostic check
omni stats      # View token savings after your first session
```

## Upgrading

```bash
# Homebrew
brew upgrade omni
omni init --hook    # Reinstall hooks after upgrade

# Manual
curl -fsSL https://raw.githubusercontent.com/fajarhide/omni/main/scripts/install.sh | sh
omni init --hook
```

See [MIGRATION.md](MIGRATION.md) for detailed upgrade instructions from 0.4.x.

## Uninstalling

```bash
# Homebrew
brew uninstall omni

# Manual
rm ~/.local/bin/omni
rm -rf ~/.omni/              # Remove config and database
omni init --uninstall        # Remove hooks (run before deleting binary)
```
