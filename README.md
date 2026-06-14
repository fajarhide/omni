<div align="center">
  <img src="media/hero.svg" alt="OMNI" width="800" />
  
  **The Context Operating System for AI Agents. Less noise. More signal. Cut token consumption by up to 90%.**

  [🇺🇸 English](README.md) | [🇯🇵 日本語](i18n/README-ja.md) | [🇨🇳 简体中文](i18n/README-zh.md) | [🇸🇦 العربية](i18n/README-ar.md) | [🇮🇩 Bahasa Indonesia](i18n/README-id.md) | [🇻🇳 Tiếng Việt](i18n/README-vi.md) | [🇰🇷 한국어](i18n/README-ko.md)

  [![CI](https://github.com/fajarhide/omni/actions/workflows/ci.yml/badge.svg)](https://github.com/fajarhide/omni/actions/workflows/ci.yml)
  [![Release](https://img.shields.io/github/v/release/fajarhide/omni)](https://github.com/fajarhide/omni/releases)
  [![Rust](https://img.shields.io/badge/built_with-Rust-dca282.svg)](https://www.rust-lang.org/)
  [![MCP](https://img.shields.io/badge/MCP-compatible-green.svg?style=flat-square)](https://modelcontextprotocol.io/)
  [![License: MIT](https://img.shields.io/github/license/fajarhide/omni)](https://github.com/fajarhide/omni/blob/main/LICENSE)
  [![Hits](https://hits.sh/github.com/fajarhide/omni.svg)](https://hits.sh/github.com/fajarhide/omni/)
</div>

<br/>

> **OMNI** is the **Context Operating System for Autonomous AI Agents**. 
> It acts as a high-performance semantic filter between your terminal and your LLM. By intelligently distilling noisy logs, caching states, and managing token budgets, OMNI ensures your agents stay focused, hallucinate less, and execute loops flawlessly—all while **cutting your API costs by up to 90%**.
> 
> *Stop paying for terminal noise. Start building with pure signal.*
---

## Table of Contents
- [The Problem: Expensive Tokens & Noisy Outputs](#the-problem-expensive-tokens--noisy-outputs)
- [The Solution: Omni](#the-solution-omni)
- [The Philosophy](#the-philosophy)
- [Real-World Use Cases](#real-world-use-cases)
- [Performance & Benchmarks](#performance--benchmarks)
- [Features Explained](#features-explained)
- [Under the Hood: How Omni Works](#under-the-hood-how-omni-works)
- [Architecture](#architecture)
- [Quick Start & Installation](#quick-start--installation)
- [How to Use It](#how-to-use-it)
  - [Multi-Agent Support & Integrations](#multi-agent-support--integrations)
  - [Documentation Index](#documentation-index)
- [Works Even Better with Heimsense](#works-even-better-with-heimsense)
- [Contributing & License](#contributing--license)

---

## The Problem: Expensive Tokens, Hallucinations & Infinite Loops

When you run autonomous AI agents (like Claude Code, Cursor, or Aider) in your terminal, they read *everything*. A simple `npm install` or `cargo test` command can easily dump 10,000 to 25,000 tokens of useless terminal noise into your AI's context window. 

This causes critical failures:
1. **Burned Budgets**: You pay real money for every single token of junk output.
2. **Agent "Amnesia" & Hallucinations**: Core errors get buried under megabytes of loading bars and dependency warnings. The AI gets confused, loses the original goal, and hallucinates fixes for the wrong problems.
3. **Model Lock-in**: You are forced to use the most expensive flagship models just to have a context window big enough to handle the bloat.
4. **Fragile Loops**: Autonomous loops break because agents lack awareness of token limits and context pressure.

## The Solution: OMNI Context OS

OMNI is the ultimate transparent middleware for Agentic AI. 

It intercepts terminal commands on the fly, strips away the noise, and feeds your AI a highly condensed, semantic summary. **The result?** You can run your agent on affordable models, feed it *zero noise*, and watch it solve complex coding tasks instantly.

Whether you are running a quick MCP tool call or orchestrating a massive multi-agent Maker-Checker loop, OMNI provides the persistent memory, budget tracking, and factual guardrails your AI needs to succeed.

Context is expensive and noisy. OMNI fixes it.

---

## The Philosophy

OMNI wasn't built just to "cut context" or "save tokens"—those are simply the happy side effects. The true philosophy behind OMNI is **Context Quality**.

AI agents like Claude are only as smart as the context you feed them. When you flood them with megabytes of dependency logs or loading bars, you force them to sift through garbage to find the actual problem. This dilutes their reasoning and leads to degraded or unhelpful responses.

**OMNI's goal is to feed your AI pure, highly-dense signal.** This means only grabbing the context that is actually important and meaningful for Claude. We clean up the noise the AI doesn't need, which means:
1. Automatically, the tokens you use are drastically fewer.
2. The AI's response is of **significantly higher quality** because its context window is laser-focused on the real problem.

**Try it for a week.** Feel the difference in the quality and speed of your AI's reasoning when it's fed on a diet of pure signal instead of raw terminal noise.

---

## Real-World Use Cases

OMNI is designed to solve the daily frustrations of Agentic AI developers. Here is how it transforms your workflow:

1. **The "Infinite Loop of Death" in Monorepos**
   - **Scenario**: You ask Claude to run `npm install` and `npm run build` in a large monorepo. It outputs 20,000 lines of dependency warnings and a small build error at the end. The AI gets distracted by the warnings and tries to fix unrelated dependency issues, burning through your tokens and trapping you in an infinite loop.
   - **OMNI's Fix**: OMNI intercepts the build. It completely mutes the hundreds of `peer dependency` warnings and only surfaces the exact `Build Error: Cannot find module 'X'` alongside the stack trace. The AI sees a 50-token output and fixes the code instantly.

2. **The "Silent Hallucination" on Large Files**
   - **Scenario**: The AI wants to understand a project and runs `cat src/utils.ts`. The file is 3,000 lines long. The AI struggles to keep all of it in working memory and starts hallucinating function signatures.
   - **OMNI's Fix**: OMNI blocks the raw `cat` and replaces it with a **Structured Outline**. It shows the AI the imports, the public API (function names and types), and risk markers, reducing the output by 80%. OMNI then warns the AI: `"This file has 12 dependents — use omni_context for full impact map."` The AI is guided to make safer, factual edits.

3. **Multi-Agent Collaboration**
   - **Scenario**: You are using Cursor IDE for quick edits and Claude Code CLI for heavy lifting. They both need to know what's happening without running redundant commands and wasting tokens.
   - **OMNI's Fix**: OMNI acts as a shared memory layer. Using `omni_agents` and its local SQLite `Store`, Cursor and Claude share the same filtered memory streams, active errors, and execution environments. They collaborate without clashing.

---

## Performance & Benchmarks
<div align="center">
<img src="https://omni.weekndlabs.com/media/performance.png" alt="OMNI" width="600" />
</div>

OMNI is built in Rust for zero-overhead execution and ruthless efficiency. Here are the actual benchmarks measured on the release binary:

| Command / Context | Input Size | Output Size | Token Savings | Impact on AI |
|-------------------|------------|-------------|---------------|--------------|
| `docker build` (multi-stage) | 9.2 KB | 49 bytes | **99.5%** | Eliminates caching noise; AI instantly sees the real build error. |
| `cargo test` (large suite) | 16.5 KB | 4.3 KB | **78.0%** | Strips hundreds of "ok" tests; AI focuses only on the failures and stack traces. |
| `git status` (dirty) | 496 bytes | 113 bytes | **77.2%** | Removes clean files and hints; keeps only modified/untracked files. |
| `kubectl get pods` | 840 bytes | 762 bytes | **10.0%** | Selectively surfaces CrashLoopBackOff/Error pods, skipping healthy ones. |
| `git diff` (multi-file) | 397 bytes | 220 bytes | **50.0%** | Preserves hunks with changes, dropping excessive context lines. |

- **Pipeline Latency**: **< 100ms** (end-to-end, including binary startup)
- **All-Time Savings**: **97.3%** token reduction across average development sessions.
- **ROI**: **$35+ USD** saved per developer/month (measured against flagship models).

*To see your own actual token savings, just run `omni stats` after a few days of usage.*

---

## Features Explained

### Core Distillation Engine
- **No More AI Confusion**: Omni acts like a smart sieve. If a test fails, it shows the AI *only* the specific error line and stack trace, blocking noisy dependency logs and loading spinners.
- **90% Token Reduction**: By eliminating useless terminal noise, you drastically cut your agentic API bills instantly.
- **Adaptive Compression**: OMNI tracks when agents retrieve omitted output. If a command family is frequently retrieved, OMNI automatically softens compression next time — self-tuning without configuration.
- **Smart High-Speed Bypass**: To ensure zero latency for small tasks, OMNI automatically bypasses distillation for outputs under a 2000-token threshold.

### Context Safety & Factual Guards
- **Zero Information Loss**: Worried Omni filtered something important? Don't be. Omni saves the raw output locally (`RewindStore`). The AI can automatically request it using `omni_retrieve`.
- **Factual Anti-Hallucination Guards**: OMNI emits warnings only when it has hard facts. If output is heavily compressed or a file has massive dependencies, OMNI injects a system warning to keep your AI grounded in reality.
- **Omission Visibility**: OMNI explicitly labels removed content (e.g., `[OMNI: omitted X lines of noise]`) in the output, giving your AI agent perfect situational awareness.

### Multi-Agent & Workspace Intelligence
- **Native MCP Server (`omni mcp`)**: OMNI operates as a high-performance Native Model Context Protocol (MCP) server. Agents can instantly query OMNI for active errors, historical engrams, token budgets, and contextual file insights via a direct `stdio` connection without any subprocess latency.
- **Multi-Agent Collaboration**: Fully aware of its environment via `OMNI_AGENT_ID`. If you have Cursor running alongside Claude CLI or Hermes, they seamlessly share the same filtered memory streams and active errors without clashing.
- **Session Intelligence**: OMNI remembers what you are doing. It knows which files you are actively editing and stops feeding the AI redundant context. Fixes are preserved permanently via `omni_knowledge`.
- **Structured ReadFile + Grep**: Instead of raw file dumps, OMNI returns structured outlines (imports, public API) and grouped grep summaries (priority lines first).
- **Lightweight Dependency Graph**: OMNI builds a fast local file relationship graph at hook time (no daemon). If your AI reads a heavily-imported file, OMNI warns it of the impact map.

### Context Fidelity & Session Recovery
- **Proactive Context Pressure**: OMNI actively acts as a "Token Traffic Light." Via the `omni_insight` MCP tool, OMNI pro-actively warns the agent when its context window hits "Warning" or "Critical" thresholds, triggering the agent to compress its memory *before* it crashes or hallucinates.
- **Engrams (Automatic Subtask Digests)**: OMNI automatically detects when a subtask is completed (e.g., resolving a compiler error, committing code, or fixing a broken test). It creates a highly compressed snapshot (an "Engram") without wasting tokens on LLM calls, so your agent never suffers from "context amnesia" during long sessions.
- **Smart Context Compaction**: When your context window gets full, OMNI doesn't blindly trim tokens. It uses a priority-aware algorithm to pack the most important data first (Pinned Files > Active Errors > Engrams > Tool Activity > Hot Files), saving massive overhead.
- **Session Handoffs**: Switching from Claude Code to Cursor or Hermes? Use the `omni_handoff` tool to instantly export the current session's memory (hot files, recent commands, active errors) into a portable summary that your new agent can instantly absorb.

### Autonomous Loop Engineering
- **Context Operating System for Loops**: OMNI manages context for iterative autonomous agent loops. Via environment variables (`OMNI_LOOP_BUDGET`, `OMNI_LOOP_GOAL`), OMNI enforces adaptive distillation limits and persistent tracking.
- **Maker-Checker Verification Pattern**: Scale your tasks cleanly by separating execution (Maker agent) from validation (Checker agent), securely exchanging context states through OMNI's multi-agent session store.
- **Predictive Goal-Driven Constraints**: Distillation automatically scales based on the task goal—if the goal contains "debug", OMNI retains more error context. If it is "refactor", OMNI compresses code traces aggressively.

### Monitoring & Debugging
- **Session Health Dashboard**: Run `omni session --health` for a beautiful visual dashboard of your context pressure, active engrams, rolling tool activity, and token savings.
- **Distill Monitor**: Track token savings over time. Use `omni_budget` and `omni_history` right inside your LLM, or run `omni stats` locally to visualize money saved.
- **Visual Impact (`omni diff`)**: Run `omni diff` to see the bulky raw output compared side-by-side to Omni's sleek, filtered version.
- **Debug Passthrough**: Need the raw output? Set `OMNI_PASSTHROUGH=1` to completely bypass the engine and see every character of the original output.

---

## Under the Hood: How Omni Works

OMNI is more than just a regex script; it's a high-performance **Semantic Signal Engine** written in Rust. But how does it actually cut 90% of token consumption in under 100ms? 

Here is the story of what happens inside the OMNI codebase when your AI Agent types a command like `cargo test`:

1. **The Interception (`src/hooks` & `src/main.rs`)**: The moment the AI hits "Enter", OMNI intercepts the execution. `main.rs` dynamically detects the context (whether it's a pipe, a hook, or an MCP call). The `hooks` module seamlessly wraps the command, allowing OMNI to capture the raw terminal output as a high-speed data stream without slowing down the actual execution.
2. **The Streaming Pipeline (`src/pipeline`)**: Instead of waiting for the command to finish and dumping megabytes of text into memory, OMNI processes the output line-by-line using a memory-efficient streaming pipeline. This ensures that even if a command spits out 10,000 lines of logs, OMNI's memory footprint remains nearly flat.
3. **The Semantic Brain (`src/distillers` & `src/guard`)**: As the text streams in, it passes through the Distillers. Powered by declarative TOML rules (`signals/`), the distillers analyze the semantic meaning of the output. 
   - Is this a loading spinner? *Drop it.* 
   - Is this a list of 500 passing tests? *Drop it.* 
   - Is this a panic stack trace? **Keep it.** 
   Meanwhile, the `guard` module ensures facts are preserved, guaranteeing that OMNI never silently alters critical diagnostic information.
4. **The Safety Net (`src/store`)**: What if the AI actually needed to see the 500 passing tests? OMNI follows a strict "Zero Information Loss" policy. Before any noise is discarded, the raw, unedited output is safely tucked away in a local, lightning-fast SQLite database (`Store`). OMNI leaves a small breadcrumb in the AI's context: `[OMNI: omitted 1,200 lines of noise. Use omni_retrieve to view]`.
5. **The Multi-Agent Interface (`src/mcp` & `src/session`)**: Finally, the distilled, high-signal output is returned to the AI. Behind the scenes, the `session` manager tracks the current token budget, while the `mcp` (Model Context Protocol) server stands ready. If the AI wants to query historical errors, fetch the omitted raw logs, or check the dependency graph (`src/graph`), the MCP tools provide instant, structured access.

**The Result:** A bloated `25,000` token terminal dump becomes a concise `400` token error report. The AI understands the problem instantly, and you save real money.

---

## Architecture

<div align="center">
  <img src="media/architecture.svg" alt="OMNI Architecture Diagram" width="100%" />
</div>

## Quick Start & Installation

Omni is incredibly easy to set up. It natively integrates into your terminal.

**macOS / Linux:**
```bash
# 1. Install via Homebrew
brew install fajarhide/tap/omni

# 2. Setup Omni (Interactive Menu for Claude, VS Code, OpenCode, Codex, Antigravity)
omni init

# 3. Verify it's working
omni doctor

# 4. Or auto-fix any issues
omni doctor --fix

# 5. Check Current Status
omni init --status
```

**Universal Installer (macOS / Linux / WSL):**
```bash 
curl -fsSL omni.weekndlabs.com/install | bash
```

**Windows (PowerShell):**
```powershell
irm omni.weekndlabs.com/install.ps1 | iex
```

---

## How to Use It

Once installed via `omni init`, OMNI works invisibly in the background. Whether your AI Agent runs a terminal command via MCP or you manually pipe output (`ls | omni`), OMNI automatically jumps in as a transparent layer. It intelligently filters terminal output, removes the noisy logs, and hands the clean signal back to the AI.

For detailed breakdown by savings, command, period, and route:
```bash
omni stats
```

To diagnose your OMNI installation (hooks, MCP, filters, database):
```bash
omni doctor
```

Need to see the filters in action or add your own custom rules?
You can easily create your own rules using simple TOML files in `~/.omni/signals/`.

### Multi-Agent Support & Integrations

By default, `omni init --claude` automatically hooks into **Claude Code**. However, OMNI works perfectly with any agentic AI through its built-in integrations! Run `omni init` to see the interactive menu.

1. **VS Code & Continue.dev**: Use our MCP context provider (`integrations/continue-dev/`).
2. **OpenCode & Codex CLI**: Built-in wrappers automatically pipe command output to OMNI.
3. **Antigravity IDE**: OMNI registers as a native MCP server in Antigravity's config (`~/.gemini/antigravity/mcp_config.json`). Run `omni init --antigravity` to set up automatically.
4. **Pi Agent**: Native OMNI package for Pi. Run `omni init --pi` to install the OMNI Pi package via Pi's package installer. Use Pi's slash commands to toggle the extension on or off.

**Multi-Agent Tuning (`~/.omni/config.toml`)**
Different agents have different pain points. Keep VS Code chat clean, whilst letting OpenCode read more data. Tune them individually:
```toml
[global]
aggressiveness = "balanced"

[agents.vscode_continue]
aggressiveness = "aggressive"
enable_readfile_distillation = true

[agents.opencode]
aggressiveness = "conservative"
enable_readfile_distillation = false
```

### Documentation Index

**For Users:**
- [The Ultimate Guide (HOW_TO_USE.md)](docs/HOW_TO_USE.md) — Everything you need: Installation, `omni learn`, Custom TOML Filters, and CLI Commands.
- [OpenClaw Integration](https://clawhub.ai/fajarhide/omni-signal-engine) — Official OpenClaw plugin for native OMNI distillation. Install: `openclaw plugins install clawhub:@fajarhide/omni-signal-engine`
- [Hermes Agent Integration](https://github.com/wysie/hermes-omni-plugin) — Community Hermes Agent plugin for native OMNI distillation. Install: `uv pip install --python ~/.hermes/hermes-agent/venv/bin/python git+https://github.com/wysie/hermes-omni-plugin.git`

**For Developers & System Integrators:**
- [Loop Engineering Guide (LOOP_ENGINEERING.md)](docs/LOOP_ENGINEERING.md) — How to integrate OMNI's context pressure with autonomous agent scripts (e.g., Maker-Checker pattern, shell loops).
- [Development Guide](docs/DEVELOPMENT.md) — How to build and contribute to the OMNI codebase.
- [Testing Architecture](docs/TESTING.md) — Quality assurance and context safety.
- [Session Continuity](docs/SESSION.md) — Deep dive into OMNI's working memory.
- [Roadmap](docs/ROADMAP.md) — Current development status and upcoming features.
- [Migration Guide](docs/MIGRATION.md) — Notes on upgrading from Node/Zig to the Rust version.

---

## Works Even Better with Heimsense

Omni is part of my personal AI toolbelt. If you use `claude-code`, I highly recommend pairing Omni with my other project: **[Heimsense](https://github.com/fajarhide/heimsense)**. 

Heimsense unlocks restricted environments like `claude-code` to run with *any* free or OpenAI-compatible model, rather than forcing you to use expensive Anthropic ones. 
**Omni + Heimsense** = Run world-class agent frameworks using affordable models with zero noise and pinpoint accuracy.

---

## Contributing & License

This is a passion project built for the era of Agentic AI. Whether you're here to save money on tokens, test out free models, or help build the ultimate agentic toolbelt, contributions are always welcome!

- **Development**: Want to build from source? Run `make ci` and `cargo build`. Read our [CONTRIBUTING.md](CONTRIBUTING.md) for details.
- **License**: [MIT License](LICENSE)

<!-- Star History -->
<p align="center">
  <a href="https://star-history.com/#fajarhide/omni&Date">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=fajarhide/omni&type=Date&theme=dark" />
      <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/svg?repos=fajarhide/omni&type=Date" />
      <img alt="Star History Chart" src="https://api.star-history.com/svg?repos=fajarhide/omni&type=Date" width="600" />
    </picture>
  </a>
</p>

Build with ❤️ by [Fajar Hidayat](https://github.com/fajarhide)