# Development Guide

Guide for contributors working on the OMNI codebase.

## Prerequisites

- **Rust** (stable, 2024 edition) — `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- **SQLite** (bundled via `rusqlite` with `bundled` feature — no system dependency)

That's it. No Node.js, no npm, no Zig, no Wasm runtime.

## Getting Started

```bash
git clone https://github.com/fajarhide/omni.git
cd omni
cargo build
cargo test
```

## Build Commands

```bash
cargo build              # Debug build
cargo build --release    # Release build (~4MB binary)
cargo test               # Run all 147 tests
cargo test <module>      # Run specific tests (e.g., cargo test distillers)
cargo clippy             # Lint (CI enforces -D warnings)
cargo fmt                # Format code
cargo insta review       # Review snapshot test changes
```

## Project Layout

```
src/
├── main.rs          # CLI entry point, Mode enum dispatch
├── lib.rs           # Library exports (for integration tests)
├── pipeline/        # Core: Classifier → Scorer → Composer
├── distillers/      # Type-specific distillers (git, build, test, etc.)
├── hooks/           # Claude Code hook handlers
├── store/           # SQLite persistence layer
├── session/         # Session tracking and auto-learn
├── guard/           # Security: env denylist, input limits, trust
├── mcp/             # MCP server (rmcp crate)
└── cli/             # CLI subcommands (init, stats, session, learn, doctor)

tests/
├── fixtures/        # 45 realistic fixture files
├── savings_assertions.rs   # Per-filter savings threshold tests
├── hook_e2e.rs      # Binary spawn E2E tests
├── security_tests.rs       # Security validation tests
└── smoke_test.sh    # Shell smoke test script
```

## Testing

### Unit Tests

Each module has inline `#[cfg(test)]` tests. Run all with:

```bash
cargo test
```

### Snapshot Tests

Distillers use `insta` for snapshot testing:

```bash
# Run tests and generate new snapshots
cargo test distillers::tests

# Review pending snapshots
cargo insta review
```

### Integration Tests

```bash
cargo test --test hook_e2e           # E2E binary spawn tests
cargo test --test savings_assertions # Savings threshold tests
cargo test --test security_tests     # Security tests
```

### Smoke Tests

```bash
chmod +x tests/smoke_test.sh
tests/smoke_test.sh ./target/debug/omni
```

## Adding a New Distiller

1. Create `src/distillers/my_type.rs` implementing the `Distiller` trait
2. Register in `src/distillers/mod.rs` → `get_distiller()`
3. Add fixture in `tests/fixtures/`
4. Add snapshot test via the `snapshot_test!` macro
5. Run `cargo test` then `cargo insta review`

## Adding a Content Type

1. Add variant to `ContentType` enum in `src/pipeline/mod.rs`
2. Add classification rules in `src/pipeline/classifier.rs`
3. Add scoring rules in `src/pipeline/scorer.rs`
4. Create a distiller (see above)

## CI/CD

GitHub Actions runs on every push/PR:

1. **fmt** — `cargo fmt --check`
2. **clippy** — `cargo clippy -- -D warnings`
3. **test** — matrix `[ubuntu, macOS]` → `cargo test --all`
4. **security** — `cargo audit` + dangerous pattern scan
5. **binary-check** — size check + smoke tests

Releases are triggered by pushing a `v*` tag (see `scripts/omni-release.sh`).

## Key Design Principles

- **Never crash the host**: All hooks use `catch_unwind`, return exit 0
- **Graceful degradation**: DB failure → hooks still work (no session context)
- **Deterministic**: Same input → same output (no randomness)
- **Fast**: Target <2ms for typical inputs
- **Never drop**: RewindStore preserves everything
