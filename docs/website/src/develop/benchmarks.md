# Benchmarks

One developer's real command history, replayed on 0.7.5. Every figure below comes
from the same run, including the ones that do not flatter us.

**Corpus**: 5,984 traces, 23,086,649 bytes, 2026-08-11 11:03:00 to 2026-08-14
18:11:10 UTC, all `agent_id='claude_code'`, 123 terminal rows excluded from 6,107,
0 errored. Replayed in 1,238 s.

## The headline

**32.6% fewer bytes from the filters. 69.6% with the ledger.**
23,086,649 to 15,557,823 to 7,026,021.

| | |
|---|---|
| tokens, filters only | 7,682,124 to 4,874,124, **36.6%** |
| bytes per token | 3.005 raw, 3.192 distilled (the shipped estimate is 3.6) |
| calls that saved nothing | **96.1%**, 5,748 of 5,984 |
| calls that shrank | 3.9%, 236 |
| calls that grew | **0** |
| ledger folds | 882 calls, 3,231 session markers, 86 project markers |
| raw bytes already shown once | **68.4%** before filters, 64.7% after |
| that repetition, by scope | 67.3% same session, 1.1% earlier session, same project |

Filtering and repetition are orthogonal. That is the argument for the ledger, and on
this corpus the ledger is worth more than twice what the filters are.

**Read the corpus before the number.** This window is unusual and it inflates
everything below. 148 of the 5,984 calls carry 64.7% of all bytes, 286 groups of
byte-identical payloads account for 80.6% of the total, and the single largest
contributor is five traces of exactly 820,000 bytes whose content is one sentence
repeated to fill. It is the week this machine did nothing but develop and benchmark
OMNI. A corpus of ordinary work reads far lower: the same harness on 6,656 traces in
August 2026 read 2.7% and 14.9%.

## Which commands benefit

| class | calls | input | filters | + ledger |
|---|---:|---:|---:|---:|
| other | 3,703 | 11.05 MB | 29.1% | **56.2%** |
| file read (`cat`, `sed`, `head`, `tail`) | 884 | 10.93 MB | 39.2% | **89.6%** |
| search (`grep`, `rg`, `find`) | 600 | 540 KB | 2.3% | **4.3%** |
| `git`, `gh` | 696 | 475 KB | 2.5% | **7.0%** |
| infra (`kubectl`, `az`, `docker`) | 65 | 70 KB | **0.0%** | **6.8%** |
| build and test | 36 | 24 KB | 10.8% | **10.8%** |
| **aggregate** | **5,984** | **23.09 MB** | **32.6%** | **69.6%** |

