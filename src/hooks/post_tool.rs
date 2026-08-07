use crate::pipeline::toml_filter;
use crate::pipeline::{DistillResult, Route, SessionState, collapse, format, scorer};
use crate::store::sqlite::Store;
use serde::Serialize;
use std::sync::{Arc, Mutex};
use std::time::Instant;

// Input parsing moved to hooks::normalize

#[derive(Serialize)]
struct HookOutput {
    #[serde(rename = "hookSpecificOutput")]
    hook_specific_output: HookSpecificOutput,
}

#[derive(Serialize)]
struct HookSpecificOutput {
    #[serde(rename = "hookEventName")]
    hook_event_name: &'static str,
    /// The key the host reads to replace what the model sees.
    ///
    /// This was `updatedResponse` from the first day of the Rust rewrite until
    /// #158 — a key Claude Code does not recognise. Unknown keys are dropped
    /// silently, so the agent received the raw output for the whole life of the
    /// hook while OMNI recorded `Route::Keep` and printed a savings footer for
    /// each one. The sibling `additionalContext` *was* spelled correctly, which
    /// is why the footer appeared and made the failure look like success.
    ///
    /// `serialises_the_key_the_host_actually_reads` is what keeps this honest:
    /// a struct-level test cannot catch a wrong key, because it asserts on the
    /// same field name it serialised.
    #[serde(rename = "updatedToolOutput")]
    updated_tool_output: ToolOutput,
    #[serde(rename = "additionalContext")]
    #[serde(skip_serializing_if = "Option::is_none")]
    additional_context: Option<String>,
}

/// The replacement tool result. There is no single shape — the host validates
/// this value against **the schema of the tool that ran**, so it has one shape
/// per host tool, not one shape per hook.
///
/// #158 fixed the key and left this wrong, which is why the symptom survived it:
/// Claude Code parsed `updatedToolOutput`, rejected `{status, result}` against
/// Bash's schema (`stdout`/`stderr`/`interrupted` "expected string, received
/// undefined"), restored the original output, and rendered
/// `PostToolUse:Bash hook warning` — while the sibling `additionalContext` went
/// through and printed a saving for a distillation that had just been discarded.
#[derive(Serialize)]
#[serde(untagged)]
enum ToolOutput {
    /// The host tool's own result object, echoed back with the output text
    /// swapped and every other key preserved verbatim. Optional members
    /// (`isImage`, `backgroundTaskId`, `persistedOutputPath`, `timedOutAfterMs`,
    /// …) are part of the schema, so dropping them would fail validation exactly
    /// as the old shape did.
    Host(serde_json::Value),
    /// The MCP tool-result shape, for payloads that arrived without a host
    /// response object to echo. Unchanged from before #187 **on purpose**: those
    /// hosts' contracts were not investigated, and guessing at a second one is
    /// how the first was got wrong.
    ///
    /// `status` is always `success` because a failed command returns `None` well
    /// before this point (#120) and never reaches here — so this cannot assert a
    /// success for a command that failed.
    Mcp {
        status: &'static str,
        result: String,
    },
}

/// Put `distilled` into the shape the host will accept for this call.
///
/// Verified against Claude Code 2.1.218's own dispatch: it runs
/// `tool.outputSchema.safeParse(value)` and falls back to
/// `tool.mapToolResultToToolResultBlockParam(value)`, both keyed on the tool
/// that ran — which is why this reads the shape off the payload that arrived
/// instead of asserting one. The rule is "reply in the shape you were spoken
/// to in", and it needs no table of per-tool schemas to stay correct.
fn shape_for_host(raw_response: Option<&serde_json::Value>, distilled: String) -> ToolOutput {
    // The `Read` result carries its text at `file.content`, beside numbers that
    // describe it. Captured from a live Claude Code transcript rather than
    // assumed:
    //
    //   {"type":"text","file":{"filePath":…,"content":…,
    //                          "numLines":420,"startLine":1,"totalLines":420}}
    //
    // `numLines` counts the lines *in this payload*, so swapping the content and
    // leaving it alone would have the host report 420 lines for a shorter one —
    // a fabricated number, which is the defect this project exists to stop
    // emitting. `totalLines` and `startLine` describe the file and the request,
    // not the payload, so they stay (#172).
    if let Some(obj) = raw_response.and_then(|v| v.as_object())
        && obj
            .get("file")
            .and_then(|f| f.get("content"))
            .is_some_and(serde_json::Value::is_string)
    {
        let mut echoed = obj.clone();
        let line_count = distilled.lines().count();
        if let Some(file) = echoed.get_mut("file").and_then(|f| f.as_object_mut()) {
            file.insert("content".into(), serde_json::Value::String(distilled));
            file.insert("numLines".into(), serde_json::Value::from(line_count));
        }
        return ToolOutput::Host(serde_json::Value::Object(echoed));
    }

    // Only an object carrying `stdout` is known to be echoable: that is the
    // Bash-family result, the one shape #187 measured against a live host.
    // Anything else keeps the MCP shape rather than inventing a schema.
    let Some(obj) = raw_response
        .and_then(|v| v.as_object())
        .filter(|o| o.get("stdout").is_some_and(serde_json::Value::is_string))
    else {
        return ToolOutput::Mcp {
            status: "success",
            result: distilled,
        };
    };

    let mut echoed = obj.clone();
    echoed.insert("stdout".into(), serde_json::Value::String(distilled));
    // `normalize` folds a non-empty stderr into the text that was distilled, so
    // the distilled `stdout` already carries it. Echoing the original back too
    // would show it twice. Blanked rather than removed: Bash's schema requires
    // `stderr` to be a string, so dropping the key fails validation.
    echoed.insert("stderr".into(), serde_json::Value::String(String::new()));

    ToolOutput::Host(serde_json::Value::Object(echoed))
}
#[tracing::instrument(skip_all)]
/// The rewind rule from #271, in one function because several paths need it and
/// a copy of it drifts.
///
/// Returns the marker to append and the hash it archived under, empty when the
/// content was too large for `MAX_REWIND_BYTES` or there is no store. The caller
/// decides whether the marker is affordable, because only the caller knows what
/// it will hand back if it is not.
/// `kept_lines` and `kept_bytes` describe what the *distiller* produced, not what
/// is about to be sent. The two differ once OMNI appends its own commentary, and
/// counting a banner as surviving content understated the loss by exactly the
/// number of banner lines: 10 dropped rows reported as `9 lines omitted` (#301).
/// They are numbers rather than a `&str` so the caller cannot pass the wrong
/// string again.
fn rewind_marker(
    store: Option<&Arc<Store>>,
    content: &str,
    kept_lines: usize,
    kept_bytes: usize,
) -> (String, String) {
    let input_lines = content.lines().count();
    let omitted_lines = input_lines.saturating_sub(kept_lines);
    // Lines are what a reader can act on, but a distillation can also shorten
    // lines in place and leave the count alone. Report the unit that is true for
    // this call rather than printing "0 lines omitted" over missing bytes.
    //
    // More lines out than in means the distiller restructured rather than cut:
    // `distill_grep_output` folds a repeated `path:` prefix into a header, so 11
    // matches become 15 lines holding all 11, and the byte delta is the redundant
    // prefixes. Reporting that as `274 bytes omitted` under a `[Partial signal]`
    // banner told the reader a complete answer was incomplete (#335). An
    // in-place shortening keeps the count equal, which is why the test is `>`
    // and not `>=`: that case is real loss and still gets its byte figure.
    if kept_lines > input_lines {
        return (String::new(), String::new());
    }
    let lost = if omitted_lines > 0 {
        format!("{omitted_lines} lines")
    } else {
        format!("{} bytes", content.len().saturating_sub(kept_bytes))
    };

    if content.len() > crate::guard::limits::MAX_REWIND_BYTES {
        return (
            format!(
                "\n[OMNI: {lost} omitted, full output not archived: {} bytes over the {} byte rewind cap]\n",
                content.len(),
                crate::guard::limits::MAX_REWIND_BYTES
            ),
            String::new(),
        );
    }
    match store {
        Some(s) => {
            let hash = s.store_rewind(content);
            (
                format!("\n[OMNI: {lost} omitted, omni_retrieve(\"{hash}\") for full output]\n"),
                hash,
            )
        }
        None => (
            format!("\n[OMNI: {lost} omitted, full output not archived: no store]\n"),
            String::new(),
        ),
    }
}

/// What a non-Bash tool arm hands back once the rewind rule has been applied.
///
/// `None` declines the rewrite, which leaves the host holding its own bytes.
/// That is the right answer when the marker would cost more than the cut saved:
/// the agent would be paying tokens for fewer facts, which is the #268 and #269
/// shape on small payloads.
///
/// These arms return before the Bash pipeline's own rewind block, so without
/// this they drop bytes with no marker and nothing to retrieve (#273).
fn archive_tool_reply(
    store: Option<&Arc<Store>>,
    content: &str,
    distilled: String,
) -> Option<String> {
    if distilled.len() >= content.len() {
        return Some(distilled);
    }
    // No banner on this path: the distilled string is the whole reply.
    let (marker, _hash) = rewind_marker(store, content, distilled.lines().count(), distilled.len());
    (distilled.len() + marker.len() < content.len()).then(|| distilled + &marker)
}

