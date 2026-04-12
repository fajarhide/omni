use anyhow::Result;
use colored::Colorize;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::pipeline::{ContentType, Route, SessionState, collapse, scorer, toml_filter};
use crate::store::sqlite::Store;
use crate::store::transcript::{Transcript, TranscriptEntry};

pub fn run(
    store: Option<Arc<Store>>,
    session: Option<Arc<Mutex<SessionState>>>,
    command_name: Option<&str>,
) -> Result<()> {
    let stdin = std::io::stdin().lock();
    let stdout = std::io::stdout().lock();
    let stderr = std::io::stderr().lock();

    // Testable generic route separating IO
    run_inner(stdin, stdout, stderr, store, session, command_name)
}

struct PipelineResult {
    session_id: String,
    output: String,
    filter_name: String,
    content_type: ContentType,
    rewind_hash: Option<String>,
    segments_kept: usize,
    segments_dropped: usize,
    input_text: String,
    start_time: Instant,
}

impl PipelineResult {
    fn best_output(&self) -> &str {
        if self.output.len() >= self.input_text.len() {
            &self.input_text // 100% Passthrough fallback maintaining limits correctly
        } else {
            &self.output
        }
    }
}

pub fn run_inner<R: Read, W: Write, E: Write>(
    input: R,
    mut output: W,
    mut error: E,
    store: Option<Arc<Store>>,
    session: Option<Arc<Mutex<SessionState>>>,
    command_name: Option<&str>,
) -> Result<()> {
    let start_time = Instant::now();

    // Phase 1: Read
    let input_text = match read_input(input, &mut output)? {
        Some(text) => text,
        None => return Ok(()), // Binary data was passed through directly
    };

    // Phase 2: Guard
    if let crate::guard::limits::InputCheck::Empty = crate::guard::limits::check_input(&input_text)
    {
        // Silent passthrough: command produced no output (e.g. failed upstream).
        // Don't error — just exit cleanly so we don't pollute Claude Code's stderr.
        return Ok(());
    } else if let crate::guard::limits::InputCheck::TooLarge =
        crate::guard::limits::check_input(&input_text)
    {
        writeln!(
            error,
            "[omni: Warning] Input size exceeds 1MB, processing may take longer..."
        )?;
    }

    // Phase 3: Transcript Begin
    let command_stripped = command_name.map(|c| {
        if let Some(stripped) = c.strip_prefix("omni exec ") {
            stripped
        } else {
            c
        }
    });
    transcript_begin(&session, &input_text, command_stripped, &mut error);

    // Phase 4: Distill
    let result = distill(
        input_text,
        &session,
        command_stripped,
        start_time,
        store.as_deref(),
    );

    // Phase 5: Persist
    persist(&result, &store, &session, command_stripped, &mut error);

    // Phase 6: Output
    emit_output(&result, &mut output, &mut error)?;

    Ok(())
}

fn read_input<R: Read, W: Write>(mut input: R, mut output: W) -> Result<Option<String>> {
    let mut buffer = Vec::new();
    let mut chunk = vec![0; 8192];
    let mut total_read = 0;

    loop {
        let n = input.read(&mut chunk)?;
        if n == 0 {
            break;
        }

        total_read += n;
        if total_read > crate::guard::limits::MAX_INPUT {
            // Cap buffer up to 16MB for safety LLM limits
            buffer.extend_from_slice(&chunk[..n]);
            break;
        }
        buffer.extend_from_slice(&chunk[..n]);
    }

    match std::str::from_utf8(&buffer) {
        Ok(s) => Ok(Some(s.to_string())),
        Err(_) => {
            // Buffer invalid UTF-8 format (binary), dump as is directly safely.
            output.write_all(&buffer)?;
            Ok(None)
        }
    }
}

fn with_session<F, R>(session: &Option<Arc<Mutex<SessionState>>>, f: F) -> Option<R>
where
    F: FnOnce(&SessionState) -> R,
{
    session.as_ref().and_then(|m| m.lock().ok().map(|g| f(&g)))
}

fn transcript_begin<E: Write>(
    session: &Option<Arc<Mutex<SessionState>>>,
    input_text: &str,
    command_name: Option<&str>,
    error: &mut E,
) {
    if let Some(guard) = session.as_ref().and_then(|m| m.lock().ok()) {
        let cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string());
        let mut transcript = Transcript::load_or_new(&guard.session_id, &cwd);
        let entry = TranscriptEntry::new_input(input_text, command_name);
        if let Err(e) = transcript.append_entry(entry)
            && cfg!(debug_assertions)
        {
            let _ = writeln!(error, "[omni:debug] transcript append error: {}", e);
        }
    }
}

