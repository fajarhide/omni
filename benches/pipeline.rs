use criterion::{Criterion, black_box, criterion_group, criterion_main};
use std::time::Duration;

use omni::pipeline::{ContentType, classifier, composer, scorer};

// ─── Fixtures ───────────────────────────────────────────

const GIT_DIFF: &str = include_str!("../tests/fixtures/git_diff_multi_file.txt");
const GIT_STATUS: &str = include_str!("../tests/fixtures/git_status_dirty.txt");
const CARGO_BUILD: &str = include_str!("../tests/fixtures/cargo_build_errors.txt");
const PYTEST: &str = include_str!("../tests/fixtures/pytest_failures.txt");
const KUBECTL: &str = include_str!("../tests/fixtures/kubectl_pods_mixed.txt");
const DOCKER: &str = include_str!("../tests/fixtures/docker_build_layered.txt");

/// Generate a ~100KB input by repeating a fixture
fn make_100kb_input() -> String {
    let base = GIT_DIFF;
    let repeats = (100_000 / base.len()) + 1;
    base.repeat(repeats)
}

// ─── Classify Benchmarks ────────────────────────────────

fn bench_classify(c: &mut Criterion) {
    let mut group = c.benchmark_group("classify");
    group.measurement_time(Duration::from_secs(3));

    group.bench_function("git_diff_397B", |b| {
        b.iter(|| classifier::classify(black_box(GIT_DIFF)))
    });

    group.bench_function("git_status_496B", |b| {
        b.iter(|| classifier::classify(black_box(GIT_STATUS)))
    });

    group.bench_function("cargo_build_317B", |b| {
        b.iter(|| classifier::classify(black_box(CARGO_BUILD)))
    });

    group.bench_function("pytest_730B", |b| {
        b.iter(|| classifier::classify(black_box(PYTEST)))
    });

    group.bench_function("kubectl_386B", |b| {
        b.iter(|| classifier::classify(black_box(KUBECTL)))
    });

    // 100KB input
    let big = make_100kb_input();
    group.bench_function("100KB_input", |b| {
        b.iter(|| classifier::classify(black_box(&big)))
    });

    group.finish();
}

// ─── Scorer Benchmarks ──────────────────────────────────

fn bench_scorer(c: &mut Criterion) {
    let mut group = c.benchmark_group("scorer");
    group.measurement_time(Duration::from_secs(3));

    let ctype = classifier::classify(GIT_DIFF);
    group.bench_function("git_diff_score", |b| {
        b.iter(|| scorer::score_segments(black_box(GIT_DIFF), black_box(&ctype), None))
    });

    let ctype = classifier::classify(PYTEST);
    group.bench_function("pytest_score", |b| {
        b.iter(|| scorer::score_segments(black_box(PYTEST), black_box(&ctype), None))
    });

    let big = make_100kb_input();
    let ctype = classifier::classify(&big);
    group.bench_function("100KB_score", |b| {
        b.iter(|| scorer::score_segments(black_box(&big), black_box(&ctype), None))
    });

    group.finish();
}

// ─── Full Pipeline Benchmarks ───────────────────────────

fn run_full_pipeline(input: &str) -> String {
    let ctype = classifier::classify(input);
    let segments = scorer::score_segments(input, &ctype, None);
    let config = composer::ComposeConfig::default();
    let (output, _) = composer::compose(segments, None, &config, None, input, &ctype);
    output
}

fn bench_full_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_pipeline");
    group.measurement_time(Duration::from_secs(5));

    group.bench_function("git_diff_397B", |b| {
        b.iter(|| run_full_pipeline(black_box(GIT_DIFF)))
    });

    group.bench_function("cargo_build_317B", |b| {
        b.iter(|| run_full_pipeline(black_box(CARGO_BUILD)))
    });

    group.bench_function("pytest_730B", |b| {
        b.iter(|| run_full_pipeline(black_box(PYTEST)))
    });

    group.bench_function("kubectl_386B", |b| {
        b.iter(|| run_full_pipeline(black_box(KUBECTL)))
    });

    group.bench_function("docker_309B", |b| {
        b.iter(|| run_full_pipeline(black_box(DOCKER)))
    });

    // 100KB stress test
    let big = make_100kb_input();
    group.bench_function("100KB_stress", |b| {
        b.iter(|| run_full_pipeline(black_box(&big)))
    });

    group.finish();
}

// ─── Hook Roundtrip Benchmark ───────────────────────────

fn bench_hook_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("hook_roundtrip");
    group.measurement_time(Duration::from_secs(5));

    // Simulate PostToolUse: parse JSON → classify → score → compose → serialize
    let mock_input = serde_json::json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": "git diff HEAD~1"},
        "tool_response": {
            "content": GIT_DIFF
        }
    });
    let json_str = mock_input.to_string();

    group.bench_function("parse_classify_compose_397B", |b| {
        b.iter(|| {
            let parsed: serde_json::Value = serde_json::from_str(black_box(&json_str)).unwrap();
            let content = parsed["tool_response"]["content"].as_str().unwrap_or("");
            let output = run_full_pipeline(content);
            let _ = serde_json::json!({
                "hookSpecificOutput": {
                    "updatedResponse": output
                }
            });
        })
    });

    // 100KB roundtrip
    let big = make_100kb_input();
    let big_input = serde_json::json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": "cargo test"},
        "tool_response": {
            "content": big
        }
    });
    let big_json = big_input.to_string();

    group.bench_function("parse_classify_compose_100KB", |b| {
        b.iter(|| {
            let parsed: serde_json::Value = serde_json::from_str(black_box(&big_json)).unwrap();
            let content = parsed["tool_response"]["content"].as_str().unwrap_or("");
            let output = run_full_pipeline(content);
            let _ = serde_json::json!({
                "hookSpecificOutput": {
                    "updatedResponse": output
                }
            });
        })
    });

    group.finish();
}

// ─── Distiller Benchmarks ──────────────────────────────

fn bench_distillers(c: &mut Criterion) {
    use omni::distillers;

    let mut group = c.benchmark_group("distillers");
    group.measurement_time(Duration::from_secs(3));

    // Git distiller
    let ctype = ContentType::GitDiff;
    let segments = scorer::score_segments(GIT_DIFF, &ctype, None);
    let distiller = distillers::get_distiller(&ctype);
    group.bench_function("git_distill", |b| {
        b.iter(|| distiller.distill(black_box(&segments), black_box(GIT_DIFF)))
    });

    // Build distiller
    let ctype = ContentType::BuildOutput;
    let segments = scorer::score_segments(CARGO_BUILD, &ctype, None);
    let distiller = distillers::get_distiller(&ctype);
    group.bench_function("build_distill", |b| {
        b.iter(|| distiller.distill(black_box(&segments), black_box(CARGO_BUILD)))
    });

    // Infra distiller
    let ctype = ContentType::InfraOutput;
    let segments = scorer::score_segments(KUBECTL, &ctype, None);
    let distiller = distillers::get_distiller(&ctype);
    group.bench_function("infra_distill", |b| {
        b.iter(|| distiller.distill(black_box(&segments), black_box(KUBECTL)))
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_classify,
    bench_scorer,
    bench_full_pipeline,
    bench_hook_roundtrip,
    bench_distillers,
);
criterion_main!(benches);
