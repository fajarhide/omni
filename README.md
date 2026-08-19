<div align="center">
  <img src="media/logo.png" alt="OMNI Logo" width="300" />

<h1>OMNI</h1>
<p align="center">
    <em><b>Your agent pays twice for output it has already seen.</b> OMNI hands back a retrievable handle instead: <b>97.2%</b> off a file it reads twice, <b>89.6%</b> off file reads across the corpus. Nothing deleted, nothing invented, and every number replays on your own history.</em>
</p>

[🇺🇸 English](README.md) | [🇯🇵 日本語](i18n/README-ja.md) | [🇨🇳 简体中文](i18n/README-zh.md) | [🇸🇦 العربية](i18n/README-ar.md) | [🇮🇩 Bahasa Indonesia](i18n/README-id.md) | [🇻🇳 Tiếng Việt](i18n/README-vi.md) | [🇰🇷 한국어](i18n/README-ko.md)

[![CI](https://github.com/fajarhide/omni/actions/workflows/ci.yml/badge.svg)](https://github.com/fajarhide/omni/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/fajarhide/omni)](https://github.com/fajarhide/omni/releases)
  [![Rust](https://img.shields.io/badge/built_with-Rust-dca282.svg)](https://www.rust-lang.org/)
  [![MCP](https://img.shields.io/badge/MCP-compatible-green.svg?style=flat-square)](https://modelcontextprotocol.io/)
  [![Discord](https://img.shields.io/badge/Discord-join%20the%20server-5865F2?logo=discord&logoColor=white)](https://discord.gg/zHTuvZhF2M)
  [![License: Apache 2.0](https://img.shields.io/github/license/fajarhide/omni)](https://github.com/fajarhide/omni/blob/main/LICENSE)
  [![Hits](https://hits.sh/github.com/fajarhide/omni.svg)](https://hits.sh/github.com/fajarhide/omni/)
  [![Greptile: The War on Bugs](https://www.greptile.com/badge.svg)](https://www.greptile.com/?utm_source=oss_badge&utm_medium=readme&utm_campaign=greptile_for_open_source)
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

<img src="docs/website/src/media/where-omni-sits.svg" alt="OMNI runs as two hooks around a tool call: a pre-hook before the command, a post-hook that distills the output before the agent reads it, with everything it removes archived to a local SQLite database that omni retrieve reads back." width="820" />

It runs as two hooks around a tool call your agent host already makes. Nothing proxies
your shell, and the database never leaves the machine.

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
| `cargo test` (490 passed, 10 failed) | 16.5 KB of per-test output | the runner's own pass/fail summary | **93.0%** |
| `git status` (dirty) | 496 B of porcelain | the branch and the changed paths | **66.7%** |
| `docker build` (heavy cache noise) | 9.2 KB of layer hashes and progress bars | the build result, cache hits folded | **98.9%** |
| `git diff` (multi-file) | lockfiles, whitespace, generated churn | the code that actually changed | **37.8%** |
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

**Claude Code, from inside the session:**
```
/plugin marketplace add fajarhide/omni
/plugin install omni@omni
```

**Any agent that reads skills**, listed at
[skills.sh/fajarhide/skills/omni](https://www.skills.sh/fajarhide/skills/omni):
```bash
npx skills add fajarhide/skills --skill omni
```

Both install a skill, not the binary. The skill is what tells the agent how to get
the binary, verify it, and read the markers OMNI leaves when it shortens output.

Then run your commands normally. There is nothing to prefix and no proxy to wrap.

---

## Numbers

Every figure OMNI publishes states the corpus it came from and the week it covers,
because `execution_traces` is pruned after seven days and a number that outlives its
corpus cannot be checked by anyone, us included.

On the 2026-08-11 to 08-14 UTC window, replayed on the 0.7.5 release binary over 5,984
real command executions that reached a model:

* **32.6% from the filters, 69.6% with the ledger.** File re-reads, the largest class
  by bytes: **39.2%** from the filters and **89.6%** with the ledger, which is the gap
  the ledger exists for.
* **Read the corpus before the number.** This window is unusual and it inflates
  everything here: 286 groups of byte-identical payloads are 80.6% of these bytes, and
  148 of the 5,984 calls carry 64.7% of them. It was a week of building and
  benchmarking OMNI. The same harness over a week of ordinary work reads **14.9%**.
* **It fires where your bytes are.** File re-reads are the largest class in this
  corpus and the ledger takes **89.6%** off them. Where there is nothing safe to
  take, a two-line `git status` or a JSON payload a later step parses, OMNI hands the
  output back untouched rather than inventing a saving. **No call came back larger**
  in this measurement. Two did until ([#398](https://github.com/fajarhide/omni/issues/398)), and we published them while they stood.
* **21 ms per command**, growing with your history rather than with the payload. On a
  205 MB database it is 61 ms.
* **End to end, the gap favours you.** These are bytes per command, which is not the
  same as your bill: billed input tokens track roughly turns times prefix size. Measured
  on whole sessions the saving averages **larger** than this table, because a payload
  shortened once is a payload every later turn stops re-reading. It is an average and
  not a promise, and some sessions did not fall at all.

Per class, over the same 5,984 traces, with what the filters take and what the ledger
adds on top:

| Class | Calls | Input | Filters | + ledger |
|---|---|---|---|---|
| other | 3,703 | 11.05 MB | 29.1% | **56.2%** |
| file read (`cat`, `sed`, `head`, `tail`) | 884 | 10.93 MB | 39.2% | **89.6%** |
| search (`grep`, `rg`, `find`) | 600 | 540 KB | 2.3% | **4.3%** |
| `git`, `gh` | 696 | 475 KB | 2.5% | **7.0%** |
| build and test | 36 | 24 KB | 10.8% | **10.8%** |
| infra (`kubectl`, `az`, `docker`) | 65 | 70 KB | 0.0% | **6.8%** |
| **aggregate** | **5,984** | **23.09 MB** | **32.6%** | **69.6%** |

`infra` reads 0.0% from the filters on purpose. It was 1.7% one release ago, bought by
summarising `kubectl get pods` tables, which deleted the pod names that were the
answer. That saving is gone and the rows are back.

Head to head on that corpus, identical bytes into every arm. **OMNI with its ledger is
the top arm at 69.6%**, ahead of headroom's dedup over our filters at 65.8%, lean-ctx
at 49.4%, caveman at 6.8% and rtk at 6.2%. Bolting our ledger onto rtk lifts it to
61.4% and onto caveman 61.7%, which is the clearest statement of where the work is.

Our filters on their own take 32.6%, and lean-ctx beats that sub-component by 16.8
points on a corpus built out of a few enormous repetitive payloads, exactly the shape
a deep-and-narrow compressor is for. Every arm is in the table on the benchmarks page,
that one included.

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
There are no filters to add. The pattern-matching layer was retired in 0.7.4 after it
measured at 2,018 bytes over 6,656 recorded commands, 0.031% of the corpus, while costing
5 to 7 ms of the hook's 10 ms budget. On infrastructure commands the corpus was better
without it. What remains is the Rust distillers and the ledger, both compiled in, so the
set that runs is the set the tests cover. If a tool needs handling, open an issue and it
ships in the binary for everyone.

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
  <a href="https://star-history.dera.page/#fajarhide/omni&Date">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://star-history.dera.page/svg?repos=fajarhide/omni&type=Date&theme=dark" />
      <source media="(prefers-color-scheme: light)" srcset="https://star-history.dera.page/svg?repos=fajarhide/omni&type=Date" />
      <img alt="Star History Chart" src="https://star-history.dera.page/svg?repos=fajarhide/omni&type=Date" width="600" />
    </picture>
  </a>
</p>

<p align="center">Built with care by <a href="https://github.com/fajarhide">Fajar Hidayat</a></p>
