# Benchmarks, in full

The short version lives in the [README](../README.md#benchmarks). This is the rest
of it, including the parts that do not flatter us.

Measured on the release binary by replaying **9,965 real command executions** from
one developer's actual usage:

```bash
cargo test --release --test bench_replay -- --ignored
```

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

* **43.3% fewer bytes** across the entire mix, noisy and quiet commands together
  (40.1 MB → 22.7 MB).
* **90.0% of calls saved nothing at all.** OMNI handed the output straight back and
  added zero bytes. Every byte of the saving comes from the other 10%.
* **Not one call in 9,965 made the output larger.**

## Which population was measured

The corpus counts only calls whose result reached a model. Terminal output is
excluded: it is 68% of the raw bytes on this installation, and including it would let
us print 79.1% instead of 43.3%. We don't, because that number is measuring a
population no model ever read.

This was a real defect, not a hypothetical. `tests/bench_replay.rs` counted terminal
bytes until #324; `omni stats` had the same bug until #212. Both are fixed and both
now print which population they used.

## Where the saving comes from

| Command | Calls | Input | Output | Saved |
|---------|-------|-------|--------|-------|
| `cargo` | 124 | 1.5 MB | 127 KB | **91.4%** |
| `git` | 931 | 12.0 MB | 1.3 MB | **89.2%** |
| `kubectl` | 456 | 5.5 MB | 1.3 MB | **76.5%** |
| `az` | 62 | 264 KB | 176 KB | **33.6%** |
| `grep` | 938 | 2.4 MB | 2.0 MB | **18.1%** |
| `gh` | 232 | 534 KB | 509 KB | **4.6%** |
| `cd` | 2,963 | 5.6 MB | 5.5 MB | **2.2%** |
| `cat`, `ls`, `find`, `sed`, `python3` | 1,235 | 4.2 MB | 4.2 MB | **0%** |

`git`, `cargo` and `kubectl` carry the entire result. The last row is the point of
the table: five of the most-run commands are deliberate passthroughs, because their
output is an enumeration where every line is a datum. They used to report savings,
and each of those savings was a row someone needed.

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
