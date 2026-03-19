# CLAUDE.md

This file provides guidance to Claude Code when working in the **OMNI** repository.

## Project Overview
**OMNI** (Optimization Middleware & Next-gen Interface) is a hybrid token-efficiency platform using a Zig core (for Wasm/performance) and a TypeScript MCP server (for integration).

## Development Commands

### Core (Zig)
- `zig build` - Build all binaries (Native & Wasm)
- `zig build run` - Build and run native engine
- `zig build test` - Run all core tests
- `zig build wasm` - Build WebAssembly binary
- `zig fmt core/src/` - Format Zig code
- `./scripts/omni-release.sh <v>` - Run full release & Homebrew sync

### OMNI CLI
- `omni monitor` - Unified system metrics and status
- `omni density` - Analyze context gain (stdin)
- `omni bench` - Run performance benchmark
- `omni generate` - Output agent templates
- `omni setup` - Integration & setup guide
- `omni update` - Check for the latest version from GitHub
- `omni uninstall` - Remove OMNI and clean up all MCP configs

### Unified Interface (Makefile)
- `make` or `make verify` - Full suite: version check + build + test + monitor
- `make build` - Build Wasm core + TypeScript server
- `make test` - Run semantic verification tests
- `make monitor` - Generate system & performance monitor
- `make check-version` - Verify version consistency
- `make clean` - Remove build artifacts

## Directory Structure
- `core/` - Zig engine core & filters
- `src/` - MCP server implementation & LRU cache
- `docs/` - Project documentation

## Design Principles
1. **Efficiency:** Minimal startup time (<1ms via Wasm & LRU cache). 
2. **Modularity:** Plugin-based filter architecture.
3. **Semantic First:** Prioritize signal over mere truncation.
4. **Local-First:** All processing data stays on the user's machine.

## Workflow Patterns
- **Zig First:** Implement core logic in `core/` with high test coverage.
- **Wasm Target:** Always verify that core logic compiles for `wasm32-wasi`.
- **MCP Standards:** Follow the Model Context Protocol strictly for tool definitions.
