# What OMNI is

A local program that edits what your AI agent reads, before the agent reads it.

That is the whole idea. Everything else in this section is about the rules it follows
while doing it, and those rules are more interesting than the editing.

## The problem it exists for

An agent working in a terminal spends most of its context on output nobody chose to
send it. A test run is 400 lines of `ok` and one line that matters. A build is a
compile log and a verdict. A file gets read, then read again three turns later
because nothing remembered the first read.

None of that is free. It fills the context window, which ends the session sooner, and
it costs a re-read every time the conversation is compacted.

The usual answers are worse than the problem. Truncating output cuts the end, which is
where the verdict lives. Summarising with a model costs a call per command. Telling
the agent to be careful works until it is busy.

## Where it sits

Every serious agent host can run a program when a tool finishes and use what that
program returns. Claude Code calls it a `PostToolUse` hook, Cursor and the others have
their own name for the same idea. OMNI installs itself there.

```
your command  →  the host runs it  →  raw output
                                          │
                                    OMNI's hook
                                          │
                              distilled output  →  the agent's context
                                          │
                                   raw output  →  local SQLite archive
```

Two consequences follow from that position, and they are the reason this shape was
chosen over a proxy.

It sees output, not requests. Your API key never passes through it, no request is
delayed waiting on it, and if it dies the host carries on with the raw bytes.

It cannot help where the host will not let it. A host that does not apply a hook's
rewrite to its built-in shell tool will show the agent the same bytes no matter how
good the filters get. That is not a bug to fix in OMNI, it is a property of the host,
and [Supported agents](../reference/agents.md) says which host is on which tier.

## What it does to a command

Four things, in order, and any of them may decide to do nothing:

1. **Refuse.** JSON, YAML, base64, terraform plans, anything a later step is going to
   parse: handed back untouched. See [What it refuses to touch](format-safety.md).
2. **Filter.** A distiller that understands this tool keeps the verdict and the
   failures and drops the ceremony. There are 12 of them, covering build, test, git
   and other version control, search, cloud, database, JavaScript and TypeScript
   tooling, file reads, security scanners and system operations, plus a generic
   fallback.
3. **Collapse.** Long runs of near-identical lines become one line saying how many
   there were.
4. **Fold.** Lines the agent has already been shown become a handle instead of a
   repeat. This is [the ledger](the-ledger.md), and on real corpora it does more work
   than the filters do.

Then the raw input goes into the archive, and the agent gets the result plus a marker
saying what happened.

## What it is not

**Not a compressor.** It is not trying to make output small. It is trying to make
output that an agent can act on, next to a number a human can check. Those pull in
different directions more often than you would expect, and when they conflict the
number loses.

**Not a summariser.** No model runs inside the pipeline. The budget for a hook is
single-digit milliseconds and nothing with an inference call fits in it.

**Not a memory product, though it has one.** `omni remember`, `omni goal` and the
session handoff exist because the same agent that reads too much also forgets
everything between sessions. [Memory across sessions](../use/memory.md) covers that
half.

## The rule it is most serious about

> A stage that recognised nothing hands back what it was given.

The failure this project keeps having to fix is not lost bytes. It is a confident
summary of input that was never parsed: a `find` that reported 99% saved by throwing
away the file paths that were the answer, a `cargo test` that said `1 passed` about a
run cargo itself called `490 passed`, a dev server reported as a passing test suite.

Every one of those compressed beautifully. All of them were wrong. So the trait
that every distiller implements returns `Option<String>`, and a distiller that
failed to parse returns `None` and the caller hands back the raw bytes. It is
enforced by the type rather than by the author remembering.
