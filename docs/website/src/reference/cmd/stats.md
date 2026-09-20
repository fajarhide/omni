# `omni stats`

Token savings analytics, read from your own database.

```sh
omni stats
```

Leads with the bytes that never reached your model, then one line per engine, each
percentage against its own base, then the command classes those bytes came from. The
aggregate below them mixes in every call OMNI deliberately declined, so it is a
diagnostic for one host's pipeline rather than a product claim.

Every view draws the same frame, `OMNI · <view> · <window>` between two rules, and
`--view context` carries no window because it reads the live session rather than a
period.

## Flags

| flag | effect |
|---|---|
| `--since <window>` | `hour`, `today`, `week`, `month` (default), `all` |
| `--view <name>` | `summary` (default), `detail`, `projects`, `context`, `rerun`, `folds`, `share` |
| `--limit <n>` | Rows in a table view, default 10, `0` for all. Read by `detail`, `projects` and `rerun`; a table it cuts says how many rows it hid |
| `--json` | Machine readable, scoped by `--since` |
| `--card` | Write the summary as an image, sized for social posts |
| `--help`, `-h` | Help |

Every earlier spelling still resolves: `--detail`, `--today`, `--day`, `-d`, `--week`,
`-w`, `--month`, `-m`, `--hour`, `-H`, `--all-commands`, `--project`, `--context`,
`--rerun`, `--share` and `--view commands`. They are not listed because there is one way
to say each thing now, and they print no deprecation notice: the rename was ours, not
yours. `--view commands` is in that list rather than the table above because it renders
the detail view and always did.

`--json` and `--card` are output formats rather than views. `--card` outranks everything,
since naming it can only mean writing the file; `--json` outranks `--view`, since there is
one machine-readable report and it is not per view. Both used to be read as views, which is
how `--view detail --card` came to write no image.

## `--view folds` is the ledger's own calibration

A fold is a claim: the reader does not need these bytes. A retrieve on that marker's
handle is the reader disagreeing. This view puts the two side by side, per marker
shape, so the guard with the highest rate is the one to look at.

```sh
omni stats --view folds              # the last 30 days
omni stats --view calibration --since week
```

`calibration` is the same view under the word most readers reach for first.

There is no threshold and no verdict colour in it. No bar has been measured yet, and a
number that looks judged when nothing judged it is the defect this project exists to
fight. A store written before markers were recorded says so rather than printing a
confident zero.

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
