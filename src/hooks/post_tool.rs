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
}
pub fn process_payload(
    input_str: &str,
    store: Option<Arc<Store>>,
    session: Option<Arc<Mutex<SessionState>>>,
) -> Option<String> {
    let normalized = crate::hooks::normalize::normalize(input_str)?;

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
            return process_file_read(&content, filepath).map(wrap_hook_output);
        }
        "Grep" => {
            if !agent_config.grep_enabled() {
                return None;
            }
            return process_grep_output(&content).map(wrap_hook_output);
        }
        "WebFetch" => {
            if !agent_config.webfetch_enabled() {
                return None;
            }
            return process_web_content(&content).map(wrap_hook_output);
        }
        _ => return None, // Edit, Write, etc. — don't need distillation
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
    let toml_match = toml_filters.iter().find(|f| f.matches(clean_command));

    let session_guard = session.as_ref().and_then(|l| l.lock().ok());
    let mut collapse_savings_data = None;
    let (final_out, filter_name) = if let Some(filter) = toml_match {
        let output = filter.apply(&content);
        (output, filter.name.clone())
    } else {
        // Pure Command Architecture: Resolve profile once
        let profile = crate::pipeline::registry::resolve_profile_for_chain(clean_command);

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

    // Determine Route based on agent config thresholds
    let ratio = 1.0 - (final_out.len() as f32 / content.len().max(1) as f32);
    let (keep_threshold, soft_threshold) = agent_config.route_thresholds();

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
        if final_out.len() < 1000 {
            return None; // Tiny output, silent passthrough
        } else {
            final_out.insert_str(0, "[OMNI: Passthrough (low compression)]\n");
        }
    }

    let latency_ms = start.elapsed().as_millis() as u32;

    if let Some(ref s) = store {
        let kept = check_segments.len() - noise_count;
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
        };
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

fn wrap_hook_output(distilled: String) -> String {
    serde_json::to_string(&HookOutput {
        hook_specific_output: HookSpecificOutput {
            hook_event_name: "PostToolUse",
            updated_response: distilled,
        },
    })
    .unwrap_or_default()
}

// ── NON-BASH TOOL DISTILLATION ───────────────────────────────────────

fn process_file_read(content: &str, filepath: &str) -> Option<String> {
    let line_count = content.lines().count();
    if line_count < 50 {
        return None; // Small files pass through
    }

    let ext = std::path::Path::new(filepath)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    let distilled = match ext {
        "rs" => distill_rust_file(content),
        "py" => distill_python_file(content),
        "ts" | "tsx" | "js" | "jsx" => distill_js_ts_file(content),
        "go" => distill_go_file(content),
        "java" | "kt" => distill_java_file(content),
        "json" => distill_json_file(content),
        "toml" | "yaml" | "yml" => distill_config_file(content, ext),
        "log" | "txt" => distill_log_file(content),
        _ => distill_unknown_file(content),
    };

    // Only return if meaningful compression achieved
    if distilled.len() < content.len() * 8 / 10 {
        Some(format!(
            "[OMNI ReadFile: {} → distilled ({} lines)]\n{}",
            filepath, line_count, distilled
        ))
    } else {
        None
    }
}

fn distill_rust_file(content: &str) -> String {
    let mut out = String::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("pub fn ")
            || trimmed.starts_with("pub async fn ")
            || trimmed.starts_with("pub struct ")
            || trimmed.starts_with("pub enum ")
            || trimmed.starts_with("pub trait ")
            || trimmed.starts_with("impl ")
            || trimmed.starts_with("pub mod ")
            || trimmed.starts_with("use ")
            || trimmed.starts_with("//!")
            || trimmed.contains("todo!")
            || trimmed.contains("unimplemented!")
            || trimmed.contains("panic!")
        {
            out.push_str(line);
            out.push('\n');
        }
    }
    if out.is_empty() {
        distill_unknown_file(content)
    } else {
        out.trim().to_string()
    }
}

