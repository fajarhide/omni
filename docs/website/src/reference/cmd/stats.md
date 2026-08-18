# `omni stats`

Token savings analytics, read from your own database.

```sh
omni stats
```

Leads with **session lifetime**, how many commands a session carries before the host
closes it. The distillation percentage below it is a diagnostic for one host's
pipeline, not a product claim.

## Flags

| flag | effect |
|---|---|
| `--detail` | Full breakdown: commands, routes, sessions, agents |
| `--hour`, `-H` | Scope to the last 60 minutes |
| `--day`, `--today`, `-d` | Today only |
| `--week`, `-w` | Last 7 days |
| `--month`, `-m` | Last 30 days, the default |
| `--all-commands` | Every command, not just the top ones |
| `--project` | Break down per project path |
| `--context` | Context composition signals |
| `--rerun` | Which distillers cost a re-run |
| `--share` | A copy-pasteable summary of your measured savings |
| `--card` | Write that summary as an image, sized for social posts |
| `--json` | Machine readable |
| `--help`, `-h` | Help |

## `--rerun` is the one to know

Reduction percentage cannot tell you whether a distiller removed something the agent
then had to fetch again. If it did, the reduction was a deferral, not a saving. This
flag is the check that percentage cannot make.

## Traps

**Terminal rows are not tokens.** Output written to a TTY is read by a human, not a
model. On one installation those rows were 73% of every byte OMNI claimed to have
saved. `stats` excludes them now, and so does the benchmark harness, but anyone
querying `~/.omni/omni.db` directly has to filter by `agent_id` themselves.

**A high number deserves suspicion.** The worst defects in this project reported the
highest reductions, because deleting the answer compresses very well. Pair any figure
with `omni diff` on a real command.

**A low aggregate is usually right, and it is not the number to judge OMNI by.** Most
calls are handed back untouched by design, so read the per-class rows to see where the
work actually happened.

**`--share` and `--card` cannot drift from the report.** Both read the same
aggregation as `omni stats` itself, which was a deliberate choice after an earlier
version computed them separately.
