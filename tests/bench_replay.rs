//! #184 — the reproducible net-savings benchmark.
//!
//! Replays every `execution_traces.raw_input` through the CURRENT pipeline and
//! aggregates raw vs distilled bytes, so the published headline (docs/PERFOMANCE.md,
//! README) can be re-measured on the shipped binary rather than trusted from a run
//! nobody kept. This is the committed reproducer the README's "Numbers you can
//! reproduce" promises.
//!
//! Ignored by default — it needs a populated trace DB. Run it explicitly:
//!
//!   OMNI_BENCH_DB=~/.omni/omni.db \
//!     cargo test --release --test bench_replay -- --ignored --nocapture
//!
//! Faithfulness to docs/PERFOMANCE.md's method:
//! - `session: None` + `store: None` → the scorer sees no history, i.e. the
//!   "fresh HOME per invocation" the method requires (a warm DB is non-deterministic).
//! - `HOME` is pointed at an empty temp dir so only the embedded signals load, not
//!   whatever `~/.omni/signals` the measuring machine happens to carry.
//! - `run_inner` is the same full pipeline (format gate → TOML → distill → guardrail)
//!   the hook and `omni exec` run, so this measures what an agent actually receives.

use std::collections::BTreeMap;
use std::io::Cursor;

fn base_command(cmd: &str) -> String {
    cmd.split_whitespace()
        .find(|t| !t.contains('=') && !t.starts_with('-'))
        .unwrap_or("")
        .rsplit('/')
        .next()
        .unwrap_or("")
        .to_string()
}

