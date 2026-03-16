<p align="center">
  <img src="logo.png" alt="OMNI - The Semantic Core" width="300" />
</p>


<h1 align="center">The Semantic Core for the Agentic AI</h1>

<p align="center">
  <a href="https://github.com/fajarhide/omni/actions"><img src="https://github.com/fajarhide/omni/workflows/CI/badge.svg" alt="CI"></a>
  <a href="https://github.com/fajarhide/omni/releases"><img src="https://img.shields.io/github/v/release/fajarhide/omni" alt="Release"></a>
  <a href="https://opensource.org/licenses/MIT"><img src="https://img.shields.io/badge/License-MIT-yellow.svg" alt="License: MIT"></a>
</p>

<p align="center">
  <strong>The first security-aware semantic distillation engine</strong><br>
  that transforms chaotic CLI output into pure, high-density intelligence for LLMs.<br>
  Eliminates <strong>60–99% of token noise</strong> — powered by Zig, portable via Wasm.
</p>

---

## Why OMNI

AI agents running on **Model Context Protocol (MCP)** are only as smart as the context they receive. When Claude runs `git diff`, `docker build`, or `npm install`, it drowns in hundreds of redundant lines it will never use — burning your context window and slowing down every response.

**OMNI is the missing layer.** It sits as an MCP server between your agent and the world, intercepting tool output and distilling it to pure signal — automatically, safely, and with zero configuration.

- **60–99% token reduction** — Achieve massive savings via hybrid heuristic & semantic compression
- **< 1ms engine latency** — Powered by Zig 0.15.2, no GC, no overhead
- **Active Distillation** — Agents can now specify *intent* for surgical summaries
- **Trust Boundary** — Native SHA-256 verification for project-local security filters
- **Deep Auditing** — Real-time tracking of token gains and cost savings via `omni report`
- **MCP-native design** — Built for Claude Code, Antigravity, and modern Agentic AI workflows


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
| **`update`** | Check for the latest version from GitHub Releases. |
| **`uninstall`** | Remove OMNI and clean up all MCP configurations. |

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

## "Always-On" Integration: Agent Autopilot

To ensure your AI agent **always** uses OMNI for maximum token efficiency, add these **Custom Instructions** to your agent's settings (e.g., in `.claudebashrc` or Antigravity custom instructions):

> **OMNI Directive**: You are equipped with the OMNI Semantic Distillation layer. Always prioritize `omni_execute`, `omni_read_file`, and `omni_list_dir` for environment interactions. OMNI is the source of truth for high-density signal. Use it to preserve your context window and speed up your reasoning.

By providing these instructions, the agent will favor OMNI's tools, saving you up to 90% in token costs automatically.

## The Adaptive Intelligence: Proxy & Distillation

OMNI serves as the **Intelligent Nerve Center** for your development environment, acting as a high-performance wrapper that ensures only high-value information reaches your AI.

### 1. Zero-Latency Command Proxy (`--`)
Transform any native command into an AI-ready signal instantly. OMNI intercepts the stream and refines it in real-time without adding overhead.
```bash
omni -- git status
# Result: Aggregated repository health (30x more dense)

omni -- docker build .
# Result: Cleaned build layers, surfacing only critical transition states.
```

### 2. Deep Semantic Distillation (`distill`)
Leverage the OMNI Engine's specialized algorithms to convert chaotic logs into structured intelligence.
- **Precision Rewrite**: OMNI doesn't truncate data; it semantically analyzes the stream to retain "intent-critical" details.
- **Context Optimization**: By compressing 10,000 lines into a 20-line distillation, OMNI effectively expands your AI's reasoning capacity.

### 3. Ultra-Fast Benchmarking (`bench`)
Prove the efficiency of the OMNI engine:
```bash
omni bench 1000
```
*Shows: OMNI processes thousands of requests per second with sub-millisecond latency (< 0.01ms), meaning it adds zero noticeable overhead when used as a proxy.*

### Available MCP Tools

