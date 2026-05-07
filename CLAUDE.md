# CLAUDE.md — OMNI Developer Guide

This file is for AI assistants (Claude, Codex, etc.) and human contributors working on OMNI.

## Quick Reference

```bash
cargo build              # Build debug binary
cargo build --release    # Build release binary
cargo test               # Run all tests
cargo test <module>      # Run specific module tests
cargo insta review       # Review snapshot test changes
cargo clippy             # Lint
cargo fmt                # Format
```

## Project Structure

```
src/
├── main.rs              # CLI dispatch
├── lib.rs               # Library re-exports for integration tests
├── paths.rs             # Path resolution and discovery
├── agents/              # Multi-agent support (Claude, Cursor, Copilot, etc.)
├── cli/                 # Command-line interfaces
│   ├── init.rs, stats.rs, doctor.rs, learn.rs, session.rs
│   └── diff.rs, exec.rs, optimize.rs, reset.rs, rewind.rs, rewrite.rs, update.rs
├── distillers/          # Content filtering logic by type
│   ├── git.rs, build.rs, test.rs, cloud.rs, database.rs, jsts.rs
│   └── readfile.rs, search.rs, security.rs, system_ops.rs, vcs.rs
├── graph/               # Code graph indexing
│   └── indexer.rs
├── guard/               # Safety, security, limits, and trust bounds
│   ├── config.rs, env.rs, limits.rs, trust.rs, update.rs
├── hooks/               # Pipeline execution hooks
│   ├── dispatcher.rs    # Universal hook router
│   ├── pre_tool.rs, post_tool.rs, post_tool_failure.rs
│   └── session_start.rs, session_end.rs, pre_compact.rs, pipe.rs
├── mcp/                 # Model Context Protocol
│   └── server.rs        # MCP server tools
├── pipeline/            # Core processing pipeline
│   ├── analyzer.rs, collapse.rs, registry.rs, scorer.rs, toml_filter.rs
├── session/             # Session tracking and learning
│   ├── tracker.rs, learn.rs, correction.rs
├── store/               # Persistence layer
│   └── sqlite.rs, transcript.rs
└── util/                # Common utilities
    └── command_family.rs, token_estimate.rs
```

## Pipeline Architecture

```
Input (raw tool output)
  │
  ▼
┌─────────────────────────────────────────────┐
│ Stage 1: Registry                           │
│ registry::resolve_profile(command)          │
│ (Matches command to specialized distiller)  │
└─────────────┬───────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────┐
│ Stage 2: Scorer                             │
│ scorer::score_with_command(input, cmd, sess)│
│ → Vec<OutputSegment> with relevance scores  │
│ (Critical=1.0, Important=0.7, Noise=0.1)    │
└─────────────┬───────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────┐
│ Stage 3: Distiller                          │
│ distillers::distill_with_command(...)       │
│ → output_string                             │
│ Filters noise                               │
└─────────────┬───────────────────────────────┘
              │
              ▼
Output (distilled signal)
```

1. Create `src/distillers/my_type.rs`:
```rust
use crate::pipeline::{OutputSegment, SessionState};
use super::Distiller;

pub struct MyDistiller;

impl Distiller for MyDistiller {
    fn distill(
        &self, 
        segments: &[OutputSegment], 
        input: &str,
        session: Option<&SessionState>,
    ) -> String {
        // Extract and summarize the critical information
        todo!()
    }
}
```

2. Register in `src/distillers/mod.rs`:
```rust
pub mod my_type;
// In get_distiller():
ContentType::MyType => Box::new(my_type::MyDistiller),
```

3. Add a fixture file in `tests/fixtures/my_type_example.txt`

4. Add a snapshot test in `src/distillers/mod.rs`:
```rust
snapshot_test!(test_my_type_distillation, "my_type_example.txt", ContentType::MyType);
```

5. Run `cargo test` then `cargo insta review` to approve the snapshot.

## How to Add a TOML Filter

