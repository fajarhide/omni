<div align="center">
  <img src="media/logo.png" alt="OMNI Logo" width="300" />

<h1>OMNI</h1>
<p align="center">
    <em><b>Stop paying Claude to read 10,000 lines of terminal noise.</b> OMNI cuts <code>git</code> by 89%, <code>cargo</code> by 91% and <code>kubectl</code> by 77% before your agent ever sees them. Everything else passes through untouched. Nothing is ever lost, and it never invents a result.</em>
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
<code>git</code> 89% &middot; <code>cargo</code> 91% &middot; <code>kubectl</code> 77% &middot; 21 ms per command &middot; 0 of 9,965 calls ever grew the output &middot; every cut recoverable byte for byte &middot; cross-session memory </b>

</br></br>

```bash
brew install fajarhide/tap/omni && omni init
```

Distills command output on Claude Code. Installs hooks, the MCP server and shared session state on Cursor, Windsurf, Codex and Roo, where rewriting depends on the host: Cursor does not let a hook replace built-in tool output.

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

Measured on the release binary by replaying **9,965 real command executions** from
one developer's actual usage:

* **On the commands that actually generate noise, 76 to 91%.** `cargo` 91.4%,
  `git` 89.2%, `kubectl` 76.5%. That is where your context budget goes, and that
  is where OMNI works.
* **OMNI acts on 1 command in 10, and adds zero bytes to the other 9.** It is a
  filter, not a summariser. When there is nothing to cut it gets out of the way
  completely.
* **Not one call in 9,965 made the output larger.**
* **43.3% fewer bytes** across the entire mix, noisy and quiet commands together.
* **21 ms per command** end to end, growing with your history rather than with the
  payload. On a 205 MB database it is 61 ms.

<div align="center">
<img src="https://omni.weekndlabs.com/media/performance.png" alt="OMNI" width="600" />
</div>

Full corpus, per-command breakdown, fixtures and latency tables:
**[docs/BENCHMARKS.md](docs/BENCHMARKS.md)**. Reproduce them with
`cargo test --release --test bench_replay -- --ignored`.

### How to read a savings number, including ours

Every tool in this category publishes a percentage. Here are the five questions that
decide whether it means anything, and our answers:

| Question | Why it matters | OMNI |
|---|---|---|
| What share of calls saved **nothing**? | A tool that saves on every command is summarising output you needed | **90.0%**, published |
| Did any call make the output **larger**? | Markers and headers cost bytes; nobody reports the ones that backfire | **0 of 9,965** |
| Which **population** was measured? | Counting terminal bytes no model reads inflates the number for free | model-facing only, and saying so costs us 36 points |
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
