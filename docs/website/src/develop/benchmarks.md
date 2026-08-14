# Benchmarks

One developer's real command history, replayed. Including the runs that did not
flatter us.

## The headline

**14.9% fewer bytes. 6,469,047 to 5,506,627.**

0.7.3, replayed 2026-08-12 over 6,656 traces, 2026-08-04 02:56 to 08-11 03:34 UTC,
all `agent_id='claude_code'`.

| | |
|---|---|
| filters | 2.7% (176,191 B) |
| ledger, on top | 12.2 points (786,229 B) |
| tokens, filters only | 2.8%, `cl100k_base` |
| bytes per token | 3.592 raw (the shipped 3.6 estimate is calibrated to this) |
| calls that saved nothing | 97.3%, so all of it comes from 181 calls |
| calls that grew | 0 of 6,656 |
| raw bytes already shown once | 22.9% before filters, 22.4% after |
| that repetition, by scope | 19.0% same session, 3.9% earlier session, same project |

Filtering and repetition are orthogonal. That is the argument for the ledger.

## Which commands benefit

| class | calls | input | filters | + ledger |
|---|---:|---:|---:|---:|
| other | 4,145 | 2.95 MB | 0.6% | **6.9%** |
| file read (`cat`, `sed`, `head`, `tail`) | 699 | 1.60 MB | 0.0% | **25.0%** |
| search (`grep`, `rg`, `find`) | 828 | 1.03 MB | 4.8% | **13.3%** |
| `git`, `gh` | 661 | 609 KB | 4.4% | **22.1%** |
| build and test | 69 | 94 KB | 76.9% | **78.0%** |
| infra (`kubectl`, `az`, `docker`) | 254 | 193 KB | 4.4% | **8.2%** |
| **aggregate** | **6,656** | **6.47 MB** | **2.7%** | **14.9%** |

Filters take 0.0% of file reads, and that is correct: you cannot strip lines from a
file the agent asked to see.

`cargo` reads 94.7% here over 16 calls and 267 KB. An earlier window had 124 calls
and 1.5 MB. The mix moves as much as the distiller does.

Byte-sink and token-sink rankings disagree: `grep` and `ls` rise in tokens, `sed` and
`cargo` fall.

## Head to head, one corpus

Competitor versions: rtk 0.45.0, lean-ctx 3.9.18, caveman 1.1.0 (binaries
`bin-v1.0.0`), headroom at `cross_turn_dedup.py`.

| | bytes | saved | claimed |
|---|---|---:|---|
| omni, filters only | 6,469,047 to 6,292,856 | **2.7%** | |
| rtk `pipe` | 6,469,047 to 6,067,012 | **6.2%** | 872 of 6,656, 461 B each |
| lean-ctx `compress` | 6,469,047 to 6,073,757 | **6.1%** | 134 of 6,656, 2,950 B each |
| omni, with the ledger | 6,469,047 to 5,506,627 | **14.9%** | |
| rtk `pipe` + omni's ledger | 6,469,047 to 5,333,483 | **17.6%** | |

**rtk's filters beat ours by 3.5 points**, and not by truncating: it marked a cut in
33 of the 872 it claimed. Broad and shallow against lean-ctx's narrow and deep, a
tenth of a point apart, which the aggregate hides completely.

The two counts are not like for like. rtk's is a mapped filter, saving or not,
because it is handed the name. lean-ctx reports no name, so its count is an actual
reduction.

Tilts **towards rtk**, stated because a benchmark listing only its own handicaps is
an advertisement: it is handed the exact filter name its own hook must infer, and
anything it has no filter for counts as passthrough rather than a miss.

No `lean-ctx + our ledger` row: its preview reports `compressed_bytes` and never
emits the text, so that row could only be estimated.

### The two arms missing above

Neither ran in the 0.7.3 replay, and the one-run rule forbids borrowing a figure.
Both are in the harness now, each off unless its variable names a binary.

| arm | variable | shape |
|---|---|---|
| headroom | `OMNI_BENCH_HEADROOM` | `cross_turn_dedup.py`, whole-conversation dedup, so it sits against the ledger and not the filters |
| caveman | `OMNI_BENCH_CAVEMAN` | filters **and** byte-exact recovery, the first competitor with both halves, and the only arm given no command hint |

### What a bad corpus does to a ranking

All six ran together once, on the corpus rejected below. The levels are not
publishable. The ranking is, on the same grounds as the version A/B: identical bytes
into every arm.

| | published corpus | rejected corpus |
|---|---:|---:|
| omni, filters only | 2.7% | 32.7% |
| rtk `pipe` | **6.2%** | **0.6%** |
| lean-ctx `compress` | **6.1%** | **49.6%** |
| caveman `tools compress` | not run | 6.0% |
| omni, with the ledger | 14.9% | 69.8% |
| rtk + our ledger | 17.6% | 61.7% |
| caveman + our ledger | not run | 61.9% |
| headroom dedup, our filters | not run | 66.0% |

rtk beats our filters by 3.5 points on one corpus and loses by 32.1 on the other.

**Why rtk fell, in one table.** Its filters only fire on a command it recognises, and
the rejected corpus is 89.5% commands it does not:

| command | bytes | rtk filter |
|---|---:|---|
| `tail` | 9,558,272 | none |
| `zsh` | 8,391,102 | none |
| `cd` | 1,454,396 | none |
| `cat` | 770,972 | none |
| `export` | 429,265 | none |
| `grep` | 403,917 | `grep` |
| `sed` | 381,331 | none |
| `git` | 259,762 | 3 subcommands of it |

