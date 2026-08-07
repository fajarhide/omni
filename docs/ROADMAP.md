# OMNI Roadmap

What OMNI is for, what it will not become, and how we will know it is finished.

This file holds direction only. The queue lives on the
[Now / Next / Later board](https://github.com/users/fajarhide/projects/2) and the
shipped history lives in [CHANGELOG.md](../CHANGELOG.md). Copying either one here
is how the previous version of this file spent six weeks announcing v0.6.0 as
in progress while 0.6.8 shipped.

**Last reviewed: 2026-07-29, against v0.6.8.** Review at every minor release, and
whenever one of the three axes below changes state.

## The goal

OMNI removes noise from what an AI agent reads, without removing the answer and
without overstating what it removed.

Compression is the easy half. A distiller that deletes a whole `kubectl` table and
reports 99% saved compressed perfectly and did the job wrongly. So the target is
not a reduction percentage. It is output an agent can act on, next to a number a
human can reproduce.

Three properties, in the order they win when they conflict:

1. **Never fabricate.** A stage that recognised nothing hands back what it was
   given. A failed command passes through verbatim. Structured payloads are never
   touched.
2. **Never lose the answer quietly.** Anything dropped leaves a marker and, where
   the content allows, a rewind hash.
3. **Then compress**, as hard as the first two allow and no harder.

## The number that decides progress

Adopted 2026-08-07 (#357), replacing blended reduction % as the north star.

**Primary: context-window pressure for the same job.** Conversation growth,
turns before compaction, and the cost of recovering task state in a new chat.
That is the meter a user watches and the one OMNI is bought to move.

**Secondary: distill %.** Always scoped by `agent_id`, always model-facing only.
It is a diagnostic for one host's pipeline, not a product claim.

Why the swap. On the reporting corpus, 81% of calls are Passthrough and
correctly do nothing, so a blended % describes the command mix more than the
product. `terminal` rows are TTY bytes no model reads. Prompt-cache reads bill
about a tenth of fresh input, so bytes saved once are not dollars saved per turn.
And on a flat-rate plan compression does not reduce a bill at all; what it buys
is session lifetime and fewer re-runs.

A host that cannot rewrite built-in tool output cannot move the secondary number
however good the distillers get, which is why the tier a host is on is product
law rather than a footnote. See the tier table in `README.md`; `omni doctor`
prints the same tiers.

**Gate on any public headline number.** It cites the `agent_id` it covers, the
corpus it was measured on, and a command a reader can run to reproduce it.
A figure that blends `terminal` with hook agents, or counts a rewrite the host
never applied, does not ship. #212 and #324 are what that rule is made of.

## Non-goals

Recorded with dates, because the useful part of a rejected option is the reason.

| not building | why | decided |
|---|---|---|
| An HTTP proxy in front of the model | It puts OMNI on the request path and routes the user's API key through a local process. The hook is the product, and the absence of that friction is most of the advantage. | 2026-07-23 |
| A model or an ML compressor inside the pipeline | Hooks have a sub-10 ms budget (`AGENTS.md`). Nothing with an inference call meets it. | 2026-07-23 |
| Chasing a higher reduction % by making distillers more aggressive | The failure mode this project keeps shipping is a confident summary that deleted the answer. More aggression buys the number and costs the product, and on a host that cannot rewrite built-in tool output it buys nothing at all. | 2026-08-07 |
| Claiming shell distillation on a Handoff-first or MCP-only host | The host does not apply the rewrite, so the model reads the same bytes it always did. Saying otherwise is the same defect as a distiller reporting a saving it did not make. | 2026-08-07 |
| Intercepting a host's shell by denying it and returning output as a hook message | Technically possible on Cursor (`beforeShellExecution` deny + `agent_message`). It tells the agent its command was blocked, loses the exit code, moves execution semantics into OMNI, and bypasses the host's approval flow. Rejected with the option recorded on #352. | 2026-08-07 |
| Filter marketplace, team mode, remote RewindStore, IDE extension | Ecosystem features for a tool whose core claims are not all true yet. Worth reopening once the three axes below are done. | 2026-07-29 |

## The three axes

Everything that counts as progress moves one of these. A change that moves none of
them can still be worth making, but it is maintenance, not direction.

### 1. Correctness — nothing is asserted that was not parsed

**Done when** a distiller cannot ship a confident summary of input it failed to
read, because the dispatch boundary enforces it instead of each author
remembering to.

**Where it stands.** `require_parsed` exists and is voluntary. On `main` at the
time of writing, 3 of 12 distiller files call it: `cloud.rs` at four sites,
`jsts.rs` at four, `security.rs` at one. Every fabrication issue in the CHANGELOG
landed in one of the other nine. Tracked as #250.

**How to check:** no open bug describes OMNI asserting a result it did not parse,
and that stays true across a full release cycle. The tracker is the measurement;
this class of issue has been filed against nine separate releases, so a quiet
month is not evidence.

### 2. Coverage — the hook reaches the tools agents actually use

**Done when** OMNI distills the tool calls that dominate a real session rather
than `Bash` alone.

**Where it stands.** The `PostToolUse` matcher is registered for `Bash` only, so
the Read, Grep and WebFetch distillers are written, tested, and have never run in
a live Claude Code session. Tracked as #172, gated on #246.

**How to check:** the installed hook configuration names more than one matcher,
and `~/.omni/omni.db` holds distillation rows for a tool other than `Bash`.

### 3. Proof — every published number can be reproduced

**Done when** a stranger with the repo can re-derive any figure the README
claims, and no headline blends measurements from environments that behave
differently.

**Where it stands.** The headline is one blended number. Terminal runs
(`omni exec`, pipe mode) and hook runs (`claude_code`) compress very differently
and are counted together; a saving is booked when OMNI produces it, whether or not
the host applied it; duplicate rows inflate the top entries. Tracked as #212,
#173 and #118.

**How to check:** every published figure states the `agent_id` it was measured
on, the corpus it was measured over, and the command that reproduces it.

## Off the axes

Dependency and CI hygiene, i18n README sync, dead-code removal, packaging and
release mechanics. Real work, regularly done, and deliberately not part of the
direction above.

## After the three axes

The ecosystem questions become worth asking once the core claims hold. The first
two are what a second agent besides Claude Code actually needs, and whether
anyone wants a filter they did not write themselves. Both are cheap to answer
later and expensive to guess at now.

## Contributing

See [DEVELOPMENT.md](DEVELOPMENT.md). The most useful contributions are a
distiller for a tool we do not cover, a TOML filter for an internal tool, and a
reproduction of any case where OMNI's output claims more than its input
supports.