**infra reads 0.0% from the filters on purpose.** It was 1.7% one release ago, bought
by summarising `kubectl get pods` tables, which deleted the pod names that were the
answer. That saving is gone and the rows are back (#562). What remains for infra is
the ledger, which folds a listing the agent has already seen and needs the rows
intact to do it.

By shell shape:

| form | calls | input | saved |
|---|---:|---:|---:|
| bare program | 782 | 10,683,924 | 40.2% |
| chain | 2,024 | 9,843,901 | 32.5% |
| `cd` prefix | 1,655 | 1,476,727 | 0.4% |
| `VAR=` assignment | 952 | 567,135 | 0.4% |
| pipe only | 571 | 514,962 | 4.6% |

Top commands by input bytes, filters only:

| command | calls | input | output | saved |
|---|---:|---:|---:|---:|
| `tail` | 441 | 9,558,272 | 5,599,666 | 41.4% |
| `zsh` | 282 | 8,391,102 | 5,202,443 | 38.0% |
| `cd` | 1,727 | 1,503,873 | 1,497,294 | 0.4% |
| `cat` | 119 | 770,972 | 442,285 | 42.6% |
| `export` | 535 | 429,265 | 428,731 | 0.1% |
| `grep` | 447 | 410,575 | 402,804 | 1.9% |
| `sed` | 217 | 381,331 | 381,331 | 0.0% |
| `git` | 401 | 262,113 | 256,786 | 2.0% |
| `gh` | 238 | 145,950 | 140,095 | 4.0% |
| `kubectl` | 68 | 71,129 | 71,129 | **0.0%** |

Byte-sink and token-sink rankings disagree at the tail: `bash` enters the token top
15 where `kubectl` sits in the byte one.

## Head to head, one corpus

Identical bytes into every arm. Versions: rtk 0.45.0, lean-ctx 3.9.18, caveman 1.1.0
(binaries `bin-v1.0.0`), headroom at `cross_turn_dedup.py`.

| | bytes | saved | claimed |
|---|---|---:|---|
| rtk `pipe` | 23,086,649 to 22,967,550 | **20.5%** | 623 of 5,984, marked a cut in 16 |
| caveman `tools compress` | 23,086,649 to 21,702,637 | **6.0%** | 149 of 5,984, no command hint |
| omni, filters only | 23,086,649 to 15,557,823 | **32.6%** | |
| lean-ctx `compress` | 23,086,649 to 11,678,975 | **49.4%** | 425 of 5,984 |
| headroom dedup, our filters | 23,086,649 to 7,905,764 | **65.8%** | |
| **omni, with the ledger** | 23,086,649 to 7,026,021 | **69.6%** | |
| rtk + our ledger | 23,086,649 to 8,906,376 | **61.4%** | |
| caveman + our ledger | 23,086,649 to 8,844,105 | **61.7%** | |

**headroom is 3.8 points behind our ledger and that is the only close race here.**
Both arms run the same filters over the same blocks, so the gap is the dedup engine
and nothing else.

**lean-ctx beats our filters by 16.8 points**, 49.4% against 32.6%, over 425 calls to
our 236. That is not argued away: this corpus is a few enormous repetitive payloads,
which is exactly the shape a deep-and-narrow compressor is built for.

**rtk reads 20.5% because its filters only fire on a command it recognises**, and this
corpus is 89.5% commands it does not:

| command | bytes | rtk filter |
|---|---:|---|
| `tail` | 9,558,272 | none |
| `zsh` | 8,391,102 | none |
| `cd` | 1,503,873 | none |
| `cat` | 770,972 | none |
| `export` | 429,265 | none |
| `grep` | 410,575 | `grep` |

Not a stale binary and not a broken arm: 0.45.0 is current, it claimed 623 calls, and
the harness names are checked against rtk's own `resolve_filter`. Of its 25 filters we
map 18; six of the rest have zero traces here, and `log` stays unmapped because rtk's
own hook maps no command to it either.

**Read our own rows with the most suspicion.** A corpus that hands OMNI a 32 point
lead over rtk is evidence about the corpus. The ledger reads any payload; rtk's
filters need a name they know, and on these bytes that difference is the whole result.

Two things that tilt this **towards rtk**, stated because a benchmark listing only its
own handicaps is an advertisement: it is handed the exact filter name its own hook
must infer, and anything it has no filter for counts as passthrough rather than a
miss. caveman gets less than either: no command hint at all.

The counts are not like for like. rtk's is a mapped filter, saving or not. lean-ctx
and caveman report no filter name, so theirs is an actual reduction.

No `lean-ctx + our ledger` row: its preview reports `compressed_bytes` and never emits
the text, so that row could only be estimated.

## Single fixtures

From `tests/fixtures/`, same build, reproducible by hand. "Delivered" includes the
marker.

| command | input | delivered | saved |
|---|---:|---:|---:|
| `docker build` (heavy noise) | 9,207 B | 102 B | **98.9%** |
| `cargo build` (large, successful) | 3,220 B | 62 B | **98.1%** |
| `cargo test` (490 passed, 10 failed) | 16,515 B | 1,153 B | **93.0%** |
| `git status` (dirty) | 496 B | 165 B | **66.7%** |
| `git diff` (multi-file) | 397 B | 247 B | **37.8%** |
| `kubectl get pods` (mixed) | 840 B | 840 B | **0.0%** |

`kubectl get pods` reading 0.0% is the design, not a gap. For one release it read
73.5%, because a summariser that had been shadowed since #110 became live when #510
retired the TOML layer, and a 10 row table arrived as three lines with seven pod names
deleted. A count of pods cannot be turned back into a pod name (#562).

`docker build` is the opposite case and worth the contrast: 251 lines of per-layer
DEBUG and INFO become `docker build: ✓ complete (50 layers, 50 cached)`, and the build
did succeed. Noise, not an enumeration.

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
| arms | `OMNI_BENCH_RTK`, `_LEANCTX`, `_CAVEMAN`, `_HEADROOM`, each off unless it names a binary, so CI never needs a competitor installed |

**Terminal output is excluded, and it is worth two different headlines.** On an
installation carrying it, it was 68% of raw bytes: 79.1% including it against 43.3%
model-facing. The harness and `omni stats` both counted it until that was fixed, and
both now print which population they used.

**Every figure comes from one run.** This file once published 15.7% and 16.1% from two
replays a day apart without saying so.

**Every window closes.** `execution_traces` prunes at 7 days, so this corpus is gone a
week after it was measured. Hold one open with `OMNI_TRACE_RETENTION_DAYS`.

**Old figures are deleted, not kept for comparison.** Releases keep changing the rule
that decides whether the ledger folds a run, so an older number describes a pipeline
that no longer exists, and printing both invites a reader to read two programs as a
trend.

**Latency was not re-measured on this build**, so no table is printed rather than an
older one relabelled. The method that produced the last one: median of 12 runs per
payload, release binary, end to end through the post-hook, against a fresh database
and a large one. Payload size barely mattered; database size did. Measure by removal,
never with a microbenchmark: a unit-test timer once said 66 ms for work an A/B on the
release binary put at 34.3 ms.

## What no figure here can tell you

Whether the removed lines were signal.

## Measure your own

```sh
omni stats
omni stats --share
```

Both read the same aggregation, so the share card cannot drift from the report.
Terminal output is excluded from both.
