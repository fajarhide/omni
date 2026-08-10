// Safety: String slicing uses ASCII delimiter positions or boundary-checked safe utilities.
#![allow(clippy::string_slice)]

use anyhow::Result;
use colored::Colorize;
use is_terminal::IsTerminal;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Guardrail: only emit distilled output if it's at least this much smaller than
/// input. Shared with `hooks::post_tool` via `guard::limits` (CLAUDE.md SSOT).
use crate::guard::limits::{MAX_OUTPUT_BYTES, MIN_REDUCTION_PCT};

use crate::pipeline::{Route, SessionState, collapse, scorer, toml_filter};
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
    rewind_hash: Option<String>,
    segments_kept: usize,
    segments_dropped: usize,
    input_text: String,
    start_time: Instant,
    collapse_savings: Option<(usize, usize)>,
    project_path: String,
    route: Route,
}

impl PipelineResult {
    fn best_output(&self) -> &str {
        let guardrail_len = self.input_text.len() * MIN_REDUCTION_PCT / 100;
        if self.output.len() >= guardrail_len {
            &self.input_text // Guardrail: never emit output ~same size as input
        } else {
            &self.output
        }
    }
}

/// The stream-mode TOML filter that governs `cmd`, if any. Stream-mode filters
/// emit distilled output line-by-line as it arrives — before a wrapped command's
/// exit code is known — so callers that gate on exit status (`omni exec`, #122)
/// must treat a stream-mode command as un-gateable and keep it streaming.
/// Semantics match Phase 0.5: the first filter that matches, and only if it is
/// stream-mode.
pub fn stream_filter_for(cmd: &str) -> Option<toml_filter::TomlFilter> {
    let filters = toml_filter::load_all_filters();
    let f = filters.iter().find(|filter| filter.matches(cmd))?;
    f.stream_mode.then(|| f.clone())
}

