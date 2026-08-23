# `omni stats`

Token savings analytics, read from your own database.

```sh
omni stats
```

One screen, and it answers one question: how many bytes never reached the model.

```
  OMNI 0.7.6 · savings                       last 30 days · 17,873 calls

    5.1 MB not sent to the model    ▁█▁▁▂▆▁▁▁▁  ▁  last 14 days

      folded        1.7 MB   31%    1,277 calls
      distilled     3.4 MB   48%    1,046 calls
      each % is of that stage's own bytes

      15,550 calls passed through untouched. Nothing deleted, nothing
      invented, no call came back larger.
```

**Two stages, two bases, and never one percentage.** The distiller's share is of the
bytes it distilled; the ledger's is of the payload of the calls it folded. Those are
different populations, so adding or averaging them produces a figure about neither. The
same rule holds in `--json`, where each stage is its own object and a stage with no
recorded base reports `null` rather than a share over the rows that happen to carry one.

**"Not sent", never "saved".** A currency figure is not computable from this data, the
marker costs a few bytes back, and a handle the agent pulls returns some of them. What
the line claims is checkable against the host's own transcript.

The sparkline is one column per day for fourteen days, both stages together, scaled to
the busiest day. A day with no recorded call is blank, because the lowest glyph would
claim activity that did not happen.

## Flags

| flag | effect |
|---|---|
| `--since <window>` | `hour`, `today`, `week`, `month` (default), `all` |
| `--view <name>` | `summary` (default), `detail`, `commands`, `projects`, `context`, `rerun`, `share` |
| `--limit <n>` | Rows in a table view, default 8, `0` for all |
| `--card` | Write the summary as an image, sized for social posts |
| `--json` | Machine readable, for the selected view |
| `--help`, `-h` | Help |

Every earlier spelling still resolves: `--detail`, `--today`, `--week`, `--month`,
`--hour`, their short forms, `--all-commands`, `--project`, `--context`, `--rerun` and
`--share`. They are not listed above because there is one way to say each thing now, and
they print no deprecation notice: the rename was ours, not yours.

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
calls are handed back untouched by design, so read the per-stage rows to see where the
work actually happened. That is why the summary no longer prints one.

**Session lifetime moved to `--view detail`.** It answers a question about the window
rather than about a call, and the summary is for the second question.

**`--share` and `--card` cannot drift from the report.** Both read the same
aggregation as `omni stats` itself, which was a deliberate choice after an earlier
version computed them separately.
