use omni::cli::handoff::run_handoff;
use omni::pipeline::scorer;
use omni::store::sqlite::Store;
use std::sync::Arc;
use std::time::Instant;
use tempfile::tempdir;

#[test]
fn bench_distillation_latency() {
    let input = "a\n".repeat(100);
    let start = Instant::now();
    let segments = scorer::score_with_command(&input, "ls", None);
    omni::distillers::distill_with_command(&segments, &input, "ls", None);
    let elapsed = start.elapsed();

    // Latency should be < 2000ms (to account for unoptimized debug builds in CI)
    assert!(elapsed.as_millis() < 2000, "Distillation latency too high");
}

#[test]
fn bench_handoff_export_latency() {
    let dir = tempdir().unwrap();
    let store = Arc::new(Store::open_path(&dir.path().join("omni.db")).unwrap());
    let mut state = omni::pipeline::SessionState::new();
    state.session_id = "bench_session".to_string();
    store.upsert_session(&state);

    // Only time the actual handoff operation
    let start = Instant::now();
    let _ = run_handoff(&["--json".to_string()], store);
    let elapsed = start.elapsed();

    // Handoff should be < 50ms
    assert!(elapsed.as_millis() < 100, "Handoff latency too high");
}

#[test]
fn bench_pressure_calculation_latency() {
    let mut state = omni::pipeline::SessionState::new();
    state.estimated_current_tokens = 150_000;

    let start = Instant::now();
    for _ in 0..100 {
        state.recalculate_pressure();
    }
    let elapsed = start.elapsed();

    // Pressure recalc < 100ms
    assert!(elapsed.as_millis() < 100, "Pressure latency too high");
}

#[test]
fn bench_environment_sanitization_latency() {
    let start = Instant::now();
    let mut malicious_env = Vec::new();
    for i in 0..1000 {
        malicious_env.push((format!("BAD_VAR_{}", i), "evil".to_string()));
    }

    let _sanitized = omni::guard::env::sanitize_vars(malicious_env);
    let elapsed = start.elapsed();

    assert!(elapsed.as_millis() < 50, "Security scan too slow");
}
