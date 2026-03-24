# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.2] - 2026-03-25

### Added
- Native support for `npm run`, `yarn run`, `pnpm run`, and `bun run` scripts in TOML filters.
- Support for `python -m pytest` and `python3 -m pytest` commands.
- Support for `bun test` and `bun run test` runners.

### Improved
- Context Safety: Enhanced preservation of multi-line test failure diffs (Vitest/Jest) by refining empty-line stripping rules.
- Accuracy: Improved token savings calculations in `omni stats` for more precise analytics.

### Fixed
- Clippy Compliance: Resolved all remaining `D warnings` including `implicit-saturating-sub` in distillation hooks.
- Filter coverage gaps: Fixed missing interceptions for common JS/Python test runner variants.

## [0.5.1] - 2026-03-24

### Added
- `omni reset` command: Safely backs up configs to `~/.omni.<ts>.bak` and removes agent integrations (MCP/Hooks).
- Automated Release Workflow: `make release` now handles version bumping, commits, and tagging in one command.

### Fixed
- `omni learn` stability: Resolved stdin "hanging" when run interactively and fixed TOML parsing errors.
- Noise Deduplication: `omni learn` now skips patterns already present in `learned.toml`.
- TOML Generation: Improved escaping for quotes and invisible ANSI control characters in generated filters.
- Project-scoped MCP Detection: `omni doctor` now correctly identifies and validates nested project keys in `~/.claude.json`.

### Improved
- Actionable Suggestions: `omni doctor` now provides direct CLI commands to fix identified issues.
- Latency Assertions: Added deterministic tests to verify sub-50ms distillation performance.
- Clippy Compliance: Resolved all nesting and code quality warnings across the codebase.

## [0.5.0] - 2026-03-23

### Changed — Breaking
- Full rewrite in Rust — zero Node.js, zero Zig runtime
- `omni monitor` renamed to `omni stats`
- Hook format changed — run `omni init --hook` to reinstall

### Added
- Session continuity via SessionStart + PreCompact hooks
- RewindStore: compressed content retrievable via `omni_retrieve(hash)`
- Session-aware distillation: hot files and active errors boost signal priority
- `omni doctor` — installation diagnostics
- `omni learn` — auto-generate TOML filters from passthrough output
- Rust edition 2024
- SQLite WAL mode + FTS5 for session search

### Fixed
- Never Drop: output never silently discarded (RewindStore replaces passthrough)
- Zero startup overhead: native binary vs Node.js V8 startup

## [0.4.5] - 2026-03-20

### Added
- **Codex CLI & OpenCode AI Integration**: Native support for top-tier AI agent platforms. Run `omni generate codex` or `omni generate opencode` to automatically register OMNI and inject specialized filter bundles for each ecosystem.
- **Extensive Polyglot Filters**: Introduced over 60+ new semantic filters covering:
  - **Node/TS**: npm, yarn, pnpm, bun, tsc, eslint, prettier, vitest, jest, cypress, playwright, next.js, vite, webpack, nx.
  - **Python**: pytest, ruff, mypy, black, isort, pip, poetry.
  - **Rust/Go/Zig**: cargo, rustfmt, clippy, go build/test, zig build/test.
  - **DevOps/Cloud**: docker, docker-compose, kubectl, terraform, terragrunt, helm, ansible, skaffold, argocd.
  - **Security**: semgrep, trivy, gitleaks, snyk, hadolint, kubesec.
  - **Mobile/Other**: flutter, react-native, android-build, composer, gradle, make.
- **Hook Integrity Verification**: Implemented SHA256-based verification for OMNI hook scripts with `omni_trust_hooks` command and automatic startup checks to prevent execution of untrusted and potentially malicious scripts.
- **Project Trust Boundary**: Secure local configuration loading via `omni_trust` command. Review and trust project-specific `omni_config.json` rules before they are applied.
- **Autonomous Discovery**: Experimental `omni_learn` tool (via Wasm `discover` export) to automatically identify and suggest filters for repetitive noise patterns.
- **Improved Filter Transparency**: Filter names are now exposed via WASM and logged in real-time in the TypeScript MCP server for better diagnostics and efficiency monitoring.
- **Test Suite Migration**: Migrated core and filter tests from JavaScript to TypeScript using Bun, adding 50+ new ecosystem fixtures for robust verification.

