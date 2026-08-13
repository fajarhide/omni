<div align="center">
  <img src="media/logo.png" alt="OMNI Logo" width="300" />

<h1>OMNI</h1>
<p align="center">
    <em><b>Stop paying to re-read the same output.</b> OMNI turns repeated bytes into retrievable handles: 97.2% off a file your agent reads twice, 14.9% across 6,656 real commands. Nothing deleted, nothing invented, and every number replays on your own corpus.</em>
</p>

[🇺🇸 English](README.md) | [🇯🇵 日本語](i18n/README-ja.md) | [🇨🇳 简体中文](i18n/README-zh.md) | [🇸🇦 العربية](i18n/README-ar.md) | [🇮🇩 Bahasa Indonesia](i18n/README-id.md) | [🇻🇳 Tiếng Việt](i18n/README-vi.md) | [🇰🇷 한국어](i18n/README-ko.md)

[![CI](https://github.com/fajarhide/omni/actions/workflows/ci.yml/badge.svg)](https://github.com/fajarhide/omni/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/fajarhide/omni)](https://github.com/fajarhide/omni/releases)
  [![Rust](https://img.shields.io/badge/built_with-Rust-dca282.svg)](https://www.rust-lang.org/)
  [![MCP](https://img.shields.io/badge/MCP-compatible-green.svg?style=flat-square)](https://modelcontextprotocol.io/)
  [![Discord](https://img.shields.io/badge/Discord-join%20the%20server-5865F2?logo=discord&logoColor=white)](https://discord.gg/zHTuvZhF2M)
  [![License: Apache 2.0](https://img.shields.io/github/license/fajarhide/omni)](https://github.com/fajarhide/omni/blob/main/LICENSE)
  [![Hits](https://hits.sh/github.com/fajarhide/omni.svg)](https://hits.sh/github.com/fajarhide/omni/)
</br></br>

```bash
brew install fajarhide/tap/omni && omni init
```

</br>
<img src="media/demo.gif" alt="OMNI distilling a noisy cargo test run down to the verdict, then omni stats" width="820" />
</div>

---

## What it does

**Drops the noise.** Build logs, Docker layer hashes, progress bars, ANSI colour. The
part of the output nobody reads is removed before it reaches the model.

**Stops re-sending what the agent has already seen.** A run of lines it was shown
earlier in the session comes back as one marker with a handle, not as the bytes again.
This is the half a filter cannot do: it removes bytes because they are already in the
context, not because a pattern calls them noise.

**Remembers across sessions.** Restart your editor or switch agents, and the project
context is still there.

**Gets out of the way.** A failing command passes through verbatim. JSON, YAML and CSV
are never touched. Most commands are handed back unchanged, and that is the intended
behaviour rather than a gap.

---

## The second read is free

An agent re-reads the same file constantly. Without OMNI it pays for every byte
again. With OMNI the second read is one marker carrying a handle, because those bytes
are already in its context, and `omni retrieve <hash>` hands the file back in full if
it is ever needed.

<table>
<tr>
<td align="center"><b>Without OMNI</b><br/><sub><code>cat</code> twice: the same 7.6 KB, twice</sub></td>
<td align="center"><b>With OMNI</b><br/><sub>second read 214 B, <b>97.2%</b> smaller</sub></td>
</tr>
<tr>
<td valign="top"><img src="media/demo-ledger-without.gif" alt="the same file read twice with no OMNI: two identical screens of source" width="400" /></td>
<td valign="top"><img src="media/demo-ledger-with.gif" alt="the same file read twice through OMNI: the second returns one marker line and a retrieval handle" width="400" /></td>
</tr>
</table>

This is the half a filter cannot reach. No pattern in that file is noise, so nothing
in it can be dropped on its own merits. It goes because the agent has already seen it.

## The same `git log`, side by side

Without OMNI, one commit's `Author` / `Date` / body already fills the screen. With
OMNI, **every commit is kept**, as one `hash subject` line. Nothing is summarised away.

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

Measured on the fixtures in `tests/fixtures/`, so you can reproduce any row:

| Command | Without OMNI | With OMNI | Saved |
|---|---|---|---|
| `cargo test` (490 passed, 10 failed) | 16.5 KB of per-test output | the runner's own pass/fail summary | **92.9%** |
| `git status` (dirty) | 496 B of porcelain | the branch and the changed paths | **61.7%** |
| `docker build` (heavy cache noise) | 9.2 KB of layer hashes and progress bars | the build result, cache hits folded | **35.9%** |
| `git diff` (multi-file) | lockfiles, whitespace, generated churn | the code that actually changed | **25.2%** |
| `kubectl get pods` (35 pods, 5 crashing) | the full table | the full table | **0%**, by design |

That last row is the point of the table. A pod listing is an enumeration where every
row is a datum, so there is nothing to drop, and OMNI reports nothing rather than
inventing a saving.

---

## Nothing is ever lost. It never makes something up.

Four guarantees, each one a link to the code or the issue that made it true rather
than a sentence asking you to trust it.

| Guarantee | How | Proof |
|---|---|---|
| **Get the original back, byte-for-byte** | everything cut is archived in a local SQLite RewindStore; the marker carries a handle, and `omni retrieve <handle>` prints it on any host, with the `omni_retrieve` MCP tool where MCP is wired | [#388](https://github.com/fajarhide/omni/issues/388) |
| **Never fabricates a result** | a distiller that parsed no signal returns the raw output, never a green `no errors` / `passed` string | [#143](https://github.com/fajarhide/omni/issues/143) |
| **Failures are never masked** | a command that exits non-zero passes through verbatim | [#120](https://github.com/fajarhide/omni/issues/120) |
| **Structured data is never touched** | JSON / YAML / NDJSON / CSV pass through byte-for-byte | `pipeline::format` |

---

## What OMNI remembers, and for how long

Three tiers, already in the schema, never written down until now. The short answer to
"will OMNI still know my project after a month away" is yes for the conclusions and no for
the raw bytes.

| Tier | What | Kept |
|---|---|---|
| **Permanent** | project knowledge, recurring error patterns, engrams, goal memory | until you delete it, except goal memory, which honours its own `ttl_days` |
| **Working, 30 days** | sessions, distillation rows, hot files, the RewindStore, the event index, the ledger | rolling window |
| **Verbatim, 7 days** | `execution_traces` and the session transcript | shorter on purpose: it is two orders of magnitude heavier per row |

The boundary this sets is worth stating plainly, because it is the one thing a handle
cannot promise: `omni retrieve` for content archived more than 30 days ago will not
resolve. Hold the shortest window open while measuring with
`OMNI_TRACE_RETENTION_DAYS=90`.

`omni reset` wipes all of it, and `omni doctor` shows the live counts.

---

## What each host lets OMNI do

| Tier | Hosts | What you get |
|---|---|---|
| **Full** | Claude Code, Codex CLI, Gemini CLI, Aider (pipe) | The host applies OMNI's rewrite, so the model reads distilled output from its own built-in tools. |
| **Handoff-first** | Cursor, Windsurf | The host cannot rewrite built-in tool output. `omni_run` distils anything you route through it, and `omni init --cursor` installs the rule that makes the agent reach for it. |
| **MCP-only** | Cline, Roo, OpenCode, VS Code, Zed, Copilot, Antigravity, Hermes, Pi | Memory, recall and session state. No shell distillation, and no claim of it. |

`omni doctor` prints the tier for every installed host. Savings are only ever counted
where the model actually received less.

Codex CLI needs one extra step. It runs only hooks it has been told to trust and skips
the rest without a word, so after `omni init --codex` start `codex` once and approve
them under "Hooks need review". `omni doctor` fails until you do. See
[#359](https://github.com/fajarhide/omni/issues/359).

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

## Numbers

Every figure OMNI publishes states the corpus it came from and the week it covers,
because `execution_traces` is pruned after seven days and a number that outlives its
corpus cannot be checked by anyone, us included.

On the 2026-08-04 to 08-11 UTC window, replayed on the 0.7.3 release binary over real
command executions that reached a model:

* Build and test output: **76.9%**. File re-reads, the largest class: **0.0%** from
  the filters and **25.0%** from the ledger, which is the gap the ledger exists for.
* **97.3% of calls saved nothing at all**, and we publish that because it tells you
  what the rest are worth. **No call came back larger** in this measurement.
  There were 2 until ([#398](https://github.com/fajarhide/omni/issues/398)), and we published them while they stood.
* **21 ms per command**, growing with your history rather than with the payload. On a
  205 MB database it is 61 ms.

Per class, over the same 6,656 traces, with what the filters take and what the ledger
adds on top:

| Class | Calls | Input | Filters | + ledger |
|---|---|---|---|---|
| other | 4,145 | 2.95 MB | 0.6% | **6.9%** |
| file read (`cat`, `sed`, `head`, `tail`) | 699 | 1.60 MB | 0.0% | **25.0%** |
| search (`grep`, `rg`, `find`) | 828 | 1.03 MB | 4.8% | **13.3%** |
| `git`, `gh` | 661 | 609 KB | 4.4% | **22.1%** |
| build and test | 69 | 94 KB | 76.9% | **78.0%** |
| infra (`kubectl`, `az`, `docker`) | 254 | 193 KB | 4.4% | **8.2%** |
| **aggregate** | **6,656** | **6.47 MB** | **2.7%** | **14.9%** |

Head to head on that corpus, including the half we lose. Filters alone: rtk 6.2%,
lean-ctx 6.1%, ours 2.7%. With our ledger, 14.9%. Run rtk's filters with our ledger
and you get 17.6%, which is the largest number on the page and not ours.

The two competitors land a tenth of a point apart from opposite shapes: rtk averages
461 bytes off each of 872 commands, lean-ctx 2,950 off each of 134.

Reproduce all of it:

```bash
OMNI_BENCH_DB=~/.omni/omni.db \
  cargo test --release --test bench_replay -- --ignored --nocapture
```

`OMNI_BENCH_RTK=/path/to/rtk` adds the competitor arm. `OMNI_BENCH_ALL=1` replays the
wider population including terminal output, and the harness prints which one it used.

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
No, and that is deliberate as of 0.7.0. The filters are compiled into the binary, so the
set that runs is the set the tests cover and there is no file on disk that changes what your
agent is shown. Two tiers were removed to get there: a project's own `.omni/signals/`, which
made a filter a thing a repository could ship to its visitors, and `~/.omni/signals/`. The
whole filter layer is worth 804 bytes over 6,656 recorded commands, so what it cost in
surface it was not paying back. If a tool needs a signal, open an issue and it ships in the
binary for everyone.

**How do I get back something OMNI folded?**
`omni retrieve <handle>`, where the handle is the 16 characters inside the marker. It works
on every host, with or without MCP. Agents that have the MCP server wired can call
`omni_retrieve` instead.

**Can I watch the numbers instead of running a command?**
`omni dashboard` serves them on `127.0.0.1`, read-only, from the same database `omni stats`
reads. It binds loopback and nothing else.

**How do I see my own savings?**
`omni stats` after a few days. It leads with session lifetime, how many commands a session
carries before the host closes it, because that is what the context window costs you. The
distillation percentage below it is a diagnostic for one host's pipeline, not a product
claim. `omni stats --share` prints a copy-pasteable summary, and `omni stats --card` writes
it as an image.

---

## Learn more

* [Contributing](CONTRIBUTING.md): the pipeline, the standards, the gates, and how to
  add a distiller. One document, not four.
* [CHANGELOG.md](CHANGELOG.md): what shipped, with the evidence behind each entry
* [SECURITY.md](SECURITY.md): reporting a vulnerability
* [Discord](https://discord.gg/zHTuvZhF2M): ask a question, or report something OMNI got wrong

---

```bash
brew install fajarhide/tap/omni && omni init
```

A passion project for the era of agentic AI. Contributions welcome.
[Apache License 2.0](LICENSE).

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