fn distill_python_file(content: &str) -> String {
    let mut out = String::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("def ")
            || trimmed.starts_with("async def ")
            || trimmed.starts_with("class ")
            || trimmed.starts_with("import ")
            || trimmed.starts_with("from ")
            || trimmed.starts_with('@')
        {
            out.push_str(line);
            out.push('\n');
        }
    }
    if out.is_empty() {
        distill_unknown_file(content)
    } else {
        out.trim().to_string()
    }
}

fn distill_js_ts_file(content: &str) -> String {
    let mut out = String::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("export ")
            || trimmed.starts_with("function ")
            || trimmed.starts_with("class ")
            || trimmed.starts_with("interface ")
            || trimmed.starts_with("type ")
            || (trimmed.starts_with("const ") && trimmed.contains("=>"))
            || trimmed.starts_with("import ")
        {
            out.push_str(line);
            out.push('\n');
        }
    }
    if out.is_empty() {
        distill_unknown_file(content)
    } else {
        out.trim().to_string()
    }
}

fn distill_go_file(content: &str) -> String {
    let mut out = String::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("func ")
            || trimmed.starts_with("type ")
            || trimmed.starts_with("var ")
            || trimmed.starts_with("const ")
            || trimmed.starts_with("package ")
            || trimmed.starts_with("import")
        {
            out.push_str(line);
            out.push('\n');
        }
    }
    if out.is_empty() {
        distill_unknown_file(content)
    } else {
        out.trim().to_string()
    }
}

fn distill_java_file(content: &str) -> String {
    let mut out = String::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if (trimmed.contains("class ")
            || trimmed.contains("interface ")
            || trimmed.contains("public ")
            || trimmed.contains("private ")
            || trimmed.contains("protected ")
            || trimmed.starts_with("import ")
            || trimmed.starts_with("package "))
            && !trimmed.starts_with("//")
            && !trimmed.is_empty()
        {
            out.push_str(line);
            out.push('\n');
        }
    }
    if out.is_empty() {
        distill_unknown_file(content)
    } else {
        out.trim().to_string()
    }
}

fn distill_json_file(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();
    if total <= 30 {
        return content.trim().to_string();
    }
    let head: Vec<&str> = lines.iter().take(15).copied().collect();
    format!(
        "{}\n... [{} more lines — full JSON in RewindStore]",
        head.join("\n"),
        total - 15
    )
}

fn distill_config_file(content: &str, ext: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();
    if total <= 40 {
        return content.trim().to_string();
    }
    let mut out = String::new();
    for line in &lines {
        let trimmed = line.trim();
        if (ext == "toml"
            && (trimmed.starts_with('[')
                || (!trimmed.starts_with('#')
                    && trimmed.contains('=')
                    && !trimmed.starts_with(' '))))
            || (matches!(ext, "yaml" | "yml")
                && !trimmed.starts_with(' ')
                && !trimmed.starts_with('#')
                && trimmed.ends_with(':'))
        {
            out.push_str(line);
            out.push('\n');
        }
    }
    if out.is_empty() {
        distill_unknown_file(content)
    } else {
        format!("[Config structure — {} lines total]\n{}", total, out.trim())
    }
}

fn distill_log_file(content: &str) -> String {
    let mut errors = 0usize;
    let mut warnings = 0usize;
    let mut error_lines: Vec<String> = vec![];
    for line in content.lines() {
        let l = line.to_lowercase();
        if l.contains("error") || l.contains("fatal") || l.contains("panic") {
            errors += 1;
            error_lines.push(line.to_string());
        } else if l.contains("warn") {
            warnings += 1;
        }
    }
    let total = content.lines().count();
    let mut out = format!(
        "Log: {} errors, {} warnings ({} total lines)\n",
        errors, warnings, total
    );
    for err in error_lines.iter().take(10) {
        out.push_str(err);
        out.push('\n');
    }
    if errors > 10 {
        out.push_str(&format!("... [{} more error lines]\n", errors - 10));
    }
    out.trim().to_string()
}