### Fixed
- **MCP Server Stability**: Isolated MCP server tests using temporary home directories to prevent interference with local user configurations.
- **Cat Filter Scoring**: Adjusted confidence scoring for structured markdown to assign lower confidence to short, single-line noise without headers.

### Changed
- **CLI References**: Extensively updated `docs/CLI_REFERENCE.md` and `README.md` to reflect the latest command capabilities and security features.
- **Streamlined Workflow**: Simplified the `CONTRIBUTING.md` pull request process to focus on automated `make verify` checks.

## [0.4.4] - 2026-03-19

### Added
- **Test Infrastructure**: Implemented a comprehensive test suite in the `tests/` directory covering core filters (Git, Docker, SQL, Node) and the MCP server gateway, supported by new test helpers and fixtures.
- **CI/CD Integration**: Fully wired the semantic verification suite (`test-semantic.mjs`) and unit tests into both the `Makefile` and GitHub Actions workflow for automated quality gating.

### Fixed
- **Shell Injection**: Switched to `execFileAsync` with array arguments for `omni_grep_search` and `omni_find_by_name` to prevent shell injection vulnerabilities.
- **Wasm Memory Leak**: Wrapped the Wasm engine compression logic in `try/finally` blocks to ensure allocated memory is always freed, even on errors.
- **SQL Parsing**: Refactored `sql.zig` to use line-based splitting (`std.mem.splitAny`) instead of space-based, and fixed a bug where `--` comments caused the entire distillation to break.
- **Docker False Positive**: Hardened `docker.zig` matching logic to require specific signals like `FROM `, `RUN `, or `COPY ` alongside `Step ` or `CACHED` indicators.
- **Dynamic Scoring**: Replaced hardcoded `1.0` scores in `git`, `docker`, `sql`, and `node` filters with dynamic signal-density calculations for better distillation accuracy.
- **MCP Exit Codes**: Modified `omni_execute` and its aliases to return the actual command exit code in the tool's response metadata for programmatic handling.

## [0.4.3] - 2026-03-19

### Changed
- **Version bump**: Synchronized version strings across all 9 manifest and source files.


## [0.4.2] - 2026-03-18

### Added
- **OMNI Design System**: New shared UI architecture (`ui.zig`) for perfectly aligned boxed layouts and high-resolution performance meters across all CLI subcommands.
- **Agent Autopilot Aliases**: Automatic interception of native agent tools (`Bash`, `run_command`, `ReadFile`, `view_file`) via MCP to ensure transparent token distillation.
- **Custom DSL Rules**: Activated and fully integrated custom token-reduction DSL rules in the main semantic engine.

### Fixed
- **DSL Engine Stability**: Fixed a critical `use-after-free` segmentation fault in the JSON config parser by explicitly allocating memory for config string slices.
- **Filter Precedence**: Ensured user-defined rules from `omni_config.json` correctly take priority over built-in internal core filters.
- **CLI Output Cleanliness**: Removed stray debug prints in the compressor pipeline.

## [0.4.1] - 2026-03-17

### Added
- **`omni examples`**: Display real-world study cases and examples.
- **Proxy Command (`--`)**: Proxy and distill output from other commands (e.g., `omni -- git log`).
- **Antigravity Filter**: Integrated filter for Google Antigravity AI agent.
- **MCP Tools**: Implemented file system exploration and declarative filtering tools.

## [0.4.0] - 2026-03-16

### Added
- **`omni update`**: Check for the latest release from GitHub and get smart update instructions (auto-detects Homebrew vs installer).
- **New Landing Page**: Introduced a redesigned OMNI landing page.
- **FUNDING**: Added `FUNDING.yml`.

### Fixed
- **Homebrew Upgrade Stability**: `omni setup` now uses stable `/opt/omni` paths instead of versioned `/Cellar/omni/X.X.X` paths, preventing broken symlinks after `brew upgrade`.
- **Self-referencing Symlink**: `omni setup` now skips symlinking when source and destination are the same path.
- **Dynamic Versioning**: `build.zig` now defaults to the current release version instead of "development" when `-Dversion` is not specified.

