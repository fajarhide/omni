use anyhow::Result;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::pipeline::{SessionState, classifier, composer, scorer};
use crate::store::sqlite::Store;

const MAX_PIPE_SIZE: usize = 16 * 1024 * 1024; // 16MB
const WARN_PIPE_SIZE: usize = 1024 * 1024; // 1MB

pub fn run(store: Option<Arc<Store>>, session: Option<Arc<Mutex<SessionState>>>) -> Result<()> {
    let stdin = std::io::stdin().lock();
    let stdout = std::io::stdout().lock();
    let stderr = std::io::stderr().lock();

    // Testable generic route separating IO
    run_inner(stdin, stdout, stderr, store, session)
}

pub fn run_inner<R: Read, W: Write, E: Write>(
    mut input: R,
    mut output: W,
    mut error: E,
    store: Option<Arc<Store>>,
    session: Option<Arc<Mutex<SessionState>>>,
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

    // 4. Run pipeline natively
    let ctype = classifier::classify(&input_text);

    let active_session = session.as_ref().map(|s| s.lock().expect("must succeed"));
    let scored_segments = scorer::score_segments(&input_text, &ctype, active_session.as_deref());

    let compose_config = composer::ComposeConfig::default();
    let decision = composer::decide_rewind(&scored_segments, &ctype);

    let final_output = if decision.should_store && store.is_some() {
        composer::compose(
            scored_segments,
            Some(input_text.clone()),
            &compose_config,
            store.as_deref(),
            &input_text,
            &ctype,
        )
        .0
    } else {
        composer::compose(
            scored_segments,
            None,
            &compose_config,
            None,
            &input_text,
            &ctype,
        )
        .0
    };

    // 5. If no significant reduction: print original
    let output_to_print = if final_output.len() >= input_text.len() {
        &input_text // 100% Passthrough fallback maintaining limits correctly
    } else {
        &final_output
    };

    output.write_all(output_to_print.as_bytes())?;
    output.flush()?;

    // 6. Latency bounfores ensuring visibility into heavy SQLite evaluations natively
    let elapsed = start_time.elapsed().as_millis();
    if elapsed > 100 {
        writeln!(error, "[omni: {}ms]", elapsed)?;
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

        run_inner(input.as_bytes(), &mut out, &mut err, None, None).expect("must succeed");

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

        run_inner(input.as_bytes(), &mut out, &mut err, None, None).expect("must succeed");
        let out_str = String::from_utf8(out).expect("must succeed");

        // No significant reduction for short inputs
        assert_eq!(out_str, input);
    }

    #[test]
    fn test_pipe_mode_exit_0_selalu_as_ok() {
        let binary_input: Vec<u8> = vec![0xFF, 0xFE, 0xFD]; // Invalid UTF-8 Binary Data Checks

        let mut out = Vec::new();
        let mut err = Vec::new();

        let res = run_inner(binary_input.as_slice(), &mut out, &mut err, None, None);
        assert!(res.is_ok()); // Exit 0 effectively gracefully returns properly
        assert_eq!(out, binary_input); // Binary is passed directly unmodified.
    }
}
