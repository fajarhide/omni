# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).


## [Unreleased]

### Fixed
- **A failed command could be distilled into output that reads as success (#120)**: OMNI's `normalize` layer parsed each agent's failure signal and then threw it away — Codex `exit_code` was never read into `CodexInput`, Pi `toolResponse.isError` sat behind `#[allow(dead_code)]`, and MCP `result.isError` was named in a comment but never deserialized — so a command that exited non-zero still ran the full distiller. A failed `docker build` (`exit_code 1`) on the heavy-noise fixture came out **9,207 → 6,090 bytes**, silently trimmed by the same `DEBUG`/`INFO` stripping a *successful* build gets; the filed case was a `vault` call that failed `exit=2` on a network timeout yet surfaced a clean, fictional `["n8n"]`. This is the worst failure mode — a fabricated success terminates investigation, while a fabricated error only costs a retry. `NormalizedInput` now carries `failed`, set from each agent's own signal, and `post_tool::process_payload` passes a failed command through verbatim at zero marker cost before any distiller runs. Successful commands are untouched — the same output with `exit_code 0` still distils to 6,090 bytes. Claude Code needed no code change: it already sends a failed command as a bare `tool_response` string (`"Error: Exit code N…"`) that never parses into a success summary, and a regression test locks that in so a future, more-lenient parser cannot silently reintroduce the fabrication. The `omni exec` / `pipe.rs` path reads piped stdout only and never sees the child exit code; hardening it is tracked separately.

## [0.6.2] - 2026-07-17

### Fixed
- **Format-safe compression (data integrity)**: The pipeline had no format awareness, so collapse squashed the repeated lines of structured output into `[N similar lines collapsed]` and left the payload unparseable — breaking any downstream `jq` / `json.load` / `kubectl apply`. A JSON dashboard piped through the hook came out with 14 collapse markers and failed `jq` outright. A new format sniffer (`pipeline::format`) now gates both choke points (`hooks::post_tool`, `hooks::pipe`): JSON, NDJSON, YAML, TSV, and CSV pass through byte-for-byte at zero marker cost, while plain text still compresses as before.
- **YAML with an embedded block scalar was not detected as YAML**: `sniff_yaml` judged every line of a document that lacked a leading `---`, so a ConfigMap carrying Vault HCL (`config.hcl: |`) made a 608-line `kubectl kustomize` manifest look like free text — 26 of its 608 lines were HCL, not YAML. The format gate stood down, collapse emitted `[8 similar lines collapsed]` markers, and `is_docker_logs` counted those markers as timestamp prefixes (12 triggers, needs 5), handing the whole manifest to `distill_docker_logs`: **13,463 bytes of Kubernetes config came out as `docker logs: 323 lines, no errors detected`**. OMNI's own collapse markers fabricated the evidence the next stage misread. A block scalar's body is now skipped rather than judged — the `|` that opened it is YAML's own signal that what follows is a value, so detection stays positive.
- **`find` deleted its payload and called it a saving**: the `SystemOpsDistiller` behaviour shipped in 0.6.1 ("strips out raw file path samples entirely", credited below with "up to 99.8% savings") reported ~99% by discarding the file paths that *were* the answer to the question. Replaced with lossless prefix factoring: the shared directory prefix is hoisted to a header and each path stated once relative to it, cut at a separator so every path round-trips byte-for-byte. Real saving on the same output: ~72%, with nothing lost. `grep` now hoists each path once instead of repeating it per match (26–39%, also lossless), and both hand back the input untouched when factoring would grow it.
- **A weak TOML filter shadowed the test distiller**: `sys_build_domain` matches every `cargo` invocation but strips only `Compiling`-style lines. It won the `find()` race, shadowed `TestDistiller`, cut 1%, and had that cut discarded by `best_output()`'s `MIN_REDUCTION_PCT` guardrail — so `cargo test` reported **0%** on output the distiller reduces by 94%. A TOML filter now only short-circuits the distiller if it actually beat the guardrail; a filter that earns its match still wins, user filters included.
- **`cargo test` totals were recounted instead of quoted**: with the distiller reachable again, it counted result-line segments and reported `1 passed` for a run cargo itself called `490 passed` — `CollapseMode::Test` folds the 330 `... ok` lines away before the distiller ever sees them. It now quotes the runner's own summary line (cargo, pytest, jest), falling back to counting only when no summary was printed.
- **No 0.6.2 binaries could be built at all**: `release.yml` asks `dtolnay/rust-toolchain@stable` for each matrix target, which installs that std into *stable* — but `rust-toolchain.toml` (added this release) makes cargo switch to the pinned 1.97.0, which has std for no target but the runner's own. Every cross-compile died with `error[E0463]: can't find crate for 'core'` before compiling a line of OMNI, and fail-fast took the rest of the matrix with it, so the tag produced no release. The matrix targets are now listed in `rust-toolchain.toml` itself, installing them into the toolchain cargo actually uses. `ci.yml` stayed green throughout — it only builds host-native, where std is already present — so this was invisible until a tag was pushed.

### Removed
- **`est_cost_usd` and `ModelPricing`**: OMNI is a hook and never sees the API's `usage` block, so it cannot know whether bytes were billed as fresh input or as a ~10× cheaper prompt-cache read. `ModelPricing` used only `.input`, and its `_cached` / `_cache_creation` fields were dead — every dollar figure OMNI printed was a fresh-input guess presented as a session cost. The formula was also duplicated across four call sites. Removed from `stats`, `diff`, and `guard::config`; unknown keys in an existing `config.toml` still parse.

### Changed
- **Published numbers replaced with measured ones**: the headline claims were not stale, they were untrue. `docs/PERFOMANCE.md` sold `docker build` as 9.2 KB → **49 bytes (99.5%)**; the fixture it names distils to 5,783 bytes (**37.2%**). `git diff` quoted **50.0%** on bytes (397 → 220) that compute to **44.6%**. The **97.3%** all-time figure traced to `find . (-12.6M tokens)` — find deleting its payload, the defect fixed above — so the number was manufactured by a bug. `$35+ USD/month` priced the estimator removed above, `~40% faster TTFT` was never measured, and three testimonials in quote marks were attributed to nobody because nobody said them. Replaced with a replay of **1,810 real execution traces** on the release binary with a fresh `HOME` per invocation: **58.9% net** (15.0 MB → 6.2 MB), `git` 91.3%, `cargo` 96.8%, `cat` 9.1%. Fixing the YAML sniffer moved reported savings **down**, 65.3% → 58.9%, because six of those points were destroyed manifests rather than removed noise. All six i18n READMEs carry the same numbers.
- **Two costs now documented rather than hidden**: OMNI's output is **not deterministic** against a warm `~/.omni/omni.db` — session history feeds the scorer, so the same binary on the same input differed on 21 of 30 traces run-to-run (one gave 1,835 bytes, then 433); any A/B measurement must isolate state per invocation. And latency is real: ~82 ms for a 496-byte `git status` against a fresh database, **~308 ms** against a 97 MB one, per hooked command, growing with history.

## [0.6.1] - 2026-06-24

### Added
- **Pain-First Positioning**: Completely refactored `README.md` to focus on the core narrative solving Terminal Noise and Context Amnesia ("Noise-canceling headphones for your AI agent").
- **Brand Evolution**: Redesigned the primary `logo.svg` with a sleek, neon-style "Cute Brain" wearing headphones, visualizing the integration of Noise-Canceling and the Adaptive Memory OS.

### Improved
- **AI-Native Distillation**: Revamped the `SystemOpsDistiller` for the `find` command. It now strips out raw file path samples entirely and emits a highly compressed, AI-friendly key-value summary of directory distributions and extension counts, maximizing token reduction (up to 99.8% savings).
- **Terminal UI Polish**: Fixed an alignment issue where the `[OMNI Active]` status tag would overlap with command outputs by ensuring trailing newlines in distilled payloads.

### Fixed
- **Clippy Strictness**: Resolved a `clippy::needless_splitn` warning in the directory classification logic, ensuring `make ci` continues to pass with zero warnings.
- **Snapshot Integrity**: Synchronized all `insta` snapshot tests to align with the new, denser `find` output format.

## [0.6.0] - 2026-06-14

### Added
- **Autonomous Loop Engineering**: Native support for iterative, autonomous agent loops (`OMNI_LOOP_BUDGET`, `OMNI_LOOP_GOAL`) with predictive goal-driven constraints.
- **Maker-Checker Verification Pattern**: Introduced `omni_verify` MCP tool to separate execution and validation securely across distinct agent sessions.
- **Test Suite Modernization**: Renamed all generic sprint-based test files to context-aware descriptive names (`session_state_tests.rs`, `security_validation_tests.rs`, etc.) and achieved 100% test coverage across 941 tests.
- **Production Hardening**: Added robust input sanitization, loop context injection prevention, and performance benchmark tests (latency thresholds adapted for CI resilience).
- **Consolidated Documentation**: Unified scattered templates into a cohesive `docs/LOOP_ENGINEERING.md` master guide and updated all global `i18n` READMEs to reflect new multi-agent loop orchestration capabilities.

### Fixed
- **Critical UTF-8 Panic Resolution**: Completely resolved `SIGABRT` crashes caused by multibyte characters (emojis, box-drawing, CJK) in terminal output. Implemented `char`-boundary safe string truncation and slicing utilities globally.
- **Pipeline Data Integrity**: Rewrote ANSI stripping and structural normalization engines to process by Unicode `char` rather than raw `byte`, preventing *mojibake* and data corruption on rich terminal outputs.
- **Release Stability**: Removed `panic = "abort"` from the release profile to allow `catch_unwind` guards to gracefully handle unexpected panics in production builds.

### Performance
- **Zero-Allocation ANSI Stripping**: Redesigned `strip_ansi` to use `Cow<'_, str>`, eliminating heap allocations entirely when processing clean terminal outputs.
- **Pattern Caching**: Implemented a thread-local LRU cache for `normalize_structural`, bypassing expensive regex and grapheme-cluster calculations on highly repetitive log lines.
- **Stream-Aware Output Limits**: Deployed `TruncatingWriter` in the streaming pipeline. Omni now tracks payload bytes on the fly and intelligently truncates multi-gigabyte outputs at valid UTF-8 boundaries, eliminating OOM vulnerabilities on massive terminal bursts.

### Changed
- **Column-Aware Truncation**: Refactored CLI formatting utilities (`diff`, `learn`, `stats`) to use `unicode-width` logic, ensuring terminal tables and summaries render perfectly aligned even when displaying full-width CJK characters or emojis.

## [0.5.9] - 2026-06-04

### Added
- **Engram (Automatic Subtask Digest)**: Rule-based state snapshots capturing subtask progress (e.g., error resolved, commits, test passes) to prevent context amnesia during long sessions.
- **Session Health Dashboard**: Introduced `omni session --health` to visualize context pressure, token savings, engrams, tool activity, and hot files.
- **Smart PreCompact v2**: Intelligent, priority-aware context packing (Errors > Engrams > Tool Summary > Hot Files) with SHA-256 delta detection to skip redundant injections.
- **Session Handoff & Portability**: Added `omni_handoff` MCP tool to export session state as portable markdown, enabling seamless context transfer between terminal sessions.
- **Rolling Tool Call Summary**: Aggregates the last 50 tool calls with success/error rates, exposed via the `omni_session("summary")` tool for agent reference when context pressure is high.
- **Periodic Context Re-injection**: Automatically re-injects critical pinned files (like `AGENTS.md`) into the agent's context when pressure is elevated and after a set interval.

## [0.5.8] - 2026-06-03

### Added
- **Streaming Distillation Pipeline**: Introduced memory-efficient, line-by-line processing (`src/hooks/pipe.rs`) to handle long-running and piped commands without memory exhaustion.
- **Expanded Semantic Distillers**: Added new declarative TOML signal profiles for 9 developer tools: `aws`, `az`, `bun`, `deno`, `docker`, `gcloud`, `npm`, `vite`, and `webpack`.
- **Brand & Documentation Modernization**: Completely overhauled the English `README.md` and all 6 `i18n` localized versions with professional SVG visual assets (`hero.svg`, `architecture.svg`), new "Under the Hood", and "Real-World Use Cases" sections.
- **Advanced Context Analytics**: Implemented context composition metrics in `omni stats` (`src/analytics/context_composition.rs`) to provide deeper visibility into token reduction and signal density.

### Improved
- **SQLite Storage Enhancements**: Updated `src/store/sqlite.rs` to seamlessly support high-throughput, chunked streaming outputs from the new distillation pipeline.
- **MCP & Tooling Synchronization**: Refined `pre_tool` and `post_tool` hook routing to align with the new semantic models.

## [0.5.8-rc3] - 2026-05-29

### Added
- **Context Pressure Management**: Implemented multi-stage context pressure warnings (Normal, Warning, Critical) to proactively manage session token budgets.
- **Critical File Pinning**: Added automatic context pinning for critical rule files (e.g., `.cursorrules`, `AGENTS.md`) during session compaction.
- **File Re-read Guard**: Introduced preventive warnings and hot-file mutation protection to stop redundant reads of files already in context.
- **Performance Documentation**: Added a comprehensive `docs/PERFOMANCE.md` showcase and updated all global `README.md` (and `i18n` translations) with actual ROI and noise-reduction benchmarks.
- **AGENTS.md**: Established `AGENTS.md` to define multi-agent coordination rules, development gates, and context lifecycle management protocols.

### Changed
- **Pipeline Fail-Open Architecture**: Reinforced pipeline hooks (`pre_tool`, `post_tool`, `pre_compact`, `session_start`) with strict fail-open logic to ensure zero disruption to agent operations.
- **Dependency Update**: Validated and integrated `rmcp` 1.7.0 updates using structured `Parameters<T>` and `JsonSchema` across the MCP integration.
- **CLI Formatting**: Minor styling improvements to `omni stats` terminal output.

## [0.5.8-rc2] - 2026-05-28

### Added
- **Pi Agent Integration**: Added first-class support for Pi Agent integration with init, reset, and doctor support, including hooks, extension, and toggle functionality.
- **VS Code MCP Initialization**: Introduced the `--vscode` flag to the `omni init` command for automatic VS Code MCP server configuration.
- **Enhanced Token Metrics**: The `omni stats --detail` pipeline now accurately tracks and displays raw vs filtered token counts in a dedicated "Tokens Reduced" column, providing precise visibility into token savings.

### Changed
- **Semantic Classification Engine**: Refactored the core pipeline filtering system to utilize a semantic classification engine for segments with tool-aware scoring logic.
- **Filter Configuration Layout**: Migrated filter definitions from the legacy `filters/` directory to structured `signals/tools/` and `signals/domains/` configurations.
- **MCP Framework Upgrade**: Upgraded `rmcp` to `1.7.0`, migrating all MCP tool handlers to strongly-typed `Parameters<T>` structs and `JsonSchema` for robust, type-safe request routing.

### Fixed
- **Claude Code Async Hooks**: Ensured Claude emits an empty matcher for async hook entries to prevent stalling.
- **Hermes Integration**: Fixed the detection logic for packaged Hermes OMNI plugins during `doctor` and initialization checks.

## [0.5.8-rc1] - 2026-05-08

### Added
- **Semantic Session Guardrails**:
    - **Hot File Detection**: Triggers `SessionEnd` hook when active files in a session show abnormal mutation frequency, prompting agents to review before committing.
    - **Build Failure Preservation**: `BuildDistiller` now preserves `CommandOutput` for build failures (non-zero exit codes) in the collapse pipeline, ensuring agents see exact errors instead of just exit status.
    - **Diagnostic Context**: `PreBuild` hook runs `cargo check` to surface compiler errors early in the session, reducing wasted tokens on broken states.
- **Passthrough & Thresholding**:
    - **OMNI_PASSTHROUGH**: Support for raw output emission via environment variable for manual debugging.
    - **Smart Bypass**: Automatic distillation bypass for small configuration files and content under a 2000-token minimum threshold.
    - **Extension Hinting**: Improved content-aware heuristics for more accurate token estimation.
- **Omission Transparency**: Added explicit `[OMNI: omitted X lines of noise]` markers in the `GenericDistiller` for improved agent situational awareness.

### Improved
- **Performance & Caching**:
    - **Filter Fingerprinting**: New caching system to reduce redundant TOML filter loading.
    - **Thread-Safe Loading**: Optimized filter registry access using Mutex and fingerprint-based verification.
- **Agent Attribution & Stats**: 
    - Standardized "terminal" as the default agent identifier for untagged sessions.
    - Improved agent distribution grouping and filtering in `omni stats` output.
    - Updated CLI visuals to use `bright_black` for secondary log signals to improve visibility.
- **Test Suite Modernization**: Systematically refactored the entire Rust test suite (~300 tests) to align with modern idiomatic standards.
    - **Naming Convention**: Dropped the redundant `test_` prefix inside `#[cfg(test)]` modules.
    - **Action-Oriented Naming**: Transitioned to behavioral function names (e.g., `returns_*`, `preserves_*`, `rejects_*`) to improve readability and maintainability.
    - **Language Standardization**: Purged all remaining Indonesian terminology and mixed-language test names, ensuring a 100% professional English testing layer.
- **CI Stability**: Verified full CI compliance after the bulk refactor, ensuring all 282 tests remain stable across platforms.

### Fixed
- **Unsafe Environment Access**: Wrapped `std::env::set_var` and `remove_var` calls in `unsafe` blocks within `src/guard/env.rs` to comply with the latest Rust edition requirements for test isolation.
- **Distillation Regression**: Resolved failures in `test_readfile_large_rust_file_distilled` by recalibrating token thresholds in test fixtures, ensuring consistent distiller triggering.
- **Snapshot Integrity**: Synchronized stale `insta` snapshots to match refined pipeline output patterns, maintaining high-fidelity regression tracking.

## [0.5.7-rc3] - 2026-05-07

### Added
- **Hermes Agent Integration**: New native plugin integration for the Hermes Agent. Features automatic `plugin.yaml` and Python hook script generation (`post_tool_call`, `pre_tool_call`, `on_session_start`) to silently filter noise in the background.

### Improved
- **Automated Doctor Fix Mode**: Massively refactored agent integrations (`cline`, `codex`, `cursor`, `gemini`, `antigravity`) to support a standardized configuration path management and automated `--fix` operations.
- **Claude Hook Cleanup**: Implemented robust `uninstall` logic in the Claude Code integration to completely scrub OMNI hooks and MCP server entries from `settings.json` and `.claude.json`.
- **OpenClaw Portability**: Refactored the OpenClaw plugin's TypeScript configuration to use standard `Node16`/`ES2022` settings instead of relying on sandbox-specific file paths. Also renamed the plugin directory from `integrations/openclaw` to `plugins/openclaw`.

### Fixed
- **Clippy Strictness**: Resolved hidden nested-if collapsible warnings (`clippy::collapsible_if`) inside `claude.rs` ensuring zero warnings under `#![deny(warnings)]`.

## [0.5.7-rc2] - 2026-05-06

### Added
- **Lightweight Anti-Hallucination Guards**: Added factual warnings when OMNI knows context is incomplete, including heavy-compression-without-rewind cases and high-impact file reads with many dependents.
- **ReadFile Dependency Context**: `ReadFile` distillation now surfaces dependency impact using graph-derived `imported_by` counts so agents know when a file change may have broad blast radius.
- **Hot File Mutation Warnings**: `PreToolUse` now warns before mutating commands touch files that are already hot in current session context.

### Improved
- **Graceful ReadFile Fallback**: `post_tool` now falls back to base `distill_readfile` flow when graph indexing is unavailable instead of dropping contextual file distillation entirely.
- **Doctor Auto-Repair Coverage**: Agent diagnostics continue to auto-repair missing integrations while preserving stronger validation for installed MCP entries.
- **Cursor MCP Validation**: Refactored Cursor integration checks around structured JSON validation and idempotent install/remove behavior for `~/.cursor/mcp.json`.
- **Code Quality**: Resolved fresh Clippy warnings, including regex construction in loops and nested conditional lint violations, while keeping fallback paths architecturally live.

### Fixed
- **ReadFile Wrapper Regression**: Restored legitimate `distill_readfile` usage through real fallback path instead of silencing dead-code warnings by removal.
- **Context Warning Reliability**: Fixed hook pipeline paths so anti-hallucination warnings only emit from factual runtime signals already known by OMNI.

## [0.5.7-rc1] - 2026-05-03

### Added
- **Automated Tool Distillation**: Implemented automated distillation for MultiEdit and unhandled tool outputs to reduce token usage natively.
- **Context-Aware Estimation**: New token estimation utility for highly accurate cost and usage calculations within the ROI monitor.
- **Positional Boosting**: Extracted positional boost logic to the semantic scorer, implementing dynamic priority-based segment distillation.

### Improved
- **Security Hardening**: Expanded the denylist of restricted environment variables in `sanitize_env` to prevent injection attacks.
- **Hash Entropy**: Increased the RewindStore archive hash length from 8 to 16 hex characters, ensuring 64-bit entropy and preventing collision.
- **Diagnostic Detection**: Enhanced the `BuildDistiller` to natively detect and prioritize single-line diagnostics and preserve git commit hashes in the collapse pipeline.
- **Code Quality**: Applied consistent `rustfmt` formatting and resolved all emerging Clippy warnings across the codebase.

### Fixed
- **Command Routing**: Fixed a critical command detection bug by stripping surrounding quotes from command names during base path extraction, allowing Antigravity IDE and other quoted-path environments to correctly route tools to specific distillers.

## [0.5.7] - 2026-04-27
### Added
- **Multi-Agent Awareness (`omni_agents`)**: New MCP tool allowing agents (e.g., Claude, Cursor, Copilot) to discover and interact with each other's state on the same project.
- **Persistent Project Knowledge (`omni_knowledge`)**: Cross-session memory for agents to permanently learn and store project-specific quirks and filter preferences.
- **Advanced ROI Diagnostics**: Added `omni_history` (distillation log) and `omni_budget` (ROI simulator) MCP tools directly to the agent toolkit.
- **Meta-Harness Outer Loop**: Implemented `omni optimize` to automatically validate generated LLM filters.
- **Non-Bash Tool Distillation**: Expanded engine routing for `ReadFile`, `Grep`, and `WebFetch` output.
- **Distiller Context Preservation**: Added `-->` contextual error block preservation to the Build and Test distillers.
- **Extended Hook Architecture**: New async hooks for `SessionEnd`, `PostToolUseFailure`, `FileChanged`, and `SubagentStart`.
- **Antigravity IDE Integration**: Native MCP server bindings for Google's Antigravity environment (`~/.gemini/antigravity/mcp_config.json`).

### Improved
- **Positional Scorer Boost**: Dynamic positional-based priority bumping for active errors in multi-line outputs.
- **Passthrough Visibility**: Short or low-compression outputs are now explicitly labeled with `[OMNI: Passthrough]` rather than silently omitted.

## [0.5.6-rc3] - 2026-04-14
- **Database Distiller**: New `DatabaseDistiller` for intelligent distillation of PostgreSQL, MySQL, and SQLite CLI output — strips verbose headers and retains only actionable error signals.
- **Security Distiller**: New `SecurityDistiller` for CVE scanners (Trivy, Snyk, Semgrep) — collapses verbose scan reports into concise vulnerability summaries.
- **VCS Distiller**: New `VcsDistiller` for version control tools beyond Git (Mercurial, SVN) with output-aware heuristics.
- **Expanded Tool Registry**: Added granular `cargo` subcommand support and new tool categories for Database, Mobile, Cloud, and CI/CD toolchains with accurate distiller routing.

### Improved
- **OpenClaw Portability**: The OpenClaw integration natively fetches plugin files directly from the public GitHub repository, allowing successful 1-click installation without requiring a full local git repository clone.
- **Robust RegEx Generation (`omni learn`)**: Fixed a critical bug where auto-learned numeric patterns used literal `#` instead of functional `\d+` in generated TOML filters. Now delegates TOML string escaping to the `toml` crate for correctness.
- **Enhanced Verify Report (`omni learn --verify`)**: Results are now grouped by source (Built-in vs. User), with clear per-category pass/fail counts and actionable tips when user-learned filters fail.
- **Auto-Clear Learn Queue**: `omni learn --apply` now automatically clears `~/.omni/learn_queue.jsonl` after successful application, preventing stale data from polluting subsequent `--discover` runs.
- **Premium Discover Table (`omni learn --discover`)**: Replaced raw text output with a structured `comfy-table` layout featuring color-coded actions (Strip/Count) and pattern previews.
- **Doctor Filter Diagnostics**: `omni doctor` now reports specific warnings for skipped filters (e.g., missing `match_command`) instead of generic error messages, and `--fix` can auto-repair invalid TOML files.
- **Distiller Robustness**: Replaced strict prefix checks with case-insensitive substring matching across all distillers for more reliable command detection.
- **Filter Loading**: Made `match_command` optional in `FilterConfig`, gracefully skipping filters with empty or missing patterns instead of crashing.

### Fixed
- **Learned Filters Concurrency (`learned.toml`)**: Replaced seconds-based timestamp resolution with `timestamp_micros()` for auto-generated filters to prevent fatal TOML duplication parse errors during high-frequency concurrent learning (fixes the infinite `doctor --fix` `.bak` failure loop).
- **Test Regression (`test_claude_code_stdout_format`)**: Resolved a persistent CI failure caused by state contamination from user-learned filters leaking into the test environment.
- **Stats UX Hint**: Added `--all-commands` usage hint to `omni stats` when showing truncated top-10 results.

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
