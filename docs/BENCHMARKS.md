# Benchmarks, in full

The short version lives in the [README](../README.md#benchmarks). This is the rest
of it, including the parts that do not flatter us.

Measured on the release binary by replaying every recorded command execution from
one developer's actual usage:

```bash
OMNI_BENCH_DB=~/.omni/omni.db \
  cargo test --release --test bench_replay -- --ignored --nocapture
```

**Every figure in this file comes from one run**, the 6,656 trace replay dated below.
That is stricter than it sounds and it was not true before: the head-to-head and the
headline once came from two replays a day apart, so the file published 15.7% and 16.1%
without saying they were different measurements (#420). One run, or each figure names
its own.

**Every figure also states the window it covers, and the reason is not pedantry.**
`execution_traces` is pruned to `TRACE_RETENTION_DAYS`, seven days, so a corpus is
gone a week after it is measured. The 9,965-trace run that earlier releases quoted
cannot be re-derived by anyone, including us. Numbers that outlive their corpus are
the thing this document exists to stop. `OMNI_TRACE_RETENTION_DAYS` holds the window
open while a measurement is in flight (#440).

**These figures were re-derived on 0.7.0 and the previous ones are gone rather than
kept for comparison.** 0.7.0 changed the rule that decides whether the ledger folds a
run (#450), so every number measured before it describes a pipeline that no longer
exists. Quoting both would invite a reader to treat the difference as a trend when it
is two different programs.

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

Replayed 2026-08-11 on 0.7.0 over **6,656 traces covering 2026-08-04 02:56 to 08-11 03:34 UTC**,
every one of them `agent_id='claude_code'`.

* **15.4% fewer bytes** across the whole mix (6.47 MB to 5.47 MB), of which the
  filters are 2.7% and the ledger is the rest.
* **2.8% fewer tokens** from the filters alone (1,800,715 to 1,750,287 by
  `cl100k_base`). This corpus measures **3.592 bytes per token**, which is what
  `util::token_estimate`'s shipped 3.6 was calibrated against.
* **97.3% of calls saved nothing at all** and handed the output straight back. Every
  byte of the saving comes from the other 2.7%, which is 183 calls.
* **Not one call of 6,656 came back larger.** Two did until #398, which was a line
  ending the stream writer invented; the count was published while it stood rather
  than rounded away.

The filter column is lower than earlier releases published and that is not a
regression: 0.7.0 deleted the user and project filter tiers (#449), so the set that
runs here is the embedded set alone, which is also the set every installation now
gets. The old figure included whatever filters the measuring machine happened to
carry.

Two numbers a byte figure cannot express, both new in #392:

* **22.9% of raw bytes are lines the agent had already been shown**, and **22.4%
  still are after every distiller has run.** Filtering and repetition are orthogonal,
  which is the entire argument for the ledger.
* Of that repetition, 19.0% is within one session and 3.9% is from an earlier
  session of the same project, which is the share the project scope reaches.

The byte-sink ranking and the token-sink ranking **disagree**: `grep` and `ls` move
up when counted in tokens, `sed` and `cargo` move down.

## Head to head, one corpus

P4 of the direction spec asked for this and set the bar: if omni does not win, the
claim does not ship. It wins, and the half it loses is published with it.

`OMNI_BENCH_RTK=/path/to/rtk` adds the arm; without it the harness runs as before, so
CI never needs a competitor installed.

| | bytes | saved |
|---|---|---|
| omni, filters only | 6,469,047 to 6,291,784 | **2.7%** |
| rtk `pipe` | 6,469,047 to 6,067,012 | **6.2%** |
| omni, with the ledger | 6,469,047 to 5,470,574 | **15.4%** |
| rtk `pipe` + omni's ledger | 6,469,047 to 5,298,714 | **18.1%** |

**rtk's filters are better than ours**, by 3.5 points on the same bytes, reached on
872 of 6,656 traces. That is not a rounding difference and it is not going to be
argued away here: on the commands both tools claim, theirs cut more. It is also not
bought by truncation, which was the first explanation tried and dropped: rtk marked a
cut in only 33 of the 872 outputs it claimed, so its patterns are simply better.

**The ledger is the difference**, and the last row is the honest way to say why. Our
ledger adds 12.7 points on top of our filters and 11.9 on top of theirs, so it is
orthogonal to whose patterns run: it removes bytes because the agent has already been
shown them, not because a pattern says they are noise. That row also says plainly
that a reader who wants the largest number would run their filters with our ledger.

What neither column measures is whether the removed lines were signal. Byte counts
cannot answer that, and no arrangement of them will.

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
| other | 4,145 | 2.95 MB | 0.7% | **7.1%** |
| file read (`cat`, `sed`, `head`, `tail`) | 699 | 1.60 MB | 0.0% | **26.3%** |
| search (`grep`, `rg`, `find`) | 828 | 1.03 MB | 4.8% | **13.5%** |
| `git`, `gh` | 661 | 609 KB | 4.4% | **22.9%** |
| build and test | 69 | 94 KB | 76.9% | **78.3%** |
| infra (`kubectl`, `az`, `docker`) | 254 | 193 KB | 4.4% | **8.2%** |
| **aggregate** | **6,656** | **6.47 MB** | **2.7%** | **15.4%** |

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
