# OMNI — Command Audit: where distillation helps an agent, and where it hurts

Measured 2026-07-26 on the current build (main `12b1d28`). Each command's real
output was piped through OMNI's pipe mode with an isolated `OMNI_DB_PATH` and a
fresh session (no scorer history), then raw vs distilled bytes were compared and
the distilled output was read in full to check whether the **answer** survived,
not just whether bytes shrank.

## The principle this surfaced

OMNI earns its keep when a command's output is **`verdict + repeated noise`** — a
build log, a test run, an infra plan. It drops the noise and keeps the signal, and
the agent reads less while knowing the same thing.

It **destroys the answer** when a command's output is a **list where every line is
a distinct datum** — a file listing, a process table, a path enumeration. There is
no noise to drop, so "compression" can only mean dropping items the agent asked
for. High reduction on these commands is a red flag, not a win.

> The rule that follows: distill `signal + noise`; pass through `a list of data`.

## ✅ Genuine wins — distill these (noise dropped, answer kept)

| Command | Input | Output | Saved | What survives |
|---|---|---|---|---|
| `terraform plan` | 916 B | 125 B | 86% | `+2 ~1 -1` summary **and every resource** (`+aws_instance.web`, `~…sg`, `-…old_worker`) |
| `cargo test` | 16,515 B | 1,100 B | 93% | `490 passed; 10 failed` + each failing test |
| `cargo build` | 3,220 B | 9 B | 99% | clean build → `Build: ok` (no error to lose) |
| `docker build` | 309 B | 31 B | 89% | build status; errors kept on failure |
| `npm run build` | 491 B | 60 B | 87% | build status |
| `npm test` (vitest) | 392 B | 25 B | 93% | `✓ 16/17 ✗ 1` (minor: does not name the failing test) |
| `docker build` (heavy noise) | 9,207 B | 5,783 B | 37% | the error inside a wall of progress lines |
| `cargo build` (errors) | 317 B | 292 B | 7% | every error, little dropped — correct |

These are OMNI's reason to exist. **The demo GIF and the marketing should lead
with `cargo test` and `terraform plan`.**

## ❌ Harms — these drop the answer (bugs)

| Command | Input | Output | "Saved" | What is LOST |
|---|---|---|---|---|
| `ls -la` | 983 B | 55 B | 94% | every filename → replaced by a count (`ls: 16 items \| 13 files, 3 dirs`) |
| `find` | 3,919 B | 636 B | 83% | 68 of 98 paths, `[Partial signal]` with no count — **#198** |
| `git log` (verbose) | 17,703 B | 1,361 B | 92% | 10 of 12 commits, output cut mid-line — **#199** |
| `wc -l` (many files) | 2,907 B | 916 B | 68% | most per-file counts |
| `ps aux` | 148,556 B | 50,425 B | 66% | most processes (keeps top-by-CPU only; no count of the rest) |
| `docker ps` | 862 B | 75 B | 91% | running containers' names/images/ports (keeps only the exited ones) |

`ps aux` and `docker ps` are borderline — they surface the "interesting" rows
(busy processes, failed containers) and drop the rest — but they drop them without
a count, so a reader cannot tell a filtered view from a complete one.

## ❌ Net-negative — OMNI *grows* the output

| Command | Input | Output | Delta |
|---|---|---|---|
| `env` | 1,224 B | 1,838 B | **+50%** |
| `df -h` | 953 B | 1,106 B | **+16%** |
| `cargo tree` | 21,983 B | 22,324 B | **+1%** (should be a clean passthrough per #170) |

Any command OMNI cannot shrink must be handed back byte-for-byte. Adding marker or
annotation bytes to output it did not distill is a token cost with no benefit,
paid on every turn once the context is cached.

## ✅ Correct passthrough (working as intended)

- `az vm list -o json` (3,969 B) → **0%**, byte-for-byte. The format-safe gate
  (`pipeline::format::sniff`) recognises JSON and stands the lossy stages down.
  This is the moat working.
- `cat`, `head`, `ls -R`, `du`, small `go test` / `jest` / `semgrep` outputs —
  passed through because they are structured, small, or have no noise to drop.
- Modest lossless trims: `git diff` 44%, `eslint` 46%, `grep -rn` 20% (hoists the
  repeated path, keeps every match), `git log --oneline` 19% (keeps every
  subject), `pytest` 18%, `npm install` 19%, `kubectl get pods` 9% (small fixture;
  a large pod table compresses more while keeping the statuses).

## What this means

1. **Scope OMNI to the commands it wins.** Enumeration commands (`ls`, `find`,
   `tree`, `wc`, `ps`, `docker ps`, verbose `git log`) should keep every item with
   a count, or pass through — never truncate to a shorter, plausible, incomplete
   list. Tracked as an umbrella issue over #198 / #199.
2. **The published savings figure is inflated** by the lossy truncation above. A
   re-benchmark (#184) must count dropped list items as lost signal, not
   compression, or the headline measures the bug.
3. **Never grow.** `env` / `df` must pass through.

## How to reproduce

```
OMNI_DB_PATH=$(mktemp -d)/d.db OMNI_QUIET=1 \
  <command> | OMNI_CMD="<command>" ./target/debug/omni
```

Compare line/byte counts against the raw command, and **read the whole distilled
output** — a byte count cannot tell a dropped item from removed noise.
