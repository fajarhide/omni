# CLAUDE.md — OMNI Developer Guide

This file is for AI assistants (Claude, Codex, etc.) and human contributors working on OMNI.

## Quick Reference

```bash
cargo build              # Build debug binary
cargo build --release    # Build release binary
cargo test               # Run all tests (147 tests)
cargo test <module>      # Run specific module tests
cargo insta review       # Review snapshot test changes
cargo clippy             # Lint
cargo fmt                # Format
```

## Project Structure

```
src/
├── main.rs              # CLI dispatch (Mode enum → match)
├── lib.rs               # Library re-exports for integration tests
├── pipeline/
│   ├── mod.rs           # Core types: ContentType, OutputSegment, DistillResult, SessionState
│   ├── classifier.rs    # Stage 1: Content type detection (10 types)
│   ├── scorer.rs        # Stage 2: Semantic signal scoring with context boost
│   ├── composer.rs      # Stage 4-5: Threshold filtering + RewindStore
│   └── toml_filter.rs   # TOML filter engine (user-defined filters)
├── distillers/
│   ├── mod.rs           # Distiller trait + dispatch + snapshot tests
│   ├── git.rs           # GitDiff/Status/Log distiller
│   ├── build.rs         # Build output distiller
│   ├── test.rs          # Test output distiller
│   ├── infra.rs         # kubectl/docker/terraform distiller
│   ├── log.rs           # Log file distiller
│   ├── tabular.rs       # Tabular data distiller
│   └── generic.rs       # Fallback distiller
├── hooks/
│   ├── dispatcher.rs    # Universal hook router (PostToolUse/SessionStart/PreCompact)
│   ├── post_tool.rs     # PostToolUse: classify → score → compose
│   ├── session_start.rs # SessionStart: inject session context
│   ├── pre_compact.rs   # PreCompact: save state before compaction
│   └── pipe.rs          # Stdin pipe mode (cmd | omni)
├── store/
│   └── sqlite.rs        # SQLite persistence (sessions, distillations, rewind, FTS5)
├── session/
│   ├── tracker.rs       # Background session context tracking
│   └── learn.rs         # Auto-learn pattern detection
├── guard/
│   ├── env.rs           # Environment variable denylist
│   ├── limits.rs        # Input size limits
│   └── trust.rs         # SHA-256 project trust boundary
├── mcp/
│   └── server.rs        # MCP server (5 tools: retrieve, learn, density, trust, compress)
└── cli/
    ├── init.rs          # omni init --hook
    ├── stats.rs         # omni stats analytics dashboard
    ├── session.rs       # omni session state inspection
    ├── learn.rs         # omni learn CLI
    └── doctor.rs        # omni doctor diagnostics
```

## Pipeline Architecture

```
Input (raw tool output)
  │
  ▼
┌─────────────────────────────────────────────┐
│ Stage 1: Classifier                         │
│ classifier::classify(input) → ContentType   │
│ (GitDiff, BuildOutput, TestOutput, etc.)    │
└─────────────┬───────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────┐
│ Stage 2: Scorer                             │
│ scorer::score_segments(input, type, session) │
│ → Vec<OutputSegment> with relevance scores  │
│ (Critical=1.0, Important=0.7, Noise=0.1)   │
└─────────────┬───────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────┐
│ Stage 4-5: Composer                         │
│ composer::compose(segments, config, store)   │
│ → (output_string, Option<rewind_hash>)      │
│ Filters segments < 0.3 threshold            │
│ Stores dropped content in RewindStore       │
└─────────────┬───────────────────────────────┘
              │
              ▼
Output (distilled, with optional rewind notice)
```

## How to Add a New Distiller

1. Create `src/distillers/my_type.rs`:
```rust
use crate::pipeline::{ContentType, OutputSegment};
use super::Distiller;

pub struct MyDistiller;

impl Distiller for MyDistiller {
    fn content_type(&self) -> ContentType { ContentType::MyType }

    fn distill(&self, segments: &[OutputSegment], input: &str) -> String {
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
- **distillations**: Every distillation event (filter, type, bytes in/out, route, score, latency)
- **file_access**: Hot file tracking per session
- **rewind_store**: Compressed content (SHA-256 hash → content, with retrieval counter)
- **session_events (FTS5)**: Full-text searchable event index

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