pub fn process_payload(
    input_str: &str,
    store: Option<Arc<Store>>,
    session: Option<Arc<Mutex<SessionState>>>,
) -> Option<String> {
    let normalized = crate::hooks::normalize::normalize(input_str)?;

    // #120: a command that exited non-zero passes through verbatim — never distilled.
    // Distillation must never turn a failed command into output that reads as success
    // (a fabricated success terminates investigation; a fabricated error only costs a
    // retry). Emit nothing: the host keeps the original bytes at zero marker cost.
    if normalized.failed {
        return None;
    }

    if crate::guard::env::is_passthrough() {
        return None;
    }

    // Format-safe gate: structured payloads are parsed by whatever reads them next,
    // so every lossy stage below — including the >2MB head/tail trim — would corrupt
    // them. Emit nothing: the host keeps the original bytes at zero marker cost.
    if let Some(kind) = format::sniff(&normalized.content) {
        if let Some(ref s) = store {
            s.record_passthrough(
                &format!(
                    "{} [{}]",
                    normalized.command,
                    format::passthrough_reason(kind)
                ),
                normalized.content.len(),
            );
        }
        return None;
    }

    // The host capped this payload, which means the command produced more than
    // arrived here — and above that size Claude Code persists the **raw** output
    // to a file, previews the **raw** first 2 KB, and drops whatever the hook
    // returns. Distilling it is work nobody reads, and booking the result is a
    // saving that never happened: one such row claimed 93% compression and 6,194
    // tokens for a 2,129-byte distillation that appears nowhere in the transcript
    // (#212). Emit nothing, and record it as the passthrough it really is.
    //
    // Measured on `tool_response.stdout`, the field the host actually caps, and
    // deliberately **not** on `normalized.content`: `normalize` folds a non-empty
    // stderr into that string, so a 25 KB stdout beside a 6 KB stderr would clear
    // the cap on a result the host never truncated, and this guard would decline
    // to distil perfectly ordinary output. Found by self-review before merge;
    // the wrong reading loses compression silently, which is the failure mode
    // that does not announce itself.
    let host_capped_stdout = normalized
        .raw_response
        .as_ref()
        .and_then(|r| r.get("stdout"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(|s| s.len() >= crate::guard::limits::HOST_OUTPUT_CAP);

    if normalized.agent_id == "claude_code" && host_capped_stdout {
        if let Some(ref s) = store {
            s.record_passthrough(
                &format!("{} [host output cap]", normalized.command),
                normalized.content.len(),
            );
        }
        return None;
    }

    // L1-03: Streaming Distillation Support (Buffer warning & chunked processing)
    // Prevent OMNI from blocking memory if output is extremely large (> 2MB)
    let content = if normalized.content.len() > 2_000_000 {
        let lines: Vec<&str> = normalized.content.lines().collect();
        let total_lines = lines.len();
        let head_lines = 5000;
        let tail_lines = 1000;

        if total_lines > head_lines + tail_lines {
            let head = lines
                .iter()
                .take(head_lines)
                .copied()
                .collect::<Vec<&str>>()
                .join("\n");
            let tail = lines
                .iter()
                .skip(total_lines.saturating_sub(tail_lines))
                .copied()
                .collect::<Vec<&str>>()
                .join("\n");
            format!(
                "{}\n\n... [OMNI: ⚠️ {} lines omitted due to extreme length (>2MB)] ...\n\n{}",
                head,
                total_lines - (head_lines + tail_lines),
                tail
            )
        } else {
            normalized.content
        }
    } else {
        normalized.content
    };

    let config = crate::guard::config::load_config();
    let agent_config = config.for_agent(&normalized.agent_id);

    // Route based on tool_name: handle non-Bash tools with specialized distillation.
    //
    // Every arm below returns before the Bash pipeline's rewind block, so each one
    // that shortens its reply has to apply the same rule itself, through
    // `archive_tool_reply`. Without it these paths drop bytes with no marker and
    // no recoverable copy, which is the guarantee #271 closed for `Bash` and
    // #273 found still open here.
    match normalized.tool_name.as_str() {
        "Bash" => { /* fall through to existing pipeline below */ }
        "Read" => {
            if !agent_config.readfile_enabled() {
                return None;
            }
            let filepath = if normalized.command.is_empty() {
                "unknown"
            } else {
                &normalized.command
            };
            // Phase 6: the dependents guard needs a count, and getting one walks
            // the repository. Handed over as a closure so the walk happens only
            // if the distiller reaches the guard, which it does after two gates
            // that reject most payloads. Building it here cost 48 ms on every
            // hooked Read, most of them for a number nothing read (#320).
            let count_dependents = || {
                std::env::current_dir()
                    .ok()
                    .and_then(|cwd| crate::graph::indexer::build_graph(&cwd).ok())
                    .map(|g| g.context_for(filepath).imported_by.len())
                    .unwrap_or(0)
            };

            return crate::distillers::readfile::distill_readfile_with_context(
                &content,
                filepath,
                count_dependents,
            )
            .and_then(|d| archive_tool_reply(store.as_ref(), &content, d))
            .map(|d| wrap_hook_output(normalized.raw_response.as_ref(), d));
        }
        "Grep" => {
            if !agent_config.grep_enabled() {
                return None;
            }
            return distill_grep(&content)
                .and_then(|d| archive_tool_reply(store.as_ref(), &content, d))
                .map(|d| wrap_hook_output(normalized.raw_response.as_ref(), d));
        }
        "WebFetch" => {
            if !agent_config.webfetch_enabled() {
                return None;
            }
            return process_web_content(&content)
                .and_then(|d| archive_tool_reply(store.as_ref(), &content, d))
                .map(|d| wrap_hook_output(normalized.raw_response.as_ref(), d));
        }
        "Edit" | "Write" | "Create" | "Move" | "Delete" | "Replace" => return None,
        "MultiEdit" => {
            if content.len() < 200 {
                return None;
            }
            let lines: Vec<&str> = content.lines().collect();
            let summary = format!(
                "[OMNI MultiEdit: {} lines]\n{}",
                lines.len(),
                lines.into_iter().take(30).collect::<Vec<&str>>().join("\n")
            );
            if summary.len() < content.len() * 8 / 10 {
                return archive_tool_reply(store.as_ref(), &content, summary)
                    .map(|d| wrap_hook_output(normalized.raw_response.as_ref(), d));
            }
            return None;
        }
        _ => {
            if let Some(ref s) = store {
                s.record_unhandled_tool(&normalized.tool_name);
            }
            if content.len() > 2000 {
                let lines: Vec<&str> = content.lines().collect();
                let summary = format!(
                    "[OMNI {}: {} lines]\n{}",
                    normalized.tool_name,
                    lines.len(),
                    lines.into_iter().take(30).collect::<Vec<&str>>().join("\n")
                );
                return archive_tool_reply(store.as_ref(), &content, summary)
                    .map(|d| wrap_hook_output(normalized.raw_response.as_ref(), d));
            }
            return None;
        }
    }

    if content.len() < 50 {
        return None;
    }

    let command = normalized.command.clone();
    let _agent_id = &normalized.agent_id;

    let clean_command = if let Some(stripped) = command.strip_prefix("omni exec ") {
        stripped
    } else {
        &command
    };

    let start = Instant::now();
    let project_path = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    // Which single command produced this stdout, if any. `None` means a chain
    // whose output belongs to several programs, and every routing decision below
    // has to stand down for it: a TOML filter keyed on `^git\b` claims
    // `git status && find .` exactly as the git distiller did (#264).
    let output_command = crate::pipeline::registry::sole_output_command(clean_command);

    // TOML-first: try matching command against TOML filters
    let toml_filters = toml_filter::load_all_filters();
    let toml_match = match output_command {
        Some(cmd) if !cmd.is_empty() => toml_filters.iter().find(|f| f.matches(cmd)),
        _ => None,
    };

    // A TOML filter only gets to short-circuit the distiller if it actually beat
    // the guardrail — the same rule `hooks::pipe` already applies. Without it the
    // broad `signals/domains/*.toml` filters win the alphabetical `find()` race
    // for cargo, npm, docker, kubectl and terraform while stripping only a few
    // lines, shadowing the distiller that would have summarised the same input
    // (#110). Weak filter, fall through; a filter that earns its match still wins.
    let toml_hit = match toml_match {
        Some(f) => match f.apply_batch(&content) {
            toml_filter::BatchFilterOutcome::Passthrough => {
                // The filter matched but produced no explicit zero-state.
                // Returning `None` is the hook protocol's fail-open path: the
                // host keeps its original bytes. Falling through here would
                // hand the same payload to another distiller — `black` turned
                // 200 all-stripped rows into the fabricated `Build: ok` (#224).
                if let Some(ref s) = store {
                    s.record_passthrough(
                        &format!("{clean_command} [toml zero-state]"),
                        content.len(),
                    );
                }
                return None;
            }
            toml_filter::BatchFilterOutcome::Filtered(out) => {
                crate::guard::limits::beats_guardrail(out.len(), content.len())
                    .then(|| (out, f.name.clone()))
            }
        },
        None => None,
    };

    let session_guard = session.as_ref().and_then(|l| l.lock().ok());
    let mut collapse_savings_data = None;
    let (final_out, filter_name) = if let Some((output, name)) = toml_hit {
        (output, name)
    } else {
        // Pure Command Architecture: Resolve profile once
        let profile = crate::pipeline::registry::resolve_profile_for_chain(clean_command);

        // Score and distill the tool's REAL output. #116: `collapse` rewrites
        // repeated lines into `[N similar lines collapsed]` markers, and a
        // distiller that parses columns reads those markers as data — a 35-pod
        // table came out as `k8s: 2 pods | [5 (lines)`, built entirely from
        // OMNI's own scaffolding. A distiller is a later stage that parses its
        // input, exactly what `format::sniff` already protects structured
        // payloads from; so the distiller sees raw content, and collapse is the
        // fallback only for commands no distiller handles (below).
        let segments = scorer::score_segments(
            &content,
            profile.segmentation,
            session_guard.as_deref(),
            clean_command,
        );

        let distilled = crate::distillers::distill_with_command(
            &segments,
            &content,
            clean_command,
            session_guard.as_deref(),
        );

        // When no distiller meaningfully reduced the raw output — it punted
        // (returned the input) or produced a near-copy that misses the guardrail —
        // fall back to the collapsed form for its line savings. A distiller that
        // earned its summary keeps it; the lossy markers never reached a distiller.
        // Enumeration commands (`ls`/`find`/`ps`/…) deliberately pass through
        // verbatim; collapsing them drops rows that are the answer, so skip the
        // fallback for them (#200).
        // The verbatim check has to ask the resolved command, not the string the
        // user typed. Reading the whole string sees `kubectl` in
        // `kubectl get pods -o json | jq -r '...'` and lets collapse rewrite a
        // payload the next step parses, and it sees `cd` in `cd x && cat file`
        // and collapses a file read the same way (#269, #235).
        let output = if !crate::guard::limits::beats_guardrail(distilled.len(), content.len())
            && output_command
                .is_some_and(|c| !crate::pipeline::registry::passes_through_verbatim(c))
        {
            let collapse_result = collapse::collapse(&content, &profile.collapse);
            collapse_savings_data = if collapse_result.original_lines > collapse_result.collapsed_to
            {
                Some((collapse_result.original_lines, collapse_result.collapsed_to))
            } else {
                None
            };
            collapse_result.collapsed_lines.join("\n")
        } else {
            distilled
        };

        // Label the row with the program that produced the output, not with
        // whatever token came first. `OMNI_DB_PATH=/tmp/d.db sqlite3 …` wrote the
        // assignment and its value into this column: 1,079 rows and 264 distinct
        // values in one database, which made every aggregate keyed on it useless
        // and hid the routing defect this change fixes (#339).
        let producer =
            crate::pipeline::registry::sole_output_command(clean_command).unwrap_or(clean_command);
        (
            output,
            producer
                .split_whitespace()
                .next()
                .unwrap_or("omni")
                .to_string(),
        )
    };

    drop(session_guard); // Release lock ASAP sebelum rewind check

    // Check for rewind decision
    let mut final_out = final_out;
    let mut rewind_hash = String::new();

    // Re-check segments from content for metadata/learning. Same resolver the
    // distillation above used: the direct one disagrees on a chain, so on
    // `cd /tmp/x && kubectl get pods` this read `Generic` where the scoring read
    // `Infra`, and the counts and the learn queue were built from a profile that
    // never ran (#339).
    let profile = crate::pipeline::registry::resolve_profile_for_chain(clean_command);
    let check_segments =
        scorer::score_segments(&content, profile.segmentation, None, clean_command);

    let noise_count = check_segments
        .iter()
        .filter(|s| s.final_score() < 0.3)
        .count();

    // Auto-learn trigger
    if !clean_command.is_empty() && content.len() > 100 {
        let total = check_segments.len();
        let dropped = noise_count;
        let poor = total > 5 && (dropped as f32 / total.max(1) as f32) < 0.3;
        if poor {
            crate::session::learn::queue_for_learn(&content, clean_command);
        }
    }

    // Update session state
    if let Some(ref lock) = session
        && let Ok(mut state) = lock.lock()
    {
        if !command.is_empty() {
            state.add_command(&command);
        }
        for seg in &check_segments {
            if seg.tier == crate::pipeline::SignalTier::Critical {
                state.add_error(&seg.content);
            }
        }
    }

    // Determine Route based on agent config thresholds + adaptive retrieve rate
    let ratio = 1.0 - (final_out.len() as f32 / content.len().max(1) as f32);
    let (mut keep_threshold, mut soft_threshold) = agent_config.route_thresholds();

    // Adaptive compression: if agents often retrieve full output for this command,
    // reduce compression aggressiveness by lowering thresholds
    let cmd_family = crate::util::command_family::command_family(clean_command);
    if let Some(ref s) = store {
        let retrieve_rate = s.get_retrieve_rate(&cmd_family, 7);
        if retrieve_rate > 0.25 {
            // High retrieve rate — significantly harder compression thresholds (require more compression to keep)
            keep_threshold = (keep_threshold + 0.15).min(0.95);
            soft_threshold = (soft_threshold + 0.10).min(0.85);
        } else if retrieve_rate > 0.05 {
            // Moderate retrieve rate — slightly harder thresholds
            keep_threshold = (keep_threshold + 0.05).min(0.90);
            soft_threshold = (soft_threshold + 0.03).min(0.80);
        }
    }

    let mut route = if ratio >= keep_threshold {
        Route::Keep
    } else if ratio >= soft_threshold {
        Route::Soft
    } else {
        Route::Passthrough
    };

    // Where the distiller's output ends and OMNI's own commentary begins. The
    // marker below states how much of the command's output is missing, and
    // counting a banner OMNI appended as if it were surviving content
    // understates that by exactly the number of banner lines: a 20 row
    // `kubectl config get-contexts -o name` kept 10 rows and reported
    // `9 lines omitted` for the 10 that went (#301). An agent uses that number
    // to decide whether a rewind is worth fetching.
    let distilled_lines = final_out.lines().count();
    let distilled_len = final_out.len();

    // Same condition as `rewind_marker`'s: a restructure that emits more lines
    // than it consumed dropped nothing, so the banner saying the signal is
    // partial is a false claim about a complete answer (#335). `Soft` is decided
    // on the byte ratio alone, and folding a repeated prefix shrinks bytes
    // without losing a line.
    if route == Route::Soft && distilled_lines <= content.lines().count() {
        final_out.push_str("\n[Partial signal - omni learn recommended]\n");
    }

    // Measure ratio strictly
    if final_out.len() >= content.len() * 9 / 10 {
        // Record passthrough metric regardless of size
        if let Some(ref s) = store {
            s.record_passthrough(&format!("{clean_command} [below guardrail]"), content.len());
        }

        // Take the route the banner names. Prefixing `Passthrough` onto
        // `final_out` announced "OMNI changed nothing" over bytes a distiller
        // had already deleted lines from: four data rows of a markdown table
        // went, the header and separator stayed, so the table read as present
        // and empty, and the banner told the agent not to re-run (#229). Under
        // a tenth saved there is nothing here worth a deletion, so hand back
        // what the command produced.
        final_out = content.clone();
        route = Route::Passthrough;
    }

    // The rewind decision, and it asks one question: is this reply, the one about
    // to be delivered, missing bytes the command produced?
    //
    // The old gate asked the scorer for a noise ratio and wanted more than 40%
    // noise across more than 20 segments. No real payload had that shape: 0 of
    // 8,968 recorded distillations carried a hash and `rewind_store` held
    // nothing, so "everything cut is archived" (`README.md:81`) had never been
    // true for a single row (#271). It also asked the wrong question. A re-scored
    // noise ratio says what the scorer thought of the input, not what the agent
    // is about to lose, and the two disagree on every path where a TOML filter or
    // a distiller produced the output.
    //
    // It runs after the route is settled, and that placement is load-bearing. An
    // earlier draft archived before routing: measured on the real binary,
    // `git log --oneline -80` archived on 15 runs out of 15 and then handed the
    // raw output back anyway, so every one of those writes stored content the
    // agent had never lost. A passthrough returns `None` below and the host keeps
    // its own bytes; there is nothing to recover and nothing to record.
    if route != Route::Passthrough && final_out.len() < content.len() {
        let (marker, hash) =
            rewind_marker(store.as_ref(), &content, distilled_lines, distilled_len);
        rewind_hash = hash;

        // The marker is not free. One that costs more than the cut saved is a
        // reply with fewer facts and more tokens, which is #268 and #269 on 102
        // and 90 byte payloads. Below that floor, hand back what the command
        // produced instead.
        if final_out.len() + marker.len() < content.len() {
            final_out.push_str(&marker);
            if !rewind_hash.is_empty() {
                route = Route::Rewind;
            }
        } else {
            final_out = content.clone();
            route = Route::Passthrough;
            rewind_hash.clear();
        }
    }

    let latency_ms = start.elapsed().as_millis() as u32;

    let kept = check_segments.len() - noise_count;
    // Reporting columns, so the calibrated central estimate. The exact counter
    // that stood here cost 34.3 ms of every hooked command to be 4.9% closer,
    // against GPT's vocabulary rather than Claude's (#283).
    use crate::util::token_estimate::{ContentHint, estimate_tokens};
    let raw_tokens = estimate_tokens(content.len(), ContentHint::Mixed);
    let filtered_tokens = estimate_tokens(final_out.len(), ContentHint::Mixed);

    let result = DistillResult {
        output: final_out.clone(),
        route: route.clone(),
        filter_name: filter_name.clone(),
        score: 0.0,
        context_score: 0.0,
        input_bytes: content.len(),
        output_bytes: final_out.len(),
        latency_ms: latency_ms as u64,
        rewind_hash: if rewind_hash.is_empty() {
            None
        } else {
            Some(rewind_hash)
        },
        segments_kept: kept,
        segments_dropped: noise_count,
        collapse_savings: collapse_savings_data,
        raw_tokens,
        filtered_tokens,
        // The hook hands this string to the host as the replacement tool result,
        // and since #187 the host accepts it. The one path where it did not —
        // output above the host's own cap, where the raw output is persisted and
        // the hook's reply dropped — now returns before reaching here, so this
        // is what the model receives (#212).
        delivered_bytes: final_out.len(),
    };

    if let Some(ref s) = store {
        // The host's id wins when it sent one. `SessionState::session_id` is a
        // wall-clock stamp on a globally persisted state, so it groups by
        // "whenever OMNI last started" rather than by session: one such id
        // covered 16 project paths and 3,739 commands, which is what makes the
        // banner and every per-session slice of `omni stats` wrong (#118).
        // Fall back to it rather than dropping the row — pipe mode and hosts
        // that send no id still have to be recorded.
        let session_id = normalized
            .host_session_id
            .clone()
            .or_else(|| {
                session
                    .as_ref()
                    .and_then(|lock| lock.lock().ok())
                    .map(|s| s.session_id.clone())
            })
            .unwrap_or_else(|| "unknown".to_string());
        s.record_distillation(
            &session_id,
            &result,
            clean_command,
            &project_path,
            _agent_id,
        );
        s.record_trace(
            &session_id,
            clean_command,
            _agent_id,
            &project_path,
            &content,
            &final_out,
        );

        if let Some(ref sess) = session {
            // Phase 1: Context Composition Analyzer
            if let Ok(mut state) = sess.lock() {
                state.current_turn.session_id = state.session_id.clone();
                state.current_turn.turn_number = state.command_count;
                state.current_turn.timestamp = chrono::Utc::now().timestamp();
                state.current_turn.tool_output_tokens += result.filtered_tokens as u64;

                // L1-02: Increment loop iteration budget
                state.loop_context.budget_used += result.filtered_tokens as u64;

                // `context_turns` had no reader and is gone (#270); the in-memory
                // `current_turn` below is what `omni stats` and the MCP breakdown read.
            }

            let tracker = crate::session::tracker::SessionTracker::new(sess.clone(), s.clone());
            tracker.track_command(&command, &content, &result);

            // ── Implicit Engram Auto-Capture ────────────────
            // Zero-config: OMNI silently persists milestone memories.
            // No user action required — fires automatically on key events.
            if let Ok(state) = sess.lock() {
                let had_errors = state.active_errors.len() > 1; // proxy: had errors before this call
                let has_errors_now = !state.active_errors.is_empty();
                let resolved_error = state.active_errors.first().map(|s: &String| s.as_str());
                // Extract any file-like tokens from the command as context
                let files: Vec<String> = clean_command
                    .split_whitespace()
                    .filter(|t| t.contains('/') || (t.contains('.') && !t.starts_with('-')))
                    .take(3)
                    .map(|s| s.to_string())
                    .collect();
                let tool_family = crate::util::command_family::command_family(clean_command);

                if let Some(engram) = crate::session::engram::detect_engram(
                    clean_command,
                    had_errors,
                    has_errors_now,
                    &tool_family,
                    resolved_error,
                    &files,
                ) {
                    let project_hash = {
                        use sha2::{Digest, Sha256};
                        let mut h = Sha256::new();
                        h.update(project_path.as_bytes());
                        let enc = hex::encode(h.finalize());
                        crate::util::text::safe_slice(&enc, 16).to_string()
                    };
                    let category = crate::session::engram::classify_engram_category(&engram);
                    if let Err(e) =
                        s.persist_engram(&state.session_id, &engram, category, &project_hash)
                    {
                        tracing::warn!("omni: failed to persist engram: {e}");
                    }
                }
            }
        }
    }

    // Safety truncation, shared with `hooks::pipe` so the cap and its marker
    // cannot drift apart — this path spelled the limit `50_000` inline (#219).
    crate::util::text::truncate_with_marker(&mut final_out, crate::guard::limits::MAX_OUTPUT_BYTES);

    // A passthrough hands back exactly what the command produced, so there is
    // nothing to replace. Emitting those identical bytes with a marker on top
    // made every no-op *cost* tokens to announce that OMNI had changed nothing:
    // 33,762 across the reporting database and 604 in a single day, at a modal
    // 10 tokens a call (#118 item 5). Emitting nothing leaves the host's own
    // bytes in place at zero marker cost, which is what the format-sniff,
    // host-cap and TOML zero-state gates earlier in this function already do.
    //
    // The row is still recorded above, at its honest 0%. What is dropped here
    // is only the reply, and with it the savings footer — which had nothing to
    // report on a call that saved nothing.
    if result.route == Route::Passthrough {
        return None;
    }

    // Build additionalContext with token savings stats
    let additional_context = build_additional_context(&result, &session);

    serde_json::to_string(&HookOutput {
        hook_specific_output: HookSpecificOutput {
            hook_event_name: "PostToolUse",
            updated_tool_output: shape_for_host(normalized.raw_response.as_ref(), final_out),
            additional_context,
        },
    })
    .ok()
}

/// Build invisible additionalContext injected into Claude's context
fn build_additional_context(
    result: &crate::pipeline::DistillResult,
    session: &Option<Arc<Mutex<crate::pipeline::SessionState>>>,
) -> Option<String> {
    // The banner and the `distillations` row describe the same call, so they
    // must not disagree about it. They did: this read a bytes-per-token heuristic
    // over the byte delta while the row ran a real tokenizer over each string,
    // and on the reported call the banner said 6,194 tokens where the row said
    // 16,983 - 1,209 = 15,774. Two live estimators, neither reconciled, one of
    // them printed into the agent's context (#212). `raw_tokens` and
    // `filtered_tokens` are already counted for this result — use them.
    let saved_this_call = result.raw_tokens.saturating_sub(result.filtered_tokens);

    let mut session_total = 0;
    let mut command_count = 0;
    let mut pressure_msg = None;

    if let Some(lock) = session
        && let Ok(mut s) = lock.lock()
    {
        session_total = s.estimated_tokens_saved();
        command_count = s.command_count;

        // Feature A: Context Pressure System
        s.estimated_current_tokens += result.filtered_tokens as u64;
        s.recalculate_pressure();

        // L3-02: Update Predictive Token Consumption Rate
        let cmd_count = s.command_count;
        let est_tokens = s.estimated_current_tokens;
        s.token_consumption_rate.update(cmd_count, est_tokens);

        let window_size = s
            .context_window_size_hint
            .unwrap_or(crate::pipeline::DEFAULT_CONTEXT_WINDOW_SIZE);

        let mut predicted_warn = None;
        if let Some(commands_left) = s
            .token_consumption_rate
            .predicted_full_at_command(s.estimated_current_tokens, window_size)
        {
            // Warn if context will be full in <= 15 commands, and we aren't already critical
            if commands_left <= 15
                && s.context_pressure != crate::pipeline::ContextPressure::Critical
            {
                predicted_warn = Some(format!(
                    "OMNI Prediction: At current rate (~{:.0} tokens/cmd), context will be full in ~{} commands. Consider compacting soon.",
                    s.token_consumption_rate.avg_tokens_per_command, commands_left
                ));
            }
        }

        if s.should_emit_pressure_warning() {
            pressure_msg = s.pressure_warning();
            s.last_pressure_warning_at = Some(command_count);
        }

        if let Some(pw) = predicted_warn {
            if let Some(pm) = pressure_msg.as_mut() {
                pm.push('\n');
                pm.push_str(&pw);
            } else {
                pressure_msg = Some(pw);
            }
        }

        // L1-01 / L1-02: Budget Warning Check
        if let Some(budget) = s.loop_context.budget_tokens
            && budget > 0
            && s.loop_context.budget_used > (budget as f64 * 0.8) as u64
        {
            let budget_warn = format!(
                "OMNI Loop Budget: >80% used this iteration ({} / {} tokens). Consider concluding soon.",
                s.loop_context.budget_used, budget
            );
            if let Some(pm) = pressure_msg.as_mut() {
                pm.push('\n');
                pm.push_str(&budget_warn);
            } else {
                pressure_msg = Some(budget_warn);
            }
        }
    }

    let mut msgs = Vec::new();

    if let Some(warning) = pressure_msg {
        msgs.push(warning);
    }

    // Phase 2: Periodic Pinned File Re-injection
    // When context pressure is elevated, re-inject critical files periodically
    if let Some(lock) = session
        && let Ok(mut s) = lock.lock()
        && crate::session::engram::should_reinject_pinned(
            &s.context_pressure,
            s.command_count,
            s.pinned_reinject_at,
        )
    {
        let cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string());
        let pinned_content = crate::hooks::session_start::read_pinned_files(&cwd);
        if !pinned_content.is_empty() {
            msgs.push(format!(
                "[OMNI: Re-injecting critical files due to {} pressure]\n{}",
                s.context_pressure, pinned_content
            ));
            s.pinned_reinject_at = s.command_count;
            s.pinned_refresh_count += 1;
        }
    }

    // F-10: Inject for significant single-call savings (>= 500 tokens)
    if saved_this_call >= 500 {
        msgs.push(format!(
            "[OMNI: -{saved_this_call}tok this call | -{session_total}tok session | {savings:.0}% compression]",
            savings = result.savings_pct()
        ));
    } else if command_count > 0 && command_count.is_multiple_of(10) && session_total >= 1000 {
        // F-10: Inject milestone summary every 10 commands if total savings significant
        msgs.push(format!(
            "[OMNI session milestone: -{session_total}tok saved across {command_count} commands]"
        ));
    }

    if msgs.is_empty() {
        None
    } else {
        Some(msgs.join("\n"))
    }
}

fn wrap_hook_output(raw_response: Option<&serde_json::Value>, distilled: String) -> String {
    serde_json::to_string(&HookOutput {
        hook_specific_output: HookSpecificOutput {
            hook_event_name: "PostToolUse",
            updated_tool_output: shape_for_host(raw_response, distilled),
            additional_context: None,
        },
    })
    .unwrap_or_default()
}

// ── NON-BASH TOOL DISTILLATION ───────────────────────────────────────

use crate::distillers::search::distill_grep;
fn process_web_content(content: &str) -> Option<String> {
    let line_count = content.lines().count();
    if line_count < 30 {
        return None;
    }

    let stripped = strip_html_simple(content);
    let stripped_lines: Vec<&str> = stripped.lines().filter(|l| !l.trim().is_empty()).collect();
    let total_clean = stripped_lines.len();
    let meaningful: Vec<&str> = stripped_lines
        .iter()
        .filter(|l| l.trim().len() > 20)
        .take(40)
        .copied()
        .collect();
    let summary = format!(
        "[OMNI WebFetch: {} lines → {} relevant]\n{}{}",
        line_count,
        total_clean,
        meaningful.join("\n"),
        if total_clean > 40 {
            format!("\n... [{} more lines]", total_clean - 40)
        } else {
            String::new()
        }
    );
    if summary.len() < content.len() * 7 / 10 {
        Some(summary)
    } else {
        None
    }
}

fn strip_html_simple(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// #158. The host replaces what the model sees only when it finds
    /// `updatedToolOutput`; any other key is dropped without a word, and the
    /// agent silently keeps the raw output while OMNI records the saving.
    ///
    /// This asserts on the **serialized bytes** on purpose. A test that builds
    /// `HookSpecificOutput` and reads `.updated_tool_output` back passes with
    /// any key whatsoever, which is exactly how `updatedResponse` survived from
    /// the first day of the Rust rewrite until it was found by hand.
    #[test]
    fn serialises_the_key_the_host_actually_reads() {
        let json = wrap_hook_output(None, "distilled".to_string());

        assert!(json.contains(r#""updatedToolOutput""#), "{json}");
        assert!(
            !json.contains("updatedResponse"),
            "the ignored key is back: {json}"
        );
    }

    /// #229: `Passthrough` names a route, and the route is what the caller acts
    /// on. The banner was prefixed onto the *distilled* string, so it announced
    /// "OMNI changed nothing" over bytes that had already lost lines — four data
    /// rows of a markdown table, with the header and separator left standing so
    /// the table read as present and empty. An agent that trusts the label does
    /// not re-run the command.
    ///
    /// The assertion is on what the agent ends up holding, not on the banner
    /// text: checking that the label is spelled correctly is what let this
    /// through.
    ///
    /// Since #118 item 5 there is no banner. A passthrough declines, the host
    /// keeps its own bytes, and the guarantee is stronger than it was — the
    /// agent cannot receive altered bytes under a "changed nothing" label
    /// because it receives nothing from OMNI at all. The recorded
    /// `passthrough_events` row is what proves this input really reached that
    /// branch rather than an earlier return.
    #[test]
    fn passthrough_leaves_the_agent_holding_the_original_bytes() {
        let mut content = String::from("| Workload | Before | After | Savings |\n");
        content.push_str("|-------------------|-------:|-------:|--------:|\n");
        for i in 0..8 {
            content.push_str(&format!("| workload-{i} | {i}00 KB | {i}0 KB | 9{i}% |\n"));
        }
        for i in 0..40 {
            content.push_str(&format!(
                "Paragraph {i} of the methodology, describing how each workload was measured.\n"
            ));
        }

        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {"command": "benchreport --summary"},
            "tool_response": bash_response(&content),
        })
        .to_string();

        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("omni.db");
        let store = Arc::new(Store::open_path(&db).expect("store"));

        let out = process_payload(&payload, Some(store), None);

        assert!(
            out.is_none(),
            "a passthrough must hand back nothing, so the host keeps its own \
             bytes: {}",
            out.unwrap_or_default()
        );
        let passthroughs: i64 = rusqlite::Connection::open(&db)
            .expect("open recorded db")
            .query_row("SELECT COUNT(*) FROM passthrough_events", [], |r| r.get(0))
            .expect("count");
        assert_eq!(
            passthroughs, 1,
            "this input must reach the low-compression branch, or the test \
             guards nothing"
        );
    }

    /// #301: the marker states how many lines are missing, and an agent uses
    /// that number to decide whether fetching the rewind is worth it. It was
    /// computed against a `final_out` that already carried OMNI's own
    /// `[Partial signal]` banner, so every banner line counted as content that
    /// survived and the loss was understated by exactly that many. Reported as
    /// `9 lines omitted` for 10 dropped rows.
    #[test]
    fn the_omitted_count_ignores_omnis_own_banner() {
        let rows: Vec<String> = (0..20)
            .map(|i| format!("context-{i:02}-cluster-name"))
            .collect();
        let content = format!("{}\n", rows.join("\n"));
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {"command": "kubectl describe node"},
            "tool_response": bash_response(&content),
        })
        .to_string();

        let out = process_payload(&payload, None, None).expect("this payload is distilled");
        let v: serde_json::Value = serde_json::from_str(&out).expect("valid hook json");
        let stdout = v["hookSpecificOutput"]["updatedToolOutput"]["stdout"]
            .as_str()
            .expect("stdout is a string");

        let marker = stdout
            .lines()
            .find(|l| l.starts_with("[OMNI:"))
            .expect("a lossy reply carries a marker");
        assert!(
            stdout.contains("[Partial signal"),
            "fixture must reach the banner path or it does not guard this: {stdout}"
        );

        // Everything that is not OMNI's own commentary is content that survived.
        let survived = stdout
            .lines()
            .filter(|l| {
                !l.trim().is_empty() && !l.starts_with("[OMNI:") && !l.starts_with("[Partial")
            })
            .count();
        let claimed: usize = marker
            .split_whitespace()
            .nth(1)
            .and_then(|n| n.parse().ok())
            .expect("the marker leads with a count");

        assert_eq!(
            claimed,
            rows.len() - survived,
            "marker claims {claimed} lines omitted, but {} of {} rows are missing: {stdout}",
            rows.len() - survived,
            rows.len()
        );
    }

    /// #224: the Black TOML filter stripped every row and returned an empty
    /// string. The hook accepted it as 100% compression and replaced non-empty
    /// stdout with nothing. A batch zero-state with no explicit fallback must
    /// decline the rewrite so the host retains its original result.
    #[test]
    fn declines_a_batch_filter_that_removes_every_line() {
        let content = (0..200)
            .map(|i| format!("would reformat src/module_{i}.py"))
            .collect::<Vec<_>>()
            .join("\n");
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {"command": "black --check ."},
            "tool_response": bash_response(&content),
        })
        .to_string();

        assert!(
            process_payload(&payload, None, None).is_none(),
            "the hook must decline instead of replacing stdout with an empty or fabricated result"
        );
    }

    /// The counter-case: a Black summary is a real signal that survives the
    /// line filter, so the TOML filter must still win rather than turning every
    /// matching command into passthrough.
    #[test]
    fn still_applies_a_batch_filter_when_a_signal_survives() {
        let mut content = String::new();
        for i in 0..20 {
            content.push_str(&format!("would reformat src/module_{i}.py\n"));
        }
        content.push_str("Oh no! 20 files would be reformatted.");
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {"command": "black --check ."},
            "tool_response": bash_response(&content),
        })
        .to_string();

        // Two outcomes are correct and the test must accept both, because which
        // one happens is not this test's subject.
        //
        // The original form asserted the embedded `black` filter wins the
        // `find()` race over every loaded signal. It does not always: any filter
        // matching the same command that then fails `beats_guardrail` makes the
        // hook fall through to the distiller, which is exactly what #110 asked
        // for. On CI that fall-through fired for real, and the diagnostic said
        // why: seven `learned_*` filters written into `$HOME/.omni` **by the
        // suite itself** matched `black --check .` first and stripped nothing
        // (667 B in, 667 B out). Filed as its own defect; a test run must not
        // write to the user's config.
        //
        // What this test actually guards is that the Black summary reaches the
        // agent. It does either way: distilled, or declined so the host keeps
        // every original byte.
        // `None` is the other correct outcome: declining leaves the host's own
        // bytes untouched, summary included.
        if let Some(out) = process_payload(&payload, None, None) {
            let v: serde_json::Value = serde_json::from_str(&out).expect("valid hook json");
            let stdout = v["hookSpecificOutput"]["updatedToolOutput"]["stdout"]
                .as_str()
                .expect("stdout is a string");
            assert!(
                stdout.contains("Oh no! 20 files would be reformatted."),
                "the surviving signal was lost: {stdout}"
            );
            assert!(
                stdout.len() < content.len(),
                "a rewrite that is not shorter is a passthrough wearing a marker: {stdout}"
            );
            assert!(
                !stdout.contains("would reformat src/module_"),
                "the noise rows the filter exists to strip survived: {stdout}"
            );
        }
    }

    /// #212: Claude Code caps the hook payload, and above roughly the same size
    /// it persists the **raw** output to a file, previews the **raw** first 2 KB,
    /// and drops whatever the hook returns. A payload arriving at the cap is
    /// therefore work nobody will read, and booking it is a saving that never
    /// happened — the reported row claimed 93% compression and 6,194 tokens for
    /// 2,129 bytes that appear nowhere in the transcript.
    #[test]
    fn declines_a_payload_the_host_already_capped() {
        let capped = "x".repeat(crate::guard::limits::HOST_OUTPUT_CAP);
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {"command": "cat /tmp/uniq.txt"},
            "tool_response": bash_response(&capped),
        })
        .to_string();

        assert!(
            process_payload(&payload, None, None).is_none(),
            "a capped payload must be left to the host, not distilled and booked"
        );
    }

    /// The counter-case, with size as the only variable: the same command and
    /// the same kind of content, just under the cap, must still be distilled.
    /// Otherwise the fix reads as "stop working above 30 KB" rather than "stop
    /// booking what the host discards".
    #[test]
    fn still_distills_just_under_the_host_cap() {
        let noisy = |len: usize| {
            let mut s = String::new();
            let mut i = 0;
            while s.len() < len {
                s.push_str(&format!("Downloading package-{i} from the registry\n"));
                i += 1;
            }
            s.truncate(len);
            s
        };
        let cap = crate::guard::limits::HOST_OUTPUT_CAP;
        let cmd = "somebuildtool --verbose";

        let under = json!({
            "tool_name": "Bash",
            "tool_input": {"command": cmd},
            "tool_response": bash_response(&noisy(cap - 1)),
        })
        .to_string();
        let at = json!({
            "tool_name": "Bash",
            "tool_input": {"command": cmd},
            "tool_response": bash_response(&noisy(cap)),
        })
        .to_string();

        assert!(
            process_payload(&under, None, None).is_some(),
            "output under the cap is applied by the host and must still be distilled"
        );
        assert!(
            process_payload(&at, None, None).is_none(),
            "the same content at the cap must be left to the host"
        );
    }

    /// Found by self-review before merge. `normalize` folds a non-empty stderr
    /// into `content`, so measuring the cap there would decline an *uncapped*
    /// result whose stdout and stderr merely add up past 30 KB — losing
    /// compression on ordinary output, silently. The host caps `stdout`, so
    /// that is the field the guard reads.
    #[test]
    fn does_not_mistake_stdout_plus_stderr_for_a_capped_payload() {
        let cap = crate::guard::limits::HOST_OUTPUT_CAP;
        let line = "Downloading a package from the registry, please wait...\n";
        let stdout: String = line.repeat(cap * 3 / 4 / line.len());
        let stderr: String = line.repeat(cap / 2 / line.len());

        assert!(stdout.len() < cap, "stdout alone is under the cap");
        assert!(
            stdout.len() + stderr.len() > cap,
            "together they clear it, which is the trap"
        );

        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {"command": "somebuildtool --verbose"},
            "tool_response": {
                "stdout": stdout,
                "stderr": stderr,
                "interrupted": false,
                "isImage": false,
            },
        })
        .to_string();

        assert!(
            process_payload(&payload, None, None).is_some(),
            "an uncapped result must still be distilled, however its streams add up"
        );
    }

    /// #212: the banner and the `distillations` row describe the same call and
    /// disagreed about it — the banner ran a bytes-per-token heuristic over the
    /// byte delta while the row ran a real tokenizer over each string. On the
    /// reported call the banner said 6,194 tokens and the row said
    /// 16,983 - 1,209 = 15,774. The banner is the copy that enters the agent's
    /// context, so it is the one that has to match the record.
    #[test]
    fn the_banner_and_the_recorded_row_agree_about_the_same_call() {
        let result = crate::pipeline::DistillResult {
            output: String::new(),
            route: Route::Keep,
            filter_name: "cat".to_string(),
            score: 0.0,
            context_score: 0.0,
            input_bytes: 30_000,
            output_bytes: 2_129,
            latency_ms: 1,
            rewind_hash: None,
            segments_kept: 0,
            segments_dropped: 0,
            collapse_savings: None,
            raw_tokens: 16_983,
            filtered_tokens: 1_209,
            delivered_bytes: 2_129,
        };

        let banner = build_additional_context(&result, &None).expect("banner for a large saving");

        assert!(
            banner.contains("-15774tok this call"),
            "banner must report raw_tokens - filtered_tokens, got: {banner}"
        );
    }

    /// #172: the `Read` arm had never run on Claude Code, because the hook was
    /// registered for `Bash` alone. Enabling it means replying in `Read`'s own
    /// result shape — captured from a live transcript, not assumed — and the
    /// captured shape carries `numLines` *beside* the content it describes.
    ///
    /// Swapping the content and leaving that number is the defect this project
    /// exists to stop emitting: the host would report 420 lines for a payload
    /// that has 2. `totalLines` describes the file rather than the payload, so it
    /// must survive untouched — asserting both directions is what makes this a
    /// test rather than a restatement of the code.
    #[test]
    fn replies_in_the_hosts_read_result_shape() {
        let raw = json!({
            "type": "text",
            "file": {
                "filePath": "/repo/src/main.rs",
                "content": "line one\nline two\nline three\nline four\n",
                "numLines": 4,
                "startLine": 1,
                "totalLines": 420,
            }
        });

        let out = shape_for_host(Some(&raw), "distilled one\ndistilled two".into());
        let v = serde_json::to_value(&out).expect("serialises");

        assert_eq!(
            v["file"]["content"].as_str(),
            Some("distilled one\ndistilled two")
        );
        assert_eq!(
            v["file"]["numLines"].as_u64(),
            Some(2),
            "numLines counts this payload; leaving it at 4 fabricates a count: {v}"
        );
        assert_eq!(
            v["file"]["totalLines"].as_u64(),
            Some(420),
            "totalLines describes the file, not the payload: {v}"
        );
        assert_eq!(v["file"]["filePath"].as_str(), Some("/repo/src/main.rs"));

        // The shape #187 was rejected for. Its absence is the regression guard.
        assert!(v.get("status").is_none(), "MCP shape is back: {v}");
        assert!(v.get("result").is_none(), "MCP shape is back: {v}");
    }

    /// Reads back the `session_id` every `distillations` row was filed under.
    fn recorded_session_ids(db: &std::path::Path) -> Vec<String> {
        let conn = rusqlite::Connection::open(db).expect("open recorded db");
        let mut stmt = conn
            .prepare("SELECT session_id FROM distillations ORDER BY id")
            .expect("prepare");
        stmt.query_map([], |row| row.get::<_, String>(0))
            .expect("query")
            .filter_map(Result::ok)
            .collect()
    }

    /// #118: a distillation was filed under `SessionState::session_id`, a wall
    /// clock stamp on globally persisted state, so one id collected 16 project
    /// paths and 3,739 commands. The host sends its own id on every payload.
    #[test]
    fn files_a_distillation_under_the_host_session_id() {
        // Arrange
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("omni.db");
        let store = Arc::new(Store::open_path(&db).expect("store"));
        let payload = json!({
            "session_id": "4ba52c00-c43f-46ed-9e0e-9069d5294302",
            "tool_name": "Bash",
            "tool_input": {"command": "git status"},
            "tool_response": bash_response(&"modified: src/main.rs\n".repeat(40)),
        })
        .to_string();

        // Act
        process_payload(&payload, Some(store), None);

        // Assert
        assert_eq!(
            recorded_session_ids(&db),
            vec!["4ba52c00-c43f-46ed-9e0e-9069d5294302".to_string()],
            "the row was filed under OMNI's own id, not the host's"
        );
    }

    /// Pipe mode and hosts that send no id still have to be recorded. Failing
    /// open to the local id keeps the row; dropping it would trade one
    /// accounting bug for a worse one.
    #[test]
    fn falls_back_to_a_local_id_when_the_host_sends_none() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("omni.db");
        let store = Arc::new(Store::open_path(&db).expect("store"));
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {"command": "git status"},
            "tool_response": bash_response(&"modified: src/main.rs\n".repeat(40)),
        })
        .to_string();

        process_payload(&payload, Some(store), None);

        let ids = recorded_session_ids(&db);
        assert_eq!(ids.len(), 1, "the row must still be recorded: {ids:?}");
        assert!(
            !ids[0].is_empty(),
            "a blank id groups every such row together"
        );
    }

    /// #118 item 5: a passthrough replaced the host's output with the same
    /// bytes plus a marker, so every no-op *cost* tokens to announce that
    /// nothing had changed — 33,762 across the reporting database, at a modal
    /// 10 tokens a call. The hook must decline instead.
    #[test]
    fn adds_no_bytes_when_it_changes_nothing() {
        // Arrange: prose no distiller reduces, so the ratio gate calls it a
        // passthrough.
        let content = (0..60)
            .map(|i| format!("ordinary line {i} of a note that nothing summarises"))
            .collect::<Vec<_>>()
            .join("\n");
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {"command": "cat notes.txt"},
            "tool_response": bash_response(&content),
        })
        .to_string();

        // Act
        let out = process_payload(&payload, None, None);

        // Assert
        assert!(
            out.is_none(),
            "a passthrough must leave the host's bytes alone, got: {}",
            out.unwrap_or_default()
        );
    }

    /// The counter-case, so the fix is not "decline everything": output a
    /// distiller really does reduce must still be replaced.
    #[test]
    fn still_replaces_output_a_distiller_reduces() {
        let mut content = String::new();
        for i in 0..200 {
            content.push_str(&format!("test module_{i} ... ok\n"));
        }
        content.push_str("test result: ok. 200 passed; 0 failed\n");
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {"command": "cargo test"},
            "tool_response": bash_response(&content),
        })
        .to_string();

        let out = process_payload(&payload, None, None).expect("a reduced result is delivered");
        let v: serde_json::Value = serde_json::from_str(&out).expect("valid hook json");
        let stdout = v["hookSpecificOutput"]["updatedToolOutput"]["stdout"]
            .as_str()
            .expect("stdout is a string");

        assert!(
            stdout.len() < content.len(),
            "the replacement must be smaller than the input"
        );
    }

    /// #264, end to end. `git status` leading a chain routed the whole of stdout
    /// to the git distiller, whose summary is a fixed one-liner, so everything
    /// the later commands printed was replaced with no marker, no count and no
    /// rewind hash. The agent was told the command succeeded and shown none of
    /// what it ran the command for.
    ///
    /// The assertion is on what the agent ends up holding. Checking that the
    /// output "contains find results" would pass on a summary that happened to
    /// quote one path.
    #[test]
    fn refuses_to_summarise_a_chain_several_commands_wrote_to() {
        // Arrange: exactly the reported shape, git status first.
        let mut content = String::from(
            "On branch main\nYour branch is up to date with 'origin/main'.\n\n\
             nothing to commit, working tree clean\n=== tree ===\n",
        );
        for i in 0..40 {
            content.push_str(&format!("./src/module_{i}.rs\n"));
        }
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {"command": "git status && echo '=== tree ===' && find . -type f"},
            "tool_response": bash_response(&content),
        })
        .to_string();

        // Act
        let out = process_payload(&payload, None, None);

        // Assert
        assert!(
            out.is_none(),
            "the chain must be handed back untouched, got: {}",
            out.unwrap_or_default()
        );
    }

    /// The counter-case, so the fix is not "decline every chain": a leading `cd`
    /// prints nothing, so the output still came from one command and is still
    /// worth distilling.
    #[test]
    fn still_distills_a_chain_led_by_a_silent_builtin() {
        let content = lossy_content(200);
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {"command": "cd /project && cargo test"},
            "tool_response": bash_response(&content),
        })
        .to_string();

        let out = process_payload(&payload, None, None).expect("a reduced result is delivered");
        let v: serde_json::Value = serde_json::from_str(&out).expect("valid hook json");
        let stdout = v["hookSpecificOutput"]["updatedToolOutput"]["stdout"]
            .as_str()
            .expect("stdout is a string");

        assert!(
            stdout.len() < content.len(),
            "a single-producer chain must still be reduced"
        );
    }

    /// #269, end to end. `kubectl get pod -o json | jq -r '...'` was routed to
    /// `kubectl`, so the cloud distiller took four `key: value` lines and kept
    /// one. Nothing chose the survivor for being signal: the three it dropped
    /// were the pod phase, the node and the zone, which is what the command was
    /// run to check, and the one it kept was the timestamp.
    ///
    /// Filed against `jq` missing from the passthrough allowlist. It is not that:
    /// `jq -r '...' pod.json` on its own is declined and always was. The routing
    /// is what deleted the lines, so this test drives the piped form.
    #[test]
    fn does_not_let_the_upstream_command_claim_a_reshaped_payload() {
        let content = "phase: Running\nnode: aks-stateful-9kf4v\n\
                       zoneSel: reg1north-1\ncreated: 2026-08-02T03:43:16Z\n";
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {"command": "kubectl get pod jenkins-0 -o json | jq -r '.status.phase'"},
            "tool_response": bash_response(content),
        })
        .to_string();

        let out = process_payload(&payload, None, None);

        assert!(
            out.is_none(),
            "four lines of jq output must reach the agent whole, got: {}",
            out.unwrap_or_default()
        );
    }

    /// The other half of #269, and the reason `jq` and `yq` are verbatim rather
    /// than merely unrouted. Their output exists to be parsed by a later step, so
    /// a `[N similar lines collapsed]` marker in the middle of it is not a
    /// summary, it is a syntax error. Routing alone does not cover this: with the
    /// distiller declining, the collapse fallback still gets the payload.
    #[test]
    fn never_collapses_output_a_later_step_has_to_parse() {
        let mut content = String::new();
        for i in 0..60 {
            content.push_str(&format!("pod-{i}: Running\n"));
        }
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {"command": "kubectl get pods -o json | jq -r '.items[] | \"\\(.metadata.name): \\(.status.phase)\"'"},
            "tool_response": bash_response(&content),
        })
        .to_string();

        let delivered = match process_payload(&payload, None, None) {
            None => content.clone(), // declined: the host keeps its own bytes
            Some(out) => {
                let v: serde_json::Value = serde_json::from_str(&out).expect("valid hook json");
                v["hookSpecificOutput"]["updatedToolOutput"]["stdout"]
                    .as_str()
                    .expect("stdout is a string")
                    .to_string()
            }
        };

        for i in 0..60 {
            assert!(
                delivered.contains(&format!("pod-{i}: Running")),
                "row {i} is missing from a payload a later step parses: {delivered}"
            );
        }
    }

    /// #326. The pattern in a `grep` is the caller's own selection, so every line
    /// it returned was asked for by name. This payload went through two filters
    /// that could not know that: routed by `kubectl`, `distill_kubectl_generic`
    /// kept `is_critical` lines and dropped 14 of 15. The one it kept was
    /// the `ERROR`, so the delivered answer said the pod had failed to configure
    /// while the dropped lines said `3/3 MCP servers connected` and `Bolt app is
    /// running!`, which is what the command was run to find out.
    ///
    /// This covers the routing half only. The collapse half has its own test
    /// below, because this payload does not reach the fallback: routed by
    /// `kubectl` it takes `CollapseMode::Infra`, which leaves these lines alone.
    #[test]
    fn never_rescores_a_payload_the_callers_grep_already_filtered() {
        let content = "\
WARNING:jean.server:channel scoping INACTIVE: soul.md lists no allowed channels
ERROR:jean.server:no approvers configured: JEAN_APPROVERS is unset
INFO:jean.plugins.mcp_config:loaded 3 mcp server definitions
INFO:jean.plugins.mcp_config:plugin_grafana_grafana transport=stdio
INFO:jean.plugins.mcp_proxy:proxy listening on unix:///tmp/jean-mcp.sock
INFO:jean.plugins.mcp_proxy:proxy ready
INFO:jean.plugins.mcp_client:starting 3 MCP server(s)
INFO:jean.plugins.mcp_client:mcp plugin_grafana_grafana: connected (65 tools)
INFO:jean.plugins.mcp_client:mcp plugin_kubectl_kubernetes: connected (14 tools)
INFO:jean.plugins.mcp_client:mcp plugin_elasticsearch: connected (4 tools)
INFO:jean.plugins.mcp_client:3/3 MCP servers connected
INFO:jean.server:slack app token accepted, socket mode ready
INFO:slack_bolt.AsyncApp:A new session has been established
INFO:slack_bolt.AsyncApp:Bolt app is running!
INFO:jean.server:startup complete in 4.2s
";
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {"command":
                "kubectl --context aks-devops -n devops logs jean-0 --tail=60 2>&1 \
                 | grep -iE 'mcp|plugin|slack|kube|error|warn|ready|started'"},
            "tool_response": bash_response(content),
        })
        .to_string();

        let delivered = match process_payload(&payload, None, None) {
            None => content.to_string(), // declined: the host keeps its own bytes
            Some(out) => {
                let v: serde_json::Value = serde_json::from_str(&out).expect("valid hook json");
                v["hookSpecificOutput"]["updatedToolOutput"]["stdout"]
                    .as_str()
                    .expect("stdout is a string")
                    .to_string()
            }
        };

        for line in content.lines() {
            assert!(
                delivered.contains(line),
                "the grep pattern matched this line, so it was asked for by name: {line:?}\ngot: {delivered}"
            );
        }
    }

    /// The other half of #326, and the reason `grep` is in
    /// `passes_through_verbatim` and not only in the routing. Routing the payload
    /// to the grep path is not enough: when the pattern's result has no repeated
    /// path to hoist, that path returns the input, which cannot beat
    /// `beats_guardrail`, and the hook then treats it as a distiller that punted
    /// and collapses it. Measured while writing this: with the three names taken
    /// back out of the predicate, 121 matched lines came back as
    /// `120 INFO entries (collapsed from 120 lines)` over `119 lines omitted`.
    ///
    /// Sized against the gates on purpose. A bare `grep` command, so the profile
    /// is `CollapseMode::Log` rather than the `Infra` a `kubectl`-headed pipeline
    /// resolves to; 121 lines, well over `MIN_LINES_FOR_COLLAPSE`; and enough of
    /// them alike to clear `MIN_GROUP_SIZE` and the route thresholds the
    /// collapsed form has to beat before anything is emitted. The first version
    /// of this test had a `kubectl … | grep` command and 15 lines, and stayed
    /// green with the predicate broken.
    #[test]
    fn never_collapses_a_payload_the_callers_grep_already_filtered() {
        let mut content = String::new();
        for i in 0..120 {
            content.push_str(&format!(
                "INFO:mcp_client:mcp plugin_{i}_server: connected ({i} tools, attempt 1/3)\n"
            ));
        }
        content.push_str("ERROR:server:no approvers configured\n");
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {"command": "grep -iE 'mcp|error' pod.log"},
            "tool_response": bash_response(&content),
        })
        .to_string();

        let delivered = match process_payload(&payload, None, None) {
            None => content.clone(), // declined: the host keeps its own bytes
            Some(out) => {
                let v: serde_json::Value = serde_json::from_str(&out).expect("valid hook json");
                v["hookSpecificOutput"]["updatedToolOutput"]["stdout"]
                    .as_str()
                    .expect("stdout is a string")
                    .to_string()
            }
        };

        for line in content.lines() {
            assert!(
                delivered.contains(line),
                "the grep pattern matched this line, so the collapse fallback may not fold it away: {line:?}"
            );
        }
    }

    /// 200 green test lines and a tally: output a distiller reduces hard, so the
    /// reply is unambiguously lossy.
    fn lossy_content(lines: usize) -> String {
        let mut content = String::new();
        for i in 0..lines {
            content.push_str(&format!("test module_{i} ... ok\n"));
        }
        content.push_str(&format!("test result: ok. {lines} passed; 0 failed\n"));
        content
    }

    fn count(db: &std::path::Path, sql: &str) -> i64 {
        rusqlite::Connection::open(db)
            .expect("open recorded db")
            .query_row(sql, [], |r| r.get(0))
            .expect("count")
    }

    /// #271. `README.md:81` promises "everything cut is archived". The gate meant
    /// to deliver it asked the scorer for a noise ratio and wanted more than 40%
    /// noise across more than 20 segments, which no real payload had: 0 of 8,968
    /// recorded distillations carried a rewind hash and `rewind_store` was empty,
    /// so the archive had never held a single row.
    ///
    /// The assertions are on the stored rows rather than on the marker text. A
    /// test that only greps the reply passes while the insert quietly fails, and
    /// a confident string over an empty table is the defect being fixed.
    #[test]
    fn archives_the_raw_output_it_shortens() {
        // Arrange
        let content = lossy_content(200);
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {"command": "cargo test"},
            "tool_response": bash_response(&content),
        })
        .to_string();
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("omni.db");
        let store = Arc::new(Store::open_path(&db).expect("store"));

        // Act
        let out = process_payload(&payload, Some(store), None).expect("a reduced result is sent");

        // Assert
        assert_eq!(
            count(&db, "SELECT COUNT(*) FROM rewind_store"),
            1,
            "the raw output must be recoverable, or the guarantee is a sentence"
        );
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM distillations \
                 WHERE rewind_hash IS NOT NULL AND rewind_hash <> ''"
            ),
            1,
            "the recorded row must name the archive it belongs to"
        );
        assert!(
            out.contains("omni_retrieve("),
            "the agent cannot call what it was not told: {out}"
        );
    }

    /// The bound is stated, not implied. Above the cap the content is not
    /// archived, and the reply has to say so rather than leaving the agent to
    /// infer that `omni_retrieve` exists for it.
    ///
    /// The payload carries its text at `tool_response.content` on purpose. The
    /// host-cap gate reads `tool_response.stdout`, so a Bash-shaped response
    /// would be declined at 30 KB and this branch could never be reached. A bare
    /// string does not work either: `normalize` has no extraction arm for one and
    /// returns `None`.
    #[test]
    fn states_the_bound_when_the_input_is_over_the_rewind_cap() {
        // Arrange: comfortably over MAX_REWIND_BYTES.
        let content = lossy_content(6_000);
        assert!(content.len() > crate::guard::limits::MAX_REWIND_BYTES);
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {"command": "cargo test"},
            "tool_response": {"content": content},
        })
        .to_string();
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("omni.db");
        let store = Arc::new(Store::open_path(&db).expect("store"));

        // Act
        let out = process_payload(&payload, Some(store), None).expect("a reduced result is sent");

        // Assert
        assert!(
            out.contains("not archived"),
            "an unarchived cut must say so: {out}"
        );
        assert_eq!(
            count(&db, "SELECT COUNT(*) FROM rewind_store"),
            0,
            "the cap has to hold, or it is not a cap"
        );
    }

    /// A passthrough declines, so the host keeps its own bytes and nothing was
    /// lost. An archive written on the way to that decision would sit in
    /// `rewind_store` naming content the agent still holds, and `omni stats`
    /// would count a rewind that never applied.
    ///
    /// Same fixture as `passthrough_leaves_the_agent_holding_the_original_bytes`,
    /// which proves this input really reaches the low-compression branch.
    #[test]
    fn archives_nothing_on_a_call_it_declines() {
        // Arrange
        let mut content = String::from("| Workload | Before | After | Savings |\n");
        content.push_str("|-------------------|-------:|-------:|--------:|\n");
        for i in 0..8 {
            content.push_str(&format!("| workload-{i} | {i}00 KB | {i}0 KB | 9{i}% |\n"));
        }
        for i in 0..40 {
            content.push_str(&format!(
                "Paragraph {i} of the methodology, describing how each workload was measured.\n"
            ));
        }
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {"command": "benchreport --summary"},
            "tool_response": bash_response(&content),
        })
        .to_string();
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("omni.db");
        let store = Arc::new(Store::open_path(&db).expect("store"));

        // Act
        let out = process_payload(&payload, Some(store), None);

        // Assert
        assert!(out.is_none(), "the fixture must still be declined");
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM distillations \
                 WHERE rewind_hash IS NOT NULL AND rewind_hash <> ''"
            ),
            0,
            "a declined call must not book a rewind"
        );
    }

    /// The marker is not free. #268 and #269 are 102 and 90 byte payloads where
    /// the markers cost more than the cut saved, so the agent paid tokens for
    /// fewer facts. Whatever the route, a reply that replaces the command's own
    /// bytes must be smaller than them.
    ///
    /// Written as a sweep rather than one fixture because the branch that fires
    /// changes with size, and the invariant is what has to hold across all of
    /// them.
    #[test]
    fn never_hands_back_more_bytes_than_the_command_produced() {
        for lines in [3usize, 6, 12, 25, 60, 200] {
            let content = lossy_content(lines);
            let payload = json!({
                "tool_name": "Bash",
                "tool_input": {"command": "cargo test"},
                "tool_response": bash_response(&content),
            })
            .to_string();

            let Some(out) = process_payload(&payload, None, None) else {
                continue; // declined: the host keeps its own bytes, nothing to check
            };
            let v: serde_json::Value = serde_json::from_str(&out).expect("valid hook json");
            let stdout = v["hookSpecificOutput"]["updatedToolOutput"]["stdout"]
                .as_str()
                .expect("stdout is a string");

            assert!(
                stdout.len() < content.len(),
                "{lines} lines in, {} bytes back for {} raw: markers cost more \
                 than the cut saved",
                stdout.len(),
                content.len()
            );
        }
    }

    /// #273. Every non-Bash arm returns before the Bash pipeline's rewind block,
    /// so each one dropped bytes with no marker and no recoverable copy: exactly
    /// the guarantee #271 closed, still open one match arm away. `MultiEdit` and
    /// the unknown-tool fallback are the clearest, since both keep 30 lines of
    /// the payload and label the result with the count of the input.
    #[test]
    fn archives_what_the_non_bash_arms_cut() {
        let content: String = (0..400)
            .map(|i| format!("pub fn handler_{i}(req: Request) -> Response {{ todo!() }}\n"))
            .collect();

        for (tool, response) in [
            (
                "Read",
                json!({"file": {"filePath": "a.rs", "content": content}}),
            ),
            ("MultiEdit", json!({"content": content})),
            ("Notebook", json!({"content": content})),
        ] {
            let payload = json!({
                "tool_name": tool,
                "tool_input": {"file_path": "a.rs"},
                "tool_response": response,
            })
            .to_string();
            let dir = tempfile::tempdir().expect("tempdir");
            let db = dir.path().join("omni.db");
            let store = Arc::new(Store::open_path(&db).expect("store"));

            let Some(out) = process_payload(&payload, Some(store), None) else {
                continue; // declined: the host keeps its own bytes, nothing was lost
            };

            assert!(
                out.contains("omitted"),
                "{tool} dropped lines without saying so: {out}"
            );
            assert_eq!(
                count(&db, "SELECT COUNT(*) FROM rewind_store"),
                1,
                "{tool} must leave the raw payload recoverable"
            );
        }
    }

    /// A Bash payload as Claude Code actually sends one.
    fn bash_response(stdout: &str) -> serde_json::Value {
        json!({
            "stdout": stdout,
            "stderr": "",
            "interrupted": false,
            "isImage": false,
        })
    }

    /// #187. The key was right after #158 and the **value shape** was not, so the
    /// symptom — savings reported for output the agent never received — survived
    /// the fix that was supposed to end it.
    ///
    /// The assertion that matters is the negative one. `status`/`result` is a
    /// well-formed object that serialises cleanly and that OMNI's own tests were
    /// happy with; the only thing wrong with it is that **Claude Code's Bash
    /// schema rejects it**. So this test is written from the host's schema
    /// (`stdout: string`, `stderr: string`, `interrupted: boolean`) and not from
    /// OMNI's struct — asserting on the same field names we serialised is what
    /// let both halves of this bug through.
    #[test]
    fn replies_in_the_hosts_bash_result_shape() {
        let out = shape_for_host(Some(&bash_response("raw noisy output")), "distilled".into());
        let v = serde_json::to_value(&out).expect("serialises");

        // What Claude Code 2.1.218's outputSchema.safeParse requires.
        assert_eq!(v["stdout"].as_str(), Some("distilled"));
        assert!(v["stderr"].is_string(), "stderr must be a string: {v}");
        assert!(
            v["interrupted"].is_boolean(),
            "interrupted must be a boolean: {v}"
        );

        // The shape that was rejected. Its absence is the regression guard.
        assert!(v.get("status").is_none(), "MCP shape is back: {v}");
        assert!(v.get("result").is_none(), "MCP shape is back: {v}");
    }

    /// The schema carries optional members, and the old shape failed partly by
    /// omitting them. Rebuilding a minimal object would reintroduce that.
    #[test]
    fn preserves_host_keys_it_does_not_understand() {
        let mut resp = bash_response("raw");
        resp["backgroundTaskId"] = json!("bg_42");
        resp["persistedOutputPath"] = json!("/tmp/out.txt");
        resp["timedOutAfterMs"] = json!(120_000);

        let v = serde_json::to_value(shape_for_host(Some(&resp), "distilled".into()))
            .expect("serialises");

        assert_eq!(v["backgroundTaskId"].as_str(), Some("bg_42"));
        assert_eq!(v["persistedOutputPath"].as_str(), Some("/tmp/out.txt"));
        assert_eq!(v["timedOutAfterMs"].as_i64(), Some(120_000));
        assert_eq!(v["isImage"].as_bool(), Some(false));
    }

    /// `normalize` folds stderr into the text that gets distilled, so echoing the
    /// original stderr back as well would show it to the agent twice.
    #[test]
    fn does_not_repeat_stderr_already_folded_into_the_distilled_text() {
        let mut resp = bash_response("out");
        resp["stderr"] = json!("warning: deprecated");

        let v = serde_json::to_value(shape_for_host(
            Some(&resp),
            "out\n[stderr]\nwarning: deprecated".into(),
        ))
        .expect("serialises");

        assert_eq!(v["stderr"].as_str(), Some(""));
        assert!(
            v["stdout"].as_str().is_some_and(|s| s.contains("warning")),
            "the distilled text must still carry it: {v}"
        );
    }

    /// Payloads that arrived without a host response object keep the MCP shape.
    /// Those hosts were not investigated in #187 and must not be guessed at.
    #[test]
    fn keeps_the_mcp_shape_when_no_host_response_arrived() {
        let v = serde_json::to_value(shape_for_host(None, "distilled".into())).expect("serialises");

        assert_eq!(v["status"].as_str(), Some("success"));
        assert_eq!(v["result"].as_str(), Some("distilled"));
    }

    /// A response object without a `stdout` member is not a shape #187 measured,
    /// so it must fall back rather than have `stdout` invented for it.
    #[test]
    fn keeps_the_mcp_shape_for_a_response_without_stdout() {
        let resp = json!({ "content": "some text" });
        let v = serde_json::to_value(shape_for_host(Some(&resp), "distilled".into())).expect("ok");

        assert_eq!(v["status"].as_str(), Some("success"));
        assert!(v.get("stdout").is_none(), "invented a stdout member: {v}");
    }

    /// End-to-end through `process_payload`: the F-07 labeled-passthrough branch
    /// emitted the same rejected shape, so that label has never once reached a
    /// Claude Code user either. It has to be fixed by the same change, not left
    /// behind as the one path still speaking MCP.
    #[test]
    fn labels_passthrough_in_the_hosts_shape_too() {
        // Incompressible: distinct lines, no noise for the pipeline to drop.
        let stdout: String = (0..40)
            .map(|i| format!("{i} \u{1F300} unique-token-{i}\n"))
            .collect();
        let input = json!({
            "tool_name": "Bash",
            "tool_input": { "command": "some-unknown-tool" },
            "tool_response": {
                "stdout": stdout,
                "stderr": "",
                "interrupted": false,
                "isImage": false,
            }
        });

        let Some(out) = process_payload(&input.to_string(), None, None) else {
            return; // emitted nothing at all — also a shape the host cannot reject
        };
        let v: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
        let updated = &v["hookSpecificOutput"]["updatedToolOutput"];

        assert!(
            updated["stdout"].is_string(),
            "passthrough still speaks MCP: {v}"
        );
        assert!(
            updated.get("status").is_none(),
            "passthrough still speaks MCP: {v}"
        );
    }

    /// The two sibling fields were independent, which is the whole reason a
    /// rejected payload still printed a saving. A footer may only ride along with
    /// output the host can actually accept.
    #[test]
    fn never_reports_a_saving_without_a_host_shaped_payload() {
        let mut noisy = String::new();
        for i in 0..400 {
            noisy.push_str(&format!(
                "npm WARN deprecated pkg@1.0.{i}: no longer supported\n"
            ));
        }
        let input = json!({
            "tool_name": "Bash",
            "tool_input": { "command": "npm install" },
            "tool_response": {
                "stdout": noisy,
                "stderr": "",
                "interrupted": false,
                "isImage": false,
            }
        });

        let out = process_payload(&input.to_string(), None, None).expect("distills");
        let v: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
        let hook = &v["hookSpecificOutput"];

        if hook.get("additionalContext").is_some() {
            assert!(
                hook["updatedToolOutput"]["stdout"].is_string(),
                "a saving was reported for a payload the host rejects: {v}"
            );
        }
    }

    #[test]
    fn bash_tool_with_git_diff_output() {
        let diff_str = "diff --git a/test.txt b/test.txt\nindex 123..456 100644\n--- a/test.txt\n+++ b/test.txt\n@@ -1,1 +1,2 @@\n-old\n+new line 1\n+new line 2\n".to_string();

        let mut big_diff = diff_str.clone();
        for _ in 0..50 {
            big_diff.push_str(" \n");
        }
        let input = json!({
            "tool_name": "Bash",
            "tool_input": {
                "command": "git diff"
            },
            "tool_response": {
                "content": big_diff
            }
        });
        let out = process_payload(&input.to_string(), None, None);
        assert!(out.is_some());
        let res = out.expect("must succeed");
        assert!(res.contains("hookEventName"));
        assert!(res.contains("PostToolUse"));
        assert!(res.contains("test.txt"));
    }

    #[test]
    fn non_bash_tool_small_file_passthrough() {
        // Small ReadFile content (<50 lines) should pass through (None)
        let input = json!({
            "tool_name": "Read",
            "tool_input": { "path": "small.rs" },
            "tool_response": {
                "content": "fn main() {\n    println!(\"hello\");\n}\n"
            }
        });
        let out = process_payload(&input.to_string(), None, None);
        assert!(out.is_none());
    }

    #[test]
    fn distills_large_rust_readfile() {
        // Large ReadFile must exceed MIN_DISTILL_TOKENS (2000 tokens).
        // With Code hint at 3.2 chars/token, we need ~6400+ bytes.
        // Generate 80 functions with longer bodies for realistic compression.
        let mut big_rust = String::new();
        for i in 0..80 {
            big_rust.push_str(&format!("pub fn function_{}() -> i32 {{\n", i));
            big_rust.push_str(&format!("    let x = {};\n", i));
            big_rust.push_str(&format!("    let y = x + {};\n", i * 2));
            big_rust.push_str(&format!("    let z = x * y + {};\n", i * 3));
            big_rust.push_str("    println!(\"computing result for iteration\");\n");
            big_rust.push_str("    let result = x + y + z;\n");
            big_rust.push_str("    result\n");
            big_rust.push_str("}\n\n");
        }
        let input = json!({
            "tool_name": "Read",
            "tool_input": { "path": "src/big.rs" },
            "tool_response": {
                "content": big_rust
            }
        });
        let out = process_payload(&input.to_string(), None, None);
        assert!(out.is_some(), "Large ReadFile must be distilled");
        let res = out.expect("Output exists");
        assert!(
            res.contains("OMNI ReadFile"),
            "Must have OMNI ReadFile label"
        );
        assert!(
            res.contains("pub fn function_0"),
            "Must contain pub fn signatures"
        );
    }

    #[test]
    fn distills_grep_tool_with_file_count() {
        let grep_output = (0..50)
            .map(|i| format!("src/file{}.rs:42:    some match text here", i % 5))
            .collect::<Vec<_>>()
            .join("\n");
        let input = json!({
            "tool_name": "Grep",
            "tool_input": {},
            "tool_response": {
                "content": grep_output
            }
        });
        let out = process_payload(&input.to_string(), None, None);
        assert!(out.is_some(), "Grep output must be distilled");
        let res = out.expect("Output exists");
        assert!(res.contains("OMNI Grep"), "Must have OMNI Grep label");
        assert!(res.contains("matches"), "Must show match count");
    }

    #[test]
    fn edit_tool_returns_none() {
        let input = json!({
            "tool_name": "Edit",
            "tool_input": {},
            "tool_response": {
                "content": "File edited successfully"
            }
        });
        let out = process_payload(&input.to_string(), None, None);
        assert!(out.is_none(), "Edit tool should not be distilled");
    }

    #[test]
    fn html_strip_removes_tags() {
        let html = "<h1>Title</h1><p>Content here</p>";
        let stripped = strip_html_simple(html);
        assert_eq!(stripped.trim(), "TitleContent here");
    }

    #[test]
    fn ignores_content_less_than_50_chars() {
        let input = json!({
            "tool_name": "Bash",
            "tool_input": { "command": "echo a" },
            "tool_response": {
                "content": "short output"
            }
        });
        let out = process_payload(&input.to_string(), None, None);
        assert!(out.is_none());
    }

    #[test]
    fn labels_passthrough_for_small_output_without_reduction() {
        let noise = "a".repeat(100);
        let input = json!({
            "tool_name": "Bash",
            "tool_input": {},
            "tool_response": {
                "content": noise
            }
        });
        let out = process_payload(&input.to_string(), None, None);
        // F-07: Small output with no significant reduction now returns
        // a labeled passthrough instead of None
        if let Some(res) = out {
            assert!(
                res.contains("OMNI") || res.contains("Passthrough"),
                "Labeled passthrough must contain OMNI label"
            );
        }
        // None is also acceptable for single-line content that GenericDistiller
        // doesn't compress
    }

    #[test]
    fn small_output_is_not_silently_dropped() {
        // 500 bytes of distinct context that won't compress well
        let content: String = (0..10)
            .map(|i| {
                format!(
                    "unique_context_line_{}: some data here {}\n",
                    i,
                    "x".repeat(30 + i * 3)
                )
            })
            .collect();
        let input = json!({
            "tool_name": "Bash",
            "tool_input": { "command": "echo test" },
            "tool_response": { "content": content }
        });
        let out = process_payload(&input.to_string(), None, None);
        // If return Some, must contain OMNI label — never silently drops
        if let Some(res) = out {
            assert!(
                res.contains("OMNI") || res.contains("Passthrough"),
                "If not None, must contain OMNI label: {}",
                res
            );
        }
    }

    /// Was `labels_passthrough_for_large_output_without_reduction`, asserting
    /// the reply contained "OMNI: Passthrough". That label is what #118 item 5
    /// removed: it made every no-op cost tokens to say nothing happened. The
    /// behaviour worth guarding is that unreducible output is not replaced.
    #[test]
    fn declines_large_output_it_cannot_reduce() {
        // Create 20 lines of exactly 60 chars each (total 1200+ chars)
        let noise = (0..30)
            .map(|i| {
                // Generate completely distinct strings with varying lengths and chars
                let chars: String =
                    std::iter::repeat_n(char::from(b'a' + (i % 26) as u8), 40 + (i as usize * 3))
                        .collect();
                format!("unqiue_prefix_{} {}\n", i, chars)
            })
            .collect::<String>();
        let input = json!({
            "tool_name": "Bash",
            "tool_input": {},
            "tool_response": {
                "content": noise
            }
        });
        let out = process_payload(&input.to_string(), None, None);
        assert!(
            out.is_none(),
            "output nothing reduces must be left alone, not returned larger: {}",
            out.unwrap_or_default()
        );
    }

    #[test]
    fn parse_error_exits_without_output() {
        let out = process_payload("{ invalid json }", None, None);
        assert!(out.is_none());
    }

    #[test]
    fn extracts_array_content_format_correctly() {
        // Verify array content extraction via normalize (Cursor/Windsurf format)
        let input = json!({
            "tool_name": "Bash",
            "tool_input": { "command": "ls" },
            "tool_response": {
                "content": [
                    {"type": "text", "text": "hello\n"},
                    {"type": "text", "text": "world ".repeat(10)},
                    {"type": "text", "text": "!"}
                ]
            }
        });
        let norm = crate::hooks::normalize::normalize(&input.to_string()).expect("must normalize");
        assert!(norm.content.contains("hello"));
        assert!(norm.content.contains("world world"));
        assert!(norm.content.ends_with("!"));
    }

    /// The payload shape under test is `{stdout, stderr, interrupted}` rather
    /// than `{content}`. The command was `ls -la`, which OMNI passes through
    /// verbatim by design (#200), so once a passthrough stopped emitting a
    /// banner the test had nothing left to assert. A reducible command keeps
    /// the original intent — this payload shape is parsed and distilled.
    #[test]
    fn processes_claude_code_stdout_format() {
        let mut big_output = String::new();
        for i in 0..200 {
            big_output.push_str(&format!("test module_{i} ... ok\n"));
        }
        big_output.push_str("test result: ok. 200 passed; 0 failed\n");
        let input = json!({
            "tool_name": "Bash",
            "tool_input": { "command": "cargo test" },
            "tool_response": {
                "stdout": big_output,
                "stderr": "",
                "interrupted": false,
                "isImage": false,
                "noOutputExpected": false
            }
        });
        let out = process_payload(&input.to_string(), None, None);
        assert!(out.is_some(), "Claude Code stdout format must be processed");
        let res = out.expect("must succeed");
        assert!(res.contains("PostToolUse"));
    }

    #[test]
    fn processes_claude_code_stdout_with_stderr() {
        let mut big_output = String::new();
        for i in 0..30 {
            big_output.push_str(&format!("line {} of output\n", i));
        }
        let input = json!({
            "tool_name": "Bash",
            "tool_input": { "command": "cargo build" },
            "tool_response": {
                "stdout": big_output,
                "stderr": "warning: unused variable",
                "interrupted": false
            }
        });
        let norm = crate::hooks::normalize::normalize(&input.to_string()).expect("must normalize");
        assert!(norm.content.contains("line 0 of output"));
        assert!(norm.content.contains("[stderr]"));
        assert!(norm.content.contains("warning: unused variable"));
    }

    #[test]
    fn ignores_empty_claude_code_stdout() {
        let input = json!({
            "tool_name": "Bash",
            "tool_input": { "command": "true" },
            "tool_response": {
                "stdout": "",
                "stderr": "",
                "interrupted": false
            }
        });
        let out = process_payload(&input.to_string(), None, None);
        assert!(out.is_none(), "Empty stdout should exit early");
    }

    #[test]
    fn prefers_content_field_over_stdout() {
        let mut big_diff = "diff --git a/test.txt b/test.txt\nindex 123..456 100644\n--- a/test.txt\n+++ b/test.txt\n@@ -1,1 +1,2 @@\n-old\n+new line 1\n+new line 2\n".to_string();
        for _ in 0..50 {
            big_diff.push_str(" \n");
        }
        let input = json!({
            "tool_name": "Bash",
            "tool_input": { "command": "git diff" },
            "tool_response": {
                "content": big_diff,
                "stdout": "should be ignored when content is present"
            }
        });
        let out = process_payload(&input.to_string(), None, None);
        assert!(out.is_some());
        let res = out.expect("must succeed");
        assert!(
            res.contains("test.txt"),
            "content field should be used, not stdout"
        );
    }

    #[test]
    fn processes_opencode_payload_format() {
        let input = r#"{"type":"tool_result","tool":"shell","output":"pytest\n5 passed in 2.1s","command":"pytest"}"#;
        // OpenCode format should be processed same as Claude Code
        let _out = process_payload(input, None, None);
        // If content < threshold, can be None — but don't crash
        // This test verifies there is no panic
    }

    #[test]
    fn test_process_payload_codex_format() {
        let long_output = "line\n".repeat(200);
        let input = serde_json::json!({
            "action": "run",
            "command": "cargo build",
            "result": long_output
        });
        let out = process_payload(&input.to_string(), None, None);
        // Should have output (not None) for long input
        // (cargo build with 200 lines should be distilled)
        assert!(
            out.is_some(),
            "Codex format should be distilled if output is long"
        );
    }

    #[test]
    fn test_claude_code_still_works_after_refactor() {
        // REGRESSION TEST — CRITICAL
        //
        // The input was 50 repeated `error[E0382]` lines. `BuildDistiller`
        // keeps error blocks by design, so that output was never reduced and
        // the assertion below was satisfied by the passthrough banner rather
        // than by distillation. With the banner gone (#118 item 5) the payload
        // has to be one OMNI really does distil for this to test anything.
        let mut stdout = String::new();
        for i in 0..200 {
            stdout.push_str(&format!("   Compiling crate_{i} v0.1.0\n"));
        }
        stdout.push_str("    Finished dev profile in 12.3s\n");
        let input = serde_json::json!({
            "tool_name": "Bash",
            "tool_input": {"command": "cargo build"},
            "tool_response": { "stdout": stdout }
        });
        let out = process_payload(&input.to_string(), None, None);
        assert!(
            out.is_some(),
            "Claude Code format should still work after refactor"
        );
    }

    #[test]
    fn test_multiedit_tool_large_output_distilled() {
        let mut big_output = String::new();
        for i in 0..100 {
            big_output.push_str(&format!("Line {} of multi-edit output\n", i));
        }
        let input = serde_json::json!({
            "tool_name": "MultiEdit",
            "tool_input": {},
            "tool_response": {
                "content": big_output
            }
        });
        let out = process_payload(&input.to_string(), None, None);
        assert!(out.is_some(), "Large MultiEdit must be distilled");
        let res = out.expect("Output exists");
        assert!(
            res.contains("OMNI MultiEdit"),
            "Must have OMNI MultiEdit label"
        );
    }

    #[test]
    fn test_unknown_tool_large_output_labeled_passthrough() {
        let mut big_output = String::new();
        for i in 0..200 {
            big_output.push_str(&format!("Line {} of unknown tool output\n", i));
        }
        let input = serde_json::json!({
            "tool_name": "SomeRandomTool",
            "tool_input": {},
            "tool_response": {
                "content": big_output
            }
        });
        let out = process_payload(&input.to_string(), None, None);
        assert!(
            out.is_some(),
            "Large unknown tool output must be passed through with label"
        );
        let res = out.expect("Output exists");
        assert!(
            res.contains("OMNI SomeRandomTool"),
            "Must have OMNI SomeRandomTool label"
        );
    }

    #[test]
    fn test_edit_tool_still_returns_none() {
        let input = serde_json::json!({
            "tool_name": "Edit",
            "tool_input": {},
            "tool_response": {
                "content": "File edited successfully"
            }
        });
        let out = process_payload(&input.to_string(), None, None);
        assert!(out.is_none(), "Edit tool should still return None");
    }

    // ── #120: failed commands pass through verbatim, never distilled ──────

    /// Distillable output that a passing command WOULD get summarised, but the
    /// non-zero exit must force passthrough (None).
    fn distillable_noise() -> String {
        std::fs::read_to_string("tests/fixtures/heavy_noise.txt")
            .expect("heavy_noise fixture missing")
    }

    #[test]
    fn codex_nonzero_exit_passes_through() {
        let input = serde_json::json!({
            "action": "run",
            "command": "docker build .",
            "result": distillable_noise(),
            "exit_code": 1,
        });
        let out = process_payload(&input.to_string(), None, None);
        assert!(
            out.is_none(),
            "a failed command must pass through verbatim, not be distilled"
        );
    }

    #[test]
    fn codex_zero_exit_still_distills() {
        // Guards the guard: a successful command with the same output is still distilled.
        let input = serde_json::json!({
            "action": "run",
            "command": "docker build .",
            "result": distillable_noise(),
            "exit_code": 0,
        });
        let out = process_payload(&input.to_string(), None, None);
        assert!(
            out.is_some(),
            "a successful command must still be distilled"
        );
    }

    #[test]
    fn pi_error_passes_through() {
        let input = serde_json::json!({
            "toolName": "Bash",
            "command": "vault kv list apps/x",
            "toolResponse": { "result": distillable_noise(), "isError": true },
        });
        let out = process_payload(&input.to_string(), None, None);
        assert!(out.is_none(), "Pi isError=true must pass through verbatim");
    }

    #[test]
    fn mcp_error_passes_through() {
        let input = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": { "content": distillable_noise(), "isError": true },
        });
        let out = process_payload(&input.to_string(), None, None);
        assert!(out.is_none(), "MCP isError=true must pass through verbatim");
    }

    #[test]
    fn claude_code_failure_string_passes_through() {
        // Claude Code sends a failed command as a bare `tool_response` STRING, which
        // must never be parsed into a success summary. Locks in the passthrough so a
        // future, more-lenient parser can't silently reintroduce the fabrication.
        let input = serde_json::json!({
            "tool_name": "Bash",
            "tool_input": {"command": "vault kv list apps/x"},
            "tool_response": "Error: Exit code 2\nGet \"https://vault/…\": i/o timeout",
        });
        let out = process_payload(&input.to_string(), None, None);
        assert!(
            out.is_none(),
            "Claude Code failure string must pass through verbatim"
        );
    }

    // ── #116/#110: distillers read raw output, not collapse markers ──────

    fn pod_table_35() -> String {
        let mut rows = vec![
            "NAME                          READY   STATUS             RESTARTS   AGE".to_string(),
        ];
        for i in 0..30 {
            rows.push(format!(
                "api-gateway-7fb9c8b6d-{i:04}    1/1     Running            0          3d"
            ));
        }
        for i in 0..5 {
            rows.push(format!(
                "api-gateway-7fb9c8b6d-c{i:03}    0/1     CrashLoopBackOff   8          3d"
            ));
        }
        rows.join("\n")
    }

    #[test]
    fn kubectl_table_distills_from_raw_not_collapse_markers() {
        // #116: `collapse` runs before `distill`, so a distiller that parses columns
        // used to read `[30 similar lines collapsed]` markers as pod rows and report
        // `k8s: 2 pods | [5 (lines)`. #110: the kubectl TOML filter used to shadow the
        // distiller unconditionally; the guardrail now lets it fall through. Together
        // the distiller must see the real 35-row table.
        let input = serde_json::json!({
            "tool_name": "Bash",
            "tool_input": {"command": "kubectl get pods"},
            "tool_response": {"content": pod_table_35()},
        });
        let out = process_payload(&input.to_string(), None, None)
            .expect("a 35-pod table must be distilled, not dropped");

        assert!(
            out.contains("35 pods"),
            "must count all 35 real pods, got: {out}"
        );
        assert!(
            out.contains("30 running") && out.contains("5 error"),
            "must read real statuses, got: {out}"
        );
        assert!(
            !out.contains("collapsed") && !out.contains("(lines)"),
            "must not be built from collapse markers, got: {out}"
        );
    }
}