Create a file in `~/.omni/filters/my_filter.toml`:
```toml
schema_version = 1

[filters.my_filter]
description = "My custom filter"
match_command = "^my-tool\\b"
strip_lines_matching = ["^DEBUG", "^TRACE"]
max_lines = 50

[[tests.my_filter]]
name = "basic test"
input = "DEBUG: ignore\nIMPORTANT: keep"
expected = "IMPORTANT: keep"
```

Verify with: `omni learn --verify`

## Database Schema

- **sessions**: Session state (id, timestamps, task/domain hints, state JSON)
- **distillations**: Every distillation event (filter, input/output bytes, route, score, latency, agent_id)
- **file_access**: Hot file tracking per session
- **rewind_store**: Compressed content (SHA-256 hash → content, with retrieval counter)
- **session_events (FTS5)**: Full-text searchable event index
- **passthrough_events**: Telemetry for commands that bypass the pipeline
- **unhandled_tools**: Telemetry for tools that OMNI doesn't yet support natively
- **execution_traces**: Execution traces containing raw input and distilled output per command
- **session_summaries**: Summarized metrics per session
- **project_knowledge**: Cross-session semantic memory and knowledge
- **agent_sessions**: Shared state tracking across multiple agents

## Cross-Platform OS Best Practices

OMNI must compile and pass tests gracefully across Linux, macOS, and Windows. All agents and contributors strictly adhere to the following pillars:

1. **No Hardcoded Path Separators**: Never use `/` or `\\` directly for file paths. Always use `std::path::PathBuf` and `PathBuf::push()` to build paths dynamically.
2. **Line Endings (`\n` vs `\r\n`)**: Never hard-match exactly against `\n` in string assertions. Windows uses `\r\n`. Use `.lines()` iterator which gracefully cleans lines on all OS, or use `replace("\r\n", "\n")` before assertions.
3. **OS-Specific Executable Suffixes**: Never assume binary names are exactly `./omni`. In Windows it compiles as `omni.exe`. Use `std::env::consts::EXE_SUFFIX` for dynamic matching.
4. **Environment Variables**: Windows environment variables are case-insensitive. Unix is case-sensitive. Always use `eq_ignore_ascii_case()` when reading from `std::env::vars()` or interacting with os-injected configurations to prevent false-panics.
5. **Robust CI Matrix Testing**: Before declaring success, guarantee the feature respects the Github Actions CI matrix. Changes to system integration code must be fundamentally safe to execute against `windows-latest` alongside `ubuntu-latest`.

## Key Design Decisions

- **Panic safety**: All hooks use `catch_unwind` — OMNI never crashes the host agent
- **Graceful degradation**: If DB fails, hooks still work (just without session context)
- **Deterministic**: Same input always produces same output (no randomness)
- **Sub-millisecond**: Pipeline targets <2ms for typical inputs
- **Never drop**: RewindStore ensures no information is permanently lost

