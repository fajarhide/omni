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

    // Route based on tool_name: handle non-Bash tools with specialized distillation
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
            // Phase 6: check graph for many dependents
            let graph = std::env::current_dir()
                .ok()
                .and_then(|cwd| crate::graph::indexer::build_graph(&cwd).ok());

            if let Some(g) = graph {
                let imported_by_count = g.context_for(filepath).imported_by.len();
                return crate::distillers::readfile::distill_readfile_with_context(
                    &content,
                    filepath,
                    imported_by_count,
                )
                .map(|d| wrap_hook_output(normalized.raw_response.as_ref(), d));
            }

            // Fallback if graph fails
            return crate::distillers::readfile::distill_readfile(&content, filepath)
                .map(|d| wrap_hook_output(normalized.raw_response.as_ref(), d));
        }
        "Grep" => {
            if !agent_config.grep_enabled() {
                return None;
            }
            return distill_grep(&content)
                .map(|d| wrap_hook_output(normalized.raw_response.as_ref(), d));
        }
        "WebFetch" => {
            if !agent_config.webfetch_enabled() {
                return None;
            }
            return process_web_content(&content)
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
                return Some(wrap_hook_output(normalized.raw_response.as_ref(), summary));
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
                return Some(wrap_hook_output(normalized.raw_response.as_ref(), summary));
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

    // TOML-first: try matching command against TOML filters
    let toml_filters = toml_filter::load_all_filters();
    let toml_match = if clean_command.is_empty() {
        None
    } else {
        toml_filters.iter().find(|f| f.matches(clean_command))
    };

    // A TOML filter only gets to short-circuit the distiller if it actually beat
    // the guardrail — the same rule `hooks::pipe` already applies. Without it the
    // broad `signals/domains/*.toml` filters win the alphabetical `find()` race
    // for cargo, npm, docker, kubectl and terraform while stripping only a few
    // lines, shadowing the distiller that would have summarised the same input
    // (#110). Weak filter, fall through; a filter that earns its match still wins.
    let toml_hit = toml_match.and_then(|f| {
        let out = f.apply(&content);
        crate::guard::limits::beats_guardrail(out.len(), content.len())
            .then(|| (out, f.name.clone()))
    });

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
        let output = if !crate::guard::limits::beats_guardrail(distilled.len(), content.len()) {
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

        (
            output,
            clean_command
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

    // Re-check segments from content for metadata/learning
    let profile = crate::pipeline::registry::resolve_profile(clean_command);
    let check_segments =
        scorer::score_segments(&content, profile.segmentation, None, clean_command);

    let noise_count = check_segments
        .iter()
        .filter(|s| s.final_score() < 0.3)
        .count();
    let should_store =
        noise_count as f32 / check_segments.len().max(1) as f32 > 0.4 && check_segments.len() > 20;

    let dropped_lines: usize = check_segments
        .iter()
        .filter(|s| s.final_score() < 0.3)
        .map(|s| s.content.lines().count())
        .sum();

    // Auto-learn trigger
    if !clean_command.is_empty() && content.len() > 100 {
        let total = check_segments.len();
        let dropped = noise_count;
        let poor = total > 5 && (dropped as f32 / total.max(1) as f32) < 0.3;
        if poor {
            crate::session::learn::queue_for_learn(&content, clean_command);
        }
    }

    if should_store {
        if let Some(ref s) = store {
            let hash = s.store_rewind(&content);
            final_out.push_str(&format!(
                "\n[OMNI: {} lines omitted — omni_retrieve(\"{}\") for full output]\n",
                dropped_lines, hash
            ));
            rewind_hash = hash;
        } else {
            // Phase 6: factual guard — heavy compression but no rewind store available
            final_out.push_str(&format!(
                "\n[OMNI: {} lines omitted — WARNING: full output not saved (no store), recovery impossible]\n",
                dropped_lines
            ));
        }
    } else {
        // Phase 6: heavy noise detected but not stored — warn if compression is significant
        let noise_ratio = if !check_segments.is_empty() {
            noise_count as f32 / check_segments.len() as f32
        } else {
            0.0
        };
        if noise_ratio > 0.6 && content.len() > 2000 {
            final_out.push_str(&format!(
                "\n[OMNI Guard: {:.0}% noise dropped, but full output not archived — recovery unavailable]\n",
                noise_ratio * 100.0
            ));
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

    let route = if !rewind_hash.is_empty() {
        Route::Rewind
    } else if ratio >= keep_threshold {
        Route::Keep
    } else if ratio >= soft_threshold {
        Route::Soft
    } else {
        Route::Passthrough
    };

    if route == Route::Soft {
        final_out.push_str("\n[Partial signal - omni learn recommended]\n");
    }

    // Measure ratio strictly
    if final_out.len() >= content.len() * 9 / 10 {
        // Record passthrough metric regardless of size
        if let Some(ref s) = store {
            s.record_passthrough(clean_command, content.len());
        }

        if final_out.len() < 1000 {
            // F-07: Label small passthrough output instead of silent drop
            return Some(wrap_hook_output(
                normalized.raw_response.as_ref(),
                format!(
                    "[OMNI: Passthrough — output too small for meaningful compression ({} bytes)]\n{}",
                    content.len(),
                    final_out
                ),
            ));
        } else {
            final_out.insert_str(0, "[OMNI: Passthrough (low compression)]\n");
        }
    }

    let latency_ms = start.elapsed().as_millis() as u32;

    let kept = check_segments.len() - noise_count;
    let raw_tokens = crate::util::token_estimate::count_tokens(&content, "cl100k_base");
    let filtered_tokens = crate::util::token_estimate::count_tokens(&final_out, "cl100k_base");

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
    };

    if let Some(ref s) = store {
        let session_id = session
            .as_ref()
            .and_then(|lock| lock.lock().ok())
            .map(|s| s.session_id.clone())
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

                s.record_context_turn(&state.current_turn);
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

    // Safety Truncation
    let max_chars = 50_000;
    if final_out.len() > max_chars {
        crate::util::text::safe_truncate(&mut final_out, max_chars);
        final_out.push_str("\n[OMNI: output truncated]");
    }

    // Build additionalContext with token savings stats
    let additional_context =
        build_additional_context(&result, &session, &normalized.tool_name, &command);

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
    tool_name: &str,
    command: &str,
) -> Option<String> {
    let saved_this_call = if result.input_bytes > result.output_bytes {
        let hint = crate::util::token_estimate::detect_content_hint(tool_name, command);
        crate::util::token_estimate::estimate_tokens(result.input_bytes - result.output_bytes, hint)
    } else {
        0
    };

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

    #[test]
    fn labels_passthrough_for_large_output_without_reduction() {
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
        assert!(out.is_some());
        let res = out.expect("Output exists");
        println!("PASSTHROUGH RES: {}", res);
        assert!(res.contains("OMNI: Passthrough"));
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

    #[test]
    fn processes_claude_code_stdout_format() {
        let mut big_output =
            "total 42\ndrwxr-xr-x  15 user  staff  480 Apr 10 10:00 .\n".to_string();
        for i in 0..80 {
            big_output.push_str(&format!(
                "-rw-r--r--   1 user  staff  {} Apr 10 10:00 file{}.rs\n",
                i * 100,
                i
            ));
        }
        let input = json!({
            "tool_name": "Bash",
            "tool_input": { "command": "ls -la" },
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
        let input = serde_json::json!({
            "tool_name": "Bash",
            "tool_input": {"command": "cargo build"},
            "tool_response": {
                "stdout": "error[E0382]: borrow of moved value\n  --> src/main.rs:47\n".repeat(50)
            }
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