Not a stale binary and not a broken arm: rtk 0.45.0 is current, it claimed 605 of
5,914 calls, and our filter names are checked against its own `resolve_filter`. Of
its 25 filters we map 18; six of the rest have zero traces here, and `log` stays
unmapped because rtk's own hook maps no command to it either.

Be most suspicious of our own row. A corpus handing OMNI a 32 point lead is evidence
about the corpus, not about OMNI. The ledger reads any payload; rtk's filters need a
name they know. On this corpus that difference is the entire result.

Holding across both: the ledger is orthogonal to whose filters run, and is the
largest single contributor either way.

## Single fixtures

From `tests/fixtures/`, reproducible by hand. "Delivered" includes the marker.

| command | input | delivered | saved |
|---|---:|---:|---:|
| `cargo build` (large, successful) | 3,220 B | 87 B | **97.3%** |
| `cargo test` (490 passed, 10 failed) | 16,515 B | 1,178 B | **92.9%** |
| `git status` (dirty) | 496 B | 190 B | **61.7%** |
| `docker build` (heavy noise) | 9,207 B | 5,904 B | **35.9%** |
| `git diff` (multi-file) | 397 B | 297 B | **25.2%** |
| `kubectl get pods` (mixed) | 840 B | 840 B | **0%** |

`kubectl get pods` used to report 9.3%. Losing that was the fix: a pod table is an
enumeration where every row is a datum.

## Latency

Median of 12 runs, release binary, end to end through the post-hook.

| | fresh database | 205 MB database |
|---|---:|---:|
| `git status` (496 B) | **21.1 ms** | **60.7 ms** |
| `cargo test` (16.5 KB) | **24.5 ms** | **64.5 ms** |

Payload size barely matters. Database size does.

Measure latency by removal. A unit-test timer said 66 ms for work an A/B on the
release binary put at 34.3 ms.

## Method

```sh
OMNI_BENCH_DB=~/.omni/omni.db \
  cargo test --release --test bench_replay -- --ignored --nocapture
```

| | |
|---|---|
| corpus | `execution_traces.raw_input`, real usage, replayed. Not synthetic |
| population | calls whose result reached a model. `OMNI_BENCH_ALL=1` widens it |
| state | `session: None`, `store: None`, `HOME` at an empty temp dir |
| path | `run_inner`, the same pipeline the hook and `omni exec` run, markers included |
| binary | release build |

**Terminal output is excluded, and it is worth two different headlines.** On an
installation carrying it, it was 68% of raw bytes: **79.1%** including it against
**43.3%** model-facing. The harness and `omni stats` both counted it until that was
fixed, and both now print which population they used.

**Every figure comes from one run.** This file once published 15.7% and 16.1% from
two replays a day apart without saying so.

**Every window closes.** `execution_traces` prunes at 7 days, so a corpus is gone a
week after it is measured. An earlier 9,965-trace run cannot be re-derived by anyone.
Hold it open with `OMNI_TRACE_RETENTION_DAYS`.

**Old figures are deleted, not kept.** Two releases changed the rule deciding whether
the ledger folds a run, so an older number describes a pipeline that no longer
exists.

The one movement worth stating: **15.4% to 14.9% between 0.7.0 and 0.7.2 is a fix.**
0.7.2 stopped folding any line stating a failure, so a repeated `TypeError` survives
a re-run. Half a point for the error channel. The rtk arm reproduced its earlier
figure to the byte, ruling out a corpus change.

The filter column is also lower than older releases because 0.7.0 deleted the user
and project filter tiers. What runs here is the embedded set, which is what every
installation now gets.

## What no figure here can tell you

Whether the removed lines were signal.

## Why 0.7.5 has no figures of its own

A four-arm replay ran 2026-08-15 and **was rejected, not published**. It read 32.7%
and 69.8% against 2.7% and 14.9%. Correct arithmetic, unusable corpus:

| | |
|---|---|
| traces | 5,971, 2026-08-11 11:03 to 08-14 18:05 UTC |
| bytes | 23.0 MB, against 6.47 MB for a similar call count |
| concentration | 148 calls, 2.5% of them, carry **64.7% of every byte** |
| duplication | 286 groups of byte-identical payloads are **80.6% of the total** |
| largest contributor | 5 traces of exactly 820,000 B, content is `The exact build tag is BUILD_TAG_9f3a1c.` repeated to fill |

That last row is a synthetic recall fixture, not tool output. The window is the week
this machine did nothing but develop and benchmark OMNI.

**Rule that follows: describe the corpus before reading the result, then reject the
run rather than the description.**

### How much was the code, how much was the corpus

Same frozen corpus, 5,984 traces and 23,086,649 bytes, two binaries.

| | 0.7.3 | 0.7.5 + | change |
|---|---:|---:|---:|
| filters | 32.7% | 32.6% | -0.1 |
| with the ledger | 69.8% | 69.6% | -0.2 |
| bytes delivered | 6,964,958 | 7,024,805 | +59,847 |
| **folds taken** | **976** | **882** | **-94** |
| session markers | 3,316 | 3,231 | -85 |

**The code is worth -0.2 points of the move from 14.9%. The corpus is worth the other
55.**

The 0.2 points buy [#543](https://github.com/fajarhide/omni/issues/543)'s floor on
whole-output folds: 94 runs averaging 637 B are now delivered whole instead of
replaced by a marker they barely outgrew. Same shape as 15.4% to 14.9%.

Direction settled, size not: 0.2 points is a property of this corpus too.

Published figures stay at 0.7.3 until a window of ordinary work is available. The
fixture table, the latency table and the head to head do not depend on that corpus.

## Measure your own

```sh
omni stats
omni stats --share
```

Both read the same aggregation, so the share card cannot drift from the report.
Terminal output is excluded from both.
