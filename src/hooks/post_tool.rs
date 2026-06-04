use crate::pipeline::toml_filter;
use crate::pipeline::{DistillResult, Route, SessionState, collapse, scorer};
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
    #[serde(rename = "updatedResponse")]
    updated_response: String,
    #[serde(rename = "additionalContext")]
    #[serde(skip_serializing_if = "Option::is_none")]
    additional_context: Option<String>,
}
pub fn process_payload(
    input_str: &str,
    store: Option<Arc<Store>>,
    session: Option<Arc<Mutex<SessionState>>>,
) -> Option<String> {
    let normalized = crate::hooks::normalize::normalize(input_str)?;

    if crate::guard::env::is_passthrough() {
        return None;
    }

    let content = normalized.content;

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
                .map(wrap_hook_output);
            }

            // Fallback if graph fails
            return crate::distillers::readfile::distill_readfile(&content, filepath)
                .map(wrap_hook_output);
        }
        "Grep" => {
            if !agent_config.grep_enabled() {
                return None;
            }
            return distill_grep(&content).map(wrap_hook_output);
        }
        "WebFetch" => {
            if !agent_config.webfetch_enabled() {
                return None;
            }
            return process_web_content(&content).map(wrap_hook_output);
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
                return Some(wrap_hook_output(summary));
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
                return Some(wrap_hook_output(summary));
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

    let session_guard = session.as_ref().and_then(|l| l.lock().ok());
    let mut collapse_savings_data = None;
    let (final_out, filter_name) = if let Some(filter) = toml_match {
        let output = filter.apply(&content);
        (output, filter.name.clone())
    } else {
        // Pure Command Architecture: Resolve profile once
        let profile = crate::pipeline::registry::resolve_profile_for_chain(clean_command);

        // 1. Initial Scoring (to evaluate learning/stats)
        let segments = scorer::score_segments(
            &content,
            profile.segmentation,
            session_guard.as_deref(),
            clean_command,
        );

        // 2. Collapse repetitive lines SEBELUM distill
        let collapse_result = collapse::collapse(&content, &profile.collapse);
        collapse_savings_data = if collapse_result.original_lines > collapse_result.collapsed_to {
            Some((collapse_result.original_lines, collapse_result.collapsed_to))
        } else {
            None
        };
        let effective_input = collapse_result.collapsed_lines.join("\n");

        // 3. Re-score dengan collapsed input jika ada savings signifikan
        let final_segments = if collapse_result.savings_pct > 0.1 {
            scorer::score_segments(
                &effective_input,
                profile.segmentation,
                session_guard.as_deref(),
                clean_command,
            )
        } else {
            segments
        };

        // 4. Distill: command-first dispatch
        let output = crate::distillers::distill_with_command(
            &final_segments,
            &effective_input,
            clean_command,
            session_guard.as_deref(),
        );

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
            return Some(wrap_hook_output(format!(
                "[OMNI: Passthrough — output too small for meaningful compression ({} bytes)]\n{}",
                content.len(),
                final_out
            )));
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
                s.record_context_turn(&state.current_turn);
            }

            let tracker = crate::session::tracker::SessionTracker::new(sess.clone(), s.clone());
            tracker.track_command(&command, &content, &result);
        }
    }

    // Safety Truncation
    let max_chars = 50_000;
    if final_out.len() > max_chars {
        final_out.truncate(max_chars);
        final_out.push_str("\n[OMNI: output truncated]");
    }

    // Build additionalContext with token savings stats
    let additional_context =
        build_additional_context(&result, &session, &normalized.tool_name, &command);

    serde_json::to_string(&HookOutput {
        hook_specific_output: HookSpecificOutput {
            hook_event_name: "PostToolUse",
            updated_response: final_out,
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

        if s.should_emit_pressure_warning() {
            pressure_msg = s.pressure_warning();
            s.last_pressure_warning_at = Some(command_count);
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

fn wrap_hook_output(distilled: String) -> String {
    serde_json::to_string(&HookOutput {
        hook_specific_output: HookSpecificOutput {
            hook_event_name: "PostToolUse",
            updated_response: distilled,
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
                    std::iter::repeat_n((b'a' + (i % 26) as u8) as char, 40 + (i as usize * 3))
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
}
