---
name: omni
description: "Use when installing, verifying or configuring OMNI, when command output carries an `[OMNI: ...]` marker, or when output looks shorter than expected. OMNI is a local hook that shortens shell output before the model reads it and hands back a handle for what it cut."
---

# OMNI

OMNI runs on the machine, as a Claude Code hook. It rewrites the output of a Bash
tool call before the model reads it, and every cut it makes leaves a marker and a
handle to fetch the bytes back. No account, no telemetry, nothing about the machine
leaves it.

## Install

`omni init` with no flags opens a menu and needs a terminal, so name the host.

```
brew install fajarhide/tap/omni
omni init --claude
```

Without Homebrew:

```
curl -fsSL omni.weekndlabs.com/install | bash
omni init --claude
```

That writes the hooks into `~/.claude/settings.json` and registers the MCP server
in `~/.claude.json`. **Claude Code has to restart before the hooks run**, so say
that rather than reporting the install as active.

`omni init --all` takes every supported host and also writes a `.vscode/mcp.json`
in the working directory. `omni init --help` lists the rest.

## Verify

```
omni doctor
```

A healthy install names the binary, the config directory, the database, and one
line per host:

```
  Binary:         omni v0.7.3 [LATEST]
  Config dir:     ~/.omni/ [OK]
  Database:       ~/.omni/omni.db (0 distillations, 0 sessions) [OK]

 Agent Integrations:
  Claude Code: Full, 5 checks [OK]
```

`Full` is the line to check: it means this host applies the rewrite, so the model
reads shortened output. `MCP-only` means memory and session state and no shell
distillation. `omni doctor --fix` repairs what it can, and reports what it could
not rather than claiming success.

## Reading a marker

A marker means bytes were cut and can be fetched back. Do not treat the shortened
output as the whole output, and do not write a file back from output that carries
one.

| marker | what happened |
| --- | --- |
| `[OMNI: 40 lines omitted, omni retrieve <handle> for full output]` | the distiller kept the signal and archived the rest |
| `[OMNI: 40 lines already shown, omni retrieve <handle>]` | those lines are in this session's context already |
| `[OMNI: identical to the 40 lines already shown, omni retrieve <handle>]` | the whole re-run matched an earlier one |
| `[OMNI: 40 lines from an earlier session, omni retrieve <handle>]` | another session in this project saw them |
| `[OMNI: 2 sensitive value(s) redacted]` | values under a key that names a credential |

Pull the full bytes with the command the marker prints:

```
omni retrieve <handle>
```

The `omni_retrieve` MCP tool does the same without a shell. Reach for either
whenever the answer depends on what was cut: a count, an exact line, or a file
being rewritten from what was read.

## When output looks wrong rather than short

Corrupted output, a success reported for something that failed, or a marker
written into a file is a bug, not compression. Capture the command and its output
and file it at <https://github.com/fajarhide/omni/issues>, then get a clean copy
by running the command through `omni exec` with the pipeline off:

```
OMNI_PASSTHROUGH=1 omni exec <the command>
```
