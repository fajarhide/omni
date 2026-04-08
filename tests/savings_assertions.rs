use omni::distillers;
/// Savings threshold assertions — each distiller must achieve minimum token reduction.
///
/// This integration test runs the full pipeline (classify → score → compose) on real
/// fixture files and asserts each achieves a minimum savings percentage.
use omni::pipeline::{classifier, scorer};
use std::time::Instant;
fn run_pipeline(input: &str) -> (usize, usize, f64) {
    let ctype = classifier::classify(input, None);
    let segments = scorer::score_segments(input, &ctype, None);

    // Use the actual distiller logic from post_tool.rs
    let distiller = omni::distillers::get_distiller(&ctype);
    let output = distiller.distill(&segments, input, None);

    let input_len = input.len();
    let output_len = output.len();
    let savings_pct = if input_len > 0 {
        100.0 * (1.0 - output_len as f64 / input_len as f64)
    } else {
        0.0
    };
    (input_len, output_len, savings_pct)
}

/// Fixtures paired with: (filter_name, path, min_savings_pct_if_large_enough)
/// Small fixtures (<500 bytes) may not achieve significant reduction, so we skip threshold
/// assertion for those and just verify no-crash + valid output.
const FIXTURES: &[(&str, &str, f64)] = &[
    ("git", "tests/fixtures/git_diff_multi_file.txt", 50.0),
    ("git", "tests/fixtures/git_status_dirty.txt", 70.0),
    ("build", "tests/fixtures/cargo_build_errors.txt", 70.0),
    ("test", "tests/fixtures/pytest_failures.txt", 85.0),
    ("infra", "tests/fixtures/kubectl_pods_mixed.txt", 50.0),
    ("infra", "tests/fixtures/docker_build_layered.txt", 80.0),
    ("infra", "tests/fixtures/heavy_noise.txt", 90.0),
];

#[test]
fn test_savings_thresholds() {
    for (filter, fixture, min_pct) in FIXTURES {
        let input = std::fs::read_to_string(fixture)
            .unwrap_or_else(|_| panic!("Cannot read fixture: {}", fixture));
        let (input_len, output_len, actual_pct) = run_pipeline(&input);
        println!(
            "| {:<10} | {:>9} B | {:>10} B | {:>10.1}% |",
            filter, input_len, output_len, actual_pct
        );

        // Always verify: output should not be larger than input + small overhead
        assert!(
            output_len <= input_len + 100,
            "{} on {}: output ({}) should not massively exceed input ({})",
            filter,
            fixture,
            output_len,
            input_len
        );

        // For files > 500 bytes, check savings threshold
        if input_len > 500 && *min_pct > 0.0 {
            assert!(
                actual_pct >= *min_pct,
                "{} on {}: expected >= {:.0}% savings, got {:.1}% (input={}, output={})",
                filter,
                fixture,
                min_pct,
                actual_pct,
                input_len,
                output_len
            );
        }
    }
}

#[test]
fn test_all_fixtures_produce_nonempty_output() {
    let fixture_dir = std::fs::read_dir("tests/fixtures").unwrap();
    for entry in fixture_dir {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().map(|e| e == "txt").unwrap_or(false) {
            let input = std::fs::read_to_string(&path).unwrap();
            if input.is_empty() {
                continue;
            }
            let (_, output_len, _) = run_pipeline(&input);
            // Pipeline should never produce completely empty output from non-empty input
            // (at minimum it passes through or produces a summary)
            assert!(
                output_len > 0 || input.trim().is_empty(),
                "Fixture {:?} produced empty output from {} bytes input",
                path.file_name().unwrap(),
                input.len()
            );
        }
    }
}

#[test]
fn test_short_input_not_over_expanded() {
    let short = "hello world";
    let (input_len, output_len, _) = run_pipeline(short);
    // Short input should never expand significantly
    assert!(
        output_len <= input_len + 50,
        "Short input expanded from {} to {} bytes",
        input_len,
        output_len
    );
}

#[test]
fn test_empty_input_no_crash() {
    let (_, output_len, _) = run_pipeline("");
    assert_eq!(output_len, 0);
}

#[test]
fn test_pipeline_latency_under_50ms_debug() {
    // Debug mode bisa 3-5x lebih lambat dari release
    // Release claim: <10ms -> debug threshold: <50ms
    let input = include_str!("../tests/fixtures/git_diff_multi_file.txt").repeat(3); // ~30KB input

    let start = Instant::now();
    let ctype = classifier::classify(&input, None);
    let segments = scorer::score_segments(&input, &ctype, None);
    let distiller = distillers::get_distiller(&ctype);
    distiller.distill(&segments, &input, None);
    let elapsed = start.elapsed();

    assert!(
        elapsed.as_millis() < 50,
        "Pipeline took {}ms in debug mode (should be <50ms; release target is <10ms)",
        elapsed.as_millis()
    );
}

#[test]
fn test_classifier_latency_under_5ms_debug() {
    let input = include_str!("../tests/fixtures/cargo_build_errors.txt").repeat(5);

    let start = Instant::now();
    for _ in 0..10 {
        classifier::classify(&input, None);
    }
    let elapsed = start.elapsed();
    let avg_ms = elapsed.as_millis() / 10;

    assert!(
        avg_ms < 5,
        "Classifier avg {}ms (should be <5ms debug, <1ms release)",
        avg_ms
    );
}

#[test]
fn test_hook_no_panic_on_large_input() {
    // Safety: 500KB input harus tidak crash
    let large = "error: cannot find type\n".repeat(20000);

    let ctype = classifier::classify(&large, None);
    let segments = scorer::score_segments(&large, &ctype, None);

    let distiller = omni::distillers::get_distiller(&ctype);
    let output = distiller.distill(&segments, &large, None);

    assert!(!output.is_empty());
}
