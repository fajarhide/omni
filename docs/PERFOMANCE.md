# OMNI — Measured Performance

![OMNI Performance Dashboard](https://omni.weekndlabs.com/media/performance.png)

Every number here was produced by the method below. Nothing is projected, rounded
up, or quoted from a run nobody kept. Where a number is missing, it is because it
has not been measured.

---

## Method

- **Corpus**: 1,810 command executions replayed from `execution_traces` — one
  developer's real usage, not a synthetic benchmark. 15,015,504 bytes of raw tool
  output.
- **Binary**: release build.
- **State**: a **fresh `HOME` per invocation**, so every trace sees an empty
  database.
- **Measured**: 2026-07-17.

The fresh-state requirement is not a detail. OMNI feeds session history into its
scorer, so against a warm database **the same binary on the same input does not
produce the same output** — 21 of 30 traces differed run-to-run in one check (one
gave 1,835 bytes, then 433). A benchmark that reuses a live `~/.omni/omni.db`
measures its own history, not the code. Comparing two builds requires isolating
state per invocation; a shared-database sweep reported 1,204 traces changed when
only 50 had.

---

## Headline

| Metric | Value |
|--------|-------|
| Bytes reaching the model | 15.0 MB → 6.2 MB |
| **Net savings across the whole mix** | **58.9%** |
| Calls where OMNI saved nothing (passthrough) | **63.6%** (1,151 of 1,810) |
| Calls where OMNI *added* bytes | **0** |
| Calls that actually shrank | 36.4% (659 of 1,810) |

Two-thirds of the time OMNI hands the output straight back and adds zero bytes.
All of the saving comes from the remaining third. This is the number to judge the
tool by: a claim of "90% off every command" is a claim that output you needed was
summarised away.

---

## Where the saving comes from

Same 1,810 executions, grouped by command:

| Command | Calls | Input | Output | Saved |
|---------|-------|-------|--------|-------|
| `npm` | 7 | 59 KB | 1 KB | **98.0%** |
| `cargo` | 29 | 424 KB | 13 KB | **96.8%** |
| `git` | 256 | 5.9 MB | 509 KB | **91.3%** |
| `az` | 7 | 48 KB | 4 KB | **90.7%** |
| `ls` | 52 | 71 KB | 29 KB | **59.5%** |
| `kubectl` | 212 | 4.4 MB | 2.3 MB | **48.0%** |
| `find` | 39 | 83 KB | 53 KB | **36.2%** |
| `grep` | 184 | 534 KB | 385 KB | **27.8%** |
| `sed` | 41 | 88 KB | 71 KB | **19.7%** |
| `echo` | 132 | 391 KB | 326 KB | **16.7%** |
| `cat` | 85 | 515 KB | 468 KB | **9.1%** |
| `curl` | 8 | 53 KB | 49 KB | **7.7%** |

`git` alone supplies more than half the total saving. `cat`, `curl` and `echo`
are close to no-ops — OMNI is not the reason to run them.

---

## Single fixtures

Reproducible by hand from `tests/fixtures/`:

| Command | Fixture | Input | Output | Saved |
|---------|---------|-------|--------|-------|
| `cargo build` | `cargo_build_large.txt` | 3,220 B | 9 B | **99.7%** |
| `pytest` | `pytest_pass.txt` | 501 B | 18 B | **96.4%** |
| `cargo test` | `cargo_test_500.txt` | 16,515 B | 1,100 B | **93.3%** |
| `docker build` | `docker_build_layered.txt` | 309 B | 31 B | **90.0%** |
| `pytest` | `pytest_failures.txt` | 730 B | 136 B | **81.4%** |
| `git status` | `git_status_dirty.txt` | 496 B | 113 B | **77.2%** |
| `git log` | `git_log.txt` | 158 B | 39 B | **75.3%** |
| `git diff` | `git_diff_multi_file.txt` | 397 B | 220 B | **44.6%** |
| `docker build` | `heavy_noise.txt` | 9,207 B | 5,783 B | **37.2%** |
| `kubectl get pods` | `kubectl_get_pods_mixed.txt` | 840 B | 762 B | **9.3%** |
| `cargo build` | `cargo_build_errors.txt` | 317 B | 292 B | **7.9%** |

`cargo test` at 93.3% is where OMNI is most useful: it drops hundreds of `... ok`
lines while quoting cargo's own tally (`test result: FAILED. 490 passed; 10
failed`) rather than recounting it, so the failures survive intact.

---

## Latency — a real cost

OMNI runs on every hooked command, and the price grows with your database:

| Input | Database | End-to-end (incl. process start) |
|-------|----------|----------------------------------|
| 496 B (`git status`) | fresh | **~82 ms** |
| 496 B (`git status`) | 97 MB (real) | **~308 ms** |
| 16.5 KB (`cargo test`) | fresh | **~276 ms** |

The pipeline itself is fast; the tax is process start plus SQLite against an
accumulating history. Budget for roughly a quarter-second per hooked command on a
mature database.

---

## Format safety

Compression only ever touches human-facing free text. Anything a later step parses
— JSON, NDJSON, YAML, TSV/CSV — passes through byte-for-byte, gated by
`pipeline::format::sniff` ahead of collapse. A missed compression is cheap; a
corrupted payload is not.

This is enforced, not aspirational. A `kubectl kustomize` manifest carrying an
embedded Vault HCL block scalar used to defeat the sniffer and come back as
`docker logs: 323 lines, no errors detected` — 13,463 bytes of Kubernetes config
replaced by a sentence. Fixing the sniffer moved reported savings **down**, from
65.3% to 58.9% across the corpus, because six of those points were destroyed
manifests rather than removed noise.

---

## What is not measured

- **Cost in dollars.** OMNI is a hook; it never sees the API's `usage` block, so
  it cannot know whether your bytes were billed as fresh input or as a ~10× cheaper
  prompt-cache read. It reports bytes and tokens, and no longer guesses at USD.
- **Time-to-first-token, answer quality, hallucination rate.** Plausible, never
  benchmarked here.

*To see your own numbers rather than these, run `omni stats`.*
