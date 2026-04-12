use crate::pipeline::toml_filter;
use crate::pipeline::{DistillResult, Route, SessionState, collapse, scorer};
use crate::store::sqlite::Store;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Deserialize)]
struct HookInput {
    tool_name: String,
    tool_input: Option<ToolInput>,
    tool_response: Option<ToolResponse>,
}

#[derive(Deserialize)]
struct ToolInput {
    command: Option<String>,
}

#[derive(Deserialize)]
struct ToolResponse {
    content: Option<serde_json::Value>,
    stdout: Option<String>,
    stderr: Option<String>,
}

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
}

fn extract_content(value: &serde_json::Value) -> Option<String> {
    if let Some(s) = value.as_str() {
        return Some(s.to_string());
    }
    if let Some(arr) = value.as_array() {
        let mut out = String::new();
        for item in arr {
            if let Some(obj) = item.as_object()
                && let Some(t) = obj.get("type")
                && t == "text"
                && let Some(text) = obj.get("text")
                && let Some(s) = text.as_str()
            {
                out.push_str(s);
                out.push('\n');
            }
        }
        if out.is_empty() {
            return None;
        }
        return Some(out.trim_end().to_string());
    }
    None
}

/// Extracts content from tool_response, trying `content` first (Cursor, Windsurf)
/// then falling back to `stdout`/`stderr` (Claude Code format).
fn extract_tool_content(input: &HookInput) -> Option<String> {
    let response = input.tool_response.as_ref()?;

    // Try structured `content` field first
    if let Some(ref val) = response.content
        && let Some(s) = extract_content(val)
    {
        return Some(s);
    }

    // Fall back to stdout/stderr (Claude Code format)
    if let Some(ref stdout) = response.stdout
        && !stdout.is_empty()
    {
        let mut result = stdout.clone();
        if let Some(ref stderr) = response.stderr
            && !stderr.is_empty()
        {
            result.push_str("\n[stderr]\n");
            result.push_str(stderr);
        }
        return Some(result);
    }

    None
}

pub fn process_payload(
    input_str: &str,
    store: Option<Arc<Store>>,
    session: Option<Arc<Mutex<SessionState>>>,
) -> Option<String> {
    let parsed: HookInput = match serde_json::from_str(input_str) {
        Ok(p) => p,
        Err(_) => {
            eprintln!("[omni] parse error");
            return None;
        }
    };

    if parsed.tool_name != "Bash" {
        return None;
    }

    let content = extract_tool_content(&parsed)?;

    if content.len() < 50 {
        return None;
    }

    let command = parsed
        .tool_input
        .as_ref()
        .and_then(|i| i.command.clone())
        .unwrap_or_default();

    let clean_command = if let Some(stripped) = command.strip_prefix("omni exec ") {
        stripped
    } else {
        &command
    };

    let start = Instant::now();

    // TOML-first: try matching command against TOML filters
    let toml_filters = toml_filter::load_all_filters();
    let toml_match = toml_filters.iter().find(|f| f.matches(clean_command));

    let session_guard = session.as_ref().and_then(|l| l.lock().ok());
    let mut collapse_savings_data = None;
    let (final_out, filter_name) = if let Some(filter) = toml_match {
        let output = filter.apply(&content);
        (output, filter.name.clone())
    } else {
        // Pure Command Architecture: Resolve profile once
        let profile = crate::pipeline::registry::resolve_profile(clean_command);

        // 1. Initial Scoring (to evaluate learning/stats)
        let segments =
            scorer::score_segments(&content, profile.segmentation, session_guard.as_deref());

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
    let check_segments = scorer::score_segments(&content, profile.segmentation, None);

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
            final_out.push_str(&format!("\n[OMNI: {} lines omitted]\n", dropped_lines));
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

    // Measure ratio strictly
    if final_out.len() >= content.len() * 9 / 10 {
        return None;
    }

    let latency_ms = start.elapsed().as_millis() as u32;

    if let Some(ref s) = store {
        let kept = check_segments.len() - noise_count;
        let result = DistillResult {
            output: final_out.clone(),
            route: if rewind_hash.is_empty() {
                Route::Keep
            } else {
                Route::Rewind
            },
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
        };
        let session_id = session
            .as_ref()
            .and_then(|lock| lock.lock().ok())
            .map(|s| s.session_id.clone())
            .unwrap_or_else(|| "unknown".to_string());
        s.record_distillation(&session_id, &result, &command);

        if let Some(ref sess) = session {
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

    serde_json::to_string(&HookOutput {
        hook_specific_output: HookSpecificOutput {
            hook_event_name: "PostToolUse",
            updated_response: final_out,
        },
    })
    .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_bash_tool_dengan_git_diff_output() {
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
    fn test_non_bash_tool_exit_tanpa_output() {
        let input = json!({
            "tool_name": "ReadFile",
            "tool_input": {},
            "tool_response": {
                "content": "a".repeat(100)
            }
        });
        let out = process_payload(&input.to_string(), None, None);
        assert!(out.is_none());
    }

    #[test]
    fn test_content_less_than_50_chars() {
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
    fn test_no_significant_reduction_exit() {
        let noise = "a".repeat(100);
        let input = json!({
            "tool_name": "Bash",
            "tool_input": {},
            "tool_response": {
                "content": noise
            }
        });
        let out = process_payload(&input.to_string(), None, None);
        // GenericDistiller limits to 100 lines.
        // Noise is a single line, so generic prints exactly the same thing.
        // Therefore length > 90% and exits without distillation!
        assert!(out.is_none());
    }

    #[test]
    fn test_parse_error_exit_tanpa_output() {
        let out = process_payload("{ invalid json }", None, None);
        assert!(out.is_none());
    }

    #[test]
    fn test_array_content_format_extracted_correctly() {
        let arr = json!([
            {"type": "text", "text": "hello\n"},
            {"type": "text", "text": "world ".repeat(10)},
            {"type": "text", "text": "!"}
        ]);
        let extracted = extract_content(&arr).expect("must succeed");
        assert!(extracted.contains("hello"));
        assert!(extracted.contains("world world"));
        assert!(extracted.ends_with("!"));
    }

    #[test]
    fn test_claude_code_stdout_format() {
        let mut big_output =
            "total 42\ndrwxr-xr-x  15 user  staff  480 Apr 10 10:00 .\n".to_string();
        for i in 0..30 {
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
    fn test_claude_code_stdout_with_stderr() {
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
        let parsed: HookInput = serde_json::from_value(input).expect("must parse");
        let content = extract_tool_content(&parsed).expect("must extract");
        assert!(content.contains("line 0 of output"));
        assert!(content.contains("[stderr]"));
        assert!(content.contains("warning: unused variable"));
    }

    #[test]
    fn test_claude_code_empty_stdout_ignored() {
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
    fn test_content_field_still_preferred_over_stdout() {
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
}
