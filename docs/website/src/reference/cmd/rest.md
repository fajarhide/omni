# Everything else

The commands that need a paragraph rather than a page.

## `update`

```sh
omni update
```

Fetches the latest release from GitHub and upgrades. Homebrew installations only at
present; other install methods upgrade through their own channel.

Re-run `omni init` afterwards if the release notes say the hook contract changed.

## `reset`

```sh
omni reset            # interactive menu
omni reset --all      # every integration, and offers to wipe omni.db
omni reset --claude   # one host
```

Per-host flags mirror [`init`](init.md): `--claude`, `--cursor`, `--zed`, `--cline`,
`--roo` / `--roo-code`, `--copilot`, `--gemini`, `--opencode`, `--codex`,
`--antigravity`, `--hermes`, `--pi`.

`--all` is the only one that offers to delete your database, and it asks first. It
keeps a backup of the configuration it removes.

## `dashboard`

```sh
omni dashboard
omni dashboard --port 8080     # default 7717
```

The same numbers `omni stats` prints, in a browser. Read-only, reads the same
database, and binds `127.0.0.1` and nothing else. Ctrl-C stops it.

## `diff`

```sh
omni diff
```

The last command's output, raw against distilled. The fastest way to build trust in
what OMNI is doing, and the first thing to run when a result looks wrong.

## `query`

```sh
omni query errors in last 5 commands
omni query warnings from cargo
omni query context for src/main.rs
omni query timeline today
omni query timeline today --json
```

A small fixed query language over distillation history, not free text. Four forms are
supported and they are the four above. `--json` for machine-readable output.

## `patterns`

```sh
omni patterns
omni patterns --tool cargo
```

Errors that keep coming back across sessions. `--tool <name>` scopes to one tool.

Useful for the question "have I hit this before", which is the one a fresh session
cannot answer on its own.

## `remember`

```sh
omni remember 'The staging database ignores migrations run outside the deploy job'
```

Stores a fact in persistent memory, retrievable later through `omni_recall` or the
session context injection.

Worth storing: a decision and its reason, a gotcha, a constraint no file states. Not
worth storing: anything the repository already records.

## `engram`

```sh
omni engram
omni engram --json
```

Digests of finished subtasks, written as work completes.

## `goal`

```sh
omni goal set 'Migrate the billing service off the legacy queue'
omni goal show
omni goal clear
```

Pins a north-star goal. The scorer favours output related to it, and the agent is
reminded of it rather than drifting over a long session. Goal memory honours its own
`ttl_days` rather than the standard retention tiers.

`set` is the default subcommand, so `omni goal 'some text'` also works.

## `version`

```sh
omni version
omni version --json
```

Version and environment details: build date, git hash, and the paths OMNI resolved for
its configuration and database. Worth including in any bug report.
