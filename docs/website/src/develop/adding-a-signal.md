# Adding a signal

A signal is a TOML rule for one tool's noise. Cheaper than a distiller, and the right
answer when the noise is line-shaped rather than structural.

## The file

`signals/tools/my_tool.toml`, compiled into the binary. There is no filter path on
disk.

```toml
schema_version = 1

[filters.my_tool]
description = "Drop my-tool's progress chatter"
match_command = "^my-tool\\b"
strip_ansi = true
strip_lines_matching = [
    "^DEBUG",
    "^TRACE",
    "^Checked \\d+ file",
]
max_lines = 50
on_empty = "my-tool: no issues"

[[tests.my_tool]]
name = "keeps the diagnostic, drops the chatter"
input = """
DEBUG: starting
src/index.ts:3:10  error  'foo' is unused
DEBUG: done
"""
expected = """src/index.ts:3:10  error  'foo' is unused"""
```

```sh
omni learn --verify
```

## Match narrowly

`match_command` is the whole of the routing, and a regex slightly too wide fails
silently rather than loudly. It claims the tool, saves nothing, and stops the correct
filter from ever running.

This is not hypothetical, and the comment recording it is still in the file:

> `biome` moved to its own filter: this one claimed it and saved 0.0% on it, because
> biome prints a block per diagnostic where eslint prints a line.

Two tools that look similar on the command line can have completely different output
shapes. Give them separate files.

## What never to strip

**The error channel.** `^\[stderr\]` is not noise.

**Structure.** `^metadata:`, `^spec:`, code fences. Those are what makes output
readable.

**Anything stating a failure.** Repetition of an error is the signal, not the noise:
the same TypeError on a re-run means the bug is still there.

`omni learn --discover` will suggest all of the above, because its learner treats
"repeated" as "noise". Read its output line by line and never paste it wholesale.

## `on_empty` is a trap

It fires when everything was stripped, and whatever you write there is what the agent
reads.

Do not write a verdict. `lint: no issues` is a claim about the run, and if your strip
patterns were too wide it is a false one. Say what happened, not what it means.

## Prove the test can fail

Break the rule, watch the test go red, restore it. `omni learn --verify` runs every
inline test in every signal, so a signal with no test is a signal nobody can refactor
safely.

```sh
omni learn --verify
omni doctor --test-filter my_tool
omni doctor --benchmark            # flags anything over 5 ms
omni doctor --validate signals/tools/my_tool.toml
```

The 5 ms bar is real. Hooks have a sub-10 ms budget, and 249 line-filter regexes once
compiled on every command whether or not their filter matched, costing 17.1 ms per
hook for work only one filter read.

## Is a signal even the right tool

Measure first. The whole signal layer is worth 804 bytes over 6,656 recorded commands,
and on `kubectl` it is negative.

Signals are excellent where output is ceremony around a verdict (build, test, lint,
76.9% on that class) and worthless where every line is a datum (file reads, 0.0%,
correctly). If your tool is in the second group, the honest answer is that there is
nothing to filter and [the ledger](../concepts/the-ledger.md) is what will help.
