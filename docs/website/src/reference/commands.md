# Commands

Every subcommand, grouped the way `omni --help` groups them: by what you are trying
to do, not alphabetically.

```
omni <COMMAND> [FLAGS]
cmd | omni                # distill any command's output through a pipe
```

## Set up

| command | what it does |
|---|---|
| [`init`](cmd/init.md) | Install OMNI into your agent, hooks and MCP |
| [`doctor`](cmd/doctor.md) | Check the install is healthy, and fix what is not |
| [`update`](cmd/rest.md#update) | Upgrade to the latest release |
| [`reset`](cmd/rest.md#reset) | Uninstall cleanly, keeping a backup of your config |

## See what it saved

| command | what it does |
|---|---|
| [`stats`](cmd/stats.md) | How many tokens were cut, and from which commands |
| [`retrieve`](cmd/retrieve.md) | Print the content a marker archived, by its handle |
| [`dashboard`](cmd/rest.md#dashboard) | The same numbers in a browser, on 127.0.0.1 |
| [`diff`](cmd/rest.md#diff) | The last command's output, before against after |
| [`session`](cmd/session.md) | What this session has spent, and on what |

## Tune it

| command | what it does |
|---|---|
| [`learn`](cmd/learn.md) | Build filters from the noise in your own history |
| [`exec`](cmd/exec.md) | Run one command through OMNI, to see what it would do |
| [`query`](cmd/rest.md#query) | Search past distillations |
| [`patterns`](cmd/rest.md#patterns) | Errors that keep coming back |

## Memory

| command | what it does |
|---|---|
| [`remember`](cmd/rest.md#remember) | Save a fact for future sessions |
| [`engram`](cmd/rest.md#engram) | Digests of finished subtasks |
| [`goal`](cmd/rest.md#goal) | Pin a north-star goal so scoring favours it |
| [`version`](cmd/rest.md#version) | Version and environment details |

## Hook entry points

Not for typing. These are what an agent host invokes, and they are documented in
[Hooks](hooks.md).

```
omni --pre-hook      omni --post-hook     omni --hook
omni --session-start omni --session-end   omni --pre-compact
omni --mcp
```

## A note on how flags are parsed

`clap` routes the subcommand and then hands each module the raw `env::args()`, so
every module parses its own flags. Two consequences worth knowing:

An unknown flag is not always an error. `omni stats --detial` once printed the default
overview and exited 0, and `omni init --curser` ran the interactive default while
installing nothing that was asked for. Check the output matches what you asked for.

Per-command help is real and worth reading: `omni <command> --help`. A few commands
(`exec`, `diff`, `remember`, `version`) fall through to a generic clap help that
documents less than the module actually accepts. Where that happens, this reference
records what the source accepts rather than what the help prints.
