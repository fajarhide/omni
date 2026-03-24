use crate::distillers;
use crate::pipeline::toml_filter;
use crate::pipeline::{DistillResult, Route, SessionState, classifier, composer, scorer};
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

    let start = Instant::now();

    // ─── Distillation Engine ────────────────────────────────
    let mut scored_segments = Vec::new();

    // TOML-first: try matching command against TOML filters
    let toml_filters = toml_filter::load_all_filters();
    let toml_match = toml_filters.iter().find(|f| f.matches(&command));

    let (mut final_out, filter_name, ctype) = if let Some(filter) = toml_match {
        // Use TOML filter
        let output = filter.apply(&content);
        let fname = filter.name.clone();
        (output, fname, None)
    } else {
        // Fallback to Rust distiller pipeline
        let ct = classifier::classify(&content);

        let segments = if let Some(ref lock) = session {
            if let Ok(state) = lock.lock() {
                scorer::score_segments(&content, &ct, Some(&*state))
            } else {
                scorer::score_segments(&content, &ct, None)
            }
        } else {
            scorer::score_segments(&content, &ct, None)
        };

        let distiller = distillers::get_distiller(&ct);
        let output = distiller.distill(&segments, &content);
        let fname = format!("{:?}", ct);
        scored_segments = segments;
        (output, fname, Some(ct))
    };

    // ─── Rewind & Indicators ─────────────────────────────────
    let mut rewind_hash = String::new();

    let (should_rewind, dropped_lines, critical_kept) = if let Some(ref ct) = ctype {
        let decision = composer::decide_rewind(&scored_segments, ct);
        let dropped = scored_segments
            .iter()
            .filter(|s| s.final_score() < decision.threshold)
            .map(|s| s.content.lines().count())
            .sum::<usize>();
        let critical = scored_segments
            .iter()
            .filter(|s| s.tier == crate::pipeline::SignalTier::Critical)
            .map(|s| s.content.lines().count())
            .sum::<usize>();
        (decision.should_store, dropped, critical)
    } else {
        // TOML filter case: estimate via line count
        let orig_lines = content.lines().count();
        let final_lines = final_out.lines().count();
        let dropped = if orig_lines > final_lines {
            orig_lines - final_lines
        } else {
            0
        };
        // If reduced by more than 30% or > 50 lines dropped, trigger rewind
        let should = dropped > 50 || (orig_lines > 10 && final_lines < orig_lines * 7 / 10);
        (should, dropped, 0)
    };

    if should_rewind {
        if let Some(ref s) = store {
            let hash = s.store_rewind(&content);
            final_out.push_str(&format!(
                "\n[omni: filtered {} noise lines. {} critical lines kept. id: {}]",
                dropped_lines, critical_kept, hash
            ));
            rewind_hash = hash;
        } else {
            final_out.push_str(&format!(
                "\n[omni: filtered {} noise lines. {} critical lines kept]",
                dropped_lines, critical_kept
            ));
        }
    }

    // ─── Session Updates ─────────────────────────────────────
    if let Some(ref lock) = session
        && let Ok(mut state) = lock.lock()
    {
        if !command.is_empty() {
            state.add_command(&command);
        }
        // Only add errors from Rust distillers (scored segments)
        for seg in &scored_segments {
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
        };
        let session_id = session
            .as_ref()
            .and_then(|lock| lock.lock().ok())
            .map(|s| s.session_id.clone())
            .unwrap_or_else(|| "unknown".to_string());
        s.record_distillation(&session_id, &result, &command);
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
