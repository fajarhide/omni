# OMNI

**Your AI agent pays to read the same output over and over. OMNI stops that.**

It is one small program that sits between your terminal and your agent. It runs
locally, it needs no API key, and once it is installed you never type its name again.

```bash
brew install fajarhide/tap/omni && omni init
```

## The problem, in one screen

Your agent runs a test suite. Four hundred lines come back, one of them matters.

```
$ cargo test
    Compiling omni v0.7.3
     Running unittests src/lib.rs
running 412 tests
test pipeline::scorer::tests::scores_errors_critical ... ok
... 409 more lines of "ok" ...
test result: FAILED. 411 passed; 1 failed
```

Here is what your agent reads instead:

```
cargo test: 411 passed, 1 failed
  FAILED ledger::tests::renders_identical_bytes_for_identical_state
  assertion `left == right` failed at src/ledger/mod.rs:601
[OMNI: 406 lines omitted, omni retrieve 3f7bfd89bc5d7cee for full output]
```

The failure survived. The 406 lines of `ok` did not. And that handle on the last line
brings every one of them back, byte for byte, if anything ever needs them.

## The part nobody else does

Filtering noise is the easy half, and several tools do it. Here is the half that is
harder, and it is where most of OMNI's saving comes from.

Your agent reads a file. Three turns later it reads the same file again, because
nothing remembered the first read. You pay full price both times.

OMNI remembers. The second read comes back as one line:

```
[OMNI: 178 lines already shown, omni retrieve 77a0c474f2e55351]
```

**A 7.6 KB file read twice costs 7.6 KB and then 214 bytes. That is 97.2% off the
second read.** Nothing was deleted: those lines are already sitting in your agent's
context from the first read, so sending them again buys nothing. The handle is there
in case they scroll out of reach.

This is called the ledger, and on real command histories it does more work than every
filter combined.

## What you get

| | |
|---|---|
| **Longer sessions** | Less context spent on ceremony means more turns before you hit the wall, and fewer compactions that lose your thread. |
| **Lower bills** | 14.9% fewer bytes across 6,656 real commands. On file reads, 25.0%. On `git`, 22.1%. On build and test output, 78.0%. |
| **Nothing lost** | Everything removed is archived locally. `omni retrieve <handle>` prints it back. |
| **Nothing invented** | If OMNI cannot understand output, it hands it back untouched rather than guessing. |
| **Memory between sessions** | Close your editor, come back tomorrow, switch from Claude Code to Codex: the project context is still there. |
| **Nothing to change** | No proxy, no API key, no command to prefix. Install it and use your terminal normally. |

## Where it actually helps

[Where OMNI helps](concepts/use-cases.md) walks through six situations with the real
numbers attached, including the two where it does nothing and why that is correct.

## Start here

**Just want it working.** [Install](use/install.md) takes about five minutes. Then read
[Reading the markers](use/markers.md), which is the one page worth your time, because
the markers are how OMNI tells you what it did.

**Want to understand it first.** [What OMNI is](concepts/what-it-is.md), then
[The ledger](concepts/the-ledger.md).

**Want to work on it.** [Architecture](develop/architecture.md) and
[The pipeline, stage by stage](develop/pipeline.md) are the map.
[Adding a distiller](develop/adding-a-distiller.md) is the most common change.

## Three things it will not do

**It will not send anything anywhere.** Every stage runs on your machine and the
archive is a SQLite file in your home directory.

**It will not sit between you and your model.** There is no proxy and no API key handed
to a local process. That was [decided against](develop/direction.md#non-goals) on
purpose, and the reasoning is written down.

**It will not quietly guess.** A stage that failed to understand its input hands the
input back unchanged. Structured data like JSON and YAML is never touched at all.
Anything removed leaves a marker saying so. Those three rules outrank compression, in
that order, every time they conflict.

## The honest version of the numbers

Across 6,656 real commands, **97.3% of calls saved nothing at all**, because there was
nothing to save. A two-line `git status` has no ceremony to drop and no repeats to
fold, so OMNI hands it straight back rather than inventing a saving to report.

The 14.9% is what is left after counting all of those zeroes. It is a real average over
a real mix, not a best case picked from a good day.

We publish the comparison we lose, too: on filtering alone, rtk gets 6.2% on that
corpus and OMNI gets 2.7%. It is the ledger that puts OMNI ahead overall, and running
rtk's filters with OMNI's ledger would beat both. [Benchmarks](develop/benchmarks.md)
has the method and the command to reproduce every row on your own history.

If you want a number that describes your machine rather than someone else's, run
`omni stats` after a few days.

## Where to ask

[Discord](https://discord.gg/zHTuvZhF2M) for questions, and especially for the case
this project cares about most: OMNI stating a result its input does not support. The
[issue tracker](https://github.com/fajarhide/omni/issues) works too. A report with the
raw and distilled output side by side gets fixed either way.