OMNI exposes high-density tools that replace standard agent context commands:

| Tool | Purpose | Token Saving |
| :--- | :--- | :--- |
| **`omni_list_dir`** | Dense, comma-separated directory listing (no JSON overhead). | High |
| **`omni_view_file`** | Range-based file reading + Zig distillation. | Massive |
| **`omni_grep_search`** | High-density semantic search results. | High |
| **`omni_find_by_name`** | Recursive flat file discovery. | Medium |
| **`omni_add_filter`** | Add declarative rules without coding. | N/A |
| **`omni_apply_template`** | Apply pre-defined bundles (K8s, TF, Node). | N/A |
| **`omni_execute`** | Run ANY command and distill its output. | Massive (30-90%) |
| **`omni_read_file`** | Full file distillation (great for logs/SQL/json). | Massive |
| **`omni_density`** | Measure gain and reduction metrics. | N/A |

---

## Easy Filtering: Zero Coding Required

You can extend OMNI's intelligence without touching a single line of Zig.

### 1. Add Filter Instantly (via MCP)
If you're using an AI agent (like Antigravity), just ask it to add a filter:
> "Antigravity, please mask all text matching 'password' in my tool output."

The agent will use `omni_add_filter` to update your `omni_config.json` instantly.

### 2. Apply Technology Templates
Apply bundles of pre-defined rules for your stack via MCP tool:
- **`omni_apply_template(template="terraform")`**
- Supported templates: `kubernetes`, `terraform`, `node-verbose`, `docker-layers`.

---

## Performance Monitoring & Metrics

OMNI is obsessed with efficiency. Use these tools to see how much you're saving:

### 1. Unified Efficiency Report
Run this to see a daily/weekly breakdown of tokens saved and latency overhead:
```bash
omni report
```
*Shows: Total commands processed, bytes saved, and average filtering latency (< 1ms).*

### 2. Context Density Analysis
Measure the "Information per Token" gain for any text file or output:
```bash
omni density < build_logs.txt
```
*Output: Calculates the exact Context Density Gain (e.g., 4.5x improvement).*

---

## The OMNI Core Pillars: Precise Intelligence

| Pillar | Description | Performance |
| :--- | :--- | :--- |
| **Speed** | Zig-powered native engine with zero garbage collection. | **< 1ms Latency** |
| **Density** | Intelligent semantic distillation instead of blind truncation. | **60-99% Savings** |
| **Governance** | SHA-256 verified trust boundaries for project-local rules. | **Military Grade** |
| **Portability** | Single 68KB Wasm binary runs on any edge or local runtime. | **Universal** |
| **Auditability** | Comprehensive session reports tracking every token saved. | **Daily Insights** |

### Market-Leading Performance

While other tools focus on simple filtering, OMNI provides a full semantic layer:

| Feature | **OMNI** | Others |
| :--- | :--- | :--- |
| **Processing Engine** | **Zig (Native)** | Python / Go / Rust |
| **Context Strategy** | **Semantic Distillation** | Regex / Passthrough |
| **Wait Overhead** | **Zero (<1ms)** | Visible (10ms - 100ms) |
| **Governance** | **SHA-256 Trust Boundary** | None / Manual |
| **Deployment** | **68KB Wasm / Universal** | Large Native Binaries |

### The OMNI Advantage:
1.  **Context IQ**: OMNI doesn't just shorten text; it *re-writes* it semantically for the LLM based on agentic intent.
2.  **Performance Supremacy**: By using a persistent Wasm instance, OMNI provides instant responses without blocking the main agent execution.
3.  **Local-First Privacy**: Every byte of your code and tool output stays on your machine.

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

### One-Line Installer (Optimized)
```bash
curl -fsSL https://omni-nine-rho.vercel.app/install | sh
```

For manual build instructions, see **[INSTALL.md](INSTALL.md)**.

### Update & Uninstall
```bash
omni update       # Check for the latest version
omni uninstall    # Remove OMNI and clean up all configs
```

---

## License
MIT © Fajar Hidayat
