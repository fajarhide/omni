# OMNI Tests

This directory contains integration tests, fixtures, and snapshot data for the OMNI Semantic Signal Engine.

## Structure

```
tests/
├── fixtures/              # 45 realistic tool output samples
│   ├── git_diff_multi_file.txt
│   ├── cargo_build_errors.txt
│   ├── pytest_failures.txt
│   └── ...
├── savings_assertions.rs  # Per-filter savings threshold tests
├── hook_e2e.rs            # Binary spawn E2E tests
├── security_tests.rs      # Security validation tests
└── smoke_test.sh          # Shell smoke test script
```

## Running Tests

```bash
# All tests (147 total)
cargo test

# Specific suites
cargo test --test hook_e2e            # 10 E2E tests
cargo test --test savings_assertions  # 4 savings tests
cargo test --test security_tests      # 6 security tests

# Snapshot tests
cargo test distillers::tests
cargo insta review                    # Review changes

# Smoke tests
chmod +x tests/smoke_test.sh
tests/smoke_test.sh ./target/debug/omni
```

## Adding a New Fixture

1. Save realistic CLI output to `tests/fixtures/my_tool_output.txt`
2. Reference it in a snapshot test or savings assertion
3. Run `cargo test` to verify
