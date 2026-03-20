# OMNI CLI Reference

OMNI helps you clean up noisy terminal output so it becomes shorter, clearer, and easier for both humans and AI agents to use.

If you are new to OMNI, start with these four commands:
- `omni -- <command>` to run a normal command through OMNI
- `omni generate <platform>` to connect OMNI to Claude Code, Codex, OpenCode, or Antigravity
- `omni doctor` to check whether your setup is healthy
- `omni doctor --fix` to repair missing or broken setup automatically

## Quick Start

### 1. Connect OMNI to your AI tool
Pick the one you use:

```bash
omni generate claude-code
omni generate codex
omni generate opencode
omni generate antigravity
```

These commands usually:
- register OMNI as an MCP server for the selected platform
- add the right global filters to `~/.omni/omni_config.json`
- print verification steps after setup

### 2. Check your setup

```bash
omni doctor
```

If something is missing or broken:

```bash
omni doctor --fix
```

### 3. Use OMNI in daily work

```bash
omni -- git diff
omni -- npm install
omni -- docker build .
```

If you already have raw output:

```bash
cat raw.log | omni distill
```

## Daily Commands

### `omni -- <command>`
This is the easiest way to use OMNI.

Use it when you want to run a normal shell command, but want OMNI to summarize the output first.

Examples:

```bash
omni -- git status
omni -- git diff
omni -- npm install
omni -- pnpm install
omni -- docker build .
```

Good for:
- long build logs
- test runner output
- package manager installs
- noisy output from `git`, `docker`, `terraform`, `kubectl`, and similar CLI tools

### `omni distill`
This is the manual version of OMNI.

Use it when you already have text or a log file and only want to clean up the output.

Examples:

```bash
cat build.log | omni distill
cat pytest-output.txt | omni distill
```

### `omni density`
Shows how much context OMNI saves.

Examples:

```bash
omni density < large_file.json
cat build.log | omni density
```

Use it to:
- check whether a large output is worth distilling
- measure how much noise OMNI removes

### `omni doctor`
Checks whether your OMNI installation and agent integrations are healthy.

It checks:
- the OMNI MCP entrypoint
- the global config at `~/.omni/omni_config.json`
- Claude Code integration
- Codex integration
- OpenCode integration
- Antigravity integration

Example:

```bash
omni doctor
```

### `omni doctor --fix`
Tries to repair missing or broken setup automatically.

Example:

```bash
omni doctor --fix
```

It usually:
- creates or repairs the global OMNI config
- re-registers missing MCP integrations

## Monitoring And Analysis

### `omni monitor`
Shows a summary of OMNI usage and context savings.

Example:

```bash
omni monitor
```

It typically shows:
- how many commands were processed
- input size vs output size
- savings percentage
- which filters were used most often
- usage grouped by agent

### `omni monitor --trend`
Shows savings trends over time.

```bash
omni monitor --trend
```

### `omni monitor --log`
Shows recent distillation history.

```bash
omni monitor --log
```

### `omni monitor --by day`
Shows a daily breakdown.

```bash
omni monitor --by day
```

Similar options:
- `omni monitor --by week`
- `omni monitor --by month`

### `omni monitor --json`
Prints monitor output as JSON for scripts or observability tools.

```bash
omni monitor --json
```

### `omni monitor scan`
Scans your shell history to find commands that should probably be using OMNI but are not yet.

```bash
omni monitor scan
```

This is useful when you want to find:
- commands that waste the most tokens or context
- habits you should move to `omni --`

## Agent Setup And Integration

### `omni generate claude-code`
Registers OMNI for Claude Code or Claude CLI.

```bash
omni generate claude-code
claude mcp list
```

This also:
- adds the polyglot coding filter bundle to the global config
- works well for JS, TS, Python, Rust, Go, Zig, and pnpm workflows

### `omni generate codex`
Registers OMNI for Codex CLI.

```bash
omni generate codex
codex mcp list
```

This also:
- keeps the `omni` MCP entry idempotent instead of duplicating it
- updates the entry when the path or arguments change
- adds the `codex-polyglot` bundle to the global OMNI config

### `omni generate opencode`
Registers OMNI for OpenCode AI.

```bash
omni generate opencode
opencode mcp list
```

