# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.6-rc2] - 2026-04-12

### Added
- **OpenClaw Support**: Introduced a native integration plugin for the **OpenClaw** agent framework. Includes a dedicated `omni_shell` tool for distilled execution and an `omni_rewind` tool for full log retrieval directly within the OpenClaw agent loop.
- **Command Grouping & Aggregation**: Enhanced `omni stats` to group identical or structurally similar commands (e.g., variant file paths in `ls -la`) into unified entries. This provides a significantly cleaner and more actionable signal report for repetitive tasks.

### Improved
- **CLI Semantic Clarity**: Renamed the `omni learn` flag `--status` to `--discover` to better align with its role in noise pattern discovery and candidate generation.
- **Null-Safe Telemetry**: Enforced robust null-safety handling in the SQLite storage layer using `COALESCE` for metric summations, preventing potential aggregation errors in sparse data environments.
- **Release Automation**: Hardened `bump_version.sh` and `omni-release.sh` to automatically synchronize version strings across the core Rust engine and the new OpenClaw integration plugin.

### Fixed
- **Integration Test Stability**: Updated `tests/savings_assertions.rs` to handle the expanded 4-tuple telemetry format, ensuring full CI compliance for the new grouping logic.
- **CLI Type Safety**: Resolved a type mismatch in the `omni stats --detail` view that caused formatting failures when processing heavily grouped filter entries.

## [0.5.6-rc1] - 2026-04-12

### Added
- **Magic Pipe Detection V2**: Automatic command source discovery via PGID inspection and parent shell fallback. This eliminates the need for manual `OMNI_CMD` labeling for piped commands in both interactive and scripted environments.
- **Configurable Token Pricing**: Introduced support for custom pricing models in `~/.omni/config.toml`, enabling accurate cost tracking for various models (e.g., GPT-4o, Claude Haiku).
- **Soft Route**: Fully implemented the 'Soft' distillation route for more flexible semantic engine behavior.
- **CLI ROI Metrics**: Expanded `omni stats --json` payload to include `savings_pct` for deeper CI/CD monitoring integration.

### Improved
- **High-Performance Filter Cache**: Implemented `OnceLock` caching for built-in TOML filters, significantly reducing overhead during high-frequency hook execution.
- **Command-First Architecture**: Completed the migration to a simplified engine by removing legacy `Classifier` and `Composer` modules.
- **Refined Stats UX**: Updated `omni stats` to strip redundant `omni exec` prefixes from automatically detected manual pipes for cleaner reports.

### Fixed
- **Post-Tool Telemetry**: Fixed a logic error in `src/hooks/post_tool.rs` that caused `segments_kept` and `segments_dropped` to be recorded as zero.
- **Stats Dead Code**: Cleaned up the "Distill" dead code path in stats color mapping and resolved various Clippy lints across the codebase.
- **Test Stability**: Hardened fragile assertions in `pipe.rs` unit tests to allow for benign local environment warnings.

## [0.5.5] - 2026-04-08

### Added
- **Command-Aware Intelligence**: Implemented path-aware classification heuristics to accurately detect terminal commands (e.g., `git`, `docker`, `kubectl`, `npm`) even when invoked via absolute paths.
- **Historical Data Re-classification**: Integrated "Intelligence Upgrade" into `omni doctor --fix`, allowing users to calibrate legacy 'Unknown' records with the latest classification models.
- **Cloud & Infra Heuristics**: Added native classification support for `kubernetes`, `terraform`, `aws`, `gcloud`, `helm`, and `azure` CLI tools.

### Improved
- **Real-time Update Notifications**: Reduced update check cache from 24 hours to **4 hours** and integrated proactive alerts directly into the `omni stats` dashboard.
- **Statistics UX**: Simplified `Unknown` category labels in the main signal report for a cleaner, more professional analytics display.
- **Classification Performance**: Optimized command-base matching to ensure sub-millisecond overhead during toolchain execution.

