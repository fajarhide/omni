# How it decides what to cut

The pipeline is fixed and every payload walks the same six stages:

```
Read → Guard → Score → Collapse → Distill → Persist
```

None of them is allowed to invent anything, and each one is allowed to decline.

## Guard

The gate. It answers one question: is this payload something a later step is going to
parse? If yes, nothing downstream runs and the bytes come back exactly as they
arrived. [What it refuses to touch](format-safety.md) is the whole of this stage and
it is worth its own page, because "OMNI did nothing" is usually this working
correctly rather than a failure.

## Score

Every line gets a relevance tier. The scorer is a pure function of the text, the
command that produced it, and whatever session history exists.

| tier | weight | what lands here |
|---|---|---|
| Critical | 1.0 | errors, failures, the verdict line, anything naming a file and a line number |
| Important | 0.7 | warnings, counts, state that changed |
| Noise | 0.1 | progress, timing, decoration, repeated ceremony |

The tiering happens **before** any distiller sees the block, which matters when you
are debugging why a distiller behaved oddly: the tier may already have decided the
outcome, so probe the segment tiers before rewriting the distiller.

## Collapse

Runs of near-identical lines become one line stating the count. Twenty
`Downloading foo v1.2.3` lines become one.

Two things about this stage surprise people. It runs **before** the distiller, so a
distiller is often reading `[N similar lines collapsed]` markers rather than the
original output, and a distiller written as though it sees raw text will misbehave.
And which collapse mode fires is chosen by specificity, so a `kubectl` command piped
into `grep` may take the infrastructure path rather than the log path.

## Distill

Now a tool-specific filter runs, chosen by matching the command. The `cargo test`
distiller keeps the counts and every failure with its assertion. The `git` distiller
keeps the changed paths. The search distiller keeps the match lines with their
filenames.

Each one implements the same trait, and the signature is the design:

```rust
fn distill(&self, segments: &[OutputSegment], input: &str,
           session: Option<&SessionState>) -> Option<String>;
```

`Option`, not `String`. A distiller that did not understand its input returns `None`
and the caller hands back the raw bytes. That is the difference between "I read this
and here is what matters" and "I recognised nothing and here is a confident summary
of it", and it is enforced by the compiler for all 12 rather than by each author
remembering to check.

## Persist

The raw input is archived, keyed by SHA-256, and the marker the agent sees carries a
handle into that archive. Covered in [Nothing is deleted](nothing-is-deleted.md).

Archiving happens even when the projection saved nothing. A block is worth
remembering because it may be seen again, not because it compressed today.

## What decides the order

Correctness beats compression at every stage, and the order they win in is written
down:

1. **Never fabricate.** A stage that recognised nothing hands back what it was given.
   A failed command passes through verbatim. Structured payloads are never touched.
2. **Never lose the answer quietly.** Anything dropped leaves a marker, and where the
   content allows, a handle that retrieves it.
3. **Then compress**, as hard as the first two allow and no harder.

The reason that ordering is explicit is that the project has broken it before. A
`kubectl` table once came out as `k8s: 2 pods` because a pod table is an enumeration
where every row is a datum. It reported a large saving. There was no noise in the
input to remove, so the saving was the answer.