fn distill(
    input_text: String,
    session: &Option<Arc<Mutex<SessionState>>>,
    command_name: Option<&str>,
    start_time: Instant,
    store: Option<&Store>,
) -> PipelineResult {
    let session_id = with_session(session, |g| g.session_id.clone())
        .unwrap_or_else(|| "pipe_session".to_string());

    let mut matched_toml = None;
    if let Some(cmd) = command_name {
        let filters = toml_filter::load_all_filters();
        if let Some(f) = filters.iter().find(|filter| filter.matches(cmd)) {
            matched_toml = Some(f.clone());
        }
    }

    let (output, filter_name, ctype, rewind_hash, kept_count, dropped_count) = if let Some(filter) =
        matched_toml
    {
        let out = filter.apply(&input_text);
        (out, filter.name.clone(), ContentType::Unknown, None, 0, 0)
    } else {
        let cmd = command_name.unwrap_or("");

        // Command-first scoring (new unified API)
        let (segments, c) = scorer::score_with_command(
            &input_text,
            cmd,
            session.as_ref().and_then(|m| m.lock().ok()).as_deref(),
        );

        // Collapse
        let collapse_result = collapse::collapse(&input_text, &c);
        let effective_input = collapse_result.collapsed_lines.join("\n");

        let final_segments = if collapse_result.savings_pct > 0.1 {
            scorer::score_with_command(
                &effective_input,
                cmd,
                session.as_ref().and_then(|m| m.lock().ok()).as_deref(),
            )
            .0
        } else {
            segments
        };

        // Distill with command context
        let mut out = crate::distillers::distill_with_command(
            &final_segments,
            &effective_input,
            cmd,
            &c,
            session.as_ref().and_then(|m| m.lock().ok()).as_deref(),
        );

        // Rewind decision — inline (no composer needed)
        let noise_count = final_segments
            .iter()
            .filter(|s| s.final_score() < 0.3)
            .count();
        let should_store = noise_count as f32 / final_segments.len().max(1) as f32 > 0.4
            && final_segments.len() > 20;
        let d_count = noise_count;
        let k_count = final_segments.len() - d_count;

        // Auto-learn trigger (inline)
        if !cmd.is_empty() && input_text.len() > 100 {
            let poor = final_segments.len() > 5
                && (d_count as f32 / final_segments.len().max(1) as f32) < 0.3;
            if poor || matches!(c, ContentType::Unknown) {
                crate::session::learn::queue_for_learn(&input_text, cmd);
            }
        }

        let mut r_hash = None;
        if should_store && let Some(s) = store {
            let hash = s.store_rewind(&input_text);
            out.push_str(&format!(
                "\n{} {} {} {} lines. The hash {} stores the full output in RewindStore for retrieval.\n",
                "⏺".cyan(),
                "OMNI".bold().bright_white(),
                "distilled".bright_green(),
                d_count,
                hash.cyan().bold()
            ));
            r_hash = Some(hash);
        }

        // Safety truncation (was in composer::ComposeConfig::default().max_output_chars)
        const MAX_OUTPUT: usize = 50_000;
        if out.len() > MAX_OUTPUT {
            out.truncate(MAX_OUTPUT);
            out.push_str("\n[OMNI: output truncated]\n");
        }

        (
            out,
            cmd.split_whitespace().next().unwrap_or("omni").to_string(),
            c,
            r_hash,
            k_count,
            d_count,
        )
    };

    PipelineResult {
        session_id,
        output,
        filter_name,
        content_type: ctype,
        rewind_hash,
        segments_kept: kept_count,
        segments_dropped: dropped_count,
        input_text,
        start_time,
    }
}