### Essential Reading
- [tests/README.md#critical-guardrails](tests/README.md#critical-guardrails) — **Mandatory: Prevent deadlocks & test hangs**

## Rust 2024 Idiomatic Standards

All contributors (AI and human) must adhere to these modern Rust practices:

### 1. Modern Statics & Concurrency
*   **Prefer `std::sync::LazyLock`**: Replace `lazy_static!` with `LazyLock`. It is built-in and more idiomatic in Rust 2024.
*   **Poison Handling**: When using `Mutex<SessionState>`, prefer `.lock().unwrap_or_else(|p| p.into_inner())` to recover from panics, especially in long-running background tasks.

### 2. Control Flow & Patterns
*   **Use `let-chains`**: Combine multiple checks into a single flattened `if let ... && ...` to reduce nesting.
*   **Pattern Matching**: Use exhaustive matching. Avoid `_ => {}` unless truly necessary; prefer explicit variants.

### 3. Performance & Memory
*   **Zero-Copy with `Cow`**: Use `std::borrow::Cow<'_, str>` for string manipulation in hot paths (like `scorer` or `collapse`) to avoid unnecessary allocations when no modification is needed.
*   **Avoid `.clone()`**: Before calling `.clone()`, check if a reference `&T` or `std::mem::take()` suffices.
*   **Capacity Hints**: Use `Vec::with_capacity()` and `String::with_capacity()` when the final size is predictable.

### 4. Safety & Lints
*   **No `unwrap()` in Production**: Never use `.unwrap()` on user input or IO operations. Use `.expect("contextual message")` or better, return a `Result`.
*   **Strict Clippy**: All code must pass `cargo clippy -- -D warnings`.

---

## Architectural Guardrails

### 1. Library-First Design
*   The `main.rs` file should be a "thin" entry point. 
*   All core logic, types, and business rules must reside in `lib.rs` or its submodules. 
*   This ensures OMNI can be tested as a crate and integrated into other tools.

### 2. Single Source of Truth (SSOT)
*   **Command Mapping**: Centralize all command-to-behavior mappings in `pipeline/registry.rs`. Do not duplicate `if matches!(cmd, ...)` blocks in distillers or scorers.
*   **Constants**: Store magic numbers (thresholds, history limits, timeouts) as named `pub const` in `pipeline/mod.rs` or `guard/limits.rs`.

### 3. Separation of Concerns
*   **IO vs. Logic**: Pure functions (scoring, filtering) should take `&str` or `impl Read` and return data, not perform filesystem or network side effects.
*   **Pipeline Stages**: Maintain the strict order: `Read → Guard → Score → Collapse → Distill → Persist`.

---

## Error Handling Standards

OMNI uses `anyhow` for top-level application errors and `thiserror` for library-level errors if specific error variants are needed for matching.

*   **Contextualize**: Always use `.with_context(|| "...")` when propagating errors from IO or third-party crates.
*   **User-Friendly Messages**: Error messages should explain *what* happened and *how* to fix it (e.g., "failed to read ~/.omni/filters: permission denied").
*   **Panic Boundaries**: Use `catch_unwind` at the highest possible entry point (e.g., `dispatcher.rs`) to ensure a single failing hook doesn't crash the entire agent session. Wrap captured state in `AssertUnwindSafe`.

---

## Security & Trust

*   **Sanitize Input**: Treat all tool output as untrusted. Sanitize ANSI codes and control characters before distillation.
*   **Environment Hygiene**: In `guard/env.rs`, ensure sensitive environment variables (like `LD_PRELOAD`) are sanitized before executing subcommands.
*   **Config Trust**: Only load `.omni/filters` from the project root if the directory is explicitly "trusted" (see `guard/trust.rs`).

---

## Rust Testing Standards

All tests in OMNI must follow idiomatic Rust testing conventions.

### Test Naming Conventions

Inside `#[cfg(test)]` modules, drop the `test_` prefix from function names. Use action-oriented, behavioral names in `snake_case`.

Preferred patterns:

```rust
fn <action>_<subject>_<condition>()
fn <expected_behavior>()
fn <action>_<result>()
```

Good examples:

```rust
fn returns_default_when_config_missing()
fn excludes_sensitive_data_from_summary()
fn treats_exit_0_as_ok()
fn removes_stale_entries()
fn ignores_non_matching_payloads()
fn preserves_errors_during_collapse()
```

Avoid:

```rust
fn test_config_ok           // Redundant 'test_' prefix
fn test_benar               // Indonesian word
fn handles_it               // Vague 'handles'
fn valid_json               // No action verb
```

Rules:

*   **No `test_` prefix**: Since the function is already inside a `#[cfg(test)]` module and marked with `#[test]`, the prefix is redundant.
*   **English only**: No Indonesian words (e.g., avoid `selalu`, `dengan`, `tanpa`).
*   **Action Verbs**: Start with or include clear verbs: `returns`, `preserves`, `skips`, `rejects`, `detects`, `computes`.
*   **Avoid vague words**: `works`, `valid`, `correct`, `handles`.

### Test Design Principles

#### 1. Test Observable Behavior
Test outputs and side effects, not implementation details.
Prefer: `assert_eq!(result.status, Status::Ready);`
Avoid: `assert!(internal_cache.len() > 0);`

#### 2. One Behavioral Assertion Per Test
Each test should validate one primary behavior.
Good: `test_session_summary_excludes_sensitive_data`
Avoid: `test_session_summary_everything`

#### 3. Arrange / Act / Assert Structure
Use explicit sections for readability.

```rust
#[test]
fn test_load_config_returns_default_when_file_missing() {
    // Arrange
    let path = tempdir().unwrap();

    // Act
    let config = load_config(path.path());

    // Assert
    assert_eq!(config.mode, Mode::Default);
}
```

---

## Required Test Coverage For New Features

Every non-trivial feature should include:

*   **Success case**: "Happy path" execution.
*   **Edge case**: Limits, boundaries, very large or small inputs.
*   **Malformed input case**: Invalid JSON, binary noise, interrupted streams.
*   **Regression case**: If fixing a bug, add a test that would have failed before.
*   **No-panic case**: Explicitly test that malformed input returns an `Err`, not a panic.

Checklist:

```text
[ ] Happy path
[ ] Edge case
[ ] Invalid input
[ ] Empty input
[ ] Cross-platform safe
[ ] Deterministic
[ ] No panic
[ ] Fast execution
```

---

## Documentation & Internationalization (i18n)

OMNI is a global project. All user-facing documentation must be synchronized across all supported languages.

### 1. README Synchronization
Whenever a feature, installation step, or technical explanation is added to the main `README.md`, it **must** be reflected in all files under the `i18n/` directory:
*   `README-ja.md` (Japanese)
*   `README-zh.md` (Chinese)
*   `README-ar.md` (Arabic)
*   `README-id.md` (Indonesian)
*   `README-vi.md` (Vietnamese)
*   `README-ko.md` (Korean)

### 2. Technical Terminology Guardrails
To prevent confusion across languages, certain technical terms should be handled with care:

| Term | Strategy | Notes |
| :--- | :--- | :--- |
| **OMNI** | Keep as is | Always uppercase. |
| **RewindStore** | Keep as is | Branding for the compression archive. |
| **MCP** | Keep as is | Refers to Model Context Protocol. |
| **Token** | Translate carefully | Use the standard local technical term (e.g., "Token" in ID, "トークン" in JA). |
| **Distillation** | Translate with context | Refers to semantic filtering (e.g., "Distilasi" in ID, "蒸留" in JA). |
| **Hook** | Keep as is or local tech term | Refers to pipeline entry points. |
| **Semantic Signal Engine** | Translate | The core description of OMNI. |

### 3. File Pathing in i18n
Files in `i18n/` are one level deeper than the root. Ensure assets and links are adjusted:
*   Use `../media/logo.png` instead of `media/logo.png`.
*   Links to root files should use `../` (e.g., `[English](../README.md)`).

### 4. Tone of Voice
*   **Professional but Passionate**: OMNI is a "passion project" for the agentic AI era.
*   **Transparent**: Emphasize that the user is always in control.
*   **Action-Oriented**: Use clear imperatives in instructions.

---

## AI Agent Instructions

When generating or modifying code:

*   **Modernize**: If you see `lazy_static!` or old `if let` nesting, refactor it to Rust 2024 standards.
*   **Preserve Style**: Maintain the consistent "pure logic" vs "IO wrapper" separation.
*   **Safety First**: Ensure `AssertUnwindSafe` is used when capturing state in panics.
*   **Run Gates**:
    1. `cargo fmt`
    2. `cargo clippy -- -D warnings`
    3. `cargo test`
    before finalizing changes.

Never silently weaken assertions or remove security checks just to make tests pass.
