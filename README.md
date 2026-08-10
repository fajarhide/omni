<div align="center">
  <img src="media/logo.png" alt="OMNI Logo" width="300" />

<h1>OMNI</h1>
<p align="center">
    <em><b>Stop paying Claude to read 10,000 lines of terminal noise.</b> Over one developer's real week, OMNI cut 88% of build and test output and a quarter of everything the agent re-read, 15.7% across the whole mix. The other 97% of calls passed through untouched. Nothing is ever lost, and it never invents a result.</em>
</p>

[🇺🇸 English](README.md) | [🇯🇵 日本語](i18n/README-ja.md) | [🇨🇳 简体中文](i18n/README-zh.md) | [🇸🇦 العربية](i18n/README-ar.md) | [🇮🇩 Bahasa Indonesia](i18n/README-id.md) | [🇻🇳 Tiếng Việt](i18n/README-vi.md) | [🇰🇷 한국어](i18n/README-ko.md)

[![CI](https://github.com/fajarhide/omni/actions/workflows/ci.yml/badge.svg)](https://github.com/fajarhide/omni/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/fajarhide/omni)](https://github.com/fajarhide/omni/releases)
  [![Rust](https://img.shields.io/badge/built_with-Rust-dca282.svg)](https://www.rust-lang.org/)
  [![MCP](https://img.shields.io/badge/MCP-compatible-green.svg?style=flat-square)](https://modelcontextprotocol.io/)
  [![License: MIT](https://img.shields.io/github/license/fajarhide/omni)](https://github.com/fajarhide/omni/blob/main/LICENSE)
  [![Hits](https://hits.sh/github.com/fajarhide/omni.svg)](https://hits.sh/github.com/fajarhide/omni/)
</br></br>
<b>
build and test 88% &middot; file re-reads 25% &middot; 15.7% across the mix &middot; 21 ms per command &middot; 2 of 7,095 calls grew the output, and we say so &middot; every cut recoverable byte for byte &middot; cross-session memory </b>

</br></br>

```bash
brew install fajarhide/tap/omni && omni init
```

Distills command output on Claude Code, Codex CLI and Gemini CLI, where the host applies OMNI's rewrite. Everywhere else you get the MCP server, shared session state, and `omni_run`, which distils any command you route through it. Run `omni doctor` to see which tier each installed host is on.


### What each host lets OMNI do

| Tier | Hosts | What you get |
|---|---|---|
| **Full** | Claude Code, Codex CLI, Gemini CLI, Aider (pipe) | The host applies OMNI's rewrite, so the model reads distilled output from its own built-in tools. |
| **Handoff-first** | Cursor, Windsurf | The host cannot rewrite built-in tool output. `omni_run` distils anything you route through it, and `omni init --cursor` installs the rule that makes the agent reach for it. |
| **MCP-only** | Cline, Roo, OpenCode, VS Code, Zed, Copilot, Antigravity, Hermes, Pi | Memory, recall and session state. No shell distillation, and no claim of it. |

`omni doctor` prints the tier for every installed host. Savings are only ever counted where the model actually received less.

Codex CLI needs one extra step. It runs only hooks it has been told to trust, and skips the rest without a word, so after `omni init --codex` start `codex` once and approve them under "Hooks need review". `omni doctor` fails until you do. See [#359](https://github.com/fajarhide/omni/issues/359).
</br>
<img src="media/demo.gif" alt="OMNI distilling a noisy cargo test run down to the verdict, then omni stats" width="820" />
</div>

---

Your agent reads every line your terminal prints. Build logs, Docker logs, CI logs,
progress bars, ANSI colors. Thousands of tokens to find one line. Claude isn't
expensive. Your terminal is.

And it forgets all of it overnight. Restart Cursor, switch to Claude Code, and you
re-explain the project from scratch.

OMNI fixes both, and stays out of the way everywhere else.

---

## The same `git log`, side by side

Without OMNI, one commit's `Author` / `Date` / body already fills the screen. With
OMNI, **every commit is kept**, as one `hash subject` line, 94% smaller. Nothing is
summarised away.

<table>
<tr>
<td align="center"><b>Without OMNI</b><br/><sub>raw <code>git log -15</code></sub></td>
<td align="center"><b>With OMNI</b><br/><sub>every commit kept, 94% smaller</sub></td>
</tr>
<tr>
<td valign="top"><img src="media/demo-git-without.gif" alt="a raw verbose git log -15: one commit's Author, Date and body fill the screen" width="400" /></td>
<td valign="top"><img src="media/demo-git-with.gif" alt="the same git log -15 through OMNI: every commit as a compact hash + subject line, 94% smaller" width="400" /></td>
</tr>
</table>

| Command | Without OMNI | With OMNI | Saved |
|---|---|---|---|
| `cargo test` (490 passed, 10 failed) | 16.5 KB of per-test output | the runner's own pass/fail summary | **92.9%** |
| `git status` (dirty) | 496 B of porcelain | the branch and the changed paths | **61.7%** |
| `docker build` (heavy cache noise) | 9.2 KB of layer hashes and progress bars | the build result, cache hits folded | **35.9%** |
| `git diff` (multi-file) | lockfiles, whitespace, generated churn | the code that actually changed | **25.2%** |
| `kubectl get pods` (35 pods, 5 crashing) | the full table | the full table | **0%**, by design |

> **Where it does nothing, on purpose.** A command that fails is passed through
> verbatim, because a hidden error costs more than an uncompressed one. Structured
> output (JSON, YAML, CSV) is never touched, because the next step in your pipeline
> is going to parse it. That is what makes it safe to leave on for every command you
> run.

---

## Nothing is ever lost. It never makes something up.

Two promises, and both are in the code rather than in this paragraph.

**Nothing is ever lost.** Every byte OMNI cuts is archived locally in the
RewindStore, keyed by SHA-256. The agent gets a hash with the distilled output and
can call `omni_retrieve` to pull the original back byte for byte, mid-conversation,
without re-running your command.

**It never makes something up.** A distiller that recognises nothing in its input
returns the raw input. That is a type, not a convention: `distill` returns
`Option<String>` and the routing layer falls back to the original whenever it gets
`None`. There is no code path that produces a green "no errors" line OMNI did not
read.

| Guarantee | How | Proof |
|---|---|---|
| **Get the original back, byte-for-byte** | everything cut is archived in a local SQLite RewindStore; the agent gets a hash and calls `omni_retrieve` | [Architecture](docs/ARCHITECTURE.md) |
| **Never fabricates a result** | a distiller that parsed no signal returns the raw output, never a green `no errors` / `passed` string | [#143](https://github.com/fajarhide/omni/issues/143) |
| **Failures are never masked** | a command that exits non-zero passes through verbatim | [#120](https://github.com/fajarhide/omni/issues/120) |
| **Structured data is never touched** | JSON / YAML / NDJSON / CSV pass through byte-for-byte | `pipeline::format` |

---

## Benchmarks

Measured on the release binary by replaying **7,095 real command executions**
covering **2026-08-03 to 08-10 UTC**, every one of them output that reached a model.
The window is part of the figure: `execution_traces` is pruned after seven days, so
a corpus is gone a week after it is measured.

* **Where there is noise, the filters take almost all of it.** Build and test
  output is 87.9%, and 92.3% once the session ledger is counted. Where there is no
  noise they take nothing, and a `kubectl get pods` table is 0%, because every row
  in it is a datum.
* **The ledger reaches what filtering cannot.** File re-reads are the largest class
  at 1.54 MB, the filters take 0.0% of them, and handing back lines the agent has
  already been shown takes 24.6%.
* **97.1% of calls saved nothing at all** and handed the output straight back.
  Every byte of the saving comes from the other 2.9%.
* **2 calls of 7,095 came back larger**, reported rather than rounded away
  ([#398](https://github.com/fajarhide/omni/issues/398)).
* **15.7% fewer bytes** across the whole mix, of which the filters are 5.2% and the
  ledger is the rest. Counted in tokens the filters alone are 5.0%.
* **21 ms per command** end to end, growing with your history rather than with the
  payload. On a 205 MB database it is 61 ms.

<div align="center">
<img src="https://omni.weekndlabs.com/media/performance.png" alt="OMNI" width="600" />
</div>

Full corpus, per-class breakdown, fixtures and latency tables:
**[docs/BENCHMARKS.md](docs/BENCHMARKS.md)**. Reproduce them with
`cargo test --release --test bench_replay -- --ignored`.

### How to read a savings number, including ours

Every tool in this category publishes a percentage. Here are the five questions that
decide whether it means anything, and our answers:

| Question | Why it matters | OMNI |
|---|---|---|
| What share of calls saved **nothing**? | A tool that saves on every command is summarising output you needed | **97.1%**, published |
| Did any call make the output **larger**? | Markers and headers cost bytes; nobody reports the ones that backfire | **2 of 7,095**, and they have an issue number |
| Which **population** was measured? | Counting terminal bytes no model reads inflates the number for free | model-facing only, which cost 36 points the last time a corpus carried terminal rows |
| Can you **re-run** it? | A number you cannot reproduce is a claim, not a measurement | one command, on your own data |
| Is the cut **recoverable**? | Lossy is fine when it is reversible, and fatal when it is not | byte for byte, via `omni_retrieve` |

We publish the share of calls where we did nothing because it is the number that
tells you what the rest are worth.

---

## Install

**macOS / Linux:**
```bash
brew install fajarhide/tap/omni
omni init      # interactive setup for Claude, Cursor, VS Code, Codex, Antigravity
omni doctor    # verify, or `omni doctor --fix`
```

**Universal (macOS / Linux / WSL):**
```bash
curl -fsSL omni.weekndlabs.com/install | bash
```

**Windows (PowerShell):**
```powershell
irm omni.weekndlabs.com/install.ps1 | iex
```

Then run your commands normally. There is nothing to prefix and no proxy to wrap.

---

## FAQ

**Does OMNI permanently delete my logs?**
No. Raw logs are stored locally in the SQLite RewindStore. The agent receives a hash
and can retrieve the full log at any time.

**Will this slow down my terminal?**
Measurably, yes, and the cost grows with your history rather than with the payload. A
496-byte `git status` takes about 21 ms against a fresh database and 61 ms against a
205 MB one. Budget for it. `OMNI_PASSTHROUGH=1` skips the pipeline entirely.

**Can I add my own filters?**
Yes, in TOML:
```toml
# ~/.omni/signals/custom.toml
[filters.my_tool]
match_command = "^internal-tool\\b"
strip_lines_matching = ["^DEBUG", "syncing..."]
```

**How do I see my own savings?**
`omni stats` after a few days. `omni stats --share` prints a copy-pasteable summary
of the same figures.

---

## Learn more

* [How it works, and what it costs](docs/ARCHITECTURE.md): pipeline, RewindStore, the Memory OS
* [Benchmarks in full](docs/BENCHMARKS.md): corpus, per-command, fixtures, latency
* [Contributing](CONTRIBUTING.md): `make ci` and you're in

---

```bash
brew install fajarhide/tap/omni && omni init
```

A passion project for the era of agentic AI. Contributions welcome.
[MIT License](LICENSE).

<p align="center">
  <a href="https://star-history.com/#fajarhide/omni&Date">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=fajarhide/omni&type=Date&theme=dark" />
      <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/svg?repos=fajarhide/omni&type=Date" />
      <img alt="Star History Chart" src="https://api.star-history.com/svg?repos=fajarhide/omni&type=Date" width="600" />
    </picture>
  </a>
</p>

<p align="center">Built with care by <a href="https://github.com/fajarhide">Fajar Hidayat</a></p>
