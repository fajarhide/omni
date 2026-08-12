# `omni learn`

Finds repeated noise in your own command history and proposes signals for it.

```sh
omni learn --discover
omni learn --dry-run
omni learn --verify
```

## Flags

| flag | effect |
|---|---|
| `--discover` | Discover and view candidate patterns |
| `--dry-run` | Preview the generated TOML without writing |
| `--from-queue` | Use the background learning queue as the source |
| `--verify` | Run the inline tests for all existing signals |
| `--apply` | Accepted so the flag is not silently ignored. Signals are compiled in now, so it writes nothing. |
| `--help`, `-h` | Help |

## What changed in 0.7.0, and why this command is smaller than it looks

Signals are compiled into the binary. OMNI reads no filter file from disk: not
`~/.omni/signals/`, not a project's `.omni/signals/`. Both tiers were removed.

The project tier was the serious one. It made a filter something a repository could
ship to its visitors, so a checkout could decide what an agent is shown. It sat behind
a trust gate that hashed one file and admitted another, and it was deleted rather than
repaired.

That leaves `learn` as a discovery aid rather than a configuration tool. It tells you
what your history looks like, and a pattern worth keeping becomes an issue, then ships
in the binary for everyone.

`--apply` still parses because a flag that vanishes silently is worse than one that
says it does nothing.

## Never paste its output wholesale

`--discover` and the `omni_find_noise` MCP tool are **advisory**, and the learner
treats "repeated" as "noise". Those are not the same thing.

Observed on 0.6.8: it learned a one-off `gh issue list` row and a truncated Terraform
`description = "Name` as noise patterns, and its suggested strip list included
`^metadata:`, `^spec:`, a code fence, and `^\[stderr\]`. Those are structure and the
error channel. Stripping them would delete exactly what a reader needs.

Read the suggestion, take only the patterns you can justify line by line, and never
paste the block anywhere.

## Verifying the signals that do ship

```sh
omni learn --verify              # inline tests for every signal
omni doctor --test-filter <name> # one signal
omni doctor --benchmark          # signals slower than 5 ms
omni doctor --coverage           # coverage against your past commands
```
