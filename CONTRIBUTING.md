# Contributing to OMNI

One document. Standards, architecture and workflow, for humans and for AI assistants
alike, because there is no useful version of this that differs between the two.

## Get it building

```bash
git clone https://github.com/fajarhide/omni
cd omni
cargo build
OMNI_DB_PATH=/tmp/omni-test.db cargo test
```

The toolchain is pinned in `rust-toolchain.toml`. `rustup` reads it automatically; a
`cargo` installed from Homebrew or a distro package ignores it and will drift from CI,
so run `source "$HOME/.cargo/env"` first if you have both.

`cargo install cargo-insta` for snapshot review.

**Set `OMNI_DB_PATH` before running tests.** Parallel tests competing for
`~/.omni/omni.db` cause SQLite locks: 79 seconds green against an isolated database,
433 seconds and then a hang against the live one.

## What OMNI is trying to be

Read this before writing code. It decides what a good change looks like here.

OMNI removes noise from what an agent reads, **without removing the answer and without
overstating what it removed.** Compression is the easy half. A distiller that deletes
a whole `kubectl` table and reports 99% saved compressed perfectly and did the job
wrongly.

Three properties, in the order they win when they conflict:

1. **Never fabricate.** A stage that recognised nothing hands back what it was given.
   A failed command passes through verbatim. Structured payloads are never touched.
2. **Never lose the answer quietly.** Anything dropped leaves a marker and, where the
   content allows, a handle that retrieves it.
3. **Then compress**, as hard as the first two allow and no harder.

Three architectural rules, from `AGENTS.md`:

- **Low latency.** Hooks run on every command and must stay under 10 ms.
- **Fail open.** A hook that panics degrades silently and lets the original output
  through. It never breaks the host agent.
- **High signal.** Trim ruthlessly, preserve the minimum needed to diagnose.

Fail open has a sharp edge worth stating: it means handing back raw bytes. It does not
mean emitting a cheerful summary. A distiller that parsed nothing returning
`vitest: ✓ 0/0 passed` is failing **closed**, and confidently.

## The pipeline

```
Read → Guard → Score → Collapse → Distill → Persist
```

**Guard** classifies the payload with `pipeline::format::sniff`. JSON, YAML, CSV or
TSV ends the pipeline and the bytes pass through untouched.

**Score** tiers every line: Critical 1.0, Important 0.7, Noise 0.1. Pure function, no
IO. `semantic::is_critical` runs before any distiller, so when a distiller misbehaves,
probe the segment tiers first.

**Collapse** turns runs of near-identical lines into `[N similar lines collapsed]`.
It runs **before** Distill, so distillers are fed collapse markers rather than raw
output. Collapse mode is picked by specificity: a `kubectl … | grep` payload takes the
Infra path, not the Log path.

**Distill** picks a filter via `registry::resolve_profile(command)` and runs it. A
matching TOML signal in `signals/` short-circuits this stage entirely, so check there
before concluding anything about the Rust code.

**Persist** archives the raw input by SHA-256, then writes the marker. That order is
not negotiable: a failed archive must leave the run verbatim, or you get a marker
pointing at content that was never stored.

**The ledger** runs after distillation, replacing runs of lines the scope has already
been shown. It is append-only, which is what keeps the upstream prompt cache intact.

`hooks/post_tool.rs` and `hooks/pipe.rs` are two doors into these same stages. Three
separate fixes have each corrected one copy and left the other. Change both, or write
down why not.

## Project layout

```
src/
├── main.rs          CLI dispatch and the single command list
├── lib.rs           library re-exports, so the crate is testable
├── agents/          one file per host: claude, cursor, codex, hermes, pi, …
├── cli/             one file per subcommand
├── distillers/      12 content filters
├── guard/           safety, limits, trust bounds, env hygiene
├── hooks/           entry points and the dispatcher
├── ledger/          cross-turn line dedup
├── mcp/             MCP server, 26 tools
├── pipeline/        scorer, collapse, registry, format gate, toml filters
├── session/         tracking, learning, correction
├── store/           SQLite and transcripts
└── util/            command families, token estimation
signals/             45 TOML signals, compiled into the binary
```

**Library first.** `main.rs` is thin. Logic lives in `lib.rs` and its submodules.

**Single source of truth.** Command-to-behaviour mapping is centralised in
`pipeline/registry.rs`. Do not duplicate `matches!(cmd, ...)` blocks in distillers or
scorers. Thresholds and limits are named constants in `pipeline/mod.rs` or
`guard/limits.rs`, never inline literals.

**IO separated from logic.** Scoring and filtering are pure functions over `&str`.

## Adding a distiller

1. `src/distillers/my_type.rs`:

```rust
use crate::pipeline::{OutputSegment, SessionState};
use super::Distiller;

pub struct MyDistiller;

impl Distiller for MyDistiller {
    fn distill(
        &self,
        segments: &[OutputSegment],
        input: &str,
        session: Option<&SessionState>,
    ) -> Option<String> {
        todo!()
    }
}
```

