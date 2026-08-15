# Reading the markers

A marker is OMNI telling you what it did. There are only a few shapes, and knowing
them is the difference between trusting the tool and suspecting it.

## The shapes

```
[OMNI: 406 lines omitted, omni retrieve 3f7bfd89bc5d7cee for full output]
```

Content was cut and archived. The 16 characters are a handle:
`omni retrieve 3f7bfd89bc5d7cee` prints the original back, byte for byte, from any
shell in any session.

```
[OMNI: 40 lines already shown, omni retrieve bc7e821a4340073e]
```

The ledger. These lines were emitted earlier **in this session**, so the claim is that
the agent is still holding them and the handle costs nothing unless it wants to
re-read.

```
[OMNI: 40 lines not shown here, omni retrieve bc7e821a4340073e]
```

Also the ledger, different claim. These lines went to a **different session** of this
project, and this agent has never seen them. The wording is deliberately not "already
shown", because that would be false. Folding them is a bet that the agent will not
need them, and it carries three times the profitability bar for that reason.

That other session may also have been a different agent. The project history is keyed
on the directory, so anything running in this repository contributes to it. See
[what two agents share](../concepts/the-ledger.md#what-two-agents-in-one-repo-share).

```
[OMNI: identical to the 40 lines already shown, omni retrieve bc7e821a4340073e]
[OMNI: identical to 40 lines from an earlier session, none shown here, omni retrieve bc7e821a4340073e]
```

The same two claims, for a reply that is repeated **in full**. When the fold covers
every line, the marker is the whole output rather than a gap inside it, so it says
`identical to` and you get one line where a re-run would have printed the same
hundreds. Anything less than the whole reply keeps the wording above.

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

Most of the time. Around 97% of calls save nothing and hand the output straight back.
That is the pipeline working, not failing. It happens when:

- The payload is JSON, YAML, CSV or TSV. Never touched, on purpose.
- The command failed. A non-zero exit passes through verbatim.
- There was no noise to remove. A `kubectl get pods` table is an enumeration where
  every row is a datum.
- The output was too short to be worth a marker.

## Getting content back

```sh
omni retrieve 3f7bfd89bc5d7cee
```

Works on every host, with or without MCP. Agents with the MCP server wired can call
`omni_retrieve` themselves without asking you.

One boundary a handle cannot promise: the archive is a rolling 30 day window, so
`omni retrieve` on content older than that will not resolve. Verbatim traces are
shorter still at seven days.