#[test]
#[ignore = "needs a populated trace DB; run with --ignored (see file header)"]
fn replay_execution_traces_net_savings() {
    // Resolve the corpus DB from the REAL home before we repoint HOME below.
    // Build the path with PathBuf, not a hardcoded `/` (CLAUDE.md cross-platform
    // rule 1) — review of #202.
    let db = std::env::var("OMNI_BENCH_DB").unwrap_or_else(|_| {
        std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default())
            .join(".omni")
            .join("omni.db")
            .to_string_lossy()
            .into_owned()
    });

    // Match the method: no user config, no passthrough shortcut.
    let tmp_home = tempfile::tempdir().expect("temp home");
    // SAFETY: this integration binary holds exactly one test, so no other test
    // reads the environment concurrently while these mutations run (review of
    // #202). If a second test is ever added here, serialize env access first.
    unsafe {
        std::env::set_var("HOME", tmp_home.path());
        std::env::remove_var("OMNI_PASSTHROUGH");
    }

    if !std::path::Path::new(&db).exists() {
        eprintln!("SKIP: no trace DB at {db} (set OMNI_BENCH_DB)");
        return;
    }

    let conn = rusqlite::Connection::open(&db).expect("open trace db");
    // Two populations, because the difference between them is the number this
    // project keeps having to correct. `terminal` rows are TTY output no model
    // ever receives, and on the reporting installation they are 888 traces
    // carrying 86 MB, 68% of the corpus by bytes. Replaying everything reported
    // **79.1%** where the model-facing population reports **43.1%**, and the
    // README quoted the harness. #212 fixed exactly this in `omni stats`, which
    // now prints "Terminal output is excluded" on its own line; the harness that
    // produces the published figure never got the same fix (#324).
    //
    // Both are printed rather than one being chosen, so the gap stays visible
    // instead of being a decision someone has to remember.
    let fetch = |sql: &str| -> Vec<(String, String)> {
        let mut stmt = conn.prepare(sql).expect("prepare");
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .expect("query")
            .filter_map(Result::ok)
            .collect()
    };

    let model_facing = fetch(
        "SELECT command, raw_input FROM execution_traces \
         WHERE agent_id IS NOT NULL AND agent_id != 'terminal'",
    );
    let everything = fetch("SELECT command, raw_input FROM execution_traces");
    let total_traces = everything.len();
    let use_everything = std::env::var("OMNI_BENCH_ALL").is_ok() || model_facing.is_empty();
    let rows = if use_everything {
        everything
    } else {
        model_facing
    };
    let population = if rows.len() == total_traces {
        "every trace, terminal included, which is not the figure to publish"
    } else {
        "traces whose result reached a model, terminal excluded per #212"
    };

    assert!(!rows.is_empty(), "trace DB has no rows to replay");
    assert!(
        rows.iter().any(|(_, r)| !r.is_empty()),
        "all trace rows have empty raw_input — nothing to measure"
    );

    let (mut n, mut raw_total, mut out_total) = (0u64, 0u64, 0u64);
    let (mut shrank, mut unchanged, mut grew, mut errored) = (0u64, 0u64, 0u64, 0u64);
    // base command -> (calls, raw_bytes, out_bytes)
    let mut per_cmd: BTreeMap<String, (u64, u64, u64)> = BTreeMap::new();

    for (cmd, raw) in &rows {
        let mut out = Vec::new();
        let mut err = std::io::sink();
        // None store + None session = fresh, deterministic, no persistence. A
        // failed replay would leave `out` empty and be counted as 100% savings,
        // inflating the honest figure (review of #202) — exclude it entirely and
        // report the count instead.
        if omni::hooks::pipe::run_inner(
            Cursor::new(raw.as_bytes()),
            &mut out,
            &mut err,
            None,
            None,
            Some(cmd),
        )
        .is_err()
        {
            errored += 1;
            continue;
        }

        let (r, o) = (raw.len() as u64, out.len() as u64);
        n += 1;
        raw_total += r;
        out_total += o;
        match o.cmp(&r) {
            std::cmp::Ordering::Less => shrank += 1,
            std::cmp::Ordering::Equal => unchanged += 1,
            std::cmp::Ordering::Greater => grew += 1,
        }
        let e = per_cmd.entry(base_command(cmd)).or_default();
        e.0 += 1;
        e.1 += r;
        e.2 += o;
    }

    assert!(
        n > 0,
        "every replayed trace errored ({errored} of {})",
        rows.len()
    );
    let net = 100.0 * (raw_total - out_total.min(raw_total)) as f64 / raw_total.max(1) as f64;
    let pct = |part: u64| 100.0 * part as f64 / n as f64;

    println!("\n=== #184 net-savings replay (current pipeline) ===");
    println!("population:        {population}");
    println!("corpus:            {n} traces from {db} ({errored} errored, excluded)");
    println!("bytes:             {raw_total} -> {out_total}");
    println!("NET SAVINGS:       {net:.1}%");
    println!(
        "saved nothing:     {:.1}% ({} passthrough + {} grew)",
        pct(unchanged + grew),
        unchanged,
        grew
    );
    println!("actually shrank:   {:.1}% ({shrank})", pct(shrank));
    println!("ADDED BYTES:       {grew} calls");
    println!("\ntop commands by input bytes:");
    let mut cmds: Vec<_> = per_cmd.into_iter().filter(|(k, _)| !k.is_empty()).collect();
    cmds.sort_by_key(|(_, v)| std::cmp::Reverse(v.1));
    println!(
        "{:<12} {:>7} {:>14} {:>14} {:>8}",
        "command", "calls", "input", "output", "saved"
    );
    for (cmd, (calls, r, o)) in cmds.into_iter().take(15) {
        let saved = if r > 0 {
            100.0 * (r - o.min(r)) as f64 / r as f64
        } else {
            0.0
        };
        println!("{cmd:<12} {calls:>7} {r:>14} {o:>14} {saved:>7.1}%");
    }
    println!();

    // The one invariant that must hold: OMNI never adds bytes across the corpus.
    assert!(
        out_total <= raw_total,
        "OMNI added bytes across the corpus: {raw_total} -> {out_total}"
    );
}
