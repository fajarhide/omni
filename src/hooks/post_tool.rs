use crate::distillers;
use crate::pipeline::toml_filter;
use crate::pipeline::{DistillResult, Route, SessionState, classifier, collapse, composer, scorer};
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

    let raw_val = parsed
        .tool_response
        .as_ref()
        .and_then(|r| r.content.as_ref())?;

    let content = extract_content(raw_val)?;

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

    let (final_out, filter_name, ctype) = if let Some(filter) = toml_match {
        // Use TOML filter
        let output = filter.apply(&content);
        let fname = filter.name.clone();
        (output, fname, None)
    } else {
        // Fallback to Rust distiller pipeline
        let ctype = classifier::classify(&content, Some(&command));

        // Pre-processing: collapse repetitive lines before scoring
        let collapse_result = collapse::collapse(&content, &ctype);
        let effective_input = collapse_result.collapsed_lines.join("\n");

        let scored_segments = if let Some(ref lock) = session {
            if let Ok(state) = lock.lock() {
                scorer::score_segments(&effective_input, &ctype, Some(&*state))
            } else {
                scorer::score_segments(&effective_input, &ctype, None)
            }
        } else {
            scorer::score_segments(&effective_input, &ctype, None)
        };

        let distiller = distillers::get_distiller(&ctype);
        let active_ctype = distiller.content_type();
        let output = distiller.distill(
            &scored_segments,
            &effective_input,
            session.as_ref().and_then(|l| l.lock().ok()).as_deref(),
        );
        (output, format!("{:?}", active_ctype), Some(active_ctype))
    };

    // Check for rewind decision (only for Rust pipeline)
    let mut final_out = final_out;
    let mut rewind_hash = String::new();

    if let Some(ref ctype) = ctype {
        let scored_segments = if let Some(ref lock) = session {
            if let Ok(state) = lock.lock() {
                scorer::score_segments(&content, ctype, Some(&*state))
            } else {
                scorer::score_segments(&content, ctype, None)
            }
        } else {
            scorer::score_segments(&content, ctype, None)
        };

        let decision = composer::decide_rewind(&scored_segments, ctype);

        let dropped_lines = scored_segments
            .iter()
            .filter(|s| s.final_score() < decision.threshold)
            .map(|s| s.content.lines().count())
            .sum::<usize>();

        // Trigger Auto-Learn
        crate::pipeline::composer::evaluate_learning(
            ctype,
            &content,
            scored_segments.len(),
            scored_segments
                .iter()
                .filter(|s| s.final_score() < decision.threshold)
                .count(),
            &command,
        );

        if decision.should_store
            && let Some(ref s) = store
        {
            let hash = s.store_rewind(&content);
            final_out.push_str(&format!(
                "\n[OMNI: {} lines omitted — omni_retrieve(\"{}\") for full output]",
                dropped_lines, hash
            ));
            rewind_hash = hash;
        } else if decision.should_store {
            final_out.push_str(&format!("\n[OMNI: {} lines omitted]", dropped_lines));
        }

        // Update session state (only for Rust pipeline)
        if let Some(ref lock) = session
            && let Ok(mut state) = lock.lock()
        {
            if !command.is_empty() {
                state.add_command(&command);
            }
            for seg in &scored_segments {
                if seg.tier == crate::pipeline::SignalTier::Critical {
                    state.add_error(&seg.content);
                }
            }
        }
    }

    // Measure ratio strictly
    if final_out.len() >= content.len() * 9 / 10 {
        return None;
    }

    let latency_ms = start.elapsed().as_millis() as u32;

    if let Some(ref s) = store {
        let result = DistillResult {
            output: final_out.clone(),
            route: if rewind_hash.is_empty() {
                Route::Keep
            } else {
                Route::Rewind
            },
            filter_name: filter_name.clone(),
            content_type: ctype
                .clone()
                .unwrap_or(crate::pipeline::ContentType::Unknown),
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
            segments_kept: 0,
            segments_dropped: 0,
            collapse_savings: None,
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
    let max_chars = composer::ComposeConfig::default().max_output_chars;
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
}
