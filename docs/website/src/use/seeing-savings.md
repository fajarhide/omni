# Seeing what it saved

```sh
omni stats
```

Everything on this page reads the same aggregation, so a figure in the share card
cannot drift from the one in the report.

## The report

```sh
omni stats                 # last 30 days, the default
omni stats --today         # or --hour, --week, --month
omni stats --detail        # commands, routes, sessions, agents
omni stats --all-commands  # every command, not just the top ones
omni stats --project       # broken down per project path
omni stats --json          # machine readable
```

It leads with **session lifetime**: how many commands a session carries before the
host closes it. That is the meter a user actually watches. The distillation
percentage below it is a diagnostic for one host's pipeline, not a product claim.

## Reading it without fooling yourself

**Split by `agent_id` before quoting anything.** Rows recorded under `terminal` are
TTY bytes no model ever read. On one installation those were 73% of every byte OMNI
claimed to have saved. `omni stats` excludes them now, but the same trap waits for
anyone querying the database directly.

**A high percentage is not automatically good.** The worst defects in this project's
history reported the highest reductions, because deleting the answer compresses very
well. Pair any number with `omni diff` on a real command.

**A low percentage is usually correct.** Around 97% of calls save nothing because
there was nothing to save. Structured payloads, failed commands and enumerations all
pass through by design.

## The check a percentage cannot make

```sh
omni stats --rerun
```

Which distillers cost a re-run. If a distiller removes something the agent then has
to go and fetch again, the reduction was not a saving, it was a deferral. Nothing in
a byte count can see that.

## Sharing it

```sh
omni stats --share     # copy-pasteable summary of your own measured savings
omni stats --card      # the same summary written as an image
```

Both come from your own database, which is the point. A ratio claim in someone else's
README cannot be verified before installing.

## In a browser

```sh
omni dashboard             # http://127.0.0.1:7717
omni dashboard --port 8080
```

Read-only, same database, binds loopback and nothing else.

## Digging further

```sh
omni stats --detail              # per-command and per-route breakdown
omni query errors in last 5 commands
omni query warnings from cargo
omni query timeline today
omni patterns                    # errors that keep coming back
omni patterns --tool cargo
```

`omni_history` gives the same per-call rows to an MCP client. There is no `omni history`
subcommand; this page listed one until 0.7.4.

`omni query` speaks a small fixed query language rather than free text. The supported
forms are listed in its own help.

## Querying the database directly

`~/.omni/omni.db` is plain SQLite and there is nothing stopping you.

> Never read `sqlite3` output through the Bash hook while investigating OMNI. The
> pipeline can fold the rows you are trying to count, and a `LIKE` filter that catches
> the wrong rows has already put a wrong figure into a published issue. List the rows
> before quoting any aggregate over them, and set `OMNI_PASSTHROUGH=1`.
