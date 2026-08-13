# Memory across sessions

The same agent that reads too much also forgets everything the moment you restart it.
OMNI carries three kinds of memory, and they are kept for different lengths of time on
purpose.

## What is kept, and for how long

| tier | what | kept |
|---|---|---|
| **Permanent** | project knowledge, recurring error patterns, engrams, goal memory | until you delete it, except goal memory which honours its own `ttl_days` |
| **Working, 30 days** | sessions, distillation rows, hot files, the archive, the event index, the ledger | rolling window |
| **Verbatim, 7 days** | execution traces and the session transcript | shorter on purpose, two orders of magnitude heavier per row |

The short answer to "will OMNI still know my project after a month away" is yes for
the conclusions and no for the raw bytes. The boundary that matters in practice:
`omni retrieve` on content archived more than 30 days ago will not resolve.

The ledger has one more way of forgetting that is not on a clock. **At compaction its
session half is dropped entirely**, because compaction is where the agent stops
holding what it was shown, and every "already shown" claim becomes false at the same
moment. If folding seems to stop after a long session compacts, that is this, working.
The project half survives, and [The ledger](../concepts/the-ledger.md#forget) explains
the split.

## Pinning a goal

```sh
omni goal set 'Migrate the billing service off the legacy queue'
omni goal show
omni goal clear
```

The scorer favours output related to the goal, and the agent is reminded of it on
every prompt rather than drifting off task over a long session.

## Facts worth keeping

```sh
omni remember 'The staging database ignores migrations run outside the deploy job'
```

Agents with MCP wired call `omni_remember` themselves, and pull facts back with
`omni_recall`, which is a semantic search across engrams, stored knowledge and
distillation history.

Store what is not derivable from the code: a decision and its reason, a gotcha, a
constraint that no file states. Do not store what the repository already records.

## Carrying a session across a restart

Session context is injected at session start, so a new agent knows which files were
hot and what the last active error was. If the host closes or you switch tools, the
project context is still there.

```sh
omni session --status
omni session --history
omni session --resume        # resume an interrupted session
omni session --transcript
omni session --health
```

For moving to a machine or a host that shares no database, `omni_handoff` exports the
current session state as portable markdown you can paste into a new session. It is an
MCP tool only; the CLI subcommand was removed.

## Engrams

Digests of finished subtasks, written as work completes rather than reconstructed
later.

```sh
omni engram
omni engram --json
```

## Knowledge that outlives a session

```sh
omni query errors in last 5 commands
omni patterns                # errors that keep coming back across sessions
omni insight                 # via MCP: top recurring issues project-wide
```

## What it cannot do

It is per machine. There is no sync, no server, and no shared store between people.
`~/.omni/omni.db` is the whole of it, and a remote archive was
[explicitly not built](../develop/direction.md#non-goals) rather than merely not built
yet.
