# Reading the markers

A marker is OMNI telling you what it did. There are only a few shapes, and knowing
them is the difference between trusting the tool and suspecting it.

## The shapes

```
[OMNI: 406 lines omitted, omni retrieve 0000000000000000 for full output]
```

Content was cut and archived. The 16 characters are a handle:
`omni retrieve <handle>` prints the original back, byte for byte, from any
shell in any session.

```
[OMNI: 40 lines already shown, omni retrieve 0000000000000000]
```

The ledger. These lines were emitted earlier **in this session**, so the claim is that
the agent is still holding them and the handle costs nothing unless it wants to
re-read.

```
[OMNI: 40 lines not shown here, omni retrieve 0000000000000000]
```

Also the ledger, different claim. These lines went to a **different session** of this
project, and this agent has never seen them. The wording is deliberately not "already
shown", because that would be false. Folding them is a bet that the agent will not
need them, and it carries three times the profitability bar for that reason.

That other session may also have been a different agent. The project history is keyed
on the directory, so anything running in this repository contributes to it. See
[what two agents share](../concepts/the-ledger.md#what-two-agents-in-one-repo-share).

```
[OMNI: identical to the 40 lines already shown, omni retrieve 0000000000000000]
[OMNI: identical to 40 lines from an earlier session, none shown here, omni retrieve 0000000000000000]
```

The same two claims, for a reply that is repeated **in full**. When the fold covers
every line, the marker is the whole output rather than a gap inside it, so it says
`identical to` and you get one line where a re-run would have printed the same
hundreds. Anything less than the whole reply keeps the wording above.

```
[OMNI: 40 lines already shown from charlie.tf, omni retrieve 0000000000000000]
```

Any of the four can carry `from <source>`, naming the command whose output first
showed those lines. It appears only when that command is **not** the one you just
ran, which is the case you cannot resolve from the marker alone: reading one file
and having a block elided because a different file showed it earlier. Without the
clause, comparing two files to check a shared block matches is answered by deleting
the evidence.

Re-reading the same file carries no clause, and that is deliberate rather than an
omission. Marker length decides what is worth folding at all, so a source on every
marker would cost savings on the common case to label the rare one.

```
[N similar lines collapsed]
```

Collapse. A run of near-identical lines, replaced by a count.

```
[OMNI Active] ⏺ 93.7% reduction (2.3 KB → 147 B) 3ms
```

The footer, on `omni exec` and pipe mode. Input size, output size, and how long the
pipeline took.

```
[Partial signal]
```

The pipeline recognised some of the output but not all of it.

## Reading a percentage correctly

The worst bugs in this project's history reported the **highest** reductions. A
distiller that deletes the answer compresses beautifully.

So a large number is not on its own good news. `omni diff` is the check:

```sh
omni diff     # the last command, raw against distilled
```

If a 99% saving turns out to have removed the file paths that were the answer, that is
a bug worth reporting, and it is the exact class this project cares about most.

## When there is no marker at all

Most of the time, and that is the pipeline working rather than failing. OMNI hands the
output straight back whenever taking anything would be unsafe or would not pay:

- The payload is JSON, YAML, CSV or TSV. Never touched, on purpose.
- The command failed. A non-zero exit passes through verbatim.
- There was no noise to remove. A `kubectl get pods` table is an enumeration where
  every row is a datum.
- The output was too short to be worth a marker.

## Getting content back

```sh
omni retrieve <handle>
```

Works on every host, with or without MCP. Agents with the MCP server wired can call
`omni_retrieve` themselves without asking you.

One boundary a handle cannot promise: the archive is a rolling 30 day window, so
`omni retrieve` on content older than that will not resolve. Verbatim traces are
shorter still at seven days.

## Telling a real marker from a printed one

Markers appear in prose too. This page is full of them, so is OMNI's source, and so
is any bug report that quotes one. That matters if you are measuring whether OMNI
was active on a run, because searching a transcript for the marker shape will find
the examples as readily as the folds.

The handle is what separates them. Every worked example in this manual and in OMNI's
own source uses one reserved value, `0000000000000000`, which no real fold can ever
be assigned:

```sh
omni retrieve 0000000000000000   # exit 1, "the documentation example"
omni retrieve <handle-you-found> # exit 0 if OMNI really folded it
```

So the exit code answers the question, and a marker copied out of documentation
cannot be mistaken for evidence that anything was shortened.
