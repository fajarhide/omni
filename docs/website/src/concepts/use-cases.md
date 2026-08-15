# Where OMNI helps

Six situations, with the measured number attached to each. Two of them are cases where
OMNI does nothing, and those are in here on purpose: a tool that claims to help
everywhere is a tool nobody can predict.

Every figure comes from the same replay of 6,656 real commands described in
[Benchmarks](../develop/benchmarks.md), so they are averages over a real mix rather
than a good day picked out of a log.

## 1. The agent keeps re-reading the same files

**The situation.** You ask for a refactor. The agent reads `auth.rs`, wanders off to
check a caller, comes back and reads `auth.rs` again. Six turns later it reads it a
third time. Every read is charged at full price, and none of the repeats told it
anything the first one did not.

**What OMNI does.** The second read comes back as a marker with a handle. The lines are
already in the agent's context; sending them again is paying twice for one fact.

**The number: 25.0%** off file reads across the corpus, and up to **97.2%** off a
single repeated read of one file.

This is the biggest single win in the whole product and it is invisible while it works,
which is why the marker exists.

## 2. A test suite fails and you cannot see why

**The situation.** 412 tests, one failure, and the failure is on line 388 of the
output. Your agent reads all 412 lines to find it, and if the run is long enough the
host truncates the tail, which is exactly where the verdict lives.

**What OMNI does.** The test distiller keeps the tally and every failure with its
assertion and file position, and drops the passing lines.

**The number: 78.0%** off build and test output.

This is the case where filtering, not the ledger, does the work. Test output is
enormously repetitive within one run, so there is real ceremony to remove before
anything has been seen twice.

## 3. `git log` and `git diff` fill the screen

**The situation.** One commit's `Author`, `Date` and wrapped body is five lines. Fifteen
commits is a screen and a half, and your agent wanted the subjects.

**What OMNI does.** Every commit is kept, as one `hash subject` line. Nothing is
summarised away and no commit disappears; the envelope around each one goes.

**The number: 22.1%** across `git` and `gh` on the corpus, and **94%** on a verbose
`git log -15` specifically.

## 4. Your session dies at the context limit, repeatedly

**The situation.** Long debugging session, and about two hours in the conversation
compacts. The agent loses the thread, re-reads files it had already understood, and you
re-explain the task.

**What OMNI does.** Two things. Less context spent per command means the wall arrives
later. And [memory across sessions](../use/memory.md) survives the compaction: project
knowledge, recurring error patterns, and the goal you pinned with `omni goal` are in
SQLite, not in the context window.

**The honest limit.** OMNI cannot stop a compaction, and at the moment one happens it
deliberately forgets what it had shown you, because the licence to replace lines with a
handle is that the agent is still holding those lines, and compaction is when that stops
being true.

## 5. You switch agents, or machines, mid-project

**The situation.** You start in Claude Code, move to Codex CLI for a change, and both
of them start from nothing.

**What OMNI does.** The store is one SQLite file keyed by project path, not by agent.
A second agent working in the same directory reads the same project knowledge, and the
ledger's project scope will hand it a handle for output an earlier session already
produced. That marker says `not shown here` rather than `already shown`,
because this agent has genuinely never seen those bytes and the wording has to be true.

**The honest number.** Cross-session repetition is **3.7%** of post-filter bytes against
**19.1%** within a session, so this is worth about a fifth of the in-session saving.
It is real, and it is not the headline.

**The honest caveat.** Two agents in one repository share that history by side effect
rather than by design. The marker used to say `from an earlier session`, which reads as
*your* earlier session when it was someone else's, and worse, as a claim the content had
already arrived; it now says `not shown here`. [The ledger](the-ledger.md#what-two-agents-in-one-repo-share) is
straight about what is and is not keyed on the agent today.

## 6. `kubectl get pods -o json | jq`

**The situation.** You pipe structured output into something that parses it.

**What OMNI does: nothing.** JSON, YAML, NDJSON, CSV and TSV pass through byte for
byte. A compressor that reformats a payload the next command is about to parse has not
saved you anything, it has broken your pipeline.

**The number: 0%**, by design. See [What it refuses to touch](format-safety.md).

## And one more where nothing happens

`kubectl get pods` with 35 pods returns a table where every row is a fact. There is no
ceremony to drop and nothing has been seen before, so OMNI hands back all 35 rows and
reports a 0% saving.

**97.3% of all calls in the corpus are like this.** That is the number worth
internalising: OMNI is not a thing that shrinks everything a little, it is a thing that
does nothing most of the time and a great deal occasionally. The 14.9% aggregate is
what is left after every one of those zeroes is counted in.

## What this adds up to

| Class of command | Calls in the corpus | Saved |
|---|---|---|
| build and test | 69 | 78.0% |
| file reads | 699 | 25.0% |
| `git`, `gh` | 661 | 22.1% |
| search (`grep`, `rg`, `find`) | 828 | 13.3% |
| infra (`kubectl`, `az`, `docker`) | 254 | 8.2% |
| everything else | 4,145 | 6.9% |
| **all of it** | **6,656** | **14.9%** |

Run `omni stats` after a few days and you get this table for your own history, which is
the only version of it that describes your work.