fn persist<E: Write>(
    result: &PipelineResult,
    store_opt: &Option<Arc<Store>>,
    session: &Option<Arc<Mutex<SessionState>>>,
    command_name: Option<&str>,
    error: &mut E,
) {
    if let Some(s) = store_opt {
        use crate::pipeline::DistillResult;
        let distill_result = DistillResult {
            output: result.best_output().to_string(), // use the best output for persistence
            route: if result.rewind_hash.is_some() {
                Route::Rewind
            } else {
                Route::Keep
            },
            filter_name: result.filter_name.clone(),
            content_type: result.content_type.clone(),
            score: 0.0,
            context_score: 0.0,
            input_bytes: result.input_text.len(),
            output_bytes: result.best_output().len(),
            latency_ms: result.start_time.elapsed().as_millis() as u64,
            rewind_hash: result.rewind_hash.clone(),
            segments_kept: result.segments_kept,
            segments_dropped: result.segments_dropped,
            collapse_savings: None,
        };

        s.record_distillation(
            &result.session_id,
            &distill_result,
            command_name.unwrap_or(""),
        );

        if let Some(sess) = session {
            let tracker = crate::session::tracker::SessionTracker::new(sess.clone(), s.clone());
            tracker.track_command(
                command_name.unwrap_or(""),
                &result.input_text,
                &distill_result,
            );
        }

        let cache_dir = dirs::home_dir()
            .unwrap_or_default()
            .join(".omni")
            .join("cache");
        if let Err(e) = std::fs::create_dir_all(&cache_dir)
            && cfg!(debug_assertions)
        {
            let _ = writeln!(error, "[omni:debug] cache dir creation error: {}", e);
        }
        if let Err(e) = std::fs::write(cache_dir.join("last_input.txt"), &result.input_text)
            && cfg!(debug_assertions)
        {
            let _ = writeln!(error, "[omni:debug] cache input write error: {}", e);
        }
        if let Err(e) = std::fs::write(cache_dir.join("last_output.txt"), result.best_output())
            && cfg!(debug_assertions)
        {
            let _ = writeln!(error, "[omni:debug] cache output write error: {}", e);
        }
    }

    let transcript_load = Transcript::load(&result.session_id);
    if let Some(mut transcript) = transcript_load {
        if let Err(e) = transcript.mark_last_completed(result.best_output())
            && cfg!(debug_assertions)
        {
            let _ = writeln!(error, "[omni:debug] transcript complete error: {}", e);
        }
        if let Some(guard) = session.as_ref().and_then(|m| m.lock().ok())
            && let Err(e) = transcript.snapshot_state(&guard)
            && cfg!(debug_assertions)
        {
            let _ = writeln!(error, "[omni:debug] transcript snapshot error: {}", e);
        }
    }
}

fn emit_output<W: Write, E: Write>(
    result: &PipelineResult,
    output: &mut W,
    error: &mut E,
) -> Result<()> {
    output.write_all(result.best_output().as_bytes())?;
    output.flush()?;

    if crate::guard::env::is_quiet() {
        return Ok(());
    }

    let elapsed = result.start_time.elapsed().as_millis();
    let reduction = if !result.input_text.is_empty() {
        100.0 * (1.0 - result.best_output().len() as f64 / result.input_text.len() as f64)
    } else {
        0.0
    };

    if reduction > 10.0 || elapsed > 100 {
        let msg = format!(
            "{} {:.1}% reduction ({} → {}) {}ms",
            "⏺".cyan(),
            reduction,
            crate::cli::stats::format_bytes(result.input_text.len() as u64).black(),
            crate::cli::stats::format_bytes(result.best_output().len() as u64).green(),
            elapsed.to_string().bright_black()
        );
        writeln!(error, "{} {}", "[OMNI Active]".bold().cyan(), msg)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipe_mode_distils_git_diff() {
        let input = "diff --git a/foo b/foo\n@@ -1,1 +1,1 @@\n-old\n+new\n";
        let mut out = Vec::new();
        let mut err = Vec::new();

        run_inner(input.as_bytes(), &mut out, &mut err, None, None, None).expect("must succeed");

        let out_str = String::from_utf8(out).expect("must succeed");
        assert!(out_str.contains("diff --git"));
        assert!(!err.iter().any(|&b| b == b'e' || b == b'E'));
    }

    #[test]
    fn test_pipe_mode_passthrough_for_short_input() {
        let input = "hello world\nthis is short";
        let mut out = Vec::new();
        let mut err = Vec::new();

        run_inner(input.as_bytes(), &mut out, &mut err, None, None, None).expect("must succeed");
        let out_str = String::from_utf8(out).expect("must succeed");

        assert_eq!(out_str, input);
    }

    #[test]
    fn test_pipe_mode_exit_0_selalu_as_ok() {
        let binary_input: Vec<u8> = vec![0xFF, 0xFE, 0xFD];

        let mut out = Vec::new();
        let mut err = Vec::new();

        let res = run_inner(
            binary_input.as_slice(),
            &mut out,
            &mut err,
            None,
            None,
            None,
        );
        assert!(res.is_ok());
        assert_eq!(out, binary_input);
    }
}
