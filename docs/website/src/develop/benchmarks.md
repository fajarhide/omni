# Benchmarks

What OMNI saves on one developer's real command history, including the parts that
do not flatter us.

## The headline

**14.9% fewer bytes** across the mix, 6.47 MB down to 5.50 MB. The filters are 2.7
points of that and the ledger is the rest.

Measured on 0.7.3, replayed 2026-08-12 over **6,656 traces covering 2026-08-04 02:56
to 08-11 03:34 UTC**, every one `agent_id='claude_code'`.

Three more numbers from the same run:

- **2.8% fewer tokens** from the filters alone, by `cl100k_base`. This corpus
  measures **3.592 bytes per token**, which is what the shipped 3.6 estimate was
  calibrated against.
- **97.3% of calls saved nothing at all.** Every byte of the saving comes from the
  other 2.7%, which is 181 calls.
- **Not one call of 6,656 came back larger.** Two did until a stream-writer
  line-ending bug was fixed, and the count was published while it stood.

Two things a byte figure cannot express:

- **22.9% of raw bytes are lines the agent had already been shown**, and **22.4%
  still are after every distiller has run.** Filtering and repetition are
  orthogonal, which is the entire argument for the ledger.
- Of that repetition, 19.0% is within one session and 3.9% is from an earlier
  session of the same project.

## Which commands actually benefit

| class | calls | input | filters | + ledger |
|---|---|---|---|---|
| other | 4,145 | 2.95 MB | 0.6% | **6.9%** |
| file read (`cat`, `sed`, `head`, `tail`) | 699 | 1.60 MB | 0.0% | **25.0%** |
| search (`grep`, `rg`, `find`) | 828 | 1.03 MB | 4.8% | **13.3%** |
| `git`, `gh` | 661 | 609 KB | 4.4% | **22.1%** |
| build and test | 69 | 94 KB | 76.9% | **78.0%** |
| infra (`kubectl`, `az`, `docker`) | 254 | 193 KB | 4.4% | **8.2%** |
| **aggregate** | **6,656** | **6.47 MB** | **2.7%** | **14.9%** |

**The filters are excellent where there is noise and irrelevant where there is
not.** File reads are 1.60 MB and the filters take 0.0% of them, which is correct:
you cannot strip lines from a file the agent asked to see without guessing which
parts it meant.

**The mix moves.** `cargo` is 94.7% in this window across 16 calls and 267 KB, where
an earlier window had 124 calls and 1.5 MB. A per-command figure describes the week
it was measured in as much as it describes the distiller.

The byte-sink ranking and the token-sink ranking **disagree**: `grep` and `ls` move
up when counted in tokens, `sed` and `cargo` move down.

## Head to head, one corpus

The bar was set before the measurement: if OMNI does not win, the claim does not
ship. It wins, and the half it loses is published with it.

| | bytes | saved | |
|---|---|---|---|
| omni, filters only | 6,469,047 to 6,292,856 | **2.7%** | |
| rtk `pipe` | 6,469,047 to 6,067,012 | **6.2%** | 872 of 6,656 claimed by a filter |
| lean-ctx `compress` | 6,469,047 to 6,073,757 | **6.1%** | 134 of 6,656 shortened |
| omni, with the ledger | 6,469,047 to 5,506,627 | **14.9%** | |
| rtk `pipe` + omni's ledger | 6,469,047 to 5,333,483 | **17.6%** | |

**rtk's filters are better than ours**, by 3.5 points on the same bytes, over 872 of
6,656 traces. That is not a rounding difference and it is not argued away here. It
is also not bought by truncation, which was the first explanation tried and dropped:
rtk marked a cut in only 33 of the 872 outputs it claimed, so its patterns are simply
better.

**rtk and lean-ctx land a tenth of a point apart, from opposite shapes.** rtk claims
872 commands and averages 461 bytes off each. lean-ctx shortens 134 and averages
2,950, six times more per command it touches. One is broad and shallow, the other
narrow and deep, and the aggregate hides that completely, which is why the counts are
in the table beside the percentages.

