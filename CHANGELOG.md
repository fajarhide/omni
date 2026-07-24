# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).


## [Unreleased]

### Fixed
- **The JS/TS fallback deleted 270 of 300 lines without saying so, and the result was published as a 90% saving (#188)**: `distill_fallback` stopped at 30 lines with a bare `break` and returned `out.trim()` — no marker on any path, no count, no rewind hash. A 300-warning `npm install` and a 30-warning one produced output that looked identical, ending mid-sequence at `npm WARN deprecated fake-package@1.0.29`, and nothing downstream could tell which it was reading. That is the never-drop violation #111 and #176 were filed for, in a distiller neither of them touched, on the path `npm`/`npx`/`pnpm`/`yarn`/`bun` all route through. Measured in the hook's own stage order — `score_segments(raw)` then `distill_with_command(raw)`, which is what `post_tool` has done since #116 — **before: 300 lines / 18,490 B → 30 lines / 1,819 B with markers found `[]`; after: → 31 lines / 1,836 B ending `… and 270 more`**. The marker list searched was `omitted`, `collapsed`, `truncated`, `more`, `not shown`, `[OMNI`; none were present. **Two facts were confirmed before any edit, because the routing here misleads**: `npm install` really does reach the Rust distiller — `sys_npm` would claim it, but `sys_build_domain` wins the alphabetical `find()` race, strips nothing, fails `beats_guardrail`, and falls through (the #110 shape already described at `post_tool.rs:283`) — and `distill_fallback` really is the arm that runs, since no `is_*_output` detector matches a wall of `npm WARN` and `30` appears nowhere else in the file. An earlier draft of the issue also claimed `generic.rs` shared the defect; **that was wrong and was retracted on the issue** — `GenericDistiller` emits `... [omitted]`, `[N noise lines omitted]` and `... [N more lines]`, all with counts, and is untouched here. The tail reuses `… and N more`, which this file already emitted from `capped_lines`; both now share `more_tail`, so **no fourth phrasing was added** to a repo that already has three (`readfile::omitted_note`, `generic.rs`, `collapse`) — and because `capped_lines`' rendered string is unchanged, no snapshot moved. The two nested loops collapsed into one iterator chain so the cap is applied once rather than re-tested per line, the pnpm/yarn filter that was written out twice became a predicate, and the zero-state sample path (nothing scored Critical or Important) was dropping lines just as silently — it now counts against every line there was, not the ten it sampled. Also deleted: an `if filtered_input.len() < input.len()` in `JsTsDistiller::distill` whose two arms called `distill_fallback(segments, session)` with identical arguments, so the condition decided nothing. **Every other cap in the file was audited and deliberately left alone**, each already carrying its count: vitest and playwright print the true failure total in the header before listing 5 and 3 names, tsc adds `+N more files`, and eslint labels its `take(3)` as "top rules" beneath a header holding the real totals. All four regression tests were proven to fail in **both** directions — neutering `more_tail` to return `None` reddens the three that assert the marker's presence and count, and forcing it to always emit reddens exactly the one that asserts silence when nothing was dropped, so none of them is decorative. This was only reachable by a user as of 0.6.5: before #187 the host rejected OMNI's payload and restored the original bytes, so no agent had ever seen the truncated output.

## [0.6.5] - 2026-07-24

### Fixed
- **The distilled output still never reached the agent on Claude Code — #158 fixed the key and left the value shape wrong (#187)**: `hookSpecificOutput.updatedToolOutput` is not a free-form object. Claude Code runs `tool.outputSchema.safeParse(value)` **keyed on the tool that ran**, so there is one shape per host tool, and OMNI was sending the MCP `{status, result}` to all of them. Bash's schema wants `{stdout: string, stderr: string, interrupted: boolean, …}`, so every payload was rejected, `Oe = re.data` restored the original bytes, and the UI rendered `PostToolUse:Bash hook warning` — while the sibling `additionalContext` went through and printed a saving for a distillation that had just been discarded. **This is #158's symptom, not a new one**: that issue was closed on a fix that corrected the key, and the observable behaviour — savings reported, nothing applied — never actually changed on this path. It survived two releases because the only thing that changed was the field name in an error nobody was reading. Measured on Claude Code 2.1.218 from the session transcript rather than from OMNI's own numbers (`grep -c hook_error_during_execution ~/.claude/projects/<proj>/<session>.jsonl`): **27 of 34 Bash calls in a single session were rejected**, each with `stdout: expected string, received undefined` / `stderr: …` / `interrupted: expected boolean, received undefined`, and one of them printed `-3247tok this call | 67% compression` for 300 `npm WARN` lines the agent received **in full**. The 7 quiet calls are the ones where OMNI correctly emits nothing at all — non-zero exit (#120) and the format-safe gate. The reply is now **the response object that arrived, with `stdout` swapped and every other key preserved**: `interrupted`, `isImage`, `backgroundTaskId`, `persistedOutputPath` and `timedOutAfterMs` are schema members, and rebuilding a minimal object would have failed validation exactly as the old shape did. The shape is read off the incoming payload rather than asserted from a table of per-tool schemas, so it stays correct for tools this code has never seen — "reply in the shape you were spoken to in" needs no table to maintain. `stderr` is **blanked rather than echoed**, because `normalize` folds a non-empty stderr into the text that gets distilled and returning the original as well would show it to the agent twice; blanked rather than dropped, since the schema requires the member to be a string. Carrying the original object that far also required `NormalizedInput` to stop discarding it: `content` is a flattened, lossy view — stdout and stderr concatenated, every other key gone — and `interrupted`/`isImage` cannot be reconstructed from it. The **labeled-passthrough branch (F-07) is fixed by the same change**; it emitted the same rejected shape, so that label had never once rendered for a Claude Code user either. Payloads arriving without a host response object (Codex, Pi, generic MCP) **keep the MCP shape deliberately** — those hosts' contracts were not investigated, and guessing at a second one is how the first was got wrong. Verified end to end through the built binary rather than the struct: the same 300-line payload now returns `{stdout, stderr, interrupted, isImage, backgroundTaskId}` with no `status`/`result`, 18,490 B → 1,819 B, and the reported 90% matches a measured **90.2%**. A stashed baseline confirms the distilled *text* is byte-identical before and after — this change moves the envelope only. The test that let both halves through asserted `"status":"success"` — **the same field name it had just serialised**, which is why a wrong key and then a wrong shape both passed it; the replacement asserts against the host's schema and specifically that `status`/`result` are **absent**, and was confirmed to fail on five tests with the old shape restored. One consequence is filed rather than hidden: making the payload apply also delivers the unmarked 30-line truncation in `distillers/jsts.rs` to agents for the first time (#188).

## [0.6.4] - 2026-07-23

### Added
- **`omni doctor` now says when the binary carries changes no release contains (#137)**: `doctor` already checked GitHub for a newer release, and that check is structurally unable to catch the case #127 filed — six correctness fixes merged and unshipped, where the newest release **is** the running version, so `doctor` printed `omni v0.6.2 [LATEST]` and nothing else. Reproduced on this tree before writing the fix: 19 commits and 15 changelog entries unreleased, and `doctor` still said `omni v0.6.3 [LATEST]` — true as an answer to "is there a newer release", useless as an answer to "am I running the current fixes". The two questions now have two answers. `build.rs` counts the entries under `## [Unreleased]` in the tree the binary is compiled from and passes the number in as `OMNI_UNRELEASED_ENTRIES`, so a properly cut release reports **0** and stays silent, while a build from a tree with pending work prints `[15 UNRELEASED] changes built into this binary are in no release — cut a tag`. The count is a compile-time fact about the source, not a network call, so it works offline and cannot disagree with the binary it describes. The parser lives in `src/util/changelog.rs` and is `include!`d by `build.rs` rather than copied into it, so the seven tests cover the code the build script actually runs; `option_env!` with a `unwrap_or(0)` fallback means a missing or malformed value reports nothing rather than raising a false alarm. Proving the tests could fail turned up dead code in the first draft: a `break` on reaching the next `## [` heading could be deleted without reddening anything, because the following assignment already ends the section — it is gone, and the reason is recorded next to the loop.

- **`omni stats --rerun` measures whether a distillation cost the agent a second run (#109)**: reduction % counts bytes removed, so a distiller that emitted `""` for every input would score 100% — and nothing in the pipeline penalised that. It is why #106 (19.5 KB → 14 B, 99.9%), #107 (89.4%) and #108 (86.2%) were each scored as wins while costing more tokens than they saved: the agent re-ran the command to get back what was deleted. The new mode measures the thing reduction cannot see — a repeat of the same command, in the same session, within 300 s of reading its distilled output. **No new instrumentation was needed**: `session_id`, `ts`, `command` and `route` are already on every `distillations` row, so step 1 of #109 ("record the normalised command") was already done, and the metric answers over 28 days of history already collected rather than starting a clock. `route` also supplies a **control arm for free** — `Passthrough` rows are commands whose raw output the agent read, so the two arms differ only in whether OMNI changed what was read. Measured on the maintainer's store (6,682 distillations, 19 sessions): `npm` 48.4% distilled vs 8.0% raw (**+40.4pp**, the #106/#129 shape), `sed` +15.1pp, `grep` +9.8pp, `git` −0.4pp. Two guards keep the metric from becoming the defect it hunts. **First, the arms are not always the same population**: distillation only fires on large output, and `kubectl` averages 244,606 B distilled against 115 B raw — a 2,127× skew that compares `get -A` dumps against one-line config reads. Its **+48.6pp is input size, not lost signal**, so any filter past `RERUN_SIZE_SKEW_LIMIT` (3×) prints `n/a` and is listed as a caveat rather than a finding; `grep` (3,196 B vs 3,240 B) and `npm` (11,419 B vs 14,065 B) are matched and survive. **Second, pre-#158 Claude Code rows are excluded**: on that path nothing was applied, so their `Keep` rows are controls wearing a treatment label. That is not cosmetic — `grep` reads **+9.8pp with them excluded and +0.0pp with them included**, so the dilution alone can hide a real finding, and the cutoff is the commit timestamp of the #158 fix rather than a guess. A filter needs 8 rows on *each* arm before its delta is reported at all; below that the number is noise, and publishing it would be the confident-but-unsupported figure this metric exists to catch. Steps 3 and 4 of #109 — net scoring and an adaptive fallback driven by the rate — are **deliberately not shipped**: feeding a guard from a metric with known confounds is how a measurement bug becomes a distillation bug, so the metric is watched for a release first. All three guards were confirmed to fail the right test when neutered: disabling the skew check reddens `flags_arms_of_wildly_different_size_as_confounded`, neutering the pre-#158 predicate (keeping the parameter count, so the failure is logic and not a SQL error) reddens exactly `excludes_claude_code_rows_recorded_before_the_post_hook_fix`, and widening the window reddens `ignores_a_repeat_that_falls_outside_the_window`.
- **`omni stats` gained short scope flags and an hour window (#154)**: `-d` (today), `-w` (last 7 days), `-m` (last 30 days) and the new `--hour` / `-H` (last 60 minutes). There was no scope shorter than a day, so "what did the last hour cost me?" — the question you ask *during* a session — had no answer. `-H` rather than `-h` is deliberate: `-h` is help across every OMNI subcommand, and re-pointing it at a time scope would have broken that reflex in one command while leaving it intact in the others. Resolving the four windows also collapsed two divergent copies of the scope logic into one function: `run_detail` and `run_project_stats` each had their own, `--month` appeared in neither, and it was honoured only by being the fall-through default in one of them and silently ignored in the other.

### Changed
- **`omni --help` is one grouped list written around what a user wants, not one flat list of nouns (#166, #152)**: there were two help texts — a hand-written one for `omni help` and bare `omni`, and clap's for `omni --help` — and they had already drifted. Six commands, `exec` among them, appeared only in clap's; `exec` is the harness every issue in this tracker asks reporters to run, and it was invisible in the help a user gets by typing `omni`. Both routes now render from a single `COMMANDS` table, and the two outputs are byte-identical (`diff <(omni --help) <(omni help)`). The list is grouped by intent — **SET UP**, **SEE WHAT IT SAVED**, **TUNE IT**, **MEMORY** — instead of alphabetically, and each line states the payoff rather than restating the name: `learn` went from *"Auto-generate filters from history"* to *"Build filters from the noise in your own history"*, `diff` from *"Compare last original input vs distilled"* to *"The last command's output, before vs after"*. This is the other half of #164: a usage audit found ten subcommands nobody had ever run, and the likeliest reason is that nothing in the help said what they were for or whether they were for you. A `lists_every_subcommand` test walks clap's own subcommand list and fails if a command is missing from `COMMANDS` (invisible in help) or listed without being a real subcommand — confirmed to fail when `patterns` was removed from the table. The footer now points at `omni <command> --help`, which became worth pointing at in #151.

### Removed
- **Three CLI subcommands nobody has ever invoked (#164)**: an audit of real usage — the whole shell history on the maintainer's machine, plus row counts after 6,446 distillations over a month — found `omni stats` run 131 times and `handoff`, `rewind` and `rewrite` run **zero** times each. `handoff` (326 LOC) and `rewind` (197 LOC) are gone entirely; `rewrite` lost its subcommand but **keeps its module**, because `cli::rewrite::rewrite_logic` is called from `hooks/pre_tool.rs:91` on every command and is the subject of open issue #157 — deleting the file would have broken the pre-hook, which is why the cut list was checked against the call graph rather than against the usage numbers alone. `rewind` was inert end-to-end and not merely unused: `rewind_hash` is hardcoded `None` on every live path, the only caller of `store_rewind` is a test with the payload `"testing_payload"`, the table held 0 rows, and `store/query.rs` filters `WHERE rewind_hash != ''` so those OmniQL branches could never match. Net **-777 lines**. The RewindStore storage layer, the `Route::Rewind` variant and the `omni_handoff` MCP tool are deliberately left in place: the usage evidence comes from shell history, which says nothing about what an agent calls over MCP, and cutting them would be acting past the evidence. Tracked in #164.

### Fixed
- **A reader that closed early made OMNI panic, and `--help` exited 1 (#155)**: Rust sets `SIGPIPE` to `SIG_IGN` before `main`, so writing to a pipe whose reader has gone returns `EPIPE` — and `println!` panics on it. `omni --help | head -1` printed a panic and a backtrace note where `ls | head` prints nothing. **The mechanism the issue filed had already moved**: it named `e.print().expect("failed to print error")` at `main.rs:309`, which is what released 0.6.3 still panics in, but #166 replaced clap's help with a hand-rolled printer, so on current `main` the panic came from `println!` at `std/src/io/stdio.rs` instead — the same bug one layer down. Fixed at the entry point rather than at the writer, because it was never specific to help: measured on released 0.6.3 with a reader that closes immediately, **`--help`, `doctor`, `stats` and `session` all panicked**; on this build none of them does. Guarding each `println!` would have fixed the one command in the title and left the other three. `main` now restores `SIG_DFL` on Unix (`#[cfg(unix)]`; Windows has no `SIGPIPE` and surfaces a closed pipe as an ordinary write error), using `libc`, which was already in the tree via `rusqlite` and `tempfile` so nothing new compiles. The clap arm keeps a matching fix — `let _ = e.print()` rather than `.expect()`, per `CLAUDE.md`'s rule against `.expect()` on IO. Separately, `--help` and `--version` now exit **0**: clap reports both as an `Err`, the arm did not distinguish them from a parse failure, and `omni --help && echo ok` printed nothing as a result. An unknown flag still exits non-zero, asserted by its own test so the fix cannot quietly swallow #151's guard. Both halves were confirmed to fail independently: removing the `SIGPIPE` restore reddens the two panic tests, reverting the exit-code classification reddens `help_and_version_exit_zero`.

- **All six translated READMEs promised "up to 90%", the exact claim the English one calls a red flag (#132)**: the English README was rewritten in #131 / #150 to lead with the measured figure, and line 119 now warns that *"a tool that claims to save 90% of every command is telling you it summarises output you need"*. The translations were never updated, so `i18n/README-{ja,ko,zh,vi,id,ar}.md` each carried that promise in the hero line, the intro blockquote and a feature bullet — 2 to 5 occurrences per file, on a public repo, in six languages, contradicting the English page they link to. All of them now carry what the English one carries: **58.9% fewer bytes across a real command mix (15.0 MB → 6.2 MB), measured by replaying 1,810 real traces on the release binary, and 63.6% of calls saving nothing at all**, which is published rather than hidden. The one place "90%" survives in each file is the sentence warning against that claim — the translation of English line 119 — which was already correct and stays. **Structural parity is not claimed**: the translations still follow an older English layout (the English page has since replaced its feature bullets with benchmark tables), so #132 stays open for the full re-sync; this change fixes the false claims only.
- **`docs/` still called `omni handoff` and `omni rewind`, deleted in #164, and two shipped example scripts failed silently (#180)**: verified against the built binary rather than the source — `./target/debug/omni handoff --json` and `omni rewind list` both exit 1 with `unknown command`, while installed 0.6.3 still answers both, which is why this went unseen. The MCP tool `omni_handoff` is unaffected and still registered; only the shell entry point is gone. Worse than stale docs, two runnable examples broke: `docs/autonomous-loops/shell-loop.sh` guarded its checkpoint with `omni handoff --json 2>/dev/null | jq … || echo "CONTINUE"`, so once the command disappeared **the loop kept running against a default it never computed** — a confident value standing in for a missing one, the defect this project exists to stop — and `mastra-integration.ts` did the same thing twice, wrapping the call in `catch { return true }` and `catch { return "Normal" }`. Those methods were **deleted rather than stubbed**, because a stub that reports a healthy loop from state it never read is the same bug with a nicer name; `getStats()` (`omni stats --json`) still exists and the example keeps it. `omni rewind` had no shell replacement to offer — it was inert end to end — so its section is removed and the MCP `omni_retrieve` path is named instead. #164's cut list was built from shell history and row counts and correctly noted it could not see MCP callers; what neither source covered was the repo's own shipped scripts, which no interactive history would ever show. Next subcommand removal should grep `docs/` and any executable example, not just the call graph.

- **`omni stats` counted savings the host had thrown away (#163)**: every `distillations` row with `agent_id = 'claude_code'` written before the #158 fix records a distillation that was computed, scored, routed and stored — and then dropped by Claude Code, because the hook emitted `updatedResponse`, a key it ignores. The agent read the raw bytes. Those rows sat in the same sums as the `omni exec` and pipe rows, where the same numbers are true, and nothing in a row says which binary wrote it. Measured on the maintainer's store, the same database through both binaries: released 0.6.3 reports `claude_code 2550 calls, 30.2%`; this build reports **`claude_code 237 calls, 44.6%, unverified=2313`**. `aider`, `terminal` and `codex_cli` are unchanged at `unverified=0` — the cutoff touches only the path that was broken. **The issue predicted the honest figure would come out lower and warned it would need explaining as a correction; on this data it goes up**, 66.9% → 69.4% overall, because the discarded rows averaged 29.2% and were dragging the mean down. What actually falls is volume: 6,728 → 4,415 calls, **−34%**. That is the number a user notices, so the excluded rows are **reported rather than removed** — `omni stats --detail` prints an `unverified` line under each agent, and `--json` carries an `unverified` field per agent, because a call count that shrinks without explanation reads as OMNI having stopped working. The gate is scoped to sums over the byte and token columns, **not** applied as a row filter: latency, command, project and file access in those rows really happened and stay queryable, and only the savings columns are fiction — deleting the rows would destroy true history to remove a false column, which is what never-drop argues against. The cutoff is `POST_HOOK_FIX_TS`, already in `pipeline/mod.rs` and already how the #109 re-run metric selects its population, so there is one rule and one constant rather than two. Known limit, chosen deliberately: someone who upgrades late has pre-fix rows written *after* that timestamp, so their figures stay generous; fixing that needs a writer-version column and was rejected as too much migration for a bounded, shrinking set. Three deliberate breaks confirmed the tests can fail — neutering the predicate reddens both exclusion tests, setting the cutoff to `i64::MAX` reddens `counts_claude_code_savings_recorded_after_the_fix` (proving it is a cutoff and not a blanket ban on the agent), and hiding the `unverified` column reddens the reporting test.

- **`readfile` deleted every function body from Python, JS/TS, Go and Java without saying so (#176)**: `distill_rust_file` appended `[Method bodies omitted…]`; the four siblings ended with a bare `out.trim()` and returned a skeleton with no indication anything had been removed. Measured through the post-hook on a 24,999 B Python file: **3,275 B out, 86.9% reported as a win**, and the rule the file was read for — `if order.total > 1000: order.apply_discount(0.1)` with a VIP branch below it — gone, along with every other body, buried under 120 repeated `def process(order):` lines. An agent asked what discount a 1,200 order gets had no way to know the answer had ever been in the file. That is the #111 never-drop violation: if bytes disappear the output says so. One shared `omitted_note(total, kept)` now carries the count and is used by all five distillers, with `distill_rust_file`'s hand-rolled line folded into it — a marker without a number does not let a reader judge whether to re-read. The same file now ends `... [601 of 723 lines omitted — bodies and comments not shown. Re-read with offset/limit for the full file.] ...`. The empty `--- … ---` sections are also qualified: the scan behind each heading runs over the **whole** file including the lines it drops, so `None` was true, but next to a visibly truncated body it could not convey whether it meant "absent from the file" or "absent from what you can see" — it now reads `None in the full file`. That half is a labelling fix, not a guard: an earlier draft of #176 called it a false claim of the #112 `helm: 0 deployed` shape, which was wrong on re-reading the loop, and was withdrawn on the issue rather than quietly fixed. **This path is unreachable today** — the Claude Code hook is registered for `Bash` only (#172) — and was fixed first for exactly that reason: widening the matcher before this would have shipped it on every source file over the threshold. The regression test walks all five languages and was confirmed to fail for each one individually with that language's marker removed.

- **`cargo tree` was distilled to `Build: ok` — 21.4 KB deleted and reported as a 100% saving (#170)**: every `cargo` command was routed to `BuildDistiller` on the executable name alone, with one exception for commands containing `test`. Most of cargo is not a build. `cargo tree` prints a dependency tree that *is* the answer, and 21,959 bytes of it came back as nine: `Build: ok`, under the footer `100.0% reduction`. This is the defect named in the 0.6.2 changelog — *"`find` reported ~99% by discarding the file paths that were the answer"* — with a different command, and `Build: ok` is additionally a claim nothing supports, since no build ran. It reached users who never type `omni`: `hooks::pre_tool` rewrites any command *starting with* `cargo ` into `omni exec <command>`, so a bare `cargo tree` was silently routed through it — **and redirecting to a file did not help**, because the rewrite wraps the entire command string including the redirect (`cargo tree > out.txt 2>&1` wrote one line where 477 were expected). Whether the tree survived depended on whether the command happened to begin with the word `cargo`; anything prefixed (`cd … && cargo tree`) escaped, which is why this went unseen. `cargo` is now routed by **subcommand**: `build`/`check`/`run`/`clippy`/`rustc`/`fix`/`doc`/`install` to `BuildDistiller`, `test`/`bench` to `TestDistiller`, everything else passthrough — the honest answer for output that is data rather than progress. The parser skips env-var prefixes, the path cargo was called by, `+toolchain` overrides and leading flags, and a missing subcommand (`cargo --version`) also passes through. `cargo metadata` was never affected: `pipeline::format::sniff` recognises its JSON and stands the lossy stages down, which is the argument for fixing this at the command level instead of teaching a content heuristic to recognise a tree. Measured end-to-end: released 0.6.3 returns 9 bytes, this build returns all 21,959. A companion test asserts noisy `cargo build` output is *still* distilled, so the fix cannot quietly end savings on the commands that do work.
- **`omni stats` filed Claude Code's work under `Terminal`, and some of it under an agent that never ran (#160)**: OMNI had **three** private ways of naming the agent, not two. `hooks::normalize::detect_agent_id` reads the payload shape on the hook path; `agents::multiagent::detect_agent_id` reads the environment; and `hooks::pipe::resolve_pipe_agent_id` — the one that actually writes the rows on the `omni exec` and pipe paths — had its own rules and consulted neither. It knew `OMNI_AGENT_ID`, then guessed **`aider`** for anything with `OMNI_CMD` set (a variable OMNI documents for its own pipe mode, which any caller may set — 3,296 rows on one machine were filed under an agent that had not run), and called everything else `terminal`. Meanwhile the env detector had no Claude Code branch at all, so even where it did run, the most widely deployed agent OMNI supports was the one it could not see. Measured end-to-end, same two commands through each binary: released 0.6.3 records `terminal|2`, this build records `claude_code|2`. The pipe resolver now defers to the single env detector, which gained a Claude Code branch keyed on `CLAUDECODE`/`CLAUDE_CODE_ENTRYPOINT` — placed **before** the VSCode branch, since Claude Code in VS Code's integrated terminal sets `VSCODE_PID` and `TERM_PROGRAM` too and would otherwise have been labelled `vscode`, trading a vague answer for a confidently wrong one. The env detector's ids were also reconciled with the payload detector's (`codex` → `codex_cli`, `continue` → `vscode_continue`), so one agent no longer splits into two rows of which neither is the true total. `agent_display_name` stops folding `unknown` into `Terminal`: "a human ran this in a shell" and "OMNI could not tell" are different facts, and collapsing them is what hid the missing branch for the life of the feature — a detection gap now reads as `Unknown`. A second, unreferenced copy of `agent_display_name` in `agents/multiagent.rs`, reachable only from its own test and still carrying the old ids, was deleted. The cross-detector agreement check lives inside the existing env test rather than beside it: as its own `#[test]` it passed alone and failed in the full suite, because env vars are process-global and the ambient `CLAUDECODE` shadowed the variable under test.
- **On Claude Code, the distilled output never reached the agent — and OMNI reported the saving anyway (#158)**: the PostToolUse hook serialised its replacement under `hookSpecificOutput.updatedResponse`. The key the host reads is **`updatedToolOutput`**, and it takes an object (`{status, result}`), not a string. Claude Code drops an unrecognised key without a word, so the model received the **full original stdout** while OMNI recorded the event as `Route::Keep` — documented in `pipeline/mod.rs` as *"score >= 0.7, full distillation"* — and printed a savings footer for it. A 60-line probe (`seq 1 60 | sed 's/^/line /'`, 470 B) arrived complete and was logged as `470 → 48`; a 3,449-byte `sed` of two source files arrived complete under the footer `[OMNI: -904tok this call | 100% compression]`. Everything OMNI exists to do was happening, being measured, and being discarded one field name short of the agent. Two things hid it: the sibling `additionalContext` **is** spelled correctly, so the footer appeared and the failure read as success; and every unit test asserted on the struct's own field, which passes with any key at all. `git log -S` puts `updatedResponse` in the initial Rust-migration commit, so this was never a regression — the Claude Code hook path had **never** applied a distillation. Confirmed from the host side rather than the docs alone: `strings` on the Claude Code 2.1.217 binary finds `updatedToolOutput` 13 times and `updatedResponse` zero (`hookSpecificOutput` 40, `additionalContext` 45). Scope was narrow but the worst-placed it could be: the `omni exec` and pipe paths write stdout directly and were always genuine — an eight-hour window on one machine had 139 `claude_code` rows claiming 377,697 → 286,648 bytes that never landed, beside 54 exec/pipe rows whose 16.9 MB → 224 KB was real. `status` is always `success` because a failed command returns `None` far earlier (#120) and never reaches this struct, so the new field cannot certify a success for a command that failed. The regression test asserts on the **serialized JSON**, not the struct, since that is the only level at which a wrong key is visible; it was confirmed to fail with the old key restored. An existing security test that asserted `updatedResponse.is_string()` — and had been holding the wrong contract in place — was corrected to the real shape.
- **`CloudDistiller` guessed which tool ran from the output's shape, and reported a tool that never ran (#112)**: the dispatcher already resolves the base command — that is how `CloudDistiller` gets selected at all — but the `Distiller` trait had no `command` parameter, so `cloud.rs` threw that away and re-derived identity from the content one stack frame later. Its first content test was `is_docker_logs`, a pure shape heuristic ("≥5 of the first 20 lines are longer than 20 chars and start with a digit or `[`") that any timestamped log matches. So `kubectl exec -it pod-0 -- sh -c 'ls -la /data'` — output beginning `ls: /data/models: No such file or directory` followed by eight heartbeat lines — was handed to the docker summariser and came out, end-to-end through the released 0.6.3 binary, as **`docker logs: 9 lines, no errors detected` (596 B → 40 B, 93.3% reduction)**. Two falsehoods in fourteen words: no `docker` was involved, and the one real error was deleted and replaced with a denial that any error existed. The #143 zero-state guard did not catch it, because `is_docker_logs` was *true* — the guard asks "did I parse anything", and a misrouted payload that matches the wrong tool's shape answers yes. Sibling branches had the same defect: `input.contains("aws ")` claimed any output mentioning `aws `, `input.contains("terraform")` any line naming terraform, `input.contains("kubectl")` any log that quoted a kubectl command. `CloudDistiller` now carries the resolved tool (`CloudDistiller { tool: &base }`) and matches on it first; content heuristics are demoted to what they can actually decide — which *format* within one tool (`docker ps` vs `docker build` vs `docker logs`). The same input now passes through unchanged at 0.0% reduction, error intact. Three zero-states that tool-gating newly makes reachable are guarded in the same change, since gating widens what reaches them: **helm** (`helm: 0 deployed, 0 failed, 0 pending` asserted a cluster state that was never read — now requires helm's own `REVISION`+`CHART`+`STATUS` header, not the `NAME`+`STATUS` prefix half of kubectl's tables also print), **terraform** (`terraform: +0 ~0 -0 resources` reported an empty plan for `terraform init` output that contained no plan — now requires a `Plan:`/`will be …`/`Apply complete!` line), and the generic **fallback**, which returned an empty string when no segment scored Critical or Important — deleting the output entirely and booking it as a 100% saving. All four regression tests were confirmed to fail with `require_parsed` neutered.
- **The CLI accepted flags it did not understand and reported success (#151, partial)**: sixteen of eighteen subcommands are declared as a `trailing_var_arg` catch-all (`extra: Vec<String>`), and each `cli::*::run` then re-parses raw argv by hand with `args.iter().any(|a| a == "--detail")`. Nothing enumerated the valid set, so nothing could detect a value outside it: `omni stats --detial` printed the default overview and exited **0**, and `omni init --curser` ran the interactive default and exited 0 while installing nothing the user asked for — the same defect this project files against its own distillers, confident output over input that was never parsed. Two properties now come from one declaration per command, a `(flag, description)` list that both the help printer and the new `cli::check_flags` read: an undeclared flag is an error with a Levenshtein suggestion (`unknown flag \`--detial\` for \`omni stats\` — did you mean \`--detail\`?`), and help cannot drift from what is accepted. Only long `--flags` are checked unconditionally; a single-letter `-x` is checked only where the command declares short flags, so free text (`omni remember "build with -O2"`) still passes through. `omni init`'s result was also being discarded with `let _ =`, which would have swallowed the new error and kept exit 0. Converted so far: `stats`, `init`, `session`, `doctor`, `learn` — the commands with real flag sets, where a swallowed typo changes what runs. The remaining eleven still accept anything; #151 stays open for them.
- **`--help` printed a stub while the real help was unreachable (#151)**: clap intercepted `--help` and `-h` for every subcommand and printed `Usage: omni stats [EXTRA]...` with no flags at all, while the actual per-command help — accurate, with flags and examples, already written in twelve modules — was reachable only through the undiscoverable bare word `omni stats help`. Twelve subcommands now set `disable_help_flag`, so `--help` and `-h` reach the module that has something to say. That alone moves 47 implemented flags from "documented nowhere" to "documented where a user looks for them". The flag column is now sized to its longest entry rather than a fixed 12, which `--all-commands` and `--validate <file.toml>` both overflowed into their own descriptions.
- **Five more distillers could certify a clean result they never parsed (#143)**: the #115 vitest fix (`✓ 0/0 passed` for a dev server) was one instance of a class — a distiller whose "nothing bad found" branch emits a green string (`no errors`, `passed`, `no problems found`, `no issues found`, `no errors detected`) that is byte-identical to a real success, with no check that any signal was actually parsed. So any upstream misdetection — a `tsc --` echo matched with no `error TS` line (#106 shape), a `@typescript-eslint/` mention in non-eslint output (#114 shape), a manifest that merely names `docker logs` (#112 misroute) — converted silently into a confident false pass. `AGENTS.md` requires hooks to *fail open*; emitting `no errors` on unparsed input is failing *closed and confident*. The shared `require_parsed(parsed, input, summary)` helper (added with #115) is now wired at every remaining zero-state: **tsc** (only claims `no errors` when it saw an `error TS` line or a `Found N errors` summary), **playwright** (only when it parsed a passing count or a `✓` line), **eslint** (only on a real `problems (` summary or a parsed finding — a bare file list like prettier's no longer qualifies), **security** (only when at least one severity token was seen), and **docker logs** (only when the input is actually log-shaped per `is_docker_logs`). With no signal, each returns the input unchanged instead of a green string, so a future misdetection degrades to passthrough, not a false claim. Each site carries a regression test that routes a detector-tripping but signal-free payload through `distill_with_command` and asserts the false claim is absent and the original content survives; all six were confirmed to fail with the guard neutered. Individual detector fixes still ship per-issue (#106/#108/#112) — this is the guardrail that makes their false-claim class un-reintroducible.

## [0.6.3] - 2026-07-21

### Fixed
- **A failed command could be distilled into output that reads as success (#120)**: OMNI's `normalize` layer parsed each agent's failure signal and then threw it away — Codex `exit_code` was never read into `CodexInput`, Pi `toolResponse.isError` sat behind `#[allow(dead_code)]`, and MCP `result.isError` was named in a comment but never deserialized — so a command that exited non-zero still ran the full distiller. A failed `docker build` (`exit_code 1`) on the heavy-noise fixture came out **9,207 → 6,090 bytes**, silently trimmed by the same `DEBUG`/`INFO` stripping a *successful* build gets; the filed case was a `vault` call that failed `exit=2` on a network timeout yet surfaced a clean, fictional `["n8n"]`. This is the worst failure mode — a fabricated success terminates investigation, while a fabricated error only costs a retry. `NormalizedInput` now carries `failed`, set from each agent's own signal, and `post_tool::process_payload` passes a failed command through verbatim at zero marker cost before any distiller runs. Successful commands are untouched — the same output with `exit_code 0` still distils to 6,090 bytes. Claude Code needed no code change: it already sends a failed command as a bare `tool_response` string (`"Error: Exit code N…"`) that never parses into a success summary, and a regression test locks that in so a future, more-lenient parser cannot silently reintroduce the fabrication. The `omni exec` / `pipe.rs` path reads piped stdout only and never sees the child exit code; that gap is closed by the next entry.
- **`omni exec` distilled failed commands too (#122)**: the same invariant as #120, one layer down and from a different cause. `cli::exec` streamed the child's stdout straight through the distiller and only called `child.wait()` *afterwards*, so by the time the non-zero exit was known the distilled output had already been written — a failed command run through OMNI's own reproduction harness came out distilled. The exit code is only knowable once stdout is fully drained (and draining before `wait()` is also what avoids a full-pipe deadlock), so exec now buffers stdout, waits, then gates: non-zero exit emits stdout **verbatim** and skips distillation; zero exit distils exactly as before. Measured on 60 identical noise lines: `exit 1` now yields 60 verbatim lines where it previously collapsed to a single `[60 similar lines collapsed]` marker; `exit 0` still collapses. Stream-mode commands (`docker`/`npm`/`bun`) emit line-by-line before the exit code exists and cannot be gated, so they keep streaming; the stream-filter lookup is now shared via `pipe::stream_filter_for` so exec and the pipeline agree on which commands stream.
- **Distillers parsed OMNI's own collapse markers as data, and weak TOML filters hid it (#116, #110)**: `collapse` runs before `distill`, rewriting repeated lines into `[N similar lines collapsed]` markers — and a distiller that parses columns then read those markers as rows. A 35-row `kubectl get pods` table (30 Running, 5 CrashLoopBackOff) came out as `k8s: 2 pods | 0 running, 0 pending, 2 error / Problems: [30 (lines), [5 (lines)` — every reported "pod" was OMNI's own scaffolding, the real statuses destroyed. A distiller is just a later stage that parses its input, exactly what `pipeline::format::sniff` already shields structured payloads from; nothing shielded the distiller. It survived unseen because the broad `signals/domains/*.toml` filters won the alphabetical `find()` race and short-circuited the distiller for cargo/npm/docker/kubectl/terraform before it ever ran (#110). Two coupled fixes: (1) the distiller now reads the tool's **raw** output — collapse feeds only scoring and the fallback for commands no distiller claims, chosen by the shared `beats_guardrail` so a distiller that punts still yields the collapsed line savings; (2) a TOML filter only short-circuits the distiller if it actually beat that guardrail (ported from `hooks::pipe`, now shared in `guard::limits`), so weak filters fall through. Together the same table now distils to `k8s: 35 pods | 30 running, 0 pending, 5 error` with the five CrashLoopBackOff pods named. `TomlFilter::priority` remains parsed-but-unread — filed as #119, not fixed here.
- **`omni exec` ran a corrupted command when a shell was involved (#125)**: `cli::exec` flagged `needs_shell` whenever *any* argv element contained a shell metacharacter, then rebuilt the command as `format!("{} {}", cmd, cmd_args.join(" "))` and ran it under `sh -c`. For `omni exec sh -c 'echo A; echo B'` that produced `sh -c "sh -c echo A; echo B"` — a second shell layer, with `join(" ")` having already flattened the original quoting — so the outer shell ran `sh -c echo A` (A became `$0`, not an argument) and then `echo B`, and the output was just `B`. `omni exec` is the repo's own reproduction harness, so any repro of the form `omni exec sh -c '…'` silently ran the wrong command. A shell is now used only when the command arrived as a single unsplit string (`omni exec 'a; b'`); when argv is already split, each element belongs to the program and is passed through verbatim — the metacharacters are the inner program's, not omni's to reinterpret.
- **Composite npm scripts were distilled as a single tool, discarding the other gates (#106)**: a chained script (`npm run verify` = `build && tsc --noEmit && eslint && check:secrets && test`) concatenates several tools' output into one buffer, but the JS/TS dispatcher picked **one** distiller from the first signature it saw and handed it the whole thing. `is_tsc_output` matched `tsc --` inside npm's own `> build && tsc --noEmit && …` echo, so **19.5 KB collapsed to 14 bytes of `tsc: no errors`** — a false success that also erased four gate verdicts, including the secret scan's positive control (the line that proves the scan could see inside the bundle at all). Over-distilling a composite is token-*negative*: the agent re-ran `npm test`, `check:secrets`, and `lint` to recover what was dropped. No per-tool distiller can safely own a composite — an `&&` chain has no delimiter between the tools' outputs — so the dispatcher now detects npm's `> … && …` echo and declines, returning the buffer for the pipeline's generic collapse, which folds the repeated build noise while keeping every gate's distinct verdict line. (`make`/`npm-run-all` composites without an `&&` echo aren't covered yet — filed as #129.)
- **`npm run format` (prettier) was reported as `eslint: no problems found` (#114)**: three compounding prettier defects in `distillers::jsts`. (1) `is_eslint_output` matched the bare word `eslint` — which appears as a *filename* (`eslint.config.js`) in prettier's file list — so a `prettier --write` run that rewrote files across 109 paths was distilled by `distill_eslint` as a clean run of a **different tool** (the same substring-in-data trap as #105/#106; a destructive command reported as finding nothing). (2) `is_prettier_output` only matched lowercase `checking `/`reformatted `, but prettier prints `Checking formatting…` and `[warn]` — so the detector was dead and never fired on real prettier output. (3) `distill_prettier` parsed **black**'s `reformatted N files` summary, which prettier never prints, so both counters stayed 0 and a *failing* `--check` and a *successful* `--write` both rendered as `prettier: 0 files reformatted, 0 unchanged`. All three are fixed: eslint detection now anchors on eslint's real line shape (`✖ N problems (`, a rule id, or a `<line>:<col> error|warning` finding) instead of a substring that occurs in filenames; prettier detection matches what prettier actually prints; and `distill_prettier` is rewritten against prettier's real output — `--check` surfaces the offending filenames (`prettier --check: N file(s) need formatting`), `--write` reports `N reformatted, M unchanged` and names the changed files (capped), and it declines (returns the input) rather than fabricate a count when neither mode is recognisable. Snapshot tests added for both modes. (Defect 4 in the report — `--check` output reordered with the `[warn]` filename dropped — is an upstream stdout/stderr-ordering observation the reporter did not trace; left for a separate issue.)

## [0.6.2] - 2026-07-17

### Fixed
- **Format-safe compression (data integrity)**: The pipeline had no format awareness, so collapse squashed the repeated lines of structured output into `[N similar lines collapsed]` and left the payload unparseable — breaking any downstream `jq` / `json.load` / `kubectl apply`. A JSON dashboard piped through the hook came out with 14 collapse markers and failed `jq` outright. A new format sniffer (`pipeline::format`) now gates both choke points (`hooks::post_tool`, `hooks::pipe`): JSON, NDJSON, YAML, TSV, and CSV pass through byte-for-byte at zero marker cost, while plain text still compresses as before.
- **YAML with an embedded block scalar was not detected as YAML**: `sniff_yaml` judged every line of a document that lacked a leading `---`, so a ConfigMap carrying Vault HCL (`config.hcl: |`) made a 608-line `kubectl kustomize` manifest look like free text — 26 of its 608 lines were HCL, not YAML. The format gate stood down, collapse emitted `[8 similar lines collapsed]` markers, and `is_docker_logs` counted those markers as timestamp prefixes (12 triggers, needs 5), handing the whole manifest to `distill_docker_logs`: **13,463 bytes of Kubernetes config came out as `docker logs: 323 lines, no errors detected`**. OMNI's own collapse markers fabricated the evidence the next stage misread. A block scalar's body is now skipped rather than judged — the `|` that opened it is YAML's own signal that what follows is a value, so detection stays positive.
- **`find` deleted its payload and called it a saving**: the `SystemOpsDistiller` behaviour shipped in 0.6.1 ("strips out raw file path samples entirely", credited below with "up to 99.8% savings") reported ~99% by discarding the file paths that *were* the answer to the question. Replaced with lossless prefix factoring: the shared directory prefix is hoisted to a header and each path stated once relative to it, cut at a separator so every path round-trips byte-for-byte. Real saving on the same output: ~72%, with nothing lost. `grep` now hoists each path once instead of repeating it per match (26–39%, also lossless), and both hand back the input untouched when factoring would grow it.
- **A weak TOML filter shadowed the test distiller**: `sys_build_domain` matches every `cargo` invocation but strips only `Compiling`-style lines. It won the `find()` race, shadowed `TestDistiller`, cut 1%, and had that cut discarded by `best_output()`'s `MIN_REDUCTION_PCT` guardrail — so `cargo test` reported **0%** on output the distiller reduces by 94%. A TOML filter now only short-circuits the distiller if it actually beat the guardrail; a filter that earns its match still wins, user filters included.
- **`cargo test` totals were recounted instead of quoted**: with the distiller reachable again, it counted result-line segments and reported `1 passed` for a run cargo itself called `490 passed` — `CollapseMode::Test` folds the 330 `... ok` lines away before the distiller ever sees them. It now quotes the runner's own summary line (cargo, pytest, jest), falling back to counting only when no summary was printed.
- **No 0.6.2 binaries could be built at all**: `release.yml` asks `dtolnay/rust-toolchain@stable` for each matrix target, which installs that std into *stable* — but `rust-toolchain.toml` (added this release) makes cargo switch to the pinned 1.97.0, which has std for no target but the runner's own. Every cross-compile died with `error[E0463]: can't find crate for 'core'` before compiling a line of OMNI, and fail-fast took the rest of the matrix with it, so the tag produced no release. The matrix targets are now listed in `rust-toolchain.toml` itself, installing them into the toolchain cargo actually uses. `ci.yml` stayed green throughout — it only builds host-native, where std is already present — so this was invisible until a tag was pushed.

### Removed
- **`est_cost_usd` and `ModelPricing`**: OMNI is a hook and never sees the API's `usage` block, so it cannot know whether bytes were billed as fresh input or as a ~10× cheaper prompt-cache read. `ModelPricing` used only `.input`, and its `_cached` / `_cache_creation` fields were dead — every dollar figure OMNI printed was a fresh-input guess presented as a session cost. The formula was also duplicated across four call sites. Removed from `stats`, `diff`, and `guard::config`; unknown keys in an existing `config.toml` still parse.

### Changed
- **Published numbers replaced with measured ones**: the headline claims were not stale, they were untrue. `docs/PERFOMANCE.md` sold `docker build` as 9.2 KB → **49 bytes (99.5%)**; the fixture it names distils to 5,783 bytes (**37.2%**). `git diff` quoted **50.0%** on bytes (397 → 220) that compute to **44.6%**. The **97.3%** all-time figure traced to `find . (-12.6M tokens)` — find deleting its payload, the defect fixed above — so the number was manufactured by a bug. `$35+ USD/month` priced the estimator removed above, `~40% faster TTFT` was never measured, and three testimonials in quote marks were attributed to nobody because nobody said them. Replaced with a replay of **1,810 real execution traces** on the release binary with a fresh `HOME` per invocation: **58.9% net** (15.0 MB → 6.2 MB), `git` 91.3%, `cargo` 96.8%, `cat` 9.1%. Fixing the YAML sniffer moved reported savings **down**, 65.3% → 58.9%, because six of those points were destroyed manifests rather than removed noise. All six i18n READMEs carry the same numbers.
- **Two costs now documented rather than hidden**: OMNI's output is **not deterministic** against a warm `~/.omni/omni.db` — session history feeds the scorer, so the same binary on the same input differed on 21 of 30 traces run-to-run (one gave 1,835 bytes, then 433); any A/B measurement must isolate state per invocation. And latency is real: ~82 ms for a 496-byte `git status` against a fresh database, **~308 ms** against a 97 MB one, per hooked command, growing with history.

## [0.6.1] - 2026-06-24

### Added
- **Pain-First Positioning**: Completely refactored `README.md` to focus on the core narrative solving Terminal Noise and Context Amnesia ("Noise-canceling headphones for your AI agent").
- **Brand Evolution**: Redesigned the primary `logo.svg` with a sleek, neon-style "Cute Brain" wearing headphones, visualizing the integration of Noise-Canceling and the Adaptive Memory OS.

### Improved
- **AI-Native Distillation**: Revamped the `SystemOpsDistiller` for the `find` command. It now strips out raw file path samples entirely and emits a highly compressed, AI-friendly key-value summary of directory distributions and extension counts, maximizing token reduction (up to 99.8% savings).
- **Terminal UI Polish**: Fixed an alignment issue where the `[OMNI Active]` status tag would overlap with command outputs by ensuring trailing newlines in distilled payloads.

### Fixed
- **Clippy Strictness**: Resolved a `clippy::needless_splitn` warning in the directory classification logic, ensuring `make ci` continues to pass with zero warnings.
- **Snapshot Integrity**: Synchronized all `insta` snapshot tests to align with the new, denser `find` output format.

## [0.6.0] - 2026-06-14

### Added
- **Autonomous Loop Engineering**: Native support for iterative, autonomous agent loops (`OMNI_LOOP_BUDGET`, `OMNI_LOOP_GOAL`) with predictive goal-driven constraints.
- **Maker-Checker Verification Pattern**: Introduced `omni_verify` MCP tool to separate execution and validation securely across distinct agent sessions.
- **Test Suite Modernization**: Renamed all generic sprint-based test files to context-aware descriptive names (`session_state_tests.rs`, `security_validation_tests.rs`, etc.) and achieved 100% test coverage across 941 tests.
- **Production Hardening**: Added robust input sanitization, loop context injection prevention, and performance benchmark tests (latency thresholds adapted for CI resilience).
- **Consolidated Documentation**: Unified scattered templates into a cohesive `docs/LOOP_ENGINEERING.md` master guide and updated all global `i18n` READMEs to reflect new multi-agent loop orchestration capabilities.

### Fixed
- **Critical UTF-8 Panic Resolution**: Completely resolved `SIGABRT` crashes caused by multibyte characters (emojis, box-drawing, CJK) in terminal output. Implemented `char`-boundary safe string truncation and slicing utilities globally.
- **Pipeline Data Integrity**: Rewrote ANSI stripping and structural normalization engines to process by Unicode `char` rather than raw `byte`, preventing *mojibake* and data corruption on rich terminal outputs.
- **Release Stability**: Removed `panic = "abort"` from the release profile to allow `catch_unwind` guards to gracefully handle unexpected panics in production builds.

### Performance
- **Zero-Allocation ANSI Stripping**: Redesigned `strip_ansi` to use `Cow<'_, str>`, eliminating heap allocations entirely when processing clean terminal outputs.
- **Pattern Caching**: Implemented a thread-local LRU cache for `normalize_structural`, bypassing expensive regex and grapheme-cluster calculations on highly repetitive log lines.
- **Stream-Aware Output Limits**: Deployed `TruncatingWriter` in the streaming pipeline. Omni now tracks payload bytes on the fly and intelligently truncates multi-gigabyte outputs at valid UTF-8 boundaries, eliminating OOM vulnerabilities on massive terminal bursts.

### Changed
- **Column-Aware Truncation**: Refactored CLI formatting utilities (`diff`, `learn`, `stats`) to use `unicode-width` logic, ensuring terminal tables and summaries render perfectly aligned even when displaying full-width CJK characters or emojis.

## [0.5.9] - 2026-06-04

### Added
- **Engram (Automatic Subtask Digest)**: Rule-based state snapshots capturing subtask progress (e.g., error resolved, commits, test passes) to prevent context amnesia during long sessions.
- **Session Health Dashboard**: Introduced `omni session --health` to visualize context pressure, token savings, engrams, tool activity, and hot files.
- **Smart PreCompact v2**: Intelligent, priority-aware context packing (Errors > Engrams > Tool Summary > Hot Files) with SHA-256 delta detection to skip redundant injections.
- **Session Handoff & Portability**: Added `omni_handoff` MCP tool to export session state as portable markdown, enabling seamless context transfer between terminal sessions.
- **Rolling Tool Call Summary**: Aggregates the last 50 tool calls with success/error rates, exposed via the `omni_session("summary")` tool for agent reference when context pressure is high.
- **Periodic Context Re-injection**: Automatically re-injects critical pinned files (like `AGENTS.md`) into the agent's context when pressure is elevated and after a set interval.

## [0.5.8] - 2026-06-03

### Added
- **Streaming Distillation Pipeline**: Introduced memory-efficient, line-by-line processing (`src/hooks/pipe.rs`) to handle long-running and piped commands without memory exhaustion.
- **Expanded Semantic Distillers**: Added new declarative TOML signal profiles for 9 developer tools: `aws`, `az`, `bun`, `deno`, `docker`, `gcloud`, `npm`, `vite`, and `webpack`.
- **Brand & Documentation Modernization**: Completely overhauled the English `README.md` and all 6 `i18n` localized versions with professional SVG visual assets (`hero.svg`, `architecture.svg`), new "Under the Hood", and "Real-World Use Cases" sections.
- **Advanced Context Analytics**: Implemented context composition metrics in `omni stats` (`src/analytics/context_composition.rs`) to provide deeper visibility into token reduction and signal density.

### Improved
- **SQLite Storage Enhancements**: Updated `src/store/sqlite.rs` to seamlessly support high-throughput, chunked streaming outputs from the new distillation pipeline.
- **MCP & Tooling Synchronization**: Refined `pre_tool` and `post_tool` hook routing to align with the new semantic models.

## [0.5.8-rc3] - 2026-05-29

### Added
- **Context Pressure Management**: Implemented multi-stage context pressure warnings (Normal, Warning, Critical) to proactively manage session token budgets.
- **Critical File Pinning**: Added automatic context pinning for critical rule files (e.g., `.cursorrules`, `AGENTS.md`) during session compaction.
- **File Re-read Guard**: Introduced preventive warnings and hot-file mutation protection to stop redundant reads of files already in context.
- **Performance Documentation**: Added a comprehensive `docs/PERFOMANCE.md` showcase and updated all global `README.md` (and `i18n` translations) with actual ROI and noise-reduction benchmarks.
- **AGENTS.md**: Established `AGENTS.md` to define multi-agent coordination rules, development gates, and context lifecycle management protocols.

### Changed
- **Pipeline Fail-Open Architecture**: Reinforced pipeline hooks (`pre_tool`, `post_tool`, `pre_compact`, `session_start`) with strict fail-open logic to ensure zero disruption to agent operations.
- **Dependency Update**: Validated and integrated `rmcp` 1.7.0 updates using structured `Parameters<T>` and `JsonSchema` across the MCP integration.
- **CLI Formatting**: Minor styling improvements to `omni stats` terminal output.

## [0.5.8-rc2] - 2026-05-28

### Added
- **Pi Agent Integration**: Added first-class support for Pi Agent integration with init, reset, and doctor support, including hooks, extension, and toggle functionality.
- **VS Code MCP Initialization**: Introduced the `--vscode` flag to the `omni init` command for automatic VS Code MCP server configuration.
- **Enhanced Token Metrics**: The `omni stats --detail` pipeline now accurately tracks and displays raw vs filtered token counts in a dedicated "Tokens Reduced" column, providing precise visibility into token savings.

### Changed
- **Semantic Classification Engine**: Refactored the core pipeline filtering system to utilize a semantic classification engine for segments with tool-aware scoring logic.
- **Filter Configuration Layout**: Migrated filter definitions from the legacy `filters/` directory to structured `signals/tools/` and `signals/domains/` configurations.
- **MCP Framework Upgrade**: Upgraded `rmcp` to `1.7.0`, migrating all MCP tool handlers to strongly-typed `Parameters<T>` structs and `JsonSchema` for robust, type-safe request routing.

### Fixed
- **Claude Code Async Hooks**: Ensured Claude emits an empty matcher for async hook entries to prevent stalling.
- **Hermes Integration**: Fixed the detection logic for packaged Hermes OMNI plugins during `doctor` and initialization checks.

## [0.5.8-rc1] - 2026-05-08

### Added
- **Semantic Session Guardrails**:
    - **Hot File Detection**: Triggers `SessionEnd` hook when active files in a session show abnormal mutation frequency, prompting agents to review before committing.
    - **Build Failure Preservation**: `BuildDistiller` now preserves `CommandOutput` for build failures (non-zero exit codes) in the collapse pipeline, ensuring agents see exact errors instead of just exit status.
    - **Diagnostic Context**: `PreBuild` hook runs `cargo check` to surface compiler errors early in the session, reducing wasted tokens on broken states.
- **Passthrough & Thresholding**:
    - **OMNI_PASSTHROUGH**: Support for raw output emission via environment variable for manual debugging.
    - **Smart Bypass**: Automatic distillation bypass for small configuration files and content under a 2000-token minimum threshold.
    - **Extension Hinting**: Improved content-aware heuristics for more accurate token estimation.
- **Omission Transparency**: Added explicit `[OMNI: omitted X lines of noise]` markers in the `GenericDistiller` for improved agent situational awareness.

### Improved
- **Performance & Caching**:
    - **Filter Fingerprinting**: New caching system to reduce redundant TOML filter loading.
    - **Thread-Safe Loading**: Optimized filter registry access using Mutex and fingerprint-based verification.
- **Agent Attribution & Stats**: 
    - Standardized "terminal" as the default agent identifier for untagged sessions.
    - Improved agent distribution grouping and filtering in `omni stats` output.
    - Updated CLI visuals to use `bright_black` for secondary log signals to improve visibility.
- **Test Suite Modernization**: Systematically refactored the entire Rust test suite (~300 tests) to align with modern idiomatic standards.
    - **Naming Convention**: Dropped the redundant `test_` prefix inside `#[cfg(test)]` modules.
    - **Action-Oriented Naming**: Transitioned to behavioral function names (e.g., `returns_*`, `preserves_*`, `rejects_*`) to improve readability and maintainability.
    - **Language Standardization**: Purged all remaining Indonesian terminology and mixed-language test names, ensuring a 100% professional English testing layer.
- **CI Stability**: Verified full CI compliance after the bulk refactor, ensuring all 282 tests remain stable across platforms.

### Fixed
- **Unsafe Environment Access**: Wrapped `std::env::set_var` and `remove_var` calls in `unsafe` blocks within `src/guard/env.rs` to comply with the latest Rust edition requirements for test isolation.
- **Distillation Regression**: Resolved failures in `test_readfile_large_rust_file_distilled` by recalibrating token thresholds in test fixtures, ensuring consistent distiller triggering.
- **Snapshot Integrity**: Synchronized stale `insta` snapshots to match refined pipeline output patterns, maintaining high-fidelity regression tracking.

## [0.5.7-rc3] - 2026-05-07

### Added
- **Hermes Agent Integration**: New native plugin integration for the Hermes Agent. Features automatic `plugin.yaml` and Python hook script generation (`post_tool_call`, `pre_tool_call`, `on_session_start`) to silently filter noise in the background.

### Improved
- **Automated Doctor Fix Mode**: Massively refactored agent integrations (`cline`, `codex`, `cursor`, `gemini`, `antigravity`) to support a standardized configuration path management and automated `--fix` operations.
- **Claude Hook Cleanup**: Implemented robust `uninstall` logic in the Claude Code integration to completely scrub OMNI hooks and MCP server entries from `settings.json` and `.claude.json`.
- **OpenClaw Portability**: Refactored the OpenClaw plugin's TypeScript configuration to use standard `Node16`/`ES2022` settings instead of relying on sandbox-specific file paths. Also renamed the plugin directory from `integrations/openclaw` to `plugins/openclaw`.

### Fixed
- **Clippy Strictness**: Resolved hidden nested-if collapsible warnings (`clippy::collapsible_if`) inside `claude.rs` ensuring zero warnings under `#![deny(warnings)]`.

## [0.5.7-rc2] - 2026-05-06

### Added
- **Lightweight Anti-Hallucination Guards**: Added factual warnings when OMNI knows context is incomplete, including heavy-compression-without-rewind cases and high-impact file reads with many dependents.
- **ReadFile Dependency Context**: `ReadFile` distillation now surfaces dependency impact using graph-derived `imported_by` counts so agents know when a file change may have broad blast radius.
- **Hot File Mutation Warnings**: `PreToolUse` now warns before mutating commands touch files that are already hot in current session context.

### Improved
- **Graceful ReadFile Fallback**: `post_tool` now falls back to base `distill_readfile` flow when graph indexing is unavailable instead of dropping contextual file distillation entirely.
- **Doctor Auto-Repair Coverage**: Agent diagnostics continue to auto-repair missing integrations while preserving stronger validation for installed MCP entries.
- **Cursor MCP Validation**: Refactored Cursor integration checks around structured JSON validation and idempotent install/remove behavior for `~/.cursor/mcp.json`.
- **Code Quality**: Resolved fresh Clippy warnings, including regex construction in loops and nested conditional lint violations, while keeping fallback paths architecturally live.

### Fixed
- **ReadFile Wrapper Regression**: Restored legitimate `distill_readfile` usage through real fallback path instead of silencing dead-code warnings by removal.
- **Context Warning Reliability**: Fixed hook pipeline paths so anti-hallucination warnings only emit from factual runtime signals already known by OMNI.

## [0.5.7-rc1] - 2026-05-03

### Added
- **Automated Tool Distillation**: Implemented automated distillation for MultiEdit and unhandled tool outputs to reduce token usage natively.
- **Context-Aware Estimation**: New token estimation utility for highly accurate cost and usage calculations within the ROI monitor.
- **Positional Boosting**: Extracted positional boost logic to the semantic scorer, implementing dynamic priority-based segment distillation.

### Improved
- **Security Hardening**: Expanded the denylist of restricted environment variables in `sanitize_env` to prevent injection attacks.
- **Hash Entropy**: Increased the RewindStore archive hash length from 8 to 16 hex characters, ensuring 64-bit entropy and preventing collision.
- **Diagnostic Detection**: Enhanced the `BuildDistiller` to natively detect and prioritize single-line diagnostics and preserve git commit hashes in the collapse pipeline.
- **Code Quality**: Applied consistent `rustfmt` formatting and resolved all emerging Clippy warnings across the codebase.

### Fixed
- **Command Routing**: Fixed a critical command detection bug by stripping surrounding quotes from command names during base path extraction, allowing Antigravity IDE and other quoted-path environments to correctly route tools to specific distillers.

## [0.5.7] - 2026-04-27
### Added
- **Multi-Agent Awareness (`omni_agents`)**: New MCP tool allowing agents (e.g., Claude, Cursor, Copilot) to discover and interact with each other's state on the same project.
- **Persistent Project Knowledge (`omni_knowledge`)**: Cross-session memory for agents to permanently learn and store project-specific quirks and filter preferences.
- **Advanced ROI Diagnostics**: Added `omni_history` (distillation log) and `omni_budget` (ROI simulator) MCP tools directly to the agent toolkit.
- **Meta-Harness Outer Loop**: Implemented `omni optimize` to automatically validate generated LLM filters.
- **Non-Bash Tool Distillation**: Expanded engine routing for `ReadFile`, `Grep`, and `WebFetch` output.
- **Distiller Context Preservation**: Added `-->` contextual error block preservation to the Build and Test distillers.
- **Extended Hook Architecture**: New async hooks for `SessionEnd`, `PostToolUseFailure`, `FileChanged`, and `SubagentStart`.
- **Antigravity IDE Integration**: Native MCP server bindings for Google's Antigravity environment (`~/.gemini/antigravity/mcp_config.json`).

### Improved
- **Positional Scorer Boost**: Dynamic positional-based priority bumping for active errors in multi-line outputs.
- **Passthrough Visibility**: Short or low-compression outputs are now explicitly labeled with `[OMNI: Passthrough]` rather than silently omitted.

## [0.5.6-rc3] - 2026-04-14
- **Database Distiller**: New `DatabaseDistiller` for intelligent distillation of PostgreSQL, MySQL, and SQLite CLI output — strips verbose headers and retains only actionable error signals.
- **Security Distiller**: New `SecurityDistiller` for CVE scanners (Trivy, Snyk, Semgrep) — collapses verbose scan reports into concise vulnerability summaries.
- **VCS Distiller**: New `VcsDistiller` for version control tools beyond Git (Mercurial, SVN) with output-aware heuristics.
- **Expanded Tool Registry**: Added granular `cargo` subcommand support and new tool categories for Database, Mobile, Cloud, and CI/CD toolchains with accurate distiller routing.

### Improved
- **OpenClaw Portability**: The OpenClaw integration natively fetches plugin files directly from the public GitHub repository, allowing successful 1-click installation without requiring a full local git repository clone.
- **Robust RegEx Generation (`omni learn`)**: Fixed a critical bug where auto-learned numeric patterns used literal `#` instead of functional `\d+` in generated TOML filters. Now delegates TOML string escaping to the `toml` crate for correctness.
- **Enhanced Verify Report (`omni learn --verify`)**: Results are now grouped by source (Built-in vs. User), with clear per-category pass/fail counts and actionable tips when user-learned filters fail.
- **Auto-Clear Learn Queue**: `omni learn --apply` now automatically clears `~/.omni/learn_queue.jsonl` after successful application, preventing stale data from polluting subsequent `--discover` runs.
- **Premium Discover Table (`omni learn --discover`)**: Replaced raw text output with a structured `comfy-table` layout featuring color-coded actions (Strip/Count) and pattern previews.
- **Doctor Filter Diagnostics**: `omni doctor` now reports specific warnings for skipped filters (e.g., missing `match_command`) instead of generic error messages, and `--fix` can auto-repair invalid TOML files.
- **Distiller Robustness**: Replaced strict prefix checks with case-insensitive substring matching across all distillers for more reliable command detection.
- **Filter Loading**: Made `match_command` optional in `FilterConfig`, gracefully skipping filters with empty or missing patterns instead of crashing.

### Fixed
- **Learned Filters Concurrency (`learned.toml`)**: Replaced seconds-based timestamp resolution with `timestamp_micros()` for auto-generated filters to prevent fatal TOML duplication parse errors during high-frequency concurrent learning (fixes the infinite `doctor --fix` `.bak` failure loop).
- **Test Regression (`test_claude_code_stdout_format`)**: Resolved a persistent CI failure caused by state contamination from user-learned filters leaking into the test environment.
- **Stats UX Hint**: Added `--all-commands` usage hint to `omni stats` when showing truncated top-10 results.

## [0.5.6-rc2] - 2026-04-12

### Added
- **OpenClaw Support**: Introduced a native integration plugin for the **OpenClaw** agent framework. Includes a dedicated `omni_shell` tool for distilled execution and an `omni_rewind` tool for full log retrieval directly within the OpenClaw agent loop.
- **Command Grouping & Aggregation**: Enhanced `omni stats` to group identical or structurally similar commands (e.g., variant file paths in `ls -la`) into unified entries. This provides a significantly cleaner and more actionable signal report for repetitive tasks.

### Improved
- **CLI Semantic Clarity**: Renamed the `omni learn` flag `--status` to `--discover` to better align with its role in noise pattern discovery and candidate generation.
- **Null-Safe Telemetry**: Enforced robust null-safety handling in the SQLite storage layer using `COALESCE` for metric summations, preventing potential aggregation errors in sparse data environments.
- **Release Automation**: Hardened `bump_version.sh` and `omni-release.sh` to automatically synchronize version strings across the core Rust engine and the new OpenClaw integration plugin.

### Fixed
- **Integration Test Stability**: Updated `tests/savings_assertions.rs` to handle the expanded 4-tuple telemetry format, ensuring full CI compliance for the new grouping logic.
- **CLI Type Safety**: Resolved a type mismatch in the `omni stats --detail` view that caused formatting failures when processing heavily grouped filter entries.

## [0.5.6-rc1] - 2026-04-12

### Added
- **Magic Pipe Detection V2**: Automatic command source discovery via PGID inspection and parent shell fallback. This eliminates the need for manual `OMNI_CMD` labeling for piped commands in both interactive and scripted environments.
- **Configurable Token Pricing**: Introduced support for custom pricing models in `~/.omni/config.toml`, enabling accurate cost tracking for various models (e.g., GPT-4o, Claude Haiku).
- **Soft Route**: Fully implemented the 'Soft' distillation route for more flexible semantic engine behavior.
- **CLI ROI Metrics**: Expanded `omni stats --json` payload to include `savings_pct` for deeper CI/CD monitoring integration.

### Improved
- **High-Performance Filter Cache**: Implemented `OnceLock` caching for built-in TOML filters, significantly reducing overhead during high-frequency hook execution.
- **Command-First Architecture**: Completed the migration to a simplified engine by removing legacy `Classifier` and `Composer` modules.
- **Refined Stats UX**: Updated `omni stats` to strip redundant `omni exec` prefixes from automatically detected manual pipes for cleaner reports.

### Fixed
- **Post-Tool Telemetry**: Fixed a logic error in `src/hooks/post_tool.rs` that caused `segments_kept` and `segments_dropped` to be recorded as zero.
- **Stats Dead Code**: Cleaned up the "Distill" dead code path in stats color mapping and resolved various Clippy lints across the codebase.
- **Test Stability**: Hardened fragile assertions in `pipe.rs` unit tests to allow for benign local environment warnings.

## [0.5.5] - 2026-04-08

### Added
- **Command-Aware Intelligence**: Implemented path-aware classification heuristics to accurately detect terminal commands (e.g., `git`, `docker`, `kubectl`, `npm`) even when invoked via absolute paths.
- **Historical Data Re-classification**: Integrated "Intelligence Upgrade" into `omni doctor --fix`, allowing users to calibrate legacy 'Unknown' records with the latest classification models.
- **Cloud & Infra Heuristics**: Added native classification support for `kubernetes`, `terraform`, `aws`, `gcloud`, `helm`, and `azure` CLI tools.

### Improved
- **Real-time Update Notifications**: Reduced update check cache from 24 hours to **4 hours** and integrated proactive alerts directly into the `omni stats` dashboard.
- **Statistics UX**: Simplified `Unknown` category labels in the main signal report for a cleaner, more professional analytics display.
- **Classification Performance**: Optimized command-base matching to ensure sub-millisecond overhead during toolchain execution.

### Fixed
- **Code Integrity**: Resolved rusqlite iterator usage issues and addressed various Clippy lints to ensure 100% CI pass rate.

## [0.5.4] - 2026-04-07

### Added
- **OMNI Filter Pack**: Migrated and enhanced 12 new TOML-based filters for modern tools (Playwright, Ruff, golangci-lint, .NET, Prisma, Bun, Cypress, Jest, mypy, Black, pnpm, PHPUnit).
- **Core Distillers**: Implemented 3 new engine Distillers: `CloudDistiller` (Docker, K8s, Terraform), `SystemOpsDistiller` (ls, env, grep, tree), and `JsTsDistiller` (ESLint, TSC).
- **Session-Aware Distillation**: Injected tracking state directly into the distillers to unlock intelligent toolchain-specific context filtering logic.
- **Enhanced Stats Engine**: Upgraded `omni stats` command to fully support multi-period views, breakdown by context type, and valid JSON payload export.
- **Output Collapse Pipeline**: Added an algorithmic pipeline stage to collapse redundant contiguous lines to improve semantic block identification.
- **Quiet Mode Execution**: Introduced `OMNI_QUIET` environment variable to surgically suppress all stderr processing metrics for shell scripts.

### Fixed
- **Silent Exit Control**: Fixed persistent stderr pollution by ensuring OMNI terminates silently on completely blank piped inputs.
- **Security Check Integrity**: Updated the guardrail layer to ensure all denylist environment variable queries are strictly case-insensitive.
- **Windows Compatibility**: Updated GitHub Actions CI to matrix Windows tests and correctly restrict updater imports on unsupported OS.

### Improved
- **Zero-Mutation Tests**: Eradicated `std::env::set_var` to stabilize parallel thread runner execution via dependency injection (fixing deep UB).
- **Zero-Allocation ANSI**: Refactored the `strip_ansi` memory strategy to leverage `Cow<str>`, eliminating allocations for clean text snippets.
- **Pipeline Architecture**: Modularized the monolithic pipeline into cleanly separated `Classify → Score → Compose → Distill → Deliver` abstractions.

## [0.5.4-rc5] - 2026-04-01

### Added
- **Transcript Persistence**: Implemented robust session transcript persistence (`src/store/transcript.rs`) ensuring state is saved atomically to disk to prevent work loss.
- **Pre-Compact Double-Guardrail**: Injected `CRITICAL` at the start and `REMINDER` at the end of the `PreCompact` hook snapshot to drastically improve instruction adherence for Sonnet 4.6+ models.
- **Session Telemetry & ROI**: Enhanced `SessionState` to auto-calculate estimated tokens saved and identify the top data-reducing command purely in-memory (<5ms).
- **Session CLI**: Added new `omni session` commands for resuming and inspecting session transcripts.

### Fixed
- **Dead Code Cleanup**: Activated unused path mapping functions (`src/paths.rs`) and cleared various compiler warnings by completely wiring up the core pipeline.
- **Formatting & Linting**: Cleaned up the repository, removed obsolete GitHub PR templates, and integrated robust error checks for session boundaries.

## [0.5.4-rc4] - 2026-03-25

### Added
- **`omni doctor --fix`**: New `--fix` flag to automatically resolve integration issues — creates missing config directory, reinstalls hooks, registers MCP server, trusts project filters, and renames invalid user filter files to `.bak`.

### Fixed
- **Example Filter Template**: Rewrote `filters/00_example.toml` from legacy `[[filters]]` array-of-tables format to the standard `[filters.name]` schema, eliminating the embedded filter parse error at startup.
- **Stats Column Overflow**: Truncated the "Command" column in `omni stats` to a maximum of 21 characters with `...` ellipsis to prevent table layout breakage from long command names.

### Improved
- **Clippy Compliance**: Collapsed nested `if` statements in `doctor.rs` to satisfy `clippy::collapsible_if` lint.
- **Code Formatting**: Applied `cargo fmt` across all modified files for consistent style.

## [0.5.4-rc3] - 2026-03-25

### Added
- **Signal Comparison Mode**: Introduced `omni diff` command for side-by-side visualization of raw input vs. distilled output with "density gain" metrics.
- **Rewind Management**: Added `omni rewind list` and `omni rewind show <hash>` for local exploration of the RewindStore archive.
- **Real-time ROI Indicator**: New `[OMNI Active]` terminal status line providing immediate feedback on token reduction and latency.
- **Marketing Data Seeding**: New `scripts/seed_marketing.py` for generating high-impact, realistic demonstration data.

### Improved
- **Analytics UI**: Refined `omni stats` with professional English headers, better alignment, and improved financial impact estimation.
- **Log Classification**: Enhanced `RE_LOG_SEV` to recognize common bracket-less severity formats (e.g., `DEBUG:`).
- **Aesthetics**: Updated distillation and retrieval notices with rich ANSI colors and detailed impact summaries.

## [0.5.4-rc2] - 2026-03-25

### Improved
- **Version Awareness**: `omni doctor` and `omni update` now explicitly distinguish between `[LATEST]`, `[UPDATE]`, and `[AHEAD/RC]` statuses.
- **Diagnostic Precision**: Updated `omni doctor` to provide more accurate version status for users on pre-release or development branches.

### Fixed
- **Version Checker**: Corrected semantic comparison in `is_newer` to properly handle pre-release suffixes (e.g., `0.5.4-rc1` is now recognized as newer than `0.5.3`).
- **Release Script**: Updated `bump_version.sh` to support Semantic Versioning with pre-release tags (e.g., `-rc1`).

## [0.5.4-rc1] - 2026-03-25

### Added
- **Filter Priority System**: Introduced alphabetical sorting for built-in filters (e.g., `00_vitest.toml` vs `npm.toml`) to ensure specialized matches take precedence.
- **Enhanced `omni exec`**:
    - Intelligent Shell Detection: Automatically detects and runs commands with pipes, redirects, or semicolons via `sh -c`.
    - Real-time Distillation: Native command output is now seamlessly piped through OMNI's semantic engine.
    - Exit Code Passthrough: Native exit codes are now correctly preserved and returned to the caller.
- **Deep Terraform Support**: Expanded Terraform filters with over 40+ new specialized rules for cleaner infrastructure distillation.

### Improved
- **Filter Precision**: Refactored Vitest and Kubectl filters for higher signal-to-noise ratios.
- **Session Tracking**: Enhanced stability in session state persistence and rule application.

### Fixed
- **Hook Reliability**: Resolved edge cases in `PreToolUse` hook handling for more consistent distillation.

## [0.5.3] - 2026-03-25

### Added
- `omni update` command: Easily upgrade OMNI to the latest version via Homebrew with a confirmation prompt.
- Automated Version Check: OMNI now checks for updates from GitHub (24h cached) and notifies you in `help` and `doctor` screens.
- Safety Confirmations: Added `[y/N]` interactive prompts to `omni reset` and `omni update` to prevent accidental uninstalls or upgrades.
- Full Hook Diagnostics: `omni doctor` now explicitly checks and displays status for all 4 OMNI hooks, including `PreToolUse`.

### Fixed
- Hook Cleanup: `omni reset` and `omni init --uninstall` now correctly remove `PreToolUse` (Bash) hooks from Claude settings.
- Hook Detection: Fixed `omni doctor` logic to correctly identify OMNI hooks using any valid flag variant.
- Clippy Compliance: Resolved `collapsible-if` and other minor lints in the new update module.

### Improved
- CLI Diagnostics: Refined `omni doctor` output with clearer labels ("OMNI Hooks", "OMNI MCP Server") for better readability.

## [0.5.2] - 2026-03-25

### Added
- Native support for `npm run`, `yarn run`, `pnpm run`, and `bun run` scripts in TOML filters.
- Support for `python -m pytest` and `python3 -m pytest` commands.
- Support for `bun test` and `bun run test` runners.

### Improved
- Context Safety: Enhanced preservation of multi-line test failure diffs (Vitest/Jest) by refining empty-line stripping rules.
- Accuracy: Improved token savings calculations in `omni stats` for more precise analytics.

### Fixed
- Clippy Compliance: Resolved all remaining `D warnings` including `implicit-saturating-sub` in distillation hooks.
- Filter coverage gaps: Fixed missing interceptions for common JS/Python test runner variants.

## [0.5.1] - 2026-03-24

### Added
- `omni reset` command: Safely backs up configs to `~/.omni.<ts>.bak` and removes agent integrations (MCP/Hooks).
- Automated Release Workflow: `make release` now handles version bumping, commits, and tagging in one command.

### Fixed
- `omni learn` stability: Resolved stdin "hanging" when run interactively and fixed TOML parsing errors.
- Noise Deduplication: `omni learn` now skips patterns already present in `learned.toml`.
- TOML Generation: Improved escaping for quotes and invisible ANSI control characters in generated filters.
- Project-scoped MCP Detection: `omni doctor` now correctly identifies and validates nested project keys in `~/.claude.json`.

### Improved
- Actionable Suggestions: `omni doctor` now provides direct CLI commands to fix identified issues.
- Latency Assertions: Added deterministic tests to verify sub-50ms distillation performance.
- Clippy Compliance: Resolved all nesting and code quality warnings across the codebase.

## [0.5.0] - 2026-03-23

### Changed — Breaking
- Full rewrite in Rust — zero Node.js, zero Zig runtime
- `omni monitor` renamed to `omni stats`
- Hook format changed — run `omni init --hook` to reinstall

### Added
- Session continuity via SessionStart + PreCompact hooks
- RewindStore: compressed content retrievable via `omni_retrieve(hash)`
- Session-aware distillation: hot files and active errors boost signal priority
- `omni doctor` — installation diagnostics
- `omni learn` — auto-generate TOML filters from passthrough output
- Rust edition 2024
- SQLite WAL mode + FTS5 for session search

### Fixed
- Never Drop: output never silently discarded (RewindStore replaces passthrough)
- Zero startup overhead: native binary vs Node.js V8 startup

## [0.4.5] - 2026-03-20

### Added
- **Codex CLI & OpenCode AI Integration**: Native support for top-tier AI agent platforms. Run `omni generate codex` or `omni generate opencode` to automatically register OMNI and inject specialized filter bundles for each ecosystem.
- **Extensive Polyglot Filters**: Introduced over 60+ new semantic filters covering:
  - **Node/TS**: npm, yarn, pnpm, bun, tsc, eslint, prettier, vitest, jest, cypress, playwright, next.js, vite, webpack, nx.
  - **Python**: pytest, ruff, mypy, black, isort, pip, poetry.
  - **Rust/Go/Zig**: cargo, rustfmt, clippy, go build/test, zig build/test.
  - **DevOps/Cloud**: docker, docker-compose, kubectl, terraform, terragrunt, helm, ansible, skaffold, argocd.
  - **Security**: semgrep, trivy, gitleaks, snyk, hadolint, kubesec.
  - **Mobile/Other**: flutter, react-native, android-build, composer, gradle, make.
- **Hook Integrity Verification**: Implemented SHA256-based verification for OMNI hook scripts with `omni_trust_hooks` command and automatic startup checks to prevent execution of untrusted and potentially malicious scripts.
- **Project Trust Boundary**: Secure local configuration loading via `omni_trust` command. Review and trust project-specific `omni_config.json` rules before they are applied.
- **Autonomous Discovery**: Experimental `omni_learn` tool (via Wasm `discover` export) to automatically identify and suggest filters for repetitive noise patterns.
- **Improved Filter Transparency**: Filter names are now exposed via WASM and logged in real-time in the TypeScript MCP server for better diagnostics and efficiency monitoring.
- **Test Suite Migration**: Migrated core and filter tests from JavaScript to TypeScript using Bun, adding 50+ new ecosystem fixtures for robust verification.

### Fixed
- **MCP Server Stability**: Isolated MCP server tests using temporary home directories to prevent interference with local user configurations.
- **Cat Filter Scoring**: Adjusted confidence scoring for structured markdown to assign lower confidence to short, single-line noise without headers.

### Changed
- **CLI References**: Extensively updated `docs/CLI_REFERENCE.md` and `README.md` to reflect the latest command capabilities and security features.
- **Streamlined Workflow**: Simplified the `CONTRIBUTING.md` pull request process to focus on automated `make verify` checks.

## [0.4.4] - 2026-03-19

### Added
- **Test Infrastructure**: Implemented a comprehensive test suite in the `tests/` directory covering core filters (Git, Docker, SQL, Node) and the MCP server gateway, supported by new test helpers and fixtures.
- **CI/CD Integration**: Fully wired the semantic verification suite (`test-semantic.mjs`) and unit tests into both the `Makefile` and GitHub Actions workflow for automated quality gating.

### Fixed
- **Shell Injection**: Switched to `execFileAsync` with array arguments for `omni_grep_search` and `omni_find_by_name` to prevent shell injection vulnerabilities.
- **Wasm Memory Leak**: Wrapped the Wasm engine compression logic in `try/finally` blocks to ensure allocated memory is always freed, even on errors.
- **SQL Parsing**: Refactored `sql.zig` to use line-based splitting (`std.mem.splitAny`) instead of space-based, and fixed a bug where `--` comments caused the entire distillation to break.
- **Docker False Positive**: Hardened `docker.zig` matching logic to require specific signals like `FROM `, `RUN `, or `COPY ` alongside `Step ` or `CACHED` indicators.
- **Dynamic Scoring**: Replaced hardcoded `1.0` scores in `git`, `docker`, `sql`, and `node` filters with dynamic signal-density calculations for better distillation accuracy.
- **MCP Exit Codes**: Modified `omni_execute` and its aliases to return the actual command exit code in the tool's response metadata for programmatic handling.

## [0.4.3] - 2026-03-19

### Changed
- **Version bump**: Synchronized version strings across all 9 manifest and source files.


## [0.4.2] - 2026-03-18

### Added
- **OMNI Design System**: New shared UI architecture (`ui.zig`) for perfectly aligned boxed layouts and high-resolution performance meters across all CLI subcommands.
- **Agent Autopilot Aliases**: Automatic interception of native agent tools (`Bash`, `run_command`, `ReadFile`, `view_file`) via MCP to ensure transparent token distillation.
- **Custom DSL Rules**: Activated and fully integrated custom token-reduction DSL rules in the main semantic engine.

### Fixed
- **DSL Engine Stability**: Fixed a critical `use-after-free` segmentation fault in the JSON config parser by explicitly allocating memory for config string slices.
- **Filter Precedence**: Ensured user-defined rules from `omni_config.json` correctly take priority over built-in internal core filters.
- **CLI Output Cleanliness**: Removed stray debug prints in the compressor pipeline.

## [0.4.1] - 2026-03-17

### Added
- **`omni examples`**: Display real-world study cases and examples.
- **Proxy Command (`--`)**: Proxy and distill output from other commands (e.g., `omni -- git log`).
- **Antigravity Filter**: Integrated filter for Google Antigravity AI agent.
- **MCP Tools**: Implemented file system exploration and declarative filtering tools.

## [0.4.0] - 2026-03-16

### Added
- **`omni update`**: Check for the latest release from GitHub and get smart update instructions (auto-detects Homebrew vs installer).
- **New Landing Page**: Introduced a redesigned OMNI landing page.
- **FUNDING**: Added `FUNDING.yml`.

### Fixed
- **Homebrew Upgrade Stability**: `omni setup` now uses stable `/opt/omni` paths instead of versioned `/Cellar/omni/X.X.X` paths, preventing broken symlinks after `brew upgrade`.
- **Self-referencing Symlink**: `omni setup` now skips symlinking when source and destination are the same path.
- **Dynamic Versioning**: `build.zig` now defaults to the current release version instead of "development" when `-Dversion` is not specified.

### Changed
- **Release script**: Now synchronizes **9 locations** (added `core/build.zig` default version).
- Simplified `.github/pull_request_template.md` to checklist-only format.

## [0.3.9] - 2026-03-16

### Added
- **`omni uninstall`**: Clean removal of `~/.omni` directory and automatic cleanup of MCP configs from Antigravity, Claude Code CLI, and Claude Desktop.
- **Custom DSL Rules**: Activated and fully integrated custom token-reduction DSL rules configurable via `omni_config.json`.
- **Semantic Confidence Scoring**: Dynamic compression strategies based on filter confidence.
- **Agent Autopilot**: Dedicated UI and documentation to guide AI agent integration.
- **AI PR Describer**: Added `.github/workflows/ai-pr-describer.yml` for automated pull request descriptions.

### Fixed
- **DSL Engine Stability**: Fixed a critical `use-after-free` segmentation fault in the JSON config parser by explicitly allocating memory for config string slices.
- **Filter Precedence**: Ensured user-defined rules from `omni_config.json` correctly take priority over built-in internal core filters.
- **CLI Output Cleanliness**: Removed stray debug prints in the compressor pipeline.

## [0.3.8] - 2026-03-16

### Fixed
- **Version Synchronization**: All 8 versioned files now fully synchronized (`package.json`, `package-lock.json`, `core/build.zig.zon`, `src/index.ts`, `src/index.js`, `scripts/omni-deploy-edge.sh`, `docs/index.html`, `omni.rb`).
- **Release Automation**: `omni-release.sh` updated to handle docs and deploy script versioning.

## [0.3.7] - 2026-03-16

### Added
- **Local Metrics System**: Every `omni distill` and MCP call now records usage to `~/.omni/metrics.csv`.
- **Expanded `omni report`**: Daily, Weekly, and Monthly breakdown tables with token savings (Cmds, Input, Output, Saved, Save%, Time).
- **Agent Filtering**: `omni report --agent=claude-code` to view per-agent metrics.
- **Agent Tagging**: `omni generate` now includes `--agent=<name>` in MCP config for automatic tracking.
- **PR Template**: Added `.github/pull_request_template.md`.

### Fixed
- **`omni setup` symlink**: Now searches 4 candidate paths for `index.js` and removes stale symlinks before creating new ones.
- **Installer (`install.sh`)**: Fixed color formatting (`%b`), version passing (`-Dversion`), and quoting issues.
- **Homebrew formula**: Replaced `post_install` with `caveats` to avoid sandbox issues with `$HOME`.

### Changed
- **Release script**: `omni-release.sh` now auto-bumps `build.zig.zon` and `package.json` versions.
- Removed `ARCHITECTURE.md` link from `CONTRIBUTING.md` and `docs/index.html`.

## [0.2.0] - 2026-03-15

### Added
- **Unified Native CLI**: Replaced shell scripts with high-performance native subcommands.
- Subcommands: `omni distill`, `omni density`, `omni report`, `omni bench`, `omni generate`, `omni setup`.
- **Agent Templates**: Support for generating Antigravity and Claude Code input templates.
- **Zig Build System**: Fully integrated `build.zig` for cross-platform native and Wasm builds.

### Changed
- Moved all legacy shell scripts to `scripts/legacy/`.
- Updated `install.sh` to use the native build pipeline.

## [0.1.3] - 2026-03-15

### Fixed
- Zig 0.15.2 IO API transition: Replaced removed `std.io.getStdOut/getStdIn` with `std.fs.File` equivalents.
- Native build failure on Homebrew environment.

## [0.1.2] - 2026-03-15

## [0.1.1] - 2026-03-15

## [0.1.0] - 2026-03-14

### Added
- Initial Zig core engine implementation.
- Basic Git and Build log filters.
- MCP Server gateway in TypeScript.
- Custom JSON-based rules for masking/removal.

---
*Follow the OMNI vision.*
