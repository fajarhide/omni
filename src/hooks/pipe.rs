use anyhow::Result;
use colored::Colorize;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::pipeline::{SessionState, classifier, composer, scorer, toml_filter};
use crate::store::sqlite::Store;
use crate::store::transcript::{Transcript, TranscriptEntry};

const MAX_PIPE_SIZE: usize = 16 * 1024 * 1024; // 16MB
const WARN_PIPE_SIZE: usize = 1024 * 1024; // 1MB

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

pub fn run_inner<R: Read, W: Write, E: Write>(
    mut input: R,
    mut output: W,
    mut error: E,
    store: Option<Arc<Store>>,
    session: Option<Arc<Mutex<SessionState>>>,
    command_name: Option<&str>,
) -> Result<()> {
    let start_time = Instant::now();

    // 1. Baca stdin sampai EOF (max 16MB)
    let mut buffer = Vec::new();
    let mut chunk = vec![0; 8192];
    let mut total_read = 0;

    loop {
        let n = input.read(&mut chunk)?;
        if n == 0 {
            break;
        }

        total_read += n;
        if total_read > MAX_PIPE_SIZE {
            // Cap buffer up to 16MB for safety LLM limits
            buffer.extend_from_slice(&chunk[..n]);
            break;
        }
        buffer.extend_from_slice(&chunk[..n]);
    }

    // 2. If empty: eprintln! + exit 1
    if buffer.is_empty() {
        writeln!(error, "omni: Error: No input provided on stdin")?;
        std::process::exit(1);
    }

    // 3. Binary input -> passthrough (output raw)
    let input_text = match std::str::from_utf8(&buffer) {
        Ok(s) => s.to_string(),
        Err(_) => {
            // Buffer invalid UTF-8 format (binary), dump as is directly safely.
            output.write_all(&buffer)?;
            return Ok(());
        }
    };

    if input_text.len() > WARN_PIPE_SIZE {
        writeln!(
            error,
            "[omni: Warning] Input size exceeds 1MB, processing may take longer..."
        )?;
    }

    // 3.5 Transcript: persist input BEFORE processing
    let transcript_session_id = if let Some(ref session_arc) = session {
        if let Ok(guard) = session_arc.lock() {
            let cwd = std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".to_string());
            let mut transcript = Transcript::load_or_new(&guard.session_id, &cwd);
            let entry = TranscriptEntry::new_input(&input_text, command_name);
            let _ = transcript.append_entry(entry);
            Some(guard.session_id.clone())
        } else {
            None
        }
    } else {
        None
    };

    // 4. Run pipeline
    let command_name = command_name.map(|c| {
        if let Some(stripped) = c.strip_prefix("omni exec ") {
            stripped
        } else {
            c
        }
    });

    let (s_id, final_output, filter_name, ctype, rewind_hash_opt, kept_count, dropped_count) = {
        let (s_id_internal, active_session_opt) = match session {
            Some(ref m) => {
                let guard = m.lock().expect("must succeed");
                (guard.session_id.clone(), Some(guard))
            }
            None => ("pipe_session".to_string(), None),
        };

        let mut matched_toml = None;
        if let Some(cmd) = command_name {
            let filters = toml_filter::load_all_filters();
            if let Some(f) = filters.iter().find(|filter| filter.matches(cmd)) {
                matched_toml = Some(f.clone());
            }
        }

        if let Some(filter) = matched_toml {
            let out = filter.apply(&input_text);
            (
                s_id_internal,
                out,
                filter.name.clone(),
                crate::pipeline::ContentType::Unknown,
                None,
                0,
                0,
            )
        } else {
            let c = classifier::classify(&input_text);
            let scored_segments =
                scorer::score_segments(&input_text, &c, active_session_opt.as_deref());
            drop(active_session_opt);

            let compose_config = composer::ComposeConfig::default();
            let decision = composer::decide_rewind(&scored_segments, &c);

            let k_count = scored_segments
                .iter()
                .filter(|s| s.final_score() >= compose_config.threshold)
                .count();
            let d_count = scored_segments.len() - k_count;

            let (out, r_hash) = if decision.should_store && store.is_some() {
                composer::compose(
                    scored_segments,
                    Some(input_text.clone()),
                    &compose_config,
                    store.as_deref(),
                    &input_text,
                    &c,
                )
            } else {
                composer::compose(
                    scored_segments,
                    None,
                    &compose_config,
                    None,
                    &input_text,
                    &c,
                )
            };
            (
                s_id_internal,
                out,
                format!("{:?}", c),
                c,
                r_hash,
                k_count,
                d_count,
            )
        }
    };

    if let Some(ref s) = store {
        use crate::pipeline::{DistillResult, Route};
        let result = DistillResult {
            output: final_output.clone(),
            route: if rewind_hash_opt.is_some() {
                Route::Rewind
            } else {
                Route::Keep
            },
            filter_name: filter_name.clone(),
            content_type: ctype.clone(),
            score: 0.0,
            context_score: 0.0,
            input_bytes: input_text.len(),
            output_bytes: final_output.len(),
            latency_ms: start_time.elapsed().as_millis() as u64,
            rewind_hash: rewind_hash_opt,
            segments_kept: kept_count,
            segments_dropped: dropped_count,
        };

        s.record_distillation(&s_id, &result, command_name.unwrap_or(""));

        // Save for `omni diff`
        let cache_dir = dirs::home_dir()
            .unwrap_or_default()
            .join(".omni")
            .join("cache");
        let _ = std::fs::create_dir_all(&cache_dir);
        let _ = std::fs::write(cache_dir.join("last_input.txt"), &input_text);
        let _ = std::fs::write(cache_dir.join("last_output.txt"), &final_output);
    }

    // 4.5 Transcript: mark completed + snapshot session state
    if let Some(ref sid) = transcript_session_id
        && let Some(mut transcript) = Transcript::load(sid)
    {
        let _ = transcript.mark_last_completed(&final_output);
        // Snapshot session state for crash recovery context
        if let Some(ref session_arc) = session
            && let Ok(guard) = session_arc.lock()
        {
            let _ = transcript.snapshot_state(&guard);
        }
    }

    // 5. If no significant reduction: print original
    let output_to_print = if final_output.len() >= input_text.len() {
        &input_text // 100% Passthrough fallback maintaining limits correctly
    } else {
        &final_output
    };

    output.write_all(output_to_print.as_bytes())?;
    output.flush()?;

    // 6. Premium status indicator
    let elapsed = start_time.elapsed().as_millis();
    let reduction = if !input_text.is_empty() {
        100.0 * (1.0 - final_output.len() as f64 / input_text.len() as f64)
    } else {
        0.0
    };

    if reduction > 10.0 || elapsed > 100 {
        let msg = format!(
            "{} {:.1}% reduction ({} → {}) {}ms",
            "⏺".cyan(),
            reduction,
            crate::cli::stats::format_bytes(input_text.len() as u64).black(),
            crate::cli::stats::format_bytes(final_output.len() as u64).green(),
            elapsed.to_string().bright_black()
        );
        writeln!(error, "{} {}", "[OMNI Active]".bold().cyan(), msg)?;
    }

    // 7. Exit 0 (Success)
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
        // Native Git Diff outputs are normally kept natively, so reduction < original_text.len isn't guaranteed heavily
        // The pipe mode should successfully print it.
        assert!(out_str.contains("diff --git"));
        assert!(!err.iter().any(|&b| b == b'e' || b == b'E')); // No errors in output pipe error block
    }

    #[test]
    fn test_pipe_mode_passthrough_for_short_input() {
        let input = "hello world\nthis is short";
        let mut out = Vec::new();
        let mut err = Vec::new();

        run_inner(input.as_bytes(), &mut out, &mut err, None, None, None).expect("must succeed");
        let out_str = String::from_utf8(out).expect("must succeed");

        // No significant reduction for short inputs
        assert_eq!(out_str, input);
    }

    #[test]
    fn test_pipe_mode_exit_0_selalu_as_ok() {
        let binary_input: Vec<u8> = vec![0xFF, 0xFE, 0xFD]; // Invalid UTF-8 Binary Data Checks

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
        assert!(res.is_ok()); // Exit 0 effectively gracefully returns properly
        assert_eq!(out, binary_input); // Binary is passed directly unmodified.
    }
}