### Fixed
- **Code Integrity**: Resolved rusqlite iterator usage issues and addressed various Clippy lints to ensure 100% CI pass rate.

## [0.5.4] - 2026-04-07

### Added
- **OMNI Filter Pack**: Migrated and enhanced 12 new TOML-based filters for modern tools (Playwright, Ruff, golangci-lint, .NET, Prisma, Bun, Cypress, Jest, mypy, Black, pnpm, PHPUnit).
- **Core Distillers**: Implemented 3 new engine Distillers: `CloudDistiller` (Docker, K8s, Terraform), `SystemOpsDistiller` (ls, env, grep, tree), and `JsTsDistiller` (ESLint, TSC).
- **Session-Aware Distillation**: Injected tracking state directly into the distillers to unlock intelligent toolchain-specific context filtering logic.
- **Enhanced Stats Engine**: Upgraded `omni stats` command to fully support multi-period views, breakdown by context type, and valid JSON payload export.
- **Output Collapse Pipeline**: Added an algorithmic pipeline stage to collapse redundant contiguous lines to improve semantic block identification.
- **Quiet Mode Execution**: Introduced `OMNI_QUIET` environment variable to surgically suppress all stderr processing metrics for shell scripts.

### Fixed
- **Silent Exit Control**: Fixed persistent stderr pollution by ensuring OMNI terminates silently on completely blank piped inputs.
- **Security Check Integrity**: Updated the guardrail layer to ensure all denylist environment variable queries are strictly case-insensitive.
- **Windows Compatibility**: Updated GitHub Actions CI to matrix Windows tests and correctly restrict updater imports on unsupported OS.

### Improved
- **Zero-Mutation Tests**: Eradicated `std::env::set_var` to stabilize parallel thread runner execution via dependency injection (fixing deep UB).
- **Zero-Allocation ANSI**: Refactored the `strip_ansi` memory strategy to leverage `Cow<str>`, eliminating allocations for clean text snippets.
- **Pipeline Architecture**: Modularized the monolithic pipeline into cleanly separated `Classify → Score → Compose → Distill → Deliver` abstractions.

## [0.5.4-rc5] - 2026-04-01

### Added
- **Transcript Persistence**: Implemented robust session transcript persistence (`src/store/transcript.rs`) ensuring state is saved atomically to disk to prevent work loss.
- **Pre-Compact Double-Guardrail**: Injected `CRITICAL` at the start and `REMINDER` at the end of the `PreCompact` hook snapshot to drastically improve instruction adherence for Sonnet 4.6+ models.
- **Session Telemetry & ROI**: Enhanced `SessionState` to auto-calculate estimated tokens saved and identify the top data-reducing command purely in-memory (<5ms).
- **Session CLI**: Added new `omni session` commands for resuming and inspecting session transcripts.

### Fixed
- **Dead Code Cleanup**: Activated unused path mapping functions (`src/paths.rs`) and cleared various compiler warnings by completely wiring up the core pipeline.
- **Formatting & Linting**: Cleaned up the repository, removed obsolete GitHub PR templates, and integrated robust error checks for session boundaries.

## [0.5.4-rc4] - 2026-03-25

### Added
- **`omni doctor --fix`**: New `--fix` flag to automatically resolve integration issues — creates missing config directory, reinstalls hooks, registers MCP server, trusts project filters, and renames invalid user filter files to `.bak`.

### Fixed
- **Example Filter Template**: Rewrote `filters/00_example.toml` from legacy `[[filters]]` array-of-tables format to the standard `[filters.name]` schema, eliminating the embedded filter parse error at startup.
- **Stats Column Overflow**: Truncated the "Command" column in `omni stats` to a maximum of 21 characters with `...` ellipsis to prevent table layout breakage from long command names.

