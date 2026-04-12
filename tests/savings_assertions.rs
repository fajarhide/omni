/// Savings threshold assertions — each distiller must achieve minimum token reduction.
///
/// This integration test runs the full pipeline (classify → score → compose) on real
/// fixture files and asserts each achieves a minimum savings percentage.
use omni::pipeline::scorer;
use std::time::Instant;
fn run_pipeline(input: &str, command: &str) -> (usize, usize, f64) {
    let (segments, ctype) = scorer::score_with_command(input, command, None);

    // Use the actual distiller logic from post_tool.rs
    let output = omni::distillers::distill_with_command(&segments, input, command, &ctype, None);

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
const FIXTURES: &[(&str, &str, f64, &str)] = &[
    (
        "git",
        "tests/fixtures/git_diff_multi_file.txt",
        50.0,
        "git diff",
    ),
    (
        "git",
        "tests/fixtures/git_status_dirty.txt",
        70.0,
        "git status",
    ),
    (
        "build",
        "tests/fixtures/cargo_build_errors.txt",
        70.0,
        "cargo build",
    ),
    ("test", "tests/fixtures/pytest_failures.txt", 85.0, "pytest"),
    (
        "infra",
        "tests/fixtures/kubectl_pods_mixed.txt",
        50.0,
        "kubectl get pods",
    ),
    (
        "infra",
        "tests/fixtures/docker_build_layered.txt",
        80.0,
        "docker build",
    ),
    (
        "infra",
        "tests/fixtures/heavy_noise.txt",
        90.0,
        "docker build",
    ), // infra fallback
];

#[test]
fn test_savings_thresholds() {
    for (filter, fixture, min_pct, command) in FIXTURES {
        let input = std::fs::read_to_string(fixture)
            .unwrap_or_else(|_| panic!("Cannot read fixture: {}", fixture));
        let (input_len, output_len, actual_pct) = run_pipeline(&input, command);
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
            let (_, output_len, _) = run_pipeline(&input, "git status"); // Generic non-empty dummy
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
    let (input_len, output_len, _) = run_pipeline(short, "echo");
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
    let (_, output_len, _) = run_pipeline("", "echo");
    assert_eq!(output_len, 0);
}

#[test]
fn test_pipeline_latency_under_50ms_debug() {
    // Debug mode bisa 3-5x lebih lambat dari release
    // Release claim: <10ms -> debug threshold: <50ms
    let input = include_str!("../tests/fixtures/git_diff_multi_file.txt").repeat(3); // ~30KB input

    let start = Instant::now();
    let (segments, ctype) = scorer::score_with_command(&input, "git diff", None);
    omni::distillers::distill_with_command(&segments, &input, "git diff", &ctype, None);
    let elapsed = start.elapsed();

    assert!(
        elapsed.as_millis() < 50,
        "Pipeline took {}ms in debug mode (should be <50ms; release target is <10ms)",
        elapsed.as_millis()
    );
}

#[test]
fn test_hook_no_panic_on_large_input() {
    // Safety: 500KB input harus tidak crash
    let large = "error: cannot find type\n".repeat(20000);

    let (segments, ctype) = scorer::score_with_command(&large, "cargo test", None);

    let output =
        omni::distillers::distill_with_command(&segments, &large, "cargo test", &ctype, None);

    assert!(!output.is_empty());
}

#[test]
fn test_command_to_content_type_cargo_build() {
    use omni::pipeline::ContentType;
    use omni::pipeline::scorer::command_to_content_type;
    assert_eq!(
        command_to_content_type("cargo build"),
        ContentType::BuildOutput
    );
    assert_eq!(
        command_to_content_type("cargo test --release"),
        ContentType::TestOutput
    );
    assert_eq!(
        command_to_content_type("git diff HEAD~1"),
        ContentType::GitDiff
    );
    assert_eq!(
        command_to_content_type("git branch -a"),
        ContentType::GitStatus
    );
    assert_eq!(command_to_content_type("docker ps"), ContentType::Cloud);
    assert_eq!(command_to_content_type("ls -la"), ContentType::SystemOps);
    assert_eq!(
        command_to_content_type("/usr/bin/git status"),
        ContentType::GitStatus
    );
    assert_eq!(command_to_content_type(""), ContentType::Unknown);
}

#[test]
fn test_score_with_command_returns_segments() {
    use omni::pipeline::scorer::score_with_command;
    let input = "error[E0382]: use of moved value\n   --> src/main.rs:10:5\nCompiling omni v0.5.6";
    let (segments, ctype) = score_with_command(input, "cargo build", None);
    assert!(!segments.is_empty());
    // Error line harus Critical
    let has_critical = segments
        .iter()
        .any(|s| s.tier == omni::pipeline::SignalTier::Critical);
    assert!(has_critical, "Error line harus jadi Critical");
    // ContentType harus BuildOutput
    assert_eq!(ctype, omni::pipeline::ContentType::BuildOutput);
}

#[test]
fn test_omni_stats_shows_command_not_content_type() {
    use omni::pipeline::{ContentType, DistillResult, Route};
    use omni::store::sqlite::Store;
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let store = Store::open_path(&dir.path().join("omni.db")).unwrap();

    let result = DistillResult {
        output: "cargo build: ok".to_string(),
        route: Route::Keep,
        filter_name: "cargo".to_string(), // v0.5.6: command base
        content_type: ContentType::BuildOutput,
        score: 0.9,
        context_score: 0.0,
        input_bytes: 1000,
        output_bytes: 100,
        latency_ms: 5,
        rewind_hash: None,
        segments_kept: 2,
        segments_dropped: 8,
        collapse_savings: None,
    };

    store.record_distillation("sess_1", &result, "cargo build --release");

    let stats = store.get_per_command_stats(0, 10).unwrap();
    assert!(!stats.is_empty());
    let (cmd, count, _) = &stats[0];
    assert!(
        cmd.contains("cargo"),
        "Command column harus berisi command asli, got: {}",
        cmd
    );
    assert_eq!(*count, 1);
}
