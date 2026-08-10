# Benchmarks, in full

The short version lives in the [README](../README.md#benchmarks). This is the rest
of it, including the parts that do not flatter us.

Measured on the release binary by replaying every recorded command execution from
one developer's actual usage:

```bash
OMNI_BENCH_DB=~/.omni/omni.db \
  cargo test --release --test bench_replay -- --ignored --nocapture
```

**Every figure below states the window it covers, and the reason is not pedantry.**
`execution_traces` is pruned to `TRACE_RETENTION_DAYS`, seven days, so a corpus is
gone a week after it is measured. The 9,965-trace run that earlier releases quoted
cannot be re-derived by anyone, including us. Numbers that outlive their corpus are
the thing this document exists to stop.

## Method

* **Corpus**: `execution_traces.raw_input` from one developer's real usage, replayed
  through the current pipeline. Not synthetic.
* **Population**: calls whose result reached a model. `OMNI_BENCH_ALL=1` replays the
  wider set including terminal output, and the harness prints which one it used.
* **State**: `session: None`, `store: None`, and `HOME` pointed at an empty temp dir,
  so the scorer sees no history and only the embedded signals load. A warm database
  makes the result non-deterministic.
* **Path**: `run_inner`, the same full pipeline (format gate, TOML, distill,
  guardrail) that the hook and `omni exec` run. This measures what an agent actually
  receives, marker bytes included.
* **Binary**: release build.

## The headline

Replayed 2026-08-10 over **7,095 traces covering 2026-08-03 to 08-10 UTC**, every one
of them `agent_id='claude_code'`.

* **15.7% fewer bytes** across the whole mix (6.95 MB → 5.86 MB), of which the
  filters are 5.2% and the session ledger is the rest.
* **5.0% fewer tokens** from the filters alone (1,960,286 → 1,862,166 by
  `cl100k_base`). Terminal output measures **3.545 bytes per token** here, which is
  what `util::token_estimate`'s shipped 3.6 was calibrated against.
* **97.1% of calls saved nothing at all** and handed the output straight back. Every
  byte of the saving comes from the other 2.9%.
* **2 calls of 7,095 came back larger.** Reported rather than rounded away; earlier
  releases published "not one call in 9,965", which was true of a corpus that no
  longer exists. Filed as fajarhide/omni#398.

Two numbers a byte figure cannot express, both new in #392:

* **25.4% of raw bytes are lines the agent had already been shown**, and **22.9%
  still are after every distiller has run.** Filtering and repetition are orthogonal,
  which is the entire argument for the ledger.
* Of that repetition, 19.1% is within one session and 3.8% is from an earlier
  session of the same project.

The byte-sink ranking and the token-sink ranking **disagree**: `grep` and `ls` move
up when counted in tokens, `sed` and `cargo` move down.

## Head to head, one corpus

P4 of the direction spec asked for this and set the bar: if omni does not win, the
claim does not ship. It wins, and the half it loses is published with it.

`OMNI_BENCH_RTK=/path/to/rtk` adds the arm; without it the harness runs as before, so
CI never needs a competitor installed.

| | bytes | saved |
|---|---|---|
| omni, filters only | 7,129,776 to 6,761,683 | **5.2%** |
| rtk `pipe` | 7,129,776 to 6,547,573 | **8.2%** |
| omni, with the ledger | 7,129,776 to 5,980,828 | **16.1%** |

**rtk's filters are better than ours**, by 3 points on the same bytes, and it reached
that on 931 of 7,198 traces. That is not a rounding difference and it is not going to
be argued away here: on the commands both tools claim, theirs cut more.

**The ledger is the difference**, roughly double rtk's figure, and it is the thing
neither tool's filters can do: it removes bytes because the agent has already been
shown them, not because a pattern says they are noise.

Two things that make this comparison tilt **towards rtk**, stated because a benchmark
that only lists its own handicaps is an advertisement. rtk is handed the exact filter
name for every command, which its own hook has to infer from the command line, and
anything it has no filter for is counted as a passthrough rather than as a miss.

## Which population was measured

The corpus counts only calls whose result reached a model. Terminal output is
excluded: on an installation that carries it, it was 68% of the raw bytes, and
including it printed 79.1% where the model-facing population printed 43.3%. We don't,
because that number is measuring a population no model ever read. The current window
happens to hold no terminal rows at all, and the harness says so on its own line
rather than leaving it to be inferred.

This was a real defect, not a hypothetical. `tests/bench_replay.rs` counted terminal
bytes until #324; `omni stats` had the same bug until #212. Both are fixed and both
now print which population they used.

## Where the saving comes from

Same run, by command class, with what the filters take and what the ledger adds on
top of them:

| Class | Calls | Input | Filters | + ledger |
|---|---|---|---|---|
| other | 4,541 | 3.20 MB | 0.5% | **5.4%** |
| file read (`cat`, `sed`, `head`, `tail`) | 668 | 1.54 MB | 0.0% | **24.6%** |
| search (`grep`, `rg`, `find`) | 801 | 1.03 MB | 4.8% | **12.0%** |
| `git`, `gh` | 710 | 672 KB | 4.6% | **19.1%** |
| build and test | 80 | 292 KB | 87.9% | **92.3%** |
| infra (`kubectl`, `az`, `docker`) | 295 | 214 KB | 4.0% | **6.7%** |
| **aggregate** | **7,095** | **6.95 MB** | **5.2%** | **15.7%** |

Two things this table says that the old one could not.

**The filters are excellent where there is noise and irrelevant where there is not.**
Build and test output is 87.9% and it is 292 KB. File reads are 1.54 MB and the
filters take **0.0%** of them, which is correct behaviour: you cannot strip lines from
a file the agent asked to see without guessing which parts it meant, and guessing is
what the trust floor forbids.

**The ledger reaches what filtering cannot**, because a run of lines the agent has
already been shown can be handed back as a handle without guessing anything. That is
where 24.6% on the largest class comes from.

The mix also moves. `cargo` is 94.7% in this window across 16 calls and 267 KB, where
an earlier window had 124 calls and 1.5 MB. A per-command figure describes the week
it was measured in as much as it describes the distiller.

## Single fixtures

From `tests/fixtures/`, if you want to reproduce one by hand:

| Command / Context | Input | Delivered | Saved |
|-------------------|-------|-----------|-------|
| `cargo build` (large, successful) | 3,220 B | 87 B | **97.3%** |
| `cargo test` (490 passed, 10 failed) | 16,515 B | 1,178 B | **92.9%** |
| `git status` (dirty) | 496 B | 190 B | **61.7%** |
| `docker build` (heavy noise) | 9,207 B | 5,904 B | **35.9%** |
| `git diff` (multi-file) | 397 B | 297 B | **25.2%** |
| `kubectl get pods` (mixed) | 840 B | 840 B | **0%** |

"Delivered" is what the agent receives, marker included. Subtract the ~77 byte
retrieval marker and these match the figures earlier releases published; the marker
is counted here because the agent pays for it.

`kubectl get pods` used to report 9.3%. It reports nothing now, because a pod table
is an enumeration where every row is a datum and there is no noise to drop. Losing
that 9.3% was the fix.

## Latency

Median of 12 runs each, release binary, measured end to end through the post-hook:

| | fresh database | 205 MB database |
|---|---|---|
| `git status` (496 B) | **21.1 ms** | **60.7 ms** |
| `cargo test` (16.5 KB) | **24.5 ms** | **64.5 ms** |

Payload size barely matters; database size does. Earlier releases measured 82 ms and
276 ms on a fresh database, and the difference is three fixes rather than a faster
machine: a GPT tokenizer that was loaded per command for a reporting column, 249
line-filter regexes compiled whether or not their filter matched, and a connection
pool opening four SQLite handles in a process that exits after one payload.

`OMNI_PASSTHROUGH=1` skips the pipeline entirely when you need the raw output back.

## Measure your own

```bash
omni stats            # your real numbers, after a few days of use
omni stats --share    # a copy-pasteable summary of them
```

Both read the same aggregation, so the figure in the share card cannot drift from
the one in the report. Terminal output is excluded from both.
