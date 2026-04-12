use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use omni::distillers;
use omni::pipeline::scorer;
use std::time::Duration;

fn bench_classify(c: &mut Criterion) {
    let commands = [
        ("git_diff", "git diff HEAD~1"),
        ("cargo_build", "cargo build"),
        ("kubectl", "kubectl get pods"),
        ("nginx_log", "cat /var/log/nginx/access.log"),
    ];

    for (name, cmd) in &commands {
        c.bench_with_input(BenchmarkId::new("classify", name), cmd, |b, &i| {
            b.iter(|| scorer::command_to_content_type(i))
        });
    }
}

fn bench_full_pipeline(c: &mut Criterion) {
    let input = include_str!("../tests/fixtures/cargo_build_errors.txt");

    c.bench_function("full_pipeline_cargo_build", |b| {
        b.iter(|| {
            let (segments, ctype) = scorer::score_with_command(input, "cargo build", None);
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
            let (segments, ctype) = scorer::score_with_command(&large_input, "git diff", None);
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
