# Install

## Get the binary

**macOS and Linux, via Homebrew:**

```sh
brew install fajarhide/tap/omni
```

**macOS, Linux, WSL:**

```sh
curl -fsSL omni.weekndlabs.com/install | bash
```

**Windows, PowerShell:**

```powershell
irm omni.weekndlabs.com/install.ps1 | iex
```

**From source**, which needs the toolchain pinned in `rust-toolchain.toml`:

```sh
git clone https://github.com/fajarhide/omni
cd omni
cargo build --release
```

## Wire it into your agent

```sh
omni init            # interactive menu
omni init --claude   # or --cursor, --codex, --gemini, and 11 more
omni init --all      # every host, and a .vscode/mcp.json in the current directory
```

`omni init` writes hooks and registers the MCP server. It is idempotent, so running it
again after an upgrade is the right move rather than a risk.

Every supported flag is in [init](../reference/cmd/init.md). Which hosts get what is
in [Supported agents](../reference/agents.md), and that page matters more than it
sounds: a host that cannot rewrite its own shell tool's output will not show the
agent distilled bytes however well the pipeline works.

## Verify

```sh
omni doctor
```

This is not optional ceremony. It checks the binary is on `PATH`, the database opens,
the hooks are actually installed where the host reads them, and the MCP server is
registered. `omni doctor --fix` repairs what it can.

**Codex CLI needs one extra step.** It runs only hooks it has been told to trust and
skips the rest silently. After `omni init --codex`, start `codex` once and approve
them under "Hooks need review". `omni doctor` will keep failing until you do.

## Confirm it is really running

`omni doctor` says the wiring is correct. This says the wiring is being used:

```sh
cat some-long-file.txt     # through your agent, not this shell
omni diff                  # raw against distilled, for the last command
omni stats
```

If `omni stats` shows rows and `omni diff` shows a difference, the hook is live.

A trap worth knowing now rather than later: the numbers in `omni stats` are split by
`agent_id`, and a row recorded under `terminal` is TTY output no model ever read.
When you are judging whether OMNI is earning its place, look at the rows for your
actual host.

## Upgrade

```sh
omni update      # Homebrew installs
brew upgrade omni
```

Re-run `omni init` afterwards if a release changes the hook contract. The changelog
says when that happens.

## Remove it

```sh
omni init --uninstall   # hooks and MCP registration for one host
omni reset --all        # every integration, and offers to wipe omni.db
```

`omni reset` without flags gives an interactive menu. Neither command touches your
shell configuration, because OMNI never wrote any.
