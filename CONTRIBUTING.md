# Contributing to OMNI

## Prerequisites

1. **Rust** (stable, 2024 edition)
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **cargo-insta** (for snapshot tests)
   ```bash
   cargo install cargo-insta
   ```

## Getting Started

```bash
git clone https://github.com/fajarhide/omni.git
cd omni
cargo build
cargo test    # All 147 tests should pass
```

## Development Workflow

1. Create a feature branch: `git checkout -b feature/my-feature`
2. Make changes
3. Run: `cargo fmt && cargo clippy && cargo test`
4. Review snapshots if changed: `cargo insta review`
5. Submit a PR

## What We Welcome

- New distillers for uncommon tools
- TOML filters for popular tools
- Performance optimizations
- Documentation improvements
- Bug fixes

## Code Style

- Run `cargo fmt` before committing
- `cargo clippy -- -D warnings` must pass (enforced in CI)
- All hooks must handle errors gracefully (no panics in production paths)
- Add tests for new functionality

## See Also

- [CLAUDE.md](CLAUDE.md) — Full developer guide (project structure, architecture)
- [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) — Detailed development setup
- [docs/FILTERS.md](docs/FILTERS.md) — How to write TOML filters