pub fn run_inner<R: Read, W: Write, E: Write>(
    input: R,
    mut output: W,
    mut error: E,
    store: Option<Arc<Store>>,
    session: Option<Arc<Mutex<SessionState>>>,
    command_name: Option<&str>,
) -> Result<()> {
    // Phase 0: Sibling detection (CRITICAL: Do this BEFORE any IO or heavy logic)
    let detected_cmd = if command_name.is_none() {
        detect_sibling_command()
    } else {
        None
    };
    let command_to_use = command_name
        .or(detected_cmd.as_deref())
        .map(crate::cli::rewrite::strip_exec_wrapper);

    // Phase 0.5: Streaming Distillation Check
    if let Some(filter) = command_to_use.and_then(stream_filter_for) {
        return stream_distill(input, output, error, filter, store, session, command_to_use);
    }

    let start_time = Instant::now();

    // Phase 1: Read
    let input_text = match read_input(input, &mut output)? {
        Some(text) => text,
        None => return Ok(()), // Binary data was passed through directly
    };

    // Phase 2: Guard
    let input_check = crate::guard::limits::check_input(&input_text);

    if let crate::guard::limits::InputCheck::Empty = input_check {
        // Silent passthrough: command produced no output (e.g. failed upstream).
        // Don't error — just exit cleanly so we don't pollute Claude Code's stderr.
        return Ok(());
    } else if matches!(
        input_check,
        crate::guard::limits::InputCheck::Warn | crate::guard::limits::InputCheck::TooLarge
    ) {
        writeln!(
            error,
            "[omni: Warning] Large input detected; processing may take longer..."
        )?;
    }

    if crate::guard::env::is_passthrough() {
        output.write_all(input_text.as_bytes())?;
        return Ok(());
    }

    // Phase 2.5: Format-safe gate. Structured payloads are machine-read downstream
    // (`jq`, `json.load`, `kubectl apply`); collapse and the distillers would leave
    // them unparseable. Emit the input verbatim, byte for byte.
    if let Some(kind) = crate::pipeline::format::sniff(&input_text) {
        output.write_all(input_text.as_bytes())?;
        output.flush()?;
        if let Some(s) = &store {
            s.record_passthrough(
                &format!(
                    "{} [{}]",
                    command_to_use.unwrap_or(""),
                    crate::pipeline::format::passthrough_reason(kind)
                ),
                input_text.len(),
            );
        }
        return Ok(());
    }

    // Phase 3: Transcript Begin
    let project_path = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    transcript_begin(&session, &input_text, command_to_use, &mut error);

    // Phase 4: Distill
    let result = distill(
        input_text,
        &session,
        command_to_use,
        start_time,
        store.as_deref(),
        project_path,
    );

    // Phase 5: Persist
    persist(&result, &store, &session, command_to_use, &mut error);

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

fn stream_distill<R: Read, W: Write, E: Write>(
    input: R,
    output: W,
    mut error: E,
    filter: toml_filter::TomlFilter,
    store: Option<Arc<Store>>,
    session: Option<Arc<Mutex<SessionState>>>,
    command_name: Option<&str>,
) -> Result<()> {
    let start_time = Instant::now();
    let mut reader = std::io::BufReader::new(input);
    let mut truncated_output = crate::util::stream::TruncatingWriter::new(output, MAX_OUTPUT_BYTES);

    let mut raw_bytes = 0;
    let mut line_buffer = Vec::new();

    // Streaming loop
    loop {
        line_buffer.clear();
        // We read byte by byte until \n or \r to support progress bars correctly
        let mut b = [0u8; 1];
        let mut eof = false;
        loop {
            match reader.read(&mut b) {
                Ok(0) => {
                    eof = true;
                    break;
                }
                Ok(_) => {
                    let ch = b[0];
                    line_buffer.push(ch);
                    if ch == b'\n' || ch == b'\r' {
                        break;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e.into()),
            }
        }

        if line_buffer.is_empty() && eof {
            break;
        }

        raw_bytes += line_buffer.len();

        let line_str = String::from_utf8_lossy(&line_buffer);
        let is_cr = line_str.ends_with('\r');
        let clean_line = line_str.trim_end_matches(['\n', '\r']);

        // We apply filter line by line, which is why this is `apply_line`: the
        // whole-payload `apply` ends in the `on_empty` zero-state, and answering
        // "this filtered down to nothing" once per stripped line turned every
        // piece of noise into a success sentence (#406).
        let filtered = filter.apply_line(clean_line);
        if !filtered.trim().is_empty() {
            truncated_output.write_all(filtered.as_bytes())?;

            if is_cr {
                truncated_output.write_all(b"\r")?;
            } else {
                truncated_output.write_all(b"\n")?;
            }
            truncated_output.flush()?;
        }

        // Stop completely if the stream limit is reached
        if truncated_output.is_truncated() {
            break;
        }

        if eof {
            break;
        }
    }

    let filtered_bytes = truncated_output.bytes_written();

    // Persist streaming stats without saving full input/output to prevent memory spikes
    if let Some(s) = store {
        use crate::pipeline::DistillResult;
        let session_id = with_session(&session, |g| g.session_id.clone())
            .unwrap_or_else(|| "pipe_session".to_string());

        let agent_id = resolve_pipe_agent_id();
        let project_path = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        let cmd = command_name.unwrap_or("");

        let reduction = if raw_bytes > 0 {
            100.0 * (1.0 - (filtered_bytes as f64 / raw_bytes as f64))
        } else {
            0.0
        };

        if !crate::guard::env::is_quiet() {
            let msg = format!(
                "{} {:.1}% reduction ({} → {}) {}ms (Streaming)",
                "⏺".cyan(),
                reduction,
                crate::cli::stats::format_bytes(raw_bytes as u64).bright_black(),
                crate::cli::stats::format_bytes(filtered_bytes as u64).green(),
                start_time.elapsed().as_millis().to_string().bright_black()
            );
            let _ = writeln!(error, "{} {}", "[OMNI Active]".bold().cyan(), msg);
        }

        let distill_result = DistillResult {
            output: format!(
                "[Streaming Mode - Output omitted from memory. Raw: {}, Filtered: {}]",
                raw_bytes, filtered_bytes
            ),
            route: Route::Keep,
            filter_name: filter.name.clone(),
            score: 0.0,
            context_score: 0.0,
            input_bytes: raw_bytes,
            output_bytes: filtered_bytes,
            latency_ms: start_time.elapsed().as_millis() as u64,
            rewind_hash: None,
            segments_kept: 0,
            segments_dropped: 0,
            collapse_savings: None,
            raw_tokens: (raw_bytes / 4),
            filtered_tokens: (filtered_bytes / 4),
            delivered_bytes: delivered_bytes(filtered_bytes),
        };

        s.record_distillation(&session_id, &distill_result, cmd, &project_path, &agent_id);

        if let Some(sess) = &session {
            let tracker = crate::session::tracker::SessionTracker::new(sess.clone(), s.clone());
            tracker.track_command(
                cmd,
                "[Streaming Mode - Input Omitted]",
                &distill_result,
                !std::io::stdout().is_terminal(),
            );
        }
    }

    Ok(())
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
    project_path: String,
) -> PipelineResult {
    let session_id = with_session(session, |g| g.session_id.clone())
        .unwrap_or_else(|| "pipe_session".to_string());

    // The single command this stdout came from, or `None` for a chain that
    // several programs wrote to. Routing anything by the first name in a chain
    // hands one distiller output it never produced (#264), and a TOML filter
    // keyed on that name does it just as readily as a Rust distiller.
    let output_command = command_name.and_then(crate::pipeline::registry::sole_output_command);

    let mut matched_toml = None;
    if let Some(cmd) = output_command {
        let filters = toml_filter::load_all_filters();
        if let Some(f) = filters.iter().find(|filter| filter.matches(cmd)) {
            matched_toml = Some(f.clone());
        }
    }

    // A TOML filter only gets to short-circuit the distiller if it actually beat
    // the guardrail. `sys_build_domain` matches every `cargo` invocation but strips
    // only `Compiling`-style lines, so on `cargo test` it won the `find()` race,
    // shadowed TestDistiller, cut 1%, and had that cut thrown away by best_output()
    // — reporting 0% on output the distiller reduces by 94%. Weak filter, fall
    // through; a filter that earns its match still wins, user filters included.
    let toml_hit = matched_toml.and_then(|f| match f.apply_batch(&input_text) {
        toml_filter::BatchFilterOutcome::Passthrough => {
            Some((input_text.clone(), f.name, Route::Passthrough))
        }
        toml_filter::BatchFilterOutcome::Filtered(out) => crate::guard::limits::beats_guardrail(
            out.len(),
            input_text.len(),
        )
        .then_some((out, f.name, Route::Keep)),
    });

    // A TOML filter used to return `None` for the rewind hash unconditionally, so
    // a filter that shortened the output archived nothing and marked nothing:
    // exactly the guarantee #271 closed for the distiller path, still open one
    // branch over. The archive decision is shared now, below, and both arms hand
    // it the same thing: the bytes about to be emitted.
    let (mut output, filter_name, kept_count, dropped_count, collapse_savings, mut route) =
        if let Some((out, name, route)) = toml_hit {
            (out, name, 0, 0, None, route)
        } else {
            let cmd = command_name.unwrap_or("");

            // Pure Command Architecture: Resolve profile
            let profile = crate::pipeline::registry::resolve_profile(cmd);

            // Score and distill the tool's REAL output. #116: a distiller parses
            // its input, so feeding it `collapse`'s `[N similar lines collapsed]`
            // markers makes it read OMNI's own scaffolding as data (a pod table
            // became `k8s: 2 pods | [5 (lines)`). Collapse is the fallback only for
            // commands no distiller handles (below).
            let segments = scorer::score_segments(
                &input_text,
                profile.segmentation,
                session.as_ref().and_then(|m| m.lock().ok()).as_deref(),
                cmd,
            );

            let mut out = crate::distillers::distill_with_command(
                &segments,
                &input_text,
                cmd,
                session.as_ref().and_then(|m| m.lock().ok()).as_deref(),
            );

            // When no distiller meaningfully reduced the raw output — it punted
            // (returned the input) or produced a near-copy that misses the
            // guardrail — fall back to the collapsed form for its line savings.
            // `best_output` still drops back to raw if even the collapse is too
            // weak, so this only ever helps. A distiller that earned its summary
            // keeps it; the markers never reached the distiller in the first place.
            // Enumeration commands (`ls`/`find`/`ps`/…) return the input verbatim
            // by design; collapsing them would drop rows that are the answer, so
            // never fall back to collapse for them (#200).
            // The verbatim check asks the resolved command, not the string the
            // user typed: the whole of `kubectl get pods -o json | jq -r '...'`
            // reads as `kubectl` and lets collapse rewrite a payload the next
            // step parses (#269).
            let collapse_savings_data =
                if crate::guard::limits::beats_guardrail(out.len(), input_text.len())
                    || !output_command
                        .is_some_and(|c| !crate::pipeline::registry::passes_through_verbatim(c))
                {
                    None
                } else {
                    let collapse_result = collapse::collapse(&input_text, &profile.collapse);
                    out = collapse_result.collapsed_lines.join("\n");
                    if collapse_result.original_lines > collapse_result.collapsed_to {
                        Some((collapse_result.original_lines, collapse_result.collapsed_to))
                    } else {
                        None
                    }
                };

            let d_count = segments.iter().filter(|s| s.final_score() < 0.3).count();
            let k_count = segments.len() - d_count;

            // Auto-learn trigger
            if !cmd.is_empty() && input_text.len() > 100 {
                let poor =
                    segments.len() > 5 && (d_count as f32 / segments.len().max(1) as f32) < 0.3;
                if poor {
                    crate::session::learn::queue_for_learn(&input_text, cmd);
                }
            }

            // Determine Route
            let ratio = 1.0 - (out.len() as f32 / input_text.len().max(1) as f32);
            let route = if ratio >= 0.7 {
                Route::Keep
            } else if ratio >= 0.3 {
                Route::Soft
            } else {
                Route::Passthrough
            };

            // A distiller that emitted more lines than it consumed restructured
            // rather than cut, so calling the result partial is a false claim
            // about a complete answer. `distill_grep_output` folds a repeated
            // `path:` prefix into a header: 11 matches become 15 lines holding all
            // 11, the byte ratio lands in `Soft`, and the banner said the output
            // was incomplete (#335). Same guard as `hooks::post_tool`.
            if route == Route::Soft && out.lines().count() <= input_text.lines().count() {
                out.push_str("\n[Partial signal - omni learn recommended]\n");
            }

            // Safety truncation. The marker carries the line count: `ps aux` lost
            // 416 of 556 rows here behind a bare `[OMNI: output truncated]` while
            // the footer reported it as a 62.2% saving (#219).
            crate::util::text::truncate_with_marker(&mut out, MAX_OUTPUT_BYTES);

            (
                out,
                cmd.split_whitespace()
                    .next()
                    .unwrap_or("[pipe]")
                    .to_string(),
                k_count,
                d_count,
                collapse_savings_data,
                route,
            )
        };

    // Rewind decision, the same question `hooks::post_tool` asks and for
    // the same reason: is the reply about to be emitted missing bytes the
    // command produced? The old gate wanted more than 40% noise across
    // more than 20 segments, a shape 0 of 8,968 recorded distillations
    // ever had, so the archive behind "everything cut is archived" stayed
    // empty (#271).
    //
    // The gate is `beats_guardrail` rather than the route, because a pipe
    // always writes something: `best_output` hands back the raw input
    // once the reply stops beating the guardrail, and only below it does
    // the caller actually lose lines. Archiving anything else stores
    // content nobody lost.
    let mut r_hash = None;
    if crate::guard::limits::beats_guardrail(output.len(), input_text.len()) {
        let omitted_lines = input_text
            .lines()
            .count()
            .saturating_sub(output.lines().count());
        let lost = if omitted_lines > 0 {
            format!("{omitted_lines} lines")
        } else {
            format!("{} bytes", input_text.len() - output.len())
        };

        let marker = if input_text.len() > crate::guard::limits::MAX_REWIND_BYTES {
            format!(
                "\n[OMNI: {lost} omitted, full output not archived: {} bytes over the {} byte rewind cap]\n",
                input_text.len(),
                crate::guard::limits::MAX_REWIND_BYTES
            )
        } else if let Some(hash) = store.and_then(|s| s.store_rewind(&input_text)) {
            let marker = if std::io::stdout().is_terminal() {
                format!(
                    "\n{} {} {} {}. The hash {} stores the full output in RewindStore for retrieval.\n",
                    "⏺".cyan(),
                    "OMNI".bold().bright_white(),
                    "distilled".bright_green(),
                    lost,
                    hash.cyan().bold()
                )
            } else {
                format!("\n[OMNI: {lost} omitted, omni_retrieve(\"{hash}\") for full output]\n")
            };
            r_hash = Some(hash);
            marker
        } else {
            // No store, or the insert did not land. Both leave the content
            // unretrievable, so neither may print a handle for it (#388).
            format!("\n[OMNI: {lost} omitted, full output not archived]\n")
        };

        // A marker that pushes the reply back over the guardrail costs
        // more than the cut saved (#268, #269 are 102 and 90 byte
        // payloads of that shape), and `best_output` would then emit the
        // raw input while the recorded row claimed a rewind.
        if crate::guard::limits::beats_guardrail(output.len() + marker.len(), input_text.len()) {
            output.push_str(&marker);
            if r_hash.is_some() {
                route = Route::Rewind;
            }
        } else {
            output = input_text.clone();
            route = Route::Passthrough;
            r_hash = None;
        }
    }

    // Safety truncation. The marker carries the line count: `ps aux` lost 416 of
    // 556 rows here behind a bare `[OMNI: output truncated]` while the footer
    // reported it as a 62.2% saving (#219).
    crate::util::text::truncate_with_marker(&mut output, MAX_OUTPUT_BYTES);

    PipelineResult {
        session_id,
        output,
        filter_name,
        rewind_hash: r_hash,
        segments_kept: kept_count,
        segments_dropped: dropped_count,
        input_text,
        start_time,
        collapse_savings,
        project_path,
        route,
    }
}

fn persist<E: Write>(
    result: &PipelineResult,
    store_opt: &Option<Arc<Store>>,
    session: &Option<Arc<Mutex<SessionState>>>,
    command_to_use: Option<&str>,
    error: &mut E,
) {
    if let Some(s) = store_opt {
        use crate::pipeline::DistillResult;
        use crate::util::token_estimate::{ContentHint, estimate_tokens};
        let raw_tokens = estimate_tokens(result.input_text.len(), ContentHint::Mixed);
        let filtered_tokens = estimate_tokens(result.best_output().len(), ContentHint::Mixed);

        let distill_result = DistillResult {
            output: result.best_output().to_string(), // use the best output for persistence
            route: result.route.clone(),
            filter_name: result.filter_name.clone(),
            score: 0.0,
            context_score: 0.0,
            input_bytes: result.input_text.len(),
            output_bytes: result.best_output().len(),
            latency_ms: result.start_time.elapsed().as_millis() as u64,
            rewind_hash: result.rewind_hash.clone(),
            segments_kept: result.segments_kept,
            segments_dropped: result.segments_dropped,
            collapse_savings: result.collapse_savings,
            raw_tokens,
            filtered_tokens,
            delivered_bytes: delivered_bytes(result.best_output().len()),
        };

        let agent_id = resolve_pipe_agent_id();
        s.record_distillation(
            &result.session_id,
            &distill_result,
            command_to_use.unwrap_or(""),
            &result.project_path,
            &agent_id,
        );
        s.record_trace(
            &result.session_id,
            command_to_use.unwrap_or(""),
            &agent_id,
            &result.project_path,
            &result.input_text,
            result.best_output(),
        );

        if let Some(sess) = session {
            let tracker = crate::session::tracker::SessionTracker::new(sess.clone(), s.clone());
            tracker.track_command(
                command_to_use.unwrap_or(""),
                &result.input_text,
                &distill_result,
                // Same test `delivered_bytes` uses: a TTY on the other end means
                // a human is reading and no context is billed for it.
                !std::io::stdout().is_terminal(),
            );
        }

        let cache_dir = crate::paths::cache_directory();
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

/// Who ran this, on the `omni exec` / pipe path.
///
/// This used to be a third, private set of rules (#160). It knew only
/// `OMNI_AGENT_ID`, then guessed **`aider`** for anything with `OMNI_CMD` set —
/// a variable OMNI documents for its own pipe mode and that any caller may set,
/// so 3,296 rows on one machine were filed under an agent that had not run.
/// Everything else became `terminal`, which is why work done inside Claude Code
/// appeared in `omni stats` as shell usage.
///
/// It now defers to the one env-based detector, so `omni exec`, the pipe, and
/// the peer-session tracker all name an agent the same way, and that name
/// matches what `hooks::normalize` derives from a payload on the hook path.
fn resolve_pipe_agent_id() -> String {
    pipe_agent_id_from(
        HOST_THAT_REWROTE.get().map(String::as_str),
        std::env::var("OMNI_AGENT_ID").ok().as_deref(),
    )
}

/// The precedence, kept pure so tests never write the process-global or the
/// environment. Writing either from a test leaks into every other test sharing
/// the binary, which is the hazard this codebase has already paid for twice.
fn pipe_agent_id_from(from_flag: Option<&str>, from_env: Option<&str>) -> String {
    // The flag outranks the environment: it is evidence about *this* command,
    // while the environment only describes whichever shell happens to be
    // outermost.
    if let Some(agent) = from_flag.filter(|a| !a.trim().is_empty()) {
        return agent.to_string();
    }

    if let Some(agent) = from_env.filter(|a| !a.trim().is_empty()) {
        return agent.to_string();
    }

    crate::agents::multiagent::detect_agent_id()
}

/// The host whose pre-hook rewrote this command into `omni exec`, passed down as
/// `--agent` because the child inherits no evidence of who spawned it.
///
/// Without it the run guesses from ambient environment and is wrong in both
/// directions: under Gemini it filed as `claude_code` when a Claude session was
/// the outer shell, and as `terminal` once that was stripped, so Gemini rows
/// could never reach Agent Distribution (#360).
static HOST_THAT_REWROTE: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Records which host asked for this run. Later calls are ignored, so a command
/// that somehow carries the flag twice cannot change identity mid-run.
pub fn set_host_that_rewrote(agent: &str) {
    if !agent.trim().is_empty() {
        let _ = HOST_THAT_REWROTE.set(agent.to_string());
    }
}

/// Bytes from this run that reach a model's context.
///
/// On the exec and pipe paths OMNI writes to its own stdout, so whether anything
/// that bills tokens is on the other end is decided by what stdout is attached
/// to: a TTY means a human is reading and no context holds the result, while a
/// pipe or a captured tool result means something downstream does. That
/// distinction is the whole of #212's first finding — `agent_id='terminal'` rows
/// were 73.4% of every byte OMNI claimed to have saved, folded into one headline
/// with the rows that really are tokens.
///
/// `is_terminal()` is a property of the process, not a guess about the caller,
/// which is what makes this measurable rather than aspirational.
fn delivered_bytes(output_len: usize) -> usize {
    if std::io::stdout().is_terminal() {
        0
    } else {
        output_len
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

    if std::env::var("OMNI_OUTPUT_JSON").is_ok() {
        let tokens_saved = if result.input_text.len() > result.best_output().len() {
            (result.input_text.len() - result.best_output().len()) / 4
        } else {
            0
        };
        let json_meta = serde_json::json!({
            "route": result.route.to_string(),
            "hash": result.rewind_hash,
            "tokens_saved": tokens_saved
        });
        writeln!(
            error,
            "{}",
            serde_json::to_string(&json_meta).unwrap_or_default()
        )?;
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
            crate::cli::stats::format_bytes(result.input_text.len() as u64).bright_black(),
            crate::cli::stats::format_bytes(result.best_output().len() as u64).green(),
            elapsed.to_string().bright_black()
        );
        writeln!(error, "{} {}", "[OMNI Active]".bold().cyan(), msg)?;
    }

    Ok(())
}

fn detect_sibling_command() -> Option<String> {
    use std::process::Command;

    // 1. Get current IDs
    let pid = std::process::id();

    // 2. Get PGID (Process Group ID)
    let pgid_out = Command::new("ps")
        .args(["-o", "pgid=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    let pgid = String::from_utf8_lossy(&pgid_out.stdout).trim().to_string();

    // 3. Get PPID (Parent Process ID)
    let ppid_out = Command::new("ps")
        .args(["-o", "ppid=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    let ppid = String::from_utf8_lossy(&ppid_out.stdout).trim().to_string();

    // 4. Find all commands in that PGID
    let siblings_out = if !pgid.is_empty() {
        Command::new("ps")
            .args(["-o", "command=", "-g", &pgid])
            .output()
            .ok()
    } else {
        None
    };

    let siblings = siblings_out
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    // 5. Pass 1: Look for an active sibling (real process) in PGID
    for line in siblings.lines() {
        let line = line.trim();
        if line.is_empty() || line.contains("omni") {
            continue;
        }

        // Exclude common shells and ps itself
        if line.starts_with("ps ")
            || line.starts_with("sh ")
            || line.starts_with("zsh ")
            || line.starts_with("bash ")
            || line.starts_with("grep ")
        {
            continue;
        }

        // Found a candidate sibling command
        return Some(line.to_string());
    }

    // 6. Pass 2: Fallback to parsing shell command lines in the PGID
    for line in siblings.lines() {
        let line = line.trim();
        if (line.contains("sh ") || line.contains("zsh ") || line.contains("bash "))
            && line.contains('|')
            && line.contains("omni")
        {
            #[allow(clippy::collapsible_if)]
            if let Some(cmd) = extract_command_from_pipeline(line) {
                return Some(cmd);
            }
        }
    }

    // 7. Pass 3: Fallback to Parent Command if no sibling found
    // Useful if we are running in a script or cargo run
    if !ppid.is_empty() && ppid != "0" && ppid != "1" {
        let parent_cmd_out = Command::new("ps")
            .args(["-o", "command=", "-p", &ppid])
            .output()
            .ok()?;
        let parent_line = String::from_utf8_lossy(&parent_cmd_out.stdout)
            .trim()
            .to_string();

        if parent_line.contains('|') && parent_line.contains("omni") {
            #[allow(clippy::collapsible_if)]
            if let Some(cmd) = extract_command_from_pipeline(&parent_line) {
                return Some(cmd);
            }
        }
    }

    None
}

fn extract_command_from_pipeline(line: &str) -> Option<String> {
    // Split by pipe and find the segment immediately before "omni"
    let pipe_parts: Vec<&str> = line.split('|').collect();
    let omni_idx = pipe_parts.iter().position(|p| p.contains("omni"));

    if let Some(idx) = omni_idx {
        #[allow(clippy::collapsible_if)]
        if idx > 0 {
            let cmd_segment = pipe_parts[idx - 1];

            // Strip shell prefix if present (-c "...")
            let mut clean = if let Some(c_idx) = cmd_segment.find("-c ") {
                &cmd_segment[c_idx + 3..]
            } else {
                cmd_segment
            };

            // Handle command chains like: source ~/.zshrc && ls -la | omni
            if let Some(last_chain_idx) = clean.rfind(['&', ';']) {
                clean = &clean[last_chain_idx + 1..];
                clean = clean.trim_start_matches('&');
            }

            let final_cmd = clean.trim().to_string();
            if !final_cmd.is_empty() {
                return Some(final_cmd);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #360: Gemini's hook rewrote the command and the resulting run filed
    /// itself under the outer shell. The flag describes this command; the
    /// environment only describes whoever launched the terminal, so the flag
    /// has to win.
    #[test]
    fn the_rewriting_host_outranks_the_surrounding_environment() {
        assert_eq!(
            pipe_agent_id_from(Some("gemini"), Some("claude_code")),
            "gemini"
        );
    }

    /// A flag that arrived empty is not an identity, and neither is an empty
    /// `OMNI_AGENT_ID`; both must fall through rather than record a blank agent.
    #[test]
    fn falls_through_when_a_source_is_present_but_empty() {
        assert_eq!(
            pipe_agent_id_from(Some("  "), Some("codex_cli")),
            "codex_cli"
        );
        assert_ne!(pipe_agent_id_from(Some(""), Some("")), "");
    }

    #[test]
    fn passes_through_when_reduction_is_too_small() {
        let input_text = "a".repeat(1000);
        let output = "b".repeat(960); // 4% reduction only
        let res = PipelineResult {
            session_id: "s".to_string(),
            output,
            filter_name: "f".to_string(),
            rewind_hash: None,
            segments_kept: 0,
            segments_dropped: 0,
            input_text: input_text.clone(),
            start_time: Instant::now(),
            collapse_savings: None,
            project_path: ".".to_string(),
            route: Route::Keep,
        };

        assert_eq!(res.best_output(), input_text.as_str());
    }

    /// #271, on the other hook path. The pipe carried the same noise-ratio gate
    /// and the same empty archive, so fixing only `hooks::post_tool` would leave
    /// "everything cut is archived" false for every `omni exec` call.
    #[test]
    fn archives_the_raw_output_it_shortens() {
        // Arrange
        let mut input = String::new();
        for i in 0..200 {
            input.push_str(&format!("test module_{i} ... ok\n"));
        }
        input.push_str("test result: ok. 200 passed; 0 failed\n");
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("omni.db");
        let store = Arc::new(Store::open_path(&db).expect("store"));
        let mut out = Vec::new();
        let mut err = Vec::new();

        // Act
        run_inner(
            input.as_bytes(),
            &mut out,
            &mut err,
            Some(store),
            None,
            Some("cargo test"),
        )
        .expect("must succeed");

        // Assert
        let archived: i64 = rusqlite::Connection::open(&db)
            .expect("open recorded db")
            .query_row("SELECT COUNT(*) FROM rewind_store", [], |r| r.get(0))
            .expect("count");
        assert_eq!(archived, 1, "the raw output must be recoverable");

        let delivered = String::from_utf8(out).expect("utf8");
        assert!(
            delivered.len() < input.len(),
            "a lossy reply must be smaller than the bytes it replaces"
        );
        assert!(
            delivered.contains("omni_retrieve("),
            "the caller cannot call what it was not told: {delivered}"
        );
    }

    #[test]
    fn distills_git_diff() {
        let input = "diff --git a/foo b/foo\n@@ -1,1 +1,1 @@\n-old\n+new\n";
        let mut out = Vec::new();
        let mut err = Vec::new();

        run_inner(input.as_bytes(), &mut out, &mut err, None, None, None).expect("must succeed");

        let out_str = String::from_utf8(out).expect("must succeed");
        assert!(out_str.contains("diff --git"));
    }

    #[test]
    fn passes_through_short_input() {
        let input = "hello world\nthis is short";
        let mut out = Vec::new();
        let mut err = Vec::new();

        run_inner(input.as_bytes(), &mut out, &mut err, None, None, None).expect("must succeed");
        let out_str = String::from_utf8(out).expect("must succeed");

        assert_eq!(out_str, input);
    }

    /// #406. `docker.toml` is stream-mode, strips `^Copying blob ` and sets
    /// `on_empty`. The stream loop applies the filter one line at a time, so the
    /// whole-payload zero-state fired for every stripped line and each piece of
    /// noise came back as `docker: image operation completed successfully`.
    ///
    /// Both halves matter: the output grew where it was meant to shrink, and it
    /// asserted a successful image operation once per line it recognised nothing
    /// in.
    #[test]
    fn never_answers_the_payload_zero_state_for_a_single_stripped_line() {
        let input = "Resolved short name alpine\n\
                     Copying blob sha256:aaa111\n\
                     Copying blob sha256:bbb222\n\
                     Copying config sha256:ccc333\n\
                     done\n";
        let mut out = Vec::new();
        let mut err = Vec::new();

        run_inner(
            input.as_bytes(),
            &mut out,
            &mut err,
            None,
            None,
            Some("podman run --rm alpine true"),
        )
        .expect("must succeed");

        let out_str = String::from_utf8_lossy(&out);
        assert!(
            !out_str.contains("image operation completed successfully"),
            "a stripped line was reported as a completed operation: {out_str:?}"
        );
        assert!(
            out.len() <= input.len(),
            "stripping three lines grew the output: {out_str:?}"
        );
    }

    /// The zero-state still exists for the payload it was written for, or the
    /// fix above would have deleted a real feature instead of scoping it.
    #[test]
    fn still_answers_the_zero_state_for_a_whole_payload() {
        let filter = toml_filter::load_all_filters()
            .into_iter()
            .find(|f| f.matches("podman run --rm alpine true"))
            .expect("docker.toml must claim this command");

        assert_eq!(
            filter.apply("Copying blob sha256:aaa111\nCopying config sha256:ccc333"),
            "docker: image operation completed successfully"
        );
    }

    /// The pipe path shares batch TOML filtering with PostToolUse. When every
    /// row is stripped and there is no explicit fallback, it must emit the
    /// original bytes rather than falling through to another distiller.
    #[test]
    fn passes_through_a_batch_filter_that_removes_every_line() {
        let input = "would reformat src/main.py\n\
                     would reformat src/lib.py\n\
                     would reformat tests/test_main.py\n";
        let mut out = Vec::new();
        let mut err = Vec::new();

        run_inner(
            input.as_bytes(),
            &mut out,
            &mut err,
            None,
            None,
            Some("black --check ."),
        )
        .expect("must succeed");

        assert_eq!(String::from_utf8(out).expect("valid UTF-8"), input);
    }

    #[test]
    fn exit_0_is_always_treated_as_ok() {
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
