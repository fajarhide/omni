---
name: omni
description: "Use when installing, verifying or configuring OMNI, when command output carries an `[OMNI: ...]` marker, or when output looks shorter than expected. OMNI is a local hook that shortens shell output before the model reads it and hands back a handle for what it cut."
---

# OMNI

OMNI runs on the machine, as a hook the agent host calls. It rewrites the output
of a shell tool call before the model reads it, and every cut it makes leaves a
marker and a handle to fetch the bytes back. No account, no telemetry, nothing
about the machine leaves it.

## Install

```
brew install fajarhide/tap/omni
omni init
```

Without Homebrew, take the release archive and check it against the `SHA256SUMS`
published beside it, then put the binary on the path:

```
V=0.7.5; T=x86_64-unknown-linux-musl   # or aarch64-unknown-linux-musl, *-apple-darwin
B=https://github.com/fajarhide/omni/releases/download/v$V
curl -fsSLO $B/omni-v$V-$T.tar.gz -O $B/SHA256SUMS
grep " omni-v$V-$T.tar.gz\$" SHA256SUMS > omni.sha256 &&
  sha256sum -c omni.sha256 &&
  tar xzf omni-v$V-$T.tar.gz &&
  install -m755 omni ~/.local/bin/omni
```

`sha256sum -c` exits non-zero on a mismatch and the `&&` carries that into the
next command; on its own line the untar would run anyway, since a shell pasting
these does not stop on a failure unless it was told to. The checksum goes through
a file rather than a pipe for the same reason: piped into `sha256sum -c -`, a
`grep` that matches nothing sends an empty stream, and Darwin's `sha256sum` reads
that as success, so a mistyped target would install unverified. Through a file it
is `grep`'s own exit code that gates the chain. On macOS the command is
`shasum -a 256 -c omni.sha256`.

**Do not name a host you have not established you are running in.** With no
terminal to draw its menu on, which is the case when an agent runs it, `omni init`
configures the host it is running inside and prints which one it picked. If it
cannot name the host, a plain shell for instance, it stops and lists the flags
rather than installing somewhere nobody asked for. Only then pick one:
`--claude`, `--cursor`, `--codex`, `--gemini` and the rest under
`omni init --help`. `omni init --all` takes every supported host and also writes
a `.vscode/mcp.json` in the working directory.

The hooks land in that host's own configuration, and **the host has to restart
before they run**, so say that rather than reporting the install as active.

## Verify

```
omni doctor
```

A healthy install names the binary, the config directory, the database, and one
line per host:

```
  Binary:         omni v0.7.5 [LATEST]
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
| `[OMNI: 40 lines not shown here, omni retrieve <handle>]` | another session in this project saw them and **this one never did**, so retrieve before relying on them |
| `[OMNI: identical to 40 lines from an earlier session, none shown here, omni retrieve <handle>]` | the whole reply matched an earlier session's, and none of it arrived here |
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