This also:
- writes OpenCode MCP config using the correct schema
- adds a full coding filter set for OpenCode workflows to the global OMNI config

### `omni generate antigravity`
Registers OMNI for Google Antigravity.

```bash
omni generate antigravity
```

This also:
- patches `~/.gemini/antigravity/mcp_config.json` without removing other servers
- adds cloud-native filters for Kubernetes, Terraform, and Docker

### `omni generate config`
Prints a starter `omni_config.json` template.

```bash
omni generate config > omni_config.json
```

Use this when you want to:
- create a new config from scratch
- add project-specific rules or filters

### `omni setup`
Shows a guided setup flow.

```bash
omni setup
```

This is useful for new users who want to see:
- how OMNI connects to agents
- how daily usage works
- which commands matter most

## Filters And Auto-Learning

### `omni learn`
Looks for repeated noise patterns in raw output and suggests filters automatically.

Examples:

```bash
docker build . | omni learn
npm install 2>&1 | omni learn
```

### `omni learn --dry-run`
Shows possible filters without writing anything to config.

```bash
npm install 2>&1 | omni learn --dry-run
```

### `omni learn --config=<path>`
Writes learned filters to a specific config file.

```bash
kubectl get pods | omni learn --config=~/.omni/omni_config.json
```

## Extra Commands

### `omni bench [iterations]`
Runs a quick benchmark for the OMNI engine.

```bash
omni bench 1000
```

Use it when you want to test:
- engine throughput
- OMNI overhead in a synthetic scenario

### `omni update`
Checks for the latest version.

```bash
omni update
```

### `omni uninstall`
Removes OMNI and related configuration.

```bash
omni uninstall
```

### `omni examples`
Shows example usage.

```bash
omni examples
```

## MCP Tools Used By Agents

This section is usually for AI agents through MCP, not for direct human typing. It is still useful so you understand what OMNI can do inside an agent workflow.

### `omni_execute`
Runs a shell command and distills the output.

Common agent use:
- `git diff`
- `npm install`
- `docker build .`

### `omni_read_file`
Reads a file and distills its contents.

Useful for:
- large logs
- SQL files
- long text files

### `omni_view_file`
Reads a file by line range and reduces noise.

### `omni_list_dir`
Shows directory contents in a compact, token-efficient format.

### `omni_grep_search`
Searches text across files and folders with condensed output.

### `omni_find_by_name`
Finds files by name.

### `omni_add_filter`
Adds a simple rule to config without editing files manually.

### `omni_apply_template`
Adds a ready-made filter bundle to the active config.

Useful templates:
- `codex-advanced`: summaries for `tsc`, `eslint`, `jest`, and `vitest`
- `codex-polyglot`: broader coding bundle across multiple languages
- `opencode-advanced`: larger bundle for many coding tools
- `pytest-advanced`
- `ruff-advanced`
- `cargo-test-advanced`
- `pnpm-advanced`
- `zig-advanced`
- `go-test-advanced`
- `kubernetes`
- `terraform`
- `docker-layers`
- `security-audit`
- `aws-cloud`

### `omni_trust`
Marks a local `omni_config.json` file as trusted.

This matters because OMNI does not automatically use project-local config until you trust it.

### `omni_trust_hooks`
Verifies OMNI hook scripts in `~/.omni/hooks`.

### `node dist/index.js --test-integrity`
Integrity check mode for OMNI hook handling in the MCP server.

## Important Files

These paths are often useful:
- `~/.omni/omni_config.json`: global OMNI config
- `./omni_config.json`: project-local config
- `~/.omni/dist/index.js`: OMNI MCP entrypoint
- `~/.gemini/antigravity/mcp_config.json`: Antigravity integration
- `~/.config/opencode/opencode.json`: OpenCode integration
- `~/.codex/config.toml`: Codex integration
- `~/.claude.json`: Claude Code integration

## Practical Recommendation

If you are a new user, this is the safest order:
1. Run `omni generate <platform>` for the agent you use.
2. Run `omni doctor`.
3. Start using `omni -- <command>` for commands that produce long output.
4. If your project has a local config, run `omni_trust`.
5. Use `omni monitor` once in a while to confirm that OMNI is actually saving context.