`Option`, and that is the design. Return `None` the moment you are not sure you parsed
the input; the caller then hands back raw bytes. The invariant lives in the trait
rather than in each author remembering, so it holds for all 12 by construction.

2. Register in `src/distillers/mod.rs`, and put the routing in `registry.rs`.
3. Add a realistic fixture in `tests/fixtures/`. Real output from the real tool.
4. Add a snapshot test, run `cargo test`, then `cargo insta review`.
5. **Break the rule deliberately and watch the test go red before restoring it.**

## Adding a signal

A file in `signals/tools/`, compiled into the binary. There is no filter path on disk.

```toml
schema_version = 1

[filters.my_tool]
description = "Drop my-tool's progress chatter"
match_command = "^my-tool\\b"
strip_lines_matching = ["^DEBUG", "^TRACE"]
max_lines = 50
on_empty = "my-tool: no issues"

[[tests.my_tool]]
name = "keeps the diagnostic"
input = "DEBUG: starting\nsrc/x.ts:3:10  error  'foo' unused"
expected = "src/x.ts:3:10  error  'foo' unused"
```

```bash
omni learn --verify
```

Match narrowly. A `match_command` slightly too wide fails silently: it claims the
tool, saves nothing, and stops the correct filter from ever running. That happened
with `biome` and `eslint`, and the comment recording it is still in the file.

Never strip the error channel (`^\[stderr\]`), structure (`^metadata:`, `^spec:`, code
fences), or anything stating a failure. `omni learn --discover` will suggest all of
those, because its learner treats "repeated" as "noise". Read its output line by line
and never paste it wholesale.

`on_empty` is what the agent reads when everything was stripped. Do not write a
verdict there; if your patterns were too wide, it is a false one.

> No config from the tree. OMNI reads no filter file from disk, neither a project's
> `.omni/signals/` nor `~/.omni/signals/`. The project tier existed behind a trust
> gate that hashed one file and admitted another, and it was deleted rather than
> repaired. Do not reintroduce a tier that lets a checkout decide what an agent is
> shown.

## Measure before you build

`~/.omni/omni.db` prices a proposal in one query, and the answer is often the opposite
of the request.

"Improve the python3 distiller" became two facts in two queries: `python3` was already
reporting 97.2%, and the savings were the collapse fallback deleting data rows. The
obvious feature, a traceback distiller, died on 9 of 7,506 traces containing a
traceback.

"Route a pipeline by its last stage" would have handed 871 of 1,035 recorded pipelines
to `head`, `tail` or `sed`, all verbatim passthroughs, stopping distillation on them
entirely.

**A measurement that kills your design is the measurement working.** Verify the signal
is correct before pricing what it is worth, too: an import-graph feature sized at 196
traces until the graph itself turned out to be wrong, then sized at 26.

Read the rows before quoting an aggregate over them, and never read `sqlite3` output
through the Bash hook while investigating OMNI, because the pipeline can fold the rows
you are counting.

## Rust standards

**Rust 2024.** `std::sync::LazyLock` over `lazy_static!`. Let-chains over nested
`if let`. Exhaustive matches; avoid `_ => {}` unless it is genuinely right.

**No `unwrap()` on IO or user input.** `.expect("contextual message")`, or better,
return a `Result`. `anyhow` at the top level with `.with_context(|| …)`, `thiserror`
where callers need to match on variants.

**Poison handling.** `.lock().unwrap_or_else(|p| p.into_inner())` on a
`Mutex<SessionState>`.

**Performance.** `Cow<'_, str>` in hot paths like the scorer and collapse.
`with_capacity` when the size is predictable. Check whether `&T` or `mem::take`
suffices before reaching for `.clone()`.

**Panic boundaries.** `catch_unwind` at the highest entry point, with
`AssertUnwindSafe` around captured state.

**Untrusted input.** All tool output is untrusted. Sanitise ANSI and control
characters. Keep `guard/env.rs` sanitising the environment before subcommands.

**Clippy is a gate, not advice.** `cargo clippy --all-targets --all-features -- -D
warnings`, zero warnings.

## Cross-platform

The CI matrix includes `windows-latest`. Four rules keep it green.

1. **No hardcoded separators.** `PathBuf` and `push`, never `/` or `\\`.
2. **No exact `\n` matching in assertions.** Use `.lines()`, or normalise `\r\n` first.
   The ledger splits with `split_inclusive('\n')` rather than `lines()` for exactly
   this reason: `lines()` drops the terminator, so rebuilding would silently rewrite
   every CRLF payload on Windows.
3. **The binary is not always `./omni`.** Use `std::env::consts::EXE_SUFFIX`.
4. **Windows environment variables are case-insensitive.** Use `eq_ignore_ascii_case`
   when reading `std::env::vars()`.

## Testing

```bash
OMNI_DB_PATH=/tmp/omni-test.db cargo test
cargo test --test hook_e2e
cargo test --test savings_assertions
cargo test --test security_tests
cargo insta review
```