Those two counts are not like for like, and the harness reports them differently for
that reason. rtk's counts a **mapped filter**, whether or not it saved a byte,
because it is handed the filter name. lean-ctx reports no filter name, so its count
is an **actual reduction**.

**The ledger is the difference**, and the last row is the honest way to say why. It
adds 12.2 points on top of our filters and 11.4 on top of theirs, so it is orthogonal
to whose patterns run. That row also says plainly that a reader who wants the largest
number would run their filters with our ledger.

Two things that tilt this comparison **towards rtk**, stated because a benchmark that
lists only its own handicaps is an advertisement: rtk is handed the exact filter name
for every command, which its own hook has to infer, and anything it has no filter for
is counted as a passthrough rather than a miss.

**lean-ctx has no ledger row**, and that is a limit of the measurement rather than of
the tool. Its preview path reports `compressed_bytes` and never emits the compressed
text, so stacking our ledger on it could only be estimated, and an estimate beside
four measurements is the blend this page exists to avoid.

### Arms the harness supports but this table does not carry

Each is off unless its environment variable names a binary, so CI never needs a
competitor installed.

| arm | variable | why it is not in the table above |
|---|---|---|
| headroom | `OMNI_BENCH_HEADROOM` | its `cross_turn_dedup.py` is a whole-conversation deduplicator, so it belongs against the ledger rather than against the filters. A prior run put the two at parity to one decimal place; re-deriving that is tracked in [#468](https://github.com/fajarhide/omni/issues/468) |
| caveman | `OMNI_BENCH_CAVEMAN` | added after this table was measured, so it has no figure from the same run, and the one run rule below forbids borrowing one from another |

caveman is the first competitor that ships **both** halves of OMNI's shape:
`tools compress` is the filter tier and `tools retrieve` recovers byte-exact
content, so it gets the same ledger-stacked row rtk gets when it is next measured.
It is also handed less than the other arms: rtk gets the filter name and lean-ctx
gets `--shell <cmd>`, while caveman accepts no command hint at all.

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

`kubectl get pods` used to report 9.3%. It reports nothing now, because a pod table
is an enumeration where every row is a datum. Losing that 9.3% was the fix.

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

## How this is measured

```sh
OMNI_BENCH_DB=~/.omni/omni.db \
  cargo test --release --test bench_replay -- --ignored --nocapture
```

- **Corpus**: `execution_traces.raw_input` from one developer's real usage, replayed
  through the current pipeline. Not synthetic.
- **Population**: calls whose result reached a model. `OMNI_BENCH_ALL=1` replays the
  wider set including terminal output, and the harness prints which one it used.
- **State**: `session: None`, `store: None`, `HOME` pointed at an empty temp
  directory, so the scorer sees no history and only the embedded signals load. A warm
  database makes the result non-deterministic.
- **Path**: `run_inner`, the same full pipeline the hook and `omni exec` run, marker
  bytes included.
- **Binary**: release build.

### Terminal output is excluded, and it is the difference between two headlines

The corpus counts only calls whose result reached a model. On an installation that
carried terminal output, it was 68% of raw bytes, and including it printed **79.1%**
where the model-facing population printed **43.3%**.

This was a real defect, not a hypothetical. The replay harness counted terminal bytes
until it was fixed, and `omni stats` had the same bug. Both now print which
population they used.

### Every figure comes from one run

The 6,656-trace replay dated above, all of it. That is stricter than it sounds and it
was not always true: the head to head and the headline once came from two replays a
day apart, and this file published 15.7% and 16.1% without saying they were different
measurements.

### Every figure names its window, and the window closes

`execution_traces` is pruned to seven days, so a corpus is gone a week after it is
measured. An earlier 9,965-trace run cannot be re-derived by anyone, us included.
Hold the window open with `OMNI_TRACE_RETENTION_DAYS` while a measurement is in
flight.

### Old figures are deleted, not kept for comparison

These were re-derived on 0.7.3 and the previous ones are gone. Two releases have
changed the rule deciding whether the ledger folds a run, so a number measured before
either describes a pipeline that no longer exists. Quoting both would invite a reader
to treat the difference as a trend when it is two programs.

The one movement worth stating: **the aggregate went from 15.4% to 14.9% between
0.7.0 and 0.7.2, and that is a fix rather than a regression.** 0.7.2 stopped folding
any line that states a failure, so a repeated `TypeError` survives a re-run instead of
being replaced by a pointer. Refusing to fold the error channel costs half a point of
ratio, and it is the trade this project says it makes whenever the two conflict. The
rtk arm reproduced its earlier figure to the byte, which is what rules out a different
corpus as the explanation.

The filter column is also lower than earlier releases published, for a separate
reason: 0.7.0 deleted the user and project filter tiers, so what runs here is the
embedded set alone, which is also the set every installation now gets. The old figure
included whatever filters the measuring machine happened to carry.

## What these numbers cannot tell you

Whether the removed lines were signal. Byte counts cannot answer that, and no
arrangement of them will.

## Why 0.7.5 has no figures of its own

A full four-arm replay was run on 2026-08-15 against the then-current corpus and
**the result was rejected rather than published.** It is recorded here because a page
that only shows the runs that worked is the advertisement this one is trying not to
be.

The run reported 32.7% for the filters and 69.8% with the ledger, against the 2.7%
and 14.9% above. Both are correct arithmetic over the corpus they were given, and the
corpus is the problem:

| | |
|---|---|
| traces | 5,971, covering 2026-08-11 11:03 to 08-14 18:05 UTC |
| bytes | 23.0 MB, against 6.47 MB for the same number of calls in the published run |
| concentration | **148 calls, 2.5% of them, carry 14.89 MB, which is 64.7% of every byte** |
| duplication | **286 groups of byte-identical payloads account for 18.53 MB, 80.6% of the total** |
| largest single contributor | five traces of exactly 820,000 bytes, whose content is the sentence `The exact build tag is BUILD_TAG_9f3a1c.` repeated to fill |

That last row is a synthetic fixture from testing OMNI's own recall behaviour, not
output any tool produced. The window is the week this machine did nothing but develop
and benchmark OMNI, so the corpus is largely the tool measuring itself measuring
itself, and a ledger scores extremely well against a payload that is one sentence
repeated 20,000 times.

Publishing 69.8% would have replaced an honest number with a flattering one on the
strength of a corpus nobody would accept if it were described first. The rule that
follows from it, and it now governs every future run: **describe the corpus before
reading the result, and reject the run rather than the description.**

The published figures therefore stay at 0.7.3 until a window of ordinary work is
available to replay. What is not affected: the fixture table, the latency table, and
the head to head, none of which depend on that corpus.

### How much of that jump was the code, and how much was the corpus

The rejected run does answer one question, as long as it is asked as a difference
rather than as a level. Same frozen corpus, 5,984 traces and 23,086,649 bytes on both
sides, two binaries:

| | 0.7.3 | 0.7.5 + | change |
|---|---:|---:|---:|
| filters | 32.7% | 32.6% | -0.1 |
| with the ledger | 69.8% | 69.6% | -0.2 |
| bytes delivered | 6,964,958 | 7,024,805 | +59,847 |
| **folds taken** | **976** | **882** | **-94** |
| session markers | 3,316 | 3,231 | -85 |

**So the code accounts for -0.2 points of the move from 14.9%, and the corpus accounts
for the other 55.** That is the cleanest available proof that the run had to be
rejected, and it is a measurement rather than a judgement.

The 0.2 points are the price of
[#543](https://github.com/fajarhide/omni/issues/543), which gave whole-output folds a
size floor, and the fold count is the mechanism: 94 runs that used to be replaced by
a marker are now delivered whole. They averaged about 637 bytes, so each was saving
roughly 550 bytes net once its own marker was paid for. What that buys is an agent
that no longer receives a marker and no content for a payload barely larger than the
marker. Same shape as 15.4% to 14.9% in 0.7.2: the ratio drops because the behaviour
improved.

The direction is settled. The size is not: 0.2 points is itself a property of this
corpus, and on a mix with more small payloads the floor would cost more.

## Measure your own

```sh
omni stats
omni stats --share
```

Both read the same aggregation, so the share card cannot drift from the report.
Terminal output is excluded from both.
