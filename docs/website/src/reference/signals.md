# Signals

A signal is a TOML rule saying which lines of a tool's output are ceremony. There are
**45** of them, all compiled into the binary.

```
signals/
├── tools/      41 files, one per tool: cargo, eslint, kubectl, terraform, pytest, …
└── domains/     3 files, cross-cutting: build, deploy, test
```

## They are compiled in, on purpose

OMNI reads no filter file from disk. Not `~/.omni/signals/`, not a project's
`.omni/signals/`. Both tiers were removed in 0.7.0.

The project tier is the one that mattered. It made a filter something a **repository
could ship to its visitors**, so a checkout could decide what an agent is shown. It
sat behind a trust gate that hashed one file and admitted another, and the decision
was to delete it rather than repair it.

The measurement made that easy. The whole filter layer is worth 804 bytes over 6,656
recorded commands, and on `kubectl` it was negative. What it cost in attack surface it
was not paying back.

So the set that runs is the set the tests cover, identically on every installation. If
a tool needs a signal, it ships in the binary for everyone.

## What one looks like

```toml
schema_version = 1

[filters.eslint]
description = "Filter noise from eslint and prettier check output"
match_command = "(eslint|prettier)"
strip_ansi = true
strip_lines_matching = [
    "^✓ ",
    "^✔ ",
    "^Checked \\d+ file",
    "^All matched files",
]
on_empty = "lint: no issues"

[[tests.eslint]]
name = "lint errors found with context"
input = """…"""
expected = """…"""
```

| key | meaning |
|---|---|
| `match_command` | Regex against the command line. This is the whole of the routing. |
| `strip_lines_matching` | Lines to drop |
| `strip_ansi` | Remove colour codes first |
| `max_lines` | Cap the output |
| `on_empty` | What to say when everything was stripped |

`[[tests.<name>]]` blocks are mandatory in practice: `omni learn --verify` runs every
one of them, and a signal with no test is a signal nobody can refactor safely.

## Signals shadow distillers

A matching signal short-circuits the Rust distiller entirely. When a hook-level
reproduction disagrees with what you expect a distiller to do, check `signals/` before
concluding anything about the Rust code.

## What they are good at, and where they are useless

From the 6,656-trace replay:

| class | filters take |
|---|---|
| build and test | 76.9% |
| search | 4.8% |
| `git`, `gh` | 4.4% |
| infra | 4.4% |
| **file reads** | **0.0%** |

That last row is correct behaviour rather than a gap. You cannot strip lines from a
file the agent asked to see without guessing which parts it meant, and guessing is
what the trust floor forbids. [The ledger](../concepts/the-ledger.md) is what reaches
that class, and it takes 26.3% of it without guessing anything.

## Checking them

```sh
omni learn --verify                    # every signal's inline tests
omni doctor --test-filter eslint       # one signal
omni doctor --benchmark                # signals slower than 5 ms
omni doctor --coverage                 # coverage against your own history
omni doctor --validate path/to/x.toml  # syntax and tests for a file
```

## Adding one

See [Adding a signal](../develop/adding-a-signal.md). It is a file in `signals/`, a
test block, and `omni learn --verify`.

## The comment in `eslint.toml` is the lesson

> `biome` moved to its own filter: this one claimed it and saved 0.0% on it, because
> biome prints a block per diagnostic where eslint prints a line.

A `match_command` regex that is slightly too wide does not fail loudly. It claims a
tool, saves nothing on it, and stops the right filter from ever running. Match
narrowly.