fn distill_unknown_file(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();
    if total <= 30 {
        return content.trim().to_string();
    }
    let head: Vec<&str> = lines.iter().take(15).copied().collect();
    let tail: Vec<&&str> = lines.iter().rev().take(5).collect();
    let tail_rev: Vec<&&str> = tail.into_iter().rev().collect();
    format!(
        "--- HEAD ({} total lines) ---\n{}\n... [{} lines omitted] ...\n--- TAIL ---\n{}",
        total,
        head.join("\n"),
        total - 20,
        tail_rev
            .iter()
            .map(|l| **l)
            .collect::<Vec<&str>>()
            .join("\n")
    )
}

fn process_grep_output(content: &str) -> Option<String> {
    let line_count = content.lines().count();
    if line_count < 20 {
        return None;
    } // Small results pass through

    let files: std::collections::HashSet<&str> = content
        .lines()
        .filter_map(|l| l.split(':').next())
        .filter(|f| !f.is_empty())
        .collect();
    let file_count = files.len();
    let top: Vec<&str> = content.lines().take(15).collect();
    let summary = format!(
        "[OMNI Grep: {} matches in {} files]\n{}{}",
        line_count,
        file_count,
        top.join("\n"),
        if line_count > 15 {
            format!("\n... [{} more matches]", line_count - 15)
        } else {
            String::new()
        }
    );
    if summary.len() < content.len() * 8 / 10 {
        Some(summary)
    } else {
        None
    }
}

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
    fn test_non_bash_tool_small_file_passthrough() {
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
    fn test_readfile_large_rust_file_distilled() {
        // Large ReadFile (>50 lines) should be distilled
        // Generate mix of pub fn signatures + private code bodies for realistic compression
        let mut big_rust = String::new();
        for i in 0..20 {
            big_rust.push_str(&format!("pub fn function_{}() -> i32 {{\n", i));
            big_rust.push_str(&format!("    let x = {};\n", i));
            big_rust.push_str(&format!("    let y = x + {};\n", i * 2));
            big_rust.push_str("    println!(\"computing result\");\n");
            big_rust.push_str("    x + y\n");
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
    fn test_grep_tool_distilled_with_file_count() {
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
    fn test_edit_tool_returns_none() {
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
    fn test_html_strip_removes_tags() {
        let html = "<h1>Title</h1><p>Content here</p>";
        let stripped = strip_html_simple(html);
        assert_eq!(stripped.trim(), "TitleContent here");
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
        // Therefore length > 90% and exits without distillation (because length < 1000)
        assert!(out.is_none());
    }

    #[test]
    fn test_no_significant_reduction_labeled_passthrough_for_large_output() {
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
    fn test_parse_error_exit_tanpa_output() {
        let out = process_payload("{ invalid json }", None, None);
        assert!(out.is_none());
    }

    #[test]
    fn test_array_content_format_extracted_correctly() {
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
    fn test_claude_code_stdout_format() {
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
        let norm = crate::hooks::normalize::normalize(&input.to_string()).expect("must normalize");
        assert!(norm.content.contains("line 0 of output"));
        assert!(norm.content.contains("[stderr]"));
        assert!(norm.content.contains("warning: unused variable"));
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

    #[test]
    fn test_process_payload_opencode_format() {
        let input = r#"{"type":"tool_result","tool":"shell","output":"pytest\n5 passed in 2.1s","command":"pytest"}"#;
        // OpenCode format harus diproses sama seperti Claude Code
        let _out = process_payload(input, None, None);
        // Jika content < threshold, bisa None — tapi jangan crash
        // Test ini memverifikasi tidak ada panic
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
        // Harus ada output (bukan None) untuk input panjang
        // (cargo build dengan 200 baris harusnya di-distilasi)
        assert!(
            out.is_some(),
            "Codex format harus di-distilasi jika output panjang"
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
            "Claude Code format harus tetap bekerja setelah refactor"
        );
    }
}