### Changed
- **Release script**: Now synchronizes **9 locations** (added `core/build.zig` default version).
- Simplified `.github/pull_request_template.md` to checklist-only format.

## [0.3.9] - 2026-03-16

### Added
- **`omni uninstall`**: Clean removal of `~/.omni` directory and automatic cleanup of MCP configs from Antigravity, Claude Code CLI, and Claude Desktop.
- **Custom DSL Rules**: Activated and fully integrated custom token-reduction DSL rules configurable via `omni_config.json`.
- **Semantic Confidence Scoring**: Dynamic compression strategies based on filter confidence.
- **Agent Autopilot**: Dedicated UI and documentation to guide AI agent integration.
- **AI PR Describer**: Added `.github/workflows/ai-pr-describer.yml` for automated pull request descriptions.

### Fixed
- **DSL Engine Stability**: Fixed a critical `use-after-free` segmentation fault in the JSON config parser by explicitly allocating memory for config string slices.
- **Filter Precedence**: Ensured user-defined rules from `omni_config.json` correctly take priority over built-in internal core filters.
- **CLI Output Cleanliness**: Removed stray debug prints in the compressor pipeline.

## [0.3.8] - 2026-03-16

### Fixed
- **Version Synchronization**: All 8 versioned files now fully synchronized (`package.json`, `package-lock.json`, `core/build.zig.zon`, `src/index.ts`, `src/index.js`, `scripts/omni-deploy-edge.sh`, `docs/index.html`, `omni.rb`).
- **Release Automation**: `omni-release.sh` updated to handle docs and deploy script versioning.

## [0.3.7] - 2026-03-16

### Added
- **Local Metrics System**: Every `omni distill` and MCP call now records usage to `~/.omni/metrics.csv`.
- **Expanded `omni report`**: Daily, Weekly, and Monthly breakdown tables with token savings (Cmds, Input, Output, Saved, Save%, Time).
- **Agent Filtering**: `omni report --agent=claude-code` to view per-agent metrics.
- **Agent Tagging**: `omni generate` now includes `--agent=<name>` in MCP config for automatic tracking.
- **PR Template**: Added `.github/pull_request_template.md`.

### Fixed
- **`omni setup` symlink**: Now searches 4 candidate paths for `index.js` and removes stale symlinks before creating new ones.
- **Installer (`install.sh`)**: Fixed color formatting (`%b`), version passing (`-Dversion`), and quoting issues.
- **Homebrew formula**: Replaced `post_install` with `caveats` to avoid sandbox issues with `$HOME`.

### Changed
- **Release script**: `omni-release.sh` now auto-bumps `build.zig.zon` and `package.json` versions.
- Removed `ARCHITECTURE.md` link from `CONTRIBUTING.md` and `docs/index.html`.

## [0.2.0] - 2026-03-15

### Added
- **Unified Native CLI**: Replaced shell scripts with high-performance native subcommands.
- Subcommands: `omni distill`, `omni density`, `omni report`, `omni bench`, `omni generate`, `omni setup`.
- **Agent Templates**: Support for generating Antigravity and Claude Code input templates.
- **Zig Build System**: Fully integrated `build.zig` for cross-platform native and Wasm builds.

### Changed
- Moved all legacy shell scripts to `scripts/legacy/`.
- Updated `install.sh` to use the native build pipeline.

## [0.1.3] - 2026-03-15

### Fixed
- Zig 0.15.2 IO API transition: Replaced removed `std.io.getStdOut/getStdIn` with `std.fs.File` equivalents.
- Native build failure on Homebrew environment.

## [0.1.2] - 2026-03-15

## [0.1.1] - 2026-03-15

## [0.1.0] - 2026-03-14

### Added
- Initial Zig core engine implementation.
- Basic Git and Build log filters.
- MCP Server gateway in TypeScript.
- Custom JSON-based rules for masking/removal.

---
*Follow the OMNI vision.*
