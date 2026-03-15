<p align="center">
  <img src="logo.png" alt="OMNI - The Semantic Core" width="300" />
</p>

<h1 align="center">OMNI: The Semantic Core for the Agentic Era</h1>

<p align="center">
  <a href="https://github.com/fajarhide/omni/actions"><img src="https://github.com/fajarhide/omni/workflows/CI/badge.svg" alt="CI"></a>
  <a href="https://github.com/fajarhide/omni/releases"><img src="https://img.shields.io/github/v/release/fajarhide/omni" alt="Release"></a>
  <a href="https://opensource.org/licenses/MIT"><img src="https://img.shields.io/badge/License-MIT-yellow.svg" alt="License: MIT"></a>
</p>

<p align="center">
  <strong>The first MCP-native semantic distillation engine</strong><br>
  that transforms chaotic CLI output into pure, high-density intelligence for LLMs.<br>
  Eliminates <strong>30–90% of token noise</strong> — powered by Zig, portable via Wasm.
</p>

---

## Why OMNI

AI agents running on **Model Context Protocol (MCP)** are only as smart as the context they receive. When Claude runs `git diff`, `docker build`, or `npm install`, it drowns in hundreds of redundant lines it will never use — burning your context window and slowing down every response.

**OMNI is the missing layer.** It sits as an MCP server between your agent and the world, intercepting tool output and distilling it to pure signal — automatically, with zero config.

- **30–90% token reduction** across Git, Docker, SQL, Node, and Build tool outputs
- **< 1ms filter latency** — powered by Zig 0.15.2, no GC, no overhead
- **68KB Wasm footprint** — runs anywhere from your local terminal to edge runtimes
- **MCP-first design** — native integration with Claude Code, Cursor, Windsurf, and any MCP agent
- **Zero config** — pipe any CLI output through OMNI and it just works


---

## CLI Subcommands: Unified Intelligence

OMNI provides a powerful, multi-purpose CLI that consolidates all diagnostic and reporting tools:

| Subcommand | Purpose |
| :--- | :--- |
| **`distill`** | The core semantic engine (default behavior via stdin). |
| **`density`** | Analyzes context gain and "Information per Token" metrics. |
| **`report`** | Generates a unified system status and performance summary. |
| **`bench`** | High-speed benchmark for semantic throughput. |
| **`generate`** | Outputs templates for Claude Code, Antigravity, and others. |
| **`setup`** | Interactive guide for integration and standard aliasing. |

---

## How OMNI Works

OMNI sits between your AI agent and the outside world — silently distilling chaotic output into pure, high-density signal.

```
                         OMNI SEMANTIC PIPELINE
  ─────────────────────────────────────────────────────────────

   Your Tool Output
  ┌──────────────────┐
  │  git diff        │   (noisy, verbose, 600+ tokens)
  │  docker build    │
  │  npm install     |
  |  etc             │
  └────────┬─────────┘
           │ stdin pipe
           ▼
  ┌───────────────────────────────────────────────────────────┐
  │                    OMNI MCP SERVER                        │
  │                                                           │
  │   ┌─────────────┐     ┌─────────────────────────────┐     │
  │   │ LRU Cache   │────▶│  Filter Engine (Zig + Wasm) │     │
  │   │  < 1ms hit  │     │  Git · SQL · Docker · Node  │     │
  │   └─────────────┘     └────────────┬────────────────┘     │
  │                                    │ Semantic Distill     │
  │             ┌──────────────────────▼──────────────────┐   │
  │             │  Pure Signal  (30–90% token reduction)  │   │
  │             └──────────────────────┬──────────────────┘   │
  └──────────────────────────────────  │ ─────────────────────┘
                                       │
                                       ▼
                          ┌────────────────────────┐
                          │   AI Agent (Claude)    │
                          │   sees only signal,    │
                          │   zero noise           │
                          └────────────────────────┘

```
No filter match → passthrough unchanged (zero overhead)
---

## The OMNI Effect

**Before OMNI** (LLM sees 600+ tokens of noise):
```
$ docker build .
Step 1/15 : FROM node:18
 ---> 4567f123
Step 2/15 : RUN npm install
... (500 lines of noise) ...
Successfully built 1234abcd
```

**After OMNI Distillation** (LLM sees 15 tokens of signal):
```
Step 1/15 : FROM node:18
Step 2/15 : RUN npm install (CACHED)
Step 3/15 : COPY . .
Successfully built!
```

That's **98% fewer tokens**. The LLM gets the same signal — all builds pass — without the noise.

---

## Integration: Using OMNI Everywhere

OMNI is a standard **Model Context Protocol (MCP)** server.

### Claude Code & Claude CLI
The OMNI CLI is for humans, but **`omni-mcp`** is for your AI. It allows Claude or Antigravity to use OMNI's distillation tools automatically.

To register OMNI as an MCP server for Claude Code automatically, run:
```bash
omni generate claude-code
```
This command will automatically detect your absolute home path and register OMNI with Claude Code.

Verify with:
```bash
claude mcp list
```

### Antigravity (Google)
Simply run the automatic generator from the terminal:
```bash
omni generate antigravity
```
*This command will automatically locate your `~/.gemini/antigravity/mcp_config.json`, safely merge OMNI's configurations into your existing servers without overwriting them, and save the file.*

### Auto-Generate Config
Use the CLI to generate ready-to-paste configurations:
```bash
omni generate claude-code    # For Claude Code / Claude CLI
omni generate antigravity     # For Google Antigravity
omni setup                    # Full interactive guide
```

---

## The Power Comparison: Precise Intelligence

| Feature | **OMNI** | RTK | Snip | Serena |
| :--- | :--- | :--- | :--- | :--- |
| **Language** | **Zig + Wasm** | Rust | Go | Python |
| **Philosophy** | **Semantic Distillation** | Tool Proxying | YAML Pipelines | IDE-like Retrieval |
| **Latency** | **< 1ms** | ~10ms | ~10ms | ~50ms+ |
| **Filter Type** | **Hardcoded (Fast)** | Hardcoded | Declarative YAML | LSP / Semantic |
| **Deployment** | **Edge (68KB Wasm)** | Native Binary | Static Binary | Python Pkg (uv) |
| **Memory** | **Manual (Zero GC)** | ARC | GC | GC |

### Why OMNI Wins:
1.  **Context IQ**: OMNI doesn't just shorten text; it *re-writes* it semantically for the LLM.
2.  **Performance Supremacy**: By using a persistent Wasm instance, OMNI is up to **50x faster** than traditional CLI tools.
3.  **Universal Deployment**: The only tool that runs as a single Wasm file on any edge runtime.

---

## Visualizing Efficiency

1.  **The "Distillation" Effect**: In your AI's tool output, raw logs are transformed into a 10-line summary.
2.  **Faster Response Times**: LLM processes 150x fewer tokens, giving you significantly faster replies.
3.  **Real-time Reports**: Run `omni report` at any time to see the global efficiency health.
4.  **Density Metrics**: Use `omni density < logs.txt` to calculate your exact Context Density Gain.

---

## Installation

### Homebrew (Recommended)
```bash
brew install fajarhide/tap/omni
```

### One-Line Installer
```bash
curl -fsSL https://raw.githubusercontent.com/fajarhide/omni/main/install.sh | sh
```

For manual build instructions, see **[INSTALL.md](INSTALL.md)**.

---

## License
MIT © Fajar Hidayat
