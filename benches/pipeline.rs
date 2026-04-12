use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use omni::distillers;
use omni::pipeline::scorer;
use std::time::Duration;

fn bench_registry_resolve(c: &mut Criterion) {
    let commands = [
        ("git_diff", "git diff HEAD~1"),
        ("cargo_build", "cargo build"),
        ("kubectl", "kubectl get pods"),
    ];

    for (name, cmd) in &commands {
        c.bench_with_input(BenchmarkId::new("registry_resolve", name), cmd, |b, &i| {
            b.iter(|| scorer::score_with_command("", i, None))
        });
    }
}

fn bench_full_pipeline(c: &mut Criterion) {
    let input = include_str!("../tests/fixtures/cargo_build_errors.txt");

    c.bench_function("full_pipeline_cargo_build", |b| {
        b.iter(|| {
            let segments = scorer::score_with_command(input, "cargo build", None);
            distillers::distill_with_command(&segments, input, "cargo build", None)
        })
    });
}

fn bench_hook_roundtrip(c: &mut Criterion) {
    // Simulate complete PostToolUse cycle
    let large_input = include_str!("../tests/fixtures/git_diff_multi_file.txt").repeat(5); // ~50KB input

    c.bench_function("hook_roundtrip_50kb", |b| {
        b.iter(|| {
            let segments = scorer::score_with_command(&large_input, "git diff", None);
            distillers::distill_with_command(&segments, &large_input, "git diff", None)
        })
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .measurement_time(Duration::from_secs(5))
        .warm_up_time(Duration::from_secs(2));
    targets = bench_registry_resolve, bench_full_pipeline, bench_hook_roundtrip
}
criterion_main!(benches);
