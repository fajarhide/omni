use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use omni::distillers;
use omni::pipeline::{classifier, scorer};
use std::time::Duration;

fn bench_classify(c: &mut Criterion) {
    let fixtures = [
        (
            "git_diff",
            include_str!("../tests/fixtures/git_diff_multi_file.txt"),
        ),
        (
            "cargo_build",
            include_str!("../tests/fixtures/cargo_build_errors.txt"),
        ),
        (
            "kubectl",
            include_str!("../tests/fixtures/kubectl_pods_mixed.txt"),
        ),
        (
            "nginx_log",
            include_str!("../tests/fixtures/nginx_access_log.txt"),
        ),
    ];

    for (name, input) in &fixtures {
        c.bench_with_input(BenchmarkId::new("classify", name), input, |b, i| {
            b.iter(|| classifier::classify(i, None))
        });
    }
}

fn bench_full_pipeline(c: &mut Criterion) {
    let input = include_str!("../tests/fixtures/cargo_build_errors.txt");

    c.bench_function("full_pipeline_cargo_build", |b| {
        b.iter(|| {
            let ctype = classifier::classify(input, None);
            let segments = scorer::score_segments(input, &ctype, None);
            let distiller = distillers::get_distiller(&ctype);
            distiller.distill(&segments, input, None)
        })
    });
}

fn bench_hook_roundtrip(c: &mut Criterion) {
    // Simulate complete PostToolUse cycle
    let large_input = include_str!("../tests/fixtures/git_diff_multi_file.txt").repeat(5); // ~50KB input

    c.bench_function("hook_roundtrip_50kb", |b| {
        b.iter(|| {
            let ctype = classifier::classify(&large_input, None);
            let segments = scorer::score_segments(&large_input, &ctype, None);
            let distiller = distillers::get_distiller(&ctype);
            distiller.distill(&segments, &large_input, None)
        })
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .measurement_time(Duration::from_secs(5))
        .warm_up_time(Duration::from_secs(2));
    targets = bench_classify, bench_full_pipeline, bench_hook_roundtrip
}
criterion_main!(benches);
