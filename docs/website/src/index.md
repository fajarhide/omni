# OMNI

Your agent reads terminal output. Most of that output is noise, and you pay for
every byte of it: progress bars, passing test lines, log preambles, the same file
printed twice because the agent forgot it had already looked.

OMNI sits on a hook your agent already has, decides what is worth keeping, and keeps
a receipt for everything it removes.

```
$ cargo test
    Compiling omni v0.7.2
     Running unittests src/lib.rs
running 412 tests
test pipeline::scorer::tests::scores_errors_critical ... ok
... 409 more lines ...
test result: FAILED. 411 passed; 1 failed

# what the agent is handed instead
cargo test: 411 passed, 1 failed
  FAILED ledger::tests::renders_identical_bytes_for_identical_state
  assertion `left == right` failed at src/ledger/mod.rs:601
[OMNI: 406 lines omitted, omni retrieve 3f7bfd89bc5d7cee for full output]
```

The failing test survived. The 406 lines of `... ok` did not, and the handle at the
end brings them back byte for byte if anything ever needs them.

## Two ways to read this

**You want to use it.** Start at [What OMNI is](concepts/what-it-is.md), then
[Install](use/install.md). You will be running in about five minutes, and
[Reading the markers](use/markers.md) is the one page worth reading properly,
because the markers are how OMNI tells you what it did.

**You want to work on it.** [Architecture](develop/architecture.md) and
[The pipeline, stage by stage](develop/pipeline.md) are the map.
[Adding a distiller](develop/adding-a-distiller.md) is the most common change, and
[Testing](develop/testing.md) carries the two traps that waste the most time here.

## What it will not do

It will not send anything anywhere. Every stage runs locally and the archive is a
SQLite file in your home directory.

It will not sit between you and your model. There is no proxy, no API key handed to a
local process, and no command to prefix. That was
[decided against](develop/direction.md#non-goals) and the reasoning is written down.

It will not quietly guess. A stage that failed to parse its input hands the input
back unchanged, structured payloads are never touched at all, and anything removed
leaves a marker saying so. Those three rules outrank compression, in that order,
whenever they conflict.

## The honest numbers

Replayed over 6,656 real commands on 2026-08-12 against 0.7.2: **14.9% fewer bytes**
across the whole mix, and **97.3% of calls saved nothing at all** because there was
nothing to save. The full method, the corpus window, and the head to head against rtk
including the part OMNI loses are in [Benchmarks](develop/benchmarks.md).

If you want a number that describes your machine rather than someone else's, run
`omni stats` after a few days.

## Where to ask

[Discord](https://discord.gg/zHTuvZhF2M) for questions, and for the case this project
cares about most: OMNI stating a result its input does not support. The
[issue tracker](https://github.com/fajarhide/omni/issues) works too, and a report with
the raw and distilled output side by side gets fixed either way.