**Two guardrails that waste the most time when ignored.** Isolate the database, as
above. And lock early, release fast: Rust mutexes are not reentrant, so nested
`lock()` calls on `session_arc` deadlock. Open a scope, take what you need, let the
guard drop.

If `cargo test` runs over a minute, suspect one of those two. Check pipe mode and the
E2E tests first.

**Naming.** Inside `#[cfg(test)]`, drop the `test_` prefix; the attribute already says
it. Start with a verb, describe the behaviour, English only.

```rust
fn returns_default_when_config_missing()
fn preserves_errors_during_collapse()
fn renders_identical_bytes_for_identical_state()
```

Not `test_config_ok`, not `handles_it`, not `valid_json`.

**Design.** One behavioural assertion per test. Arrange, act, assert. Test observable
behaviour, not internals.

**Coverage for anything non-trivial:** happy path, edge case, malformed input,
regression case if it is a fix, and an explicit no-panic case. Malformed input returns
`Err`, never panics.

**Prove the test can fail.** Break the rule, watch it go red, restore it. This repo
has shipped two regression tests that could not fail; both looked correct and both
passed with the fix reverted.

Three ways a green test here means nothing:

- Your fixture reaches a different collapse mode than you assumed, so the guard you
  are testing is never consulted.
- "No rewrite from the hook" is not proof a distiller punted. The format gate or the
  guardrail may have fired instead.
- A distiller can return a near-copy rather than the exact input. Detect "this did not
  help" with `beats_guardrail`, not `output == input`.

**Never weaken an assertion or remove a security check to make a test pass.**

## Gates

```bash
make ci      # fmt + clippy + test + security + binary-check
```

All of it green before a pull request. `cargo fmt`, then clippy, then tests.

## Pull requests

**Conventional commits**, scoped by module: `fix(cloud):`, `fix(pipeline):`,
`test(perf):`, `docs(changelog):`, `chore(ci):`.

**The subject says the outcome, not the edit.** `stop destroying YAML, and publish
numbers that are true` is the house style. `fix yaml bug` is not.

**`Closes #N` in the body, before the merge.** GitHub evaluates the keyword at merge
time only; adding it afterwards does nothing, silently.

**The body says what, why, and how it was verified.** For a distiller change,
"verified" means the before and after output, not "tests pass".

**One branch per batch, not per issue.** Every change touches `CHANGELOG.md`, so N
parallel branches cost N-1 merge conflicts and N full CI runs. Batch related work into
one branch, one commit per issue, one pull request with several `Closes` lines. The
conflict is always `CHANGELOG.md` and the resolution is always "keep both sides".

**Update `CHANGELOG.md` as work merges**, not at tag time. Keep a Changelog and
SemVer. Entries here are unusually detailed on purpose: each states the measured
evidence, the wrong number that was published, and the mechanism. A one-line entry is
a regression in that file's quality.

**CI green is not review-clean.** Read the review comments, automated and human,
validate each against the code rather than assuming the reviewer is right or wrong,
and fix or reply before merging.

## Reporting a bug

The bar is higher than "describe the problem", because a distiller bug is only
believable with both outputs side by side.

1. **A runnable reproduction using `omni exec`.** The form is exact:
   `omni exec npm run dev`, not `omni exec -- npm run dev`.
2. **Raw against distilled, quoted verbatim**, including the `[OMNI Active]` footer.
   The footer is often the point: the worst bugs here report the highest reductions.
3. **The line in `src/` that decides it**, if you traced it. Say plainly when you did
   not; an issue that overstates a mechanism gets fixed in the wrong place.
4. **Environment**: `omni --version`, OS, and the real toolchain versions.

Reproduce on a synthetic command where you can, so there is nothing to redact. Real
terminal output carries hostnames, account ids and internal addresses more often than
people expect.

Read the **whole** distilled output before filing. Grepping it hides the group headers
that often make the output lossless after all, and OMNI is non-deterministic against a
warm database, so isolate `OMNI_DB_PATH` per run.

Three classes, and they are not equally urgent:

| class | example | urgency |
|---|---|---|
| **False claim** | a success reported for a failing command; a count that does not match the runner's own | highest. An agent acts on it and is wrong. |
| **Lost signal** | lint counts without `file:line` | high. Costs a re-run, so it is token-negative. |
| **Noise** | an unfiltered progress line | low |

## What we welcome

- A distiller for a tool not covered
- A signal for a tool whose noise is line-shaped
- **A reproduction of any case where OMNI's output claims more than its input
  supports.** Worth more than it sounds, and the reason this project keeps its
  standards where it does.
- Performance work, with a before and after measured by removal rather than by
  microbenchmark
- Documentation fixes

## Translations

`README.md` changes must reach `i18n/README-{ja,zh,ar,id,vi,ko}.md`. Those files sit
one level deeper, so assets are `../media/…` and root links are `../CONTRIBUTING.md`.

Keep **OMNI**, **RewindStore**, **MCP** and **Hook** as they are. Translate
"distillation" and "Semantic Signal Engine" with local technical terms.

## Licence

Apache License 2.0. By contributing you agree your work is licensed under it.
