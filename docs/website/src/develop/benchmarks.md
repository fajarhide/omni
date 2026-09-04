# Benchmarks

One developer's real command history, replayed. Every figure in a section comes from the
same run as the rest of that section, including the ones that do not flatter us.

**Two runs are published here and they disagree by a factor of fifteen.** That is the most
useful thing on the page: the number OMNI reports is a property of the week it replays, not
a constant. Read the corpus line before the figure, every time.

## Correction, 2026-09-03: every ledger figure below is overstated

The replay built its ledger without ever telling it which command produced the payload
(#760), so every rule in the ledger that reads the command was inert here while running
normally in production: the `from` clause, the line budget that leaves a `tail -5` alone,
and the wording a re-run gets. The harness measured a ledger with its guards switched off.

Re-measured on the same frozen corpus, same commit, with only that fixed:

| arm | as published | with the guards visible |
| --- | --- | --- |
| omni, with the ledger | 4.9% | **3.0%** |
| rtk + our ledger | 5.7% | 3.7% |
| caveman + our ledger | 5.6% | 3.7% |
| headroom dedup | 5.8% | 5.8% |
| lean-ctx compress | 4.8% | 4.8% |
| omni, filters only | 1.4% | 1.4% |

Per class, with the ledger: file read 4.3% to **1.5%**, other 4.6% to 2.8%, git 8.6% to
6.4%, search 4.2% to 4.0%, infra 3.8% to 3.6%, build and test 11.1% to 10.7%. The capture
rate goes 23.3% to 10.7%.

Only the arms that use our ledger move, which is the check that the change is what it
claims to be. Filters do not touch the ledger and read 1.4% either way, and the three
competitor engines are untouched.

The tables below are left as they were measured, because deleting a published number is
worse than labelling it. Artifacts carry `harness.ledger_knows_the_command` from #760 on,
and one written without that field was measured this way.

## The current run, 2026-08-24, and the first one that will still exist next month

**Corpus**: 9,478 traces, 8,458,937 bytes, 70 sessions, all `agent_id='claude_code'`,
0 terminal rows, 0 errored. Frozen on disk and hashed as `0b63218ef78a1edb`, replayed
in 7.1 s on OMNI 0.7.8.

Every run above this line on this page was measured on `execution_traces`, which prunes
at seven days, so none of them can be re-derived. This one is a file. That is the whole
of #704: a release-over-release delta was previously a code change and a corpus change
added together, with no way to separate them.

**1.4% from the filters. 5.1% with the ledger.** 98.4% of calls saved nothing, 1.6%
shrank, and **no call came back larger**. Tokens, `cl100k_base` as a proxy for a
vocabulary Anthropic does not publish: 2,404,625 to 2,372,043, also 1.4%.

<!-- omni:corpus-table:start -->
| Class | Calls | Input | Filters | + ledger | Available | Captured |
|---|---|---|---|---|---|---|
| other | 6,457 | 4.81 MB | 0.8% | 2.8% | 15.9% | **12.4%** |
| file read | 1,056 | 1.89 MB | 0.0% | 1.5% | 17.7% | **8.3%** |
| git | 899 | 0.86 MB | 5.1% | 6.4% | 18.4% | **7.5%** |
| search | 810 | 0.77 MB | 3.4% | 4.0% | 6.5% | **10.2%** |
| infra | 215 | 0.14 MB | 3.2% | 3.6% | 5.5% | **6.8%** |
| build and test | 41 | 0.02 MB | 9.0% | 10.7% | 21.7% | **8.6%** |
| **aggregate** | 9,478 | 8.49 MB | 1.4% | 3.0% | 15.6% | **10.7%** |

| Arm | bytes | saved |
|---|---|---|
| headroom dedup, omni's filters | 8,486,830 to 7,992,449 | 5.8% |
| lean-ctx `compress` | 8,486,830 to 8,076,957 | 4.8% |
| caveman + omni's ledger | 8,486,830 to 8,177,033 | 3.7% |
| rtk + omni's ledger | 8,486,830 to 8,170,668 | 3.7% |
| **omni, with the ledger** | 8,486,830 to 8,232,391 | 3.0% |
| caveman `compress` | 8,486,830 to 8,311,999 | 2.1% |
| rtk `pipe` | 8,486,830 to 8,308,491 | 2.1% |
| omni, filters only | 8,486,830 to 8,371,362 | 1.4% |

Measured by `make bench` over 9,478 traces (8.42 MB, 70 sessions), corpus `0b63218ef78a1edb`, OMNI 0.7.9.
<!-- omni:corpus-table:end -->

**`available` and `captured` are new, and `captured` is the figure that survives a
change of workload.** The ledger substitutes lines it has already delivered, so it can
never fold what was not repeated; `available` is that ceiling and `captured` is the share
of it taken. Between this corpus and the 0.7.5 run below, the file-read saving moves by a
factor of twenty. The capture rate barely moves. Only one of those two is a statement
about OMNI.

**Why the savings column is so much lower than the run below: this corpus is mostly
one-line shell plumbing.** 4,815 of the 9,478 calls begin with `cd`, carrying 3.98 MB at
1.2%, and the median repeated run is 10 bytes. It was a week of driving OMNI's own
development from a terminal, so the payloads are `git`, `gh`, `sed` and `sqlite3` output
that is either tiny, structured, or seen once. Repetition is 16.3% of raw bytes here
against 80.6% in the window below.

**What it does not measure.** The harness replays `execution_traces`, which holds shell
commands only: zero of these 9,478 rows is a `Read` payload. The ledger's file-read path,
including the count-preserving fold added in #664, is not exercised by any figure on this
page. That gap is the honest reason the fold's own numbers live in
`changelog.d/664.changed.md` with their own corpus instead of here.

**All four arms answer now, and the table above is the result.** #711 fixed the
headroom arm. The caveman arm was calling `caveman tools compress`, and nothing on the
measuring machine answers to a bare `caveman`: `~/.caveman/bin` holds `caveman-engine`,
`caveman-shrink` and four others. Pointed at `caveman-shrink`, `tools compress` is not
a subcommand it knows, so it falls through to shrink mode and hands back the input with
`"ratio":0`. An arm reporting zero on every trace reads as an arm that lost, which is
#711's failure wearing a different binary. `caveman-engine compress` is the entry point
that works (#712).

**On this corpus OMNI is last of the four rows that carry a ledger, and both halves are
behind.** headroom's dedup takes 5.8% where ours takes 3.0% over identical filters and
identical blocks, so that 2.8 points is the dedup engine alone. It read 0.9 points until
#760, which is the correction at the top of this page and not a change in the code. Our filter tier is the
weakest of the four at 1.4%, against 2.1% for rtk and caveman and 4.8% for lean-ctx,
and that shortfall is what carries `rtk + omni's ledger` and `caveman + omni's ledger`
above our own stack: the ledger is identical in all three rows and only the filters
underneath it differ.

lean-ctx beating our filters by 3.4 points is the largest single gap here and it is not
argued away. It is a deep compressor rather than a per-command filter, and the same
shape showed up on the 0.7.5 corpus below at a much larger magnitude.

Versions: rtk 0.45.0, lean-ctx 3.9.18, caveman `bin-v1.0.0`, headroom 0.34.0. No
`lean-ctx + our ledger` row: its preview reports `compressed_bytes` and never emits the
text, so the row could only be estimated, and an estimate beside four measurements is
the blend this harness exists to avoid.

**The aggregate moved from 5.1% to 4.9% on the same corpus, and it was paid for on
purpose.** Replaying the frozen corpus at the two commits either side of #728 puts the
whole move on that one change: 5.1% and 24.1% captured before it, 4.9% and 23.3% after.
#728 stopped the project scope folding a whole reply where it should fold part of one,
and folding less is the point of that fix. The corpus hash did not change, so the delta
is code alone.

## The 0.7.5 run, 2026-08-14, which can no longer be re-derived

`execution_traces` prunes at seven days, so the corpus behind every figure in this section
is gone. It is kept because deleting a published number is worse than labelling it, and
because the gap between the two runs is the point.

**Corpus**: 5,984 traces, 23,086,649 bytes, 2026-08-11 11:03:00 to 2026-08-14
18:11:10 UTC, all `agent_id='claude_code'`, 123 terminal rows excluded from 6,107,
0 errored. Replayed in 1,238 s.

### That run's headline

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

### Which commands benefit

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

### Head to head, one corpus

Identical bytes into every arm. Versions: rtk 0.45.0, lean-ctx 3.9.18, caveman 1.1.0
(binaries `bin-v1.0.0`), headroom at `cross_turn_dedup.py`.

| | bytes | saved | claimed |
|---|---|---:|---|
| rtk `pipe` | 23,086,649 to 21,655,277 | **6.2%** | |
| caveman `tools compress` | 23,086,649 to 21,516,757 | **6.8%** | |
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

No `lean-ctx + our ledger` row: its preview reports `compressed_bytes` and never emits
the text, so that row could only be estimated.

### Single fixtures

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

Snapshot the database first if the figure has to be quotable: hooks write to it while the
replay reads, and `execution_traces` prunes at seven days, so a run against the live file
measures a corpus that is already different from the one anybody else would get.

```sh
sqlite3 ~/.omni/omni.db ".backup /tmp/bench-corpus.db"
OMNI_BENCH_DB=/tmp/bench-corpus.db \
  cargo test --release --test bench_replay -- --ignored --nocapture
```

| | |
|---|---|
| corpus | `execution_traces.raw_input`, real usage, replayed. Not synthetic. Shell commands only: no `Read` payload is in this table, so no figure here covers the ledger's file-read path |
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
