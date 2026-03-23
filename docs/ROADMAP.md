# OMNI Roadmap

## Current: v0.5.0 (Rust Rewrite)

**Status: Complete** ✅

The full rewrite from Node.js + Zig hybrid to a single Rust binary.

### What Was Delivered

- **Single binary** — no runtime dependencies (no Node.js, no Zig, no Wasm)
- **10-type content classifier** — GitDiff, GitStatus, GitLog, BuildOutput, TestOutput, InfraOutput, LogOutput, TabularData, StructuredData, Unknown
- **Semantic scoring engine** — signal tiers (Critical/Important/Context/Noise) with session context boost
- **RewindStore** — SHA-256 hashed content storage, never-drop guarantee
- **Session continuity** — hot files, active errors, domain inference, task inference
- **5 MCP tools** — retrieve, learn, density, trust, compress
- **3 Claude Code hooks** — PostToolUse, SessionStart, PreCompact
- **TOML filter engine** — user-defined filters with inline tests
- **Auto-learn** — pattern detection from passthrough output
- **Analytics dashboard** — `omni stats` with filter breakdowns and cost estimation
- **Doctor diagnostics** — `omni doctor` installation validation
- **147 tests** — unit, snapshot, E2E, security, smoke
- **Cross-platform CI/CD** — GitHub Actions with 4-target release builds

---

## Next: v0.6.0 (Intelligence Layer)

### Planned Features

#### Adaptive Scoring
- Learn from RewindStore retrievals — if Claude frequently retrieves, OMNI is too aggressive
- Automatically adjust scoring thresholds per content type
- Per-project scoring profiles

#### Multi-Agent Support
- Cursor AI integration (native hooks)
- Codex CLI integration
- Generic JSONRPC hook interface
- Agent-specific scoring profiles

#### Filter Marketplace
- Community-shared TOML filters
- `omni filter install <name>` from registry
- Filter versioning and updates

#### Enhanced Analytics
- Cost tracking per project / per agent
- Trend visualization (reduction% over time)
- Export to CSV/JSON

---

## Future: v0.7.0 (Ecosystem)

### Ideas Under Consideration

- **Team mode** — shared session state across multiple developers
- **Remote RewindStore** — cloud-backed content storage
- **Plugin system** — custom distillers as shared libraries
- **IDE integration** — VS Code extension for stats overlay
- **Streaming mode** — real-time distillation of long-running commands
- **LLM-assisted scoring** — use a small model to evaluate content importance

---

## Contributing

See [docs/DEVELOPMENT.md](DEVELOPMENT.md) for the contributor guide. We welcome:

- New distillers for uncommon tools
- TOML filters for internal company tools
- Performance optimizations
- Documentation improvements