### Improved
- **Clippy Compliance**: Collapsed nested `if` statements in `doctor.rs` to satisfy `clippy::collapsible_if` lint.
- **Code Formatting**: Applied `cargo fmt` across all modified files for consistent style.

## [0.5.4-rc3] - 2026-03-25

### Added
- **Signal Comparison Mode**: Introduced `omni diff` command for side-by-side visualization of raw input vs. distilled output with "density gain" metrics.
- **Rewind Management**: Added `omni rewind list` and `omni rewind show <hash>` for local exploration of the RewindStore archive.
- **Real-time ROI Indicator**: New `[OMNI Active]` terminal status line providing immediate feedback on token reduction and latency.
- **Marketing Data Seeding**: New `scripts/seed_marketing.py` for generating high-impact, realistic demonstration data.

### Improved
- **Analytics UI**: Refined `omni stats` with professional English headers, better alignment, and improved financial impact estimation.
- **Log Classification**: Enhanced `RE_LOG_SEV` to recognize common bracket-less severity formats (e.g., `DEBUG:`).
- **Aesthetics**: Updated distillation and retrieval notices with rich ANSI colors and detailed impact summaries.

## [0.5.4-rc2] - 2026-03-25

### Improved
- **Version Awareness**: `omni doctor` and `omni update` now explicitly distinguish between `[LATEST]`, `[UPDATE]`, and `[AHEAD/RC]` statuses.
- **Diagnostic Precision**: Updated `omni doctor` to provide more accurate version status for users on pre-release or development branches.

### Fixed
- **Version Checker**: Corrected semantic comparison in `is_newer` to properly handle pre-release suffixes (e.g., `0.5.4-rc1` is now recognized as newer than `0.5.3`).
- **Release Script**: Updated `bump_version.sh` to support Semantic Versioning with pre-release tags (e.g., `-rc1`).

## [0.5.4-rc1] - 2026-03-25

### Added
- **Filter Priority System**: Introduced alphabetical sorting for built-in filters (e.g., `00_vitest.toml` vs `npm.toml`) to ensure specialized matches take precedence.
- **Enhanced `omni exec`**:
    - Intelligent Shell Detection: Automatically detects and runs commands with pipes, redirects, or semicolons via `sh -c`.
    - Real-time Distillation: Native command output is now seamlessly piped through OMNI's semantic engine.
    - Exit Code Passthrough: Native exit codes are now correctly preserved and returned to the caller.
- **Deep Terraform Support**: Expanded Terraform filters with over 40+ new specialized rules for cleaner infrastructure distillation.

### Improved
- **Filter Precision**: Refactored Vitest and Kubectl filters for higher signal-to-noise ratios.
- **Session Tracking**: Enhanced stability in session state persistence and rule application.

### Fixed
- **Hook Reliability**: Resolved edge cases in `PreToolUse` hook handling for more consistent distillation.

## [0.5.3] - 2026-03-25

### Added
- `omni update` command: Easily upgrade OMNI to the latest version via Homebrew with a confirmation prompt.
- Automated Version Check: OMNI now checks for updates from GitHub (24h cached) and notifies you in `help` and `doctor` screens.
- Safety Confirmations: Added `[y/N]` interactive prompts to `omni reset` and `omni update` to prevent accidental uninstalls or upgrades.
- Full Hook Diagnostics: `omni doctor` now explicitly checks and displays status for all 4 OMNI hooks, including `PreToolUse`.

### Fixed
- Hook Cleanup: `omni reset` and `omni init --uninstall` now correctly remove `PreToolUse` (Bash) hooks from Claude settings.
- Hook Detection: Fixed `omni doctor` logic to correctly identify OMNI hooks using any valid flag variant.
- Clippy Compliance: Resolved `collapsible-if` and other minor lints in the new update module.

### Improved
- CLI Diagnostics: Refined `omni doctor` output with clearer labels ("OMNI Hooks", "OMNI MCP Server") for better readability.

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
