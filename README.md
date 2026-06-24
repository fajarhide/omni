<div align="center">
  <img src="media/hero.svg" alt="OMNI" width="800" />



  ---
  <p align="center">
    <em>Your AI isn't bad. It's drowning.</em>
  </p>
  
  **Up to 85% less tokens &middot; ~40% faster &middot; ~60% cheaper &middot; Zero hallucination triggers**<br>

  [🇺🇸 English](README.md) | [🇯🇵 日本語](i18n/README-ja.md) | [🇨🇳 简体中文](i18n/README-zh.md) | [🇸🇦 العربية](i18n/README-ar.md) | [🇮🇩 Bahasa Indonesia](i18n/README-id.md) | [🇻🇳 Tiếng Việt](i18n/README-vi.md) | [🇰🇷 한국어](i18n/README-ko.md)

  [![CI](https://github.com/fajarhide/omni/actions/workflows/ci.yml/badge.svg)](https://github.com/fajarhide/omni/actions/workflows/ci.yml)
  [![Release](https://img.shields.io/github/v/release/fajarhide/omni)](https://github.com/fajarhide/omni/releases)
  [![Rust](https://img.shields.io/badge/built_with-Rust-dca282.svg)](https://www.rust-lang.org/)
  [![MCP](https://img.shields.io/badge/MCP-compatible-green.svg?style=flat-square)](https://modelcontextprotocol.io/)
  [![License: MIT](https://img.shields.io/github/license/fajarhide/omni)](https://github.com/fajarhide/omni/blob/main/LICENSE)
  [![Hits](https://hits.sh/github.com/fajarhide/omni.svg)](https://hits.sh/github.com/fajarhide/omni/)
</div>


Every AI coding assistant has the same problem.

They read everything.

Build logs.  
Docker logs.  
CI logs.  
Progress bars.  
Warnings.  
ANSI colors.  
Repeated output.  

Thousands of tokens... to find one line.

Claude isn't expensive. Your terminal is. One failed `npm install` can waste more tokens than the code you're trying to write.

OMNI fixes that.

---

## The Difference

### `npm install`
**Without Omni:** 10,000 lines of "Downloading...", "Extracting...", and warnings. AI reads everything.  
**With Omni:** Package conflict. Node 20 required.

### `terraform apply`
**Without Omni:** 4,500 lines of unchanged execution plans.  
**With Omni:** The 3 resources that failed IAM permissions.

### `docker build`
**Without Omni:** Endless cache hits, layer hashes, and download progress bars.  
**With Omni:** Missing dependency `libpq-dev` at layer 12.

### `pytest`
**Without Omni:** 500 passing tests and verbose setup logs.  
**With Omni:** Only the 2 failed assertions and their stack traces.

### `git diff`
**Without Omni:** Formatting tweaks, generated lockfiles, and whitespace changes.  
**With Omni:** Only the core business logic changes.

### `kubectl logs`
**Without Omni:** Thousands of successful health checks and normal traffic logs.  
**With Omni:** The crash loop and panic stack trace.

### `cargo build`
**Without Omni:** 300 lines of compiling dependencies and warnings.  
**With Omni:** The exact line where the borrow checker failed.

### `go test`
**Without Omni:** Pages of standard output from passing packages.  
**With Omni:** The single nil pointer dereference.

### `mvn package`
**Without Omni:** Megabytes of "Downloading from maven central".  
**With Omni:** Compilation error in `UserService.java`.

### `pip install`
**Without Omni:** Resolution logs and wheel building outputs.  
**With Omni:** Dependency conflict with `numpy`.

### `webpack / vite`
**Without Omni:** 2,000 chunk asset lists and build times.  
**With Omni:** Missing module resolution in `App.tsx`.

### `git merge`
**Without Omni:** 50 files listed with fast-forward stats.  
**With Omni:** The exact files with unresolved merge conflicts.

### `helm install`
**Without Omni:** Entire rendered YAML output of all templates.  
**With Omni:** Pod scheduling failure due to missing secret.

### `ansible-playbook`
**Without Omni:** "ok" and "skipped" statuses for 50 servers.  
**With Omni:** The single "failed" task on `web-03`.

### GitHub Actions (CI/CD)
**Without Omni:** Complete workflow logs including environment setup.  
**With Omni:** Only the specific step that exited with code 1.

---

## Why this matters

The code you *don't* send to the AI is just as important as the code you do.

When you feed an AI megabytes of terminal noise, it suffers from context bloat. It gets distracted, hallucinates fixes for the wrong warnings, and burns through your API budget.

OMNI sits invisibly between your terminal and your AI. It intercepts the raw output, drops the noise, and hands your agent the pure signal.

* You save money.
* The AI responds instantly.
* The hallucinations disappear.

---

## Benchmarks

Because OMNI removes the noise before the AI even sees it, the impact is immediate:

* **Token Reduction:** 70% to 90% less tokens per command.
* **Speed:** ~40% faster Time-To-First-Token (TTFT).
* **Cost:** ~$35 USD saved per developer/month against flagship models.
* **Accuracy:** Higher first-try resolution rates because the AI is focused.

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

## Integrations

OMNI works seamlessly with the agentic tools you already use. It intercepts their terminal executions automatically.

* Claude Code
* Cursor
* Windsurf
* Roo Code
* OpenAI Codex
* Antigravity CLI

---

## How it works

Omni operates purely locally using a deterministic `Read → Guard → Score → Collapse → Distill → Persist` pipeline.

```mermaid
flowchart LR
    Command[Raw Tool Output] --> Hook[Omni Hook]
    Hook --> Score[Scorer Engine]
    Score -->|Critical=1.0, Noise=0.1| Distill[Content Distiller]
    Distill --> Clean[Clean Context]
    Command --> SQLite[(RewindStore SQLite)]
```

If the AI *really* needs the dropped noise, OMNI's local SQLite **RewindStore** keeps the full uncompressed log safely hashed, allowing the agent to retrieve it anytime.

---

## Architecture

Built in Rust for imperceptible latency.

* **Pipeline Latency**: < 10ms overhead.
* **Memory**: Operates via efficient streams, keeping memory usage flat even on 20,000-line logs.
* **Fail Open**: If OMNI panics, it fails silently and passes the raw output through. It will never crash your host agent.

```bash
# Development
cargo build --release
cargo test --all
make fmt && make clippy
```

---

## FAQ

**Does Omni permanently delete my logs?**  
No. The raw logs are compressed and stored locally in the SQLite RewindStore. The AI receives a hash and can retrieve the full log if needed.

**Will this slow down my terminal?**  
No. OMNI is written in Rust and executes the distillation pipeline in under 10ms.

**Can I add my own filters?**  
Yes. You can teach OMNI to strip noise specific to your internal tools using TOML:
```toml
# ~/.omni/signals/custom.toml
[filters.my_tool]
match_command = "^internal-tool\\b"
strip_lines_matching = ["^DEBUG", "syncing..."]
```

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