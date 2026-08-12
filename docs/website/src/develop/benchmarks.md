# Benchmarks

Including the parts that do not flatter us.

```sh
OMNI_BENCH_DB=~/.omni/omni.db \
  cargo test --release --test bench_replay -- --ignored --nocapture
```

**Every figure here comes from one run**, the 6,656-trace replay dated below. That is
stricter than it sounds and it was not always true: the head to head and the headline
once came from two replays a day apart, and the file published 15.7% and 16.1% without
saying they were different measurements.

**Every figure also names its window**, and that is not pedantry. `execution_traces`
is pruned to seven days, so a corpus is gone a week after it is measured. An earlier
9,965-trace run cannot be re-derived by anyone, us included. Hold the window open with
`OMNI_TRACE_RETENTION_DAYS` while a measurement is in flight.

**These were re-derived on 0.7.2 and the previous ones are gone rather than kept for
comparison.** Two releases have changed the rule deciding whether the ledger folds a
run, so a number measured before either describes a pipeline that no longer exists.
Quoting both would invite a reader to treat the difference as a trend when it is two
programs.

**The aggregate moved from 15.4% to 14.9% between 0.7.0 and 0.7.2, and that is a fix
rather than a regression.** 0.7.2 stopped folding any line that states a failure, so a
repeated `TypeError` survives a re-run instead of being replaced by a pointer. Refusing
to fold the error channel costs half a point of ratio, and it is the trade this project
says it makes whenever the two conflict. The rtk arm below reproduced its earlier figure
to the byte, which is what rules out a different corpus as the explanation.

## Method

- **Corpus**: `execution_traces.raw_input` from one developer's real usage, replayed
  through the current pipeline. Not synthetic.
- **Population**: calls whose result reached a model. `OMNI_BENCH_ALL=1` replays the
  wider set including terminal output, and the harness prints which one it used.
- **State**: `session: None`, `store: None`, `HOME` pointed at an empty temp directory,
  so the scorer sees no history and only the embedded signals load. A warm database
  makes the result non-deterministic.
- **Path**: `run_inner`, the same full pipeline the hook and `omni exec` run, marker
  bytes included.
- **Binary**: release build.

## The headline

Replayed 2026-08-12 on 0.7.2 over **6,656 traces covering 2026-08-04 02:56 to 08-11
03:34 UTC**, every one `agent_id='claude_code'`.

- **14.9% fewer bytes** across the mix (6.47 MB to 5.50 MB), of which the filters are
  2.7% and the ledger is the rest.
- **2.8% fewer tokens** from the filters alone, by `cl100k_base`. This corpus measures
  **3.592 bytes per token**, which is what the shipped 3.6 estimate was calibrated
  against.
- **97.3% of calls saved nothing at all.** Every byte of the saving comes from the
  other 2.7%, which is 181 calls.
- **Not one call of 6,656 came back larger.** Two did until a stream-writer line-ending
  bug was fixed, and the count was published while it stood.

The filter column is lower than earlier releases published, and that is not a
regression: 0.7.0 deleted the user and project filter tiers, so what runs here is the
embedded set alone, which is also the set every installation now gets. The old figure
included whatever filters the measuring machine happened to carry.

Two numbers a byte figure cannot express:

- **22.9% of raw bytes are lines the agent had already been shown**, and **22.4% still
  are after every distiller has run.** Filtering and repetition are orthogonal, which
  is the entire argument for the ledger.
- Of that repetition, 19.0% is within one session and 3.9% is from an earlier session
  of the same project.

The byte-sink ranking and the token-sink ranking **disagree**: `grep` and `ls` move up
when counted in tokens, `sed` and `cargo` move down.

## Head to head, one corpus

The bar was set before the measurement: if OMNI does not win, the claim does not ship.
It wins, and the half it loses is published with it.

`OMNI_BENCH_RTK=/path/to/rtk` adds the arm. Without it the harness runs as before, so
CI never needs a competitor installed.

| | bytes | saved |
|---|---|---|
| omni, filters only | 6,469,047 to 6,292,856 | **2.7%** |
| rtk `pipe` | 6,469,047 to 6,067,012 | **6.2%** |
| omni, with the ledger | 6,469,047 to 5,502,733 | **14.9%** |
| rtk `pipe` + omni's ledger | 6,469,047 to 5,330,551 | **17.6%** |

**rtk's filters are better than ours**, by 3.5 points on the same bytes, over 872 of
6,656 traces. That is not a rounding difference and it is not argued away here. It is
also not bought by truncation, which was the first explanation tried and dropped: rtk
marked a cut in only 33 of the 872 outputs it claimed, so its patterns are simply
better.

**The ledger is the difference**, and the last row is the honest way to say why. It
adds 12.2 points on top of our filters and 11.4 on top of theirs, so it is orthogonal
to whose patterns run. That row also says plainly that a reader who wants the largest
number would run their filters with our ledger.

Two things that tilt this comparison **towards rtk**, stated because a benchmark that
lists only its own handicaps is an advertisement: rtk is handed the exact filter name
for every command, which its own hook has to infer, and anything it has no filter for
is counted as a passthrough rather than a miss.

What neither column measures is whether the removed lines were signal. Byte counts
cannot answer that, and no arrangement of them will.

## Which population

The corpus counts only calls whose result reached a model. Terminal output is
excluded: on an installation that carried it, it was 68% of raw bytes, and including
it printed 79.1% where the model-facing population printed 43.3%.

This was a real defect, not a hypothetical. The replay harness counted terminal bytes
until it was fixed, and `omni stats` had the same bug. Both now print which population
they used.

## Where the saving comes from

| class | calls | input | filters | + ledger |
|---|---|---|---|---|
| other | 4,145 | 2.95 MB | 0.6% | **6.8%** |
| file read (`cat`, `sed`, `head`, `tail`) | 699 | 1.60 MB | 0.0% | **25.2%** |
| search (`grep`, `rg`, `find`) | 828 | 1.03 MB | 4.8% | **13.3%** |
| `git`, `gh` | 661 | 609 KB | 4.4% | **22.3%** |
| build and test | 69 | 94 KB | 76.9% | **78.0%** |
| infra (`kubectl`, `az`, `docker`) | 254 | 193 KB | 4.4% | **8.2%** |
| **aggregate** | **6,656** | **6.47 MB** | **2.7%** | **14.9%** |

**The filters are excellent where there is noise and irrelevant where there is not.**
File reads are 1.60 MB and the filters take 0.0% of them, which is correct: you cannot
strip lines from a file the agent asked to see without guessing which parts it meant.

**The mix moves.** `cargo` is 94.7% in this window across 16 calls and 267 KB, where an
earlier window had 124 calls and 1.5 MB. A per-command figure describes the week it was
measured in as much as it describes the distiller.

## Single fixtures

From `tests/fixtures/`, reproducible by hand:

| command | input | delivered | saved |
|---|---|---|---|
| `cargo build` (large, successful) | 3,220 B | 87 B | **97.3%** |
| `cargo test` (490 passed, 10 failed) | 16,515 B | 1,178 B | **92.9%** |
| `git status` (dirty) | 496 B | 190 B | **61.7%** |
| `docker build` (heavy noise) | 9,207 B | 5,904 B | **35.9%** |
| `git diff` (multi-file) | 397 B | 297 B | **25.2%** |
| `kubectl get pods` (mixed) | 840 B | 840 B | **0%** |

"Delivered" is what the agent receives, marker included.

`kubectl get pods` used to report 9.3%. It reports nothing now, because a pod table is
an enumeration where every row is a datum. Losing that 9.3% was the fix.

## Latency

Median of 12 runs each, release binary, end to end through the post-hook:

| | fresh database | 205 MB database |
|---|---|---|
| `git status` (496 B) | **21.1 ms** | **60.7 ms** |
| `cargo test` (16.5 KB) | **24.5 ms** | **64.5 ms** |

Payload size barely matters; database size does.

> Measure latency by removal, not with a microbenchmark. A unit-test timer said 66 ms
> for work that an A/B on the release binary put at 34.3 ms. Only the second is
> quotable.

## Measure your own

```sh
omni stats
omni stats --share
```

Both read the same aggregation, so the share card cannot drift from the report.
Terminal output is excluded from both.
