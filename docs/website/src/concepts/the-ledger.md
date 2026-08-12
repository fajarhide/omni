# The ledger

Every distiller answers the same question one command at a time: given this output,
what can be dropped.

The ledger answers a different one: given everything already shown in this session,
what is this output repeating.

The two are orthogonal, and on real corpora the second one is worth more. Replayed
over 6,656 traces, **22.9% of raw bytes were lines the agent had already been shown,
and 22.4% still were after every distiller had run.** Filtering barely dents
repetition, because repetition is not noise. Each line is perfectly good signal. It
is just signal that was already delivered.

## What it does

A run of consecutive lines that were all emitted earlier becomes one marker naming
the count and a handle. Everything else passes through byte for byte.

```
[OMNI: 40 lines already shown, omni retrieve bc7e821a4340073e]
```

It reaches the class nothing else can. File reads are the largest class in the
corpus, and the filters save **0.0%** of them, correctly: you cannot strip lines from
a file the agent asked to see without guessing which parts it meant. The ledger takes
25.2% of that same class without guessing anything, because those lines were already
delivered once.

## Two scopes, two different claims

They are not the same statement and the marker says which one it is making.

| origin | marker | what it means |
|---|---|---|
| session | `N lines already shown` | the agent is still holding these bytes, so the handle is free unless it chooses to re-read |
| project | `N lines from an earlier session` | these went to a different session of this project and this agent has **never seen them** |

The distinction is the whole reason the project scope exists. An earlier design
cancelled it on the grounds that a handle for another session's content is a lie,
which was right about the wording and wrong about the remedy: the fix is to stop
saying "already shown", not to stop remembering.

Because the project claim is not free, it carries a higher bar. A session-origin run
must save 150 bytes over its marker; a project-origin run must save three times that,
since the agent has no choice about paying a retrieval if it needs the content.

> There is a known defect here as of 0.7.2. A run that was folded gets recorded as
> shown in the session scope, even though only the marker was delivered, so the next
> occurrence in that session says `already shown` about bytes it never received and
> folds at a third of the intended bar. Tracked as
> [#465](https://github.com/fajarhide/omni/issues/465).

## The rules it inherits

**Append-only.** It only ever shortens the output of the command in flight and never
rewrites anything already delivered. That is what keeps the upstream prompt cache
intact: a cache works on a prefix, so shortening the suffix costs nothing while
retroactive compaction would destroy it.

**Deterministic.** The same ledger state renders byte-identical output. The handle is
a content address and carries no timestamp. An earlier design used
`{timestamp}_{hash}` and made 4 of 73 repeated inputs emit different bytes.

**Nothing is lost.** A run is archived before its marker is written, and a failed
archive leaves the run verbatim.

**Failures are never folded.** A line stating a failure is exempt however often it
has been shown. "You have seen this already" is sound for informational lines and
wrong for the error channel, where the repetition is the signal: the same TypeError
on a re-run means the bug is still there. Eliding it delivers source context and no
statement of what went wrong, which an agent reasonably reads as the failure being
fixed. Marking the line unseen rather than filtering it afterwards also splits the
run around it, so the frames either side still fold.

**Unknown means untouched.** Structured payloads never reach the ledger at all.

## What it is worth

From the same replay, the ledger is 12.2 points on top of OMNI's own filters and 11.4
points on top of a competitor's, which is the clearest statement that it is
orthogonal to whose patterns run:

| | bytes | saved |
|---|---|---|
| omni, filters only | 6,469,047 to 6,292,856 | 2.7% |
| rtk `pipe` | 6,469,047 to 6,067,012 | 6.2% |
| lean-ctx `compress` | 6,469,047 to 6,073,757 | 6.1% |
| omni, with the ledger | 6,469,047 to 5,506,627 | **14.9%** |
| rtk `pipe` + omni's ledger | 6,469,047 to 5,333,483 | 17.6% |

The last row is deliberate. A reader who wants the largest possible number would run
their filters with our ledger, and saying so is cheaper than being caught not saying
it.
