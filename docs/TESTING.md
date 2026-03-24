# OMNI Testing Architecture & Quality Assurance

OMNI aims to be an **invisible, lightning-fast, and hyper-reliable** native binary that runs automatically between your AI agent and your terminal. Because OMNI aggressively filters and intercepts standard output, our testing strategy must guarantee **absolute Context Safety** (zero data loss for critical signals) and **Native Latency** (<5ms).

This document outlines the testing methodology that guarantees OMNI's performance. You can run all of these tests simultaneously via `make ci` or `cargo test --all`.

---

## 1. Unit Tests (135+ Tests)
**Coverage:** Core Engine, Scorer, Composer, SQLite Store, Session Tracker  
**Goal:** Ensure the fundamental algorithms and data storage work perfectly in isolation.

- **Regex & Classifiers:** Tests verify that OMNI's heuristics correctly identify whether a line is a `Warning`, `Error`, `Compiling`, or `Context` token.
- **SQLite Persistence:** Validates that `SessionState` and `RewindStore` data can be correctly indexed, saved, and retrieved via FTS5 Full Text Search without blocking the main execution thread.
- **Pipeline Composer:** Ensures that the composer perfectly reconstructs the final output, respecting maximum character limits (`MAX_OUTPUT_CHARS`) while retaining original line order.

## 2. TOML Integration Tests (45+ Tests)
**Path:** `tests/toml_filter_integration.rs`  
**Goal:** Validate that all 20+ ecosystem-specific filters correctly match their intended commands and never accidentally match unrelated commands.

- **Positive/Negative Matching:** Ensures `npm run build` is matched by `npm.toml` but `npm_audit.toml` only triggers for `npm audit`.
- **Context Safety Guarantee:** These tests run against strict, complex mock outputs (inline tests) to mathematically guarantee that **compiler errors, stack traces, and diff assertions are never stripped**.
  - Example: Validates that Rust `-->` compilation pointers and Vitest `+ Expected` diffs always score 1.0 (Critical) and survive the distillation pipeline.

## 3. Security & Boundary Tests (10 Tests)
**Path:** `tests/security_tests.rs`  
**Goal:** Ensure OMNI never crashes the user's terminal or exposes sensitive environment variables.

- **Env Sanitization:** Validates that `BASH_ENV`, `LD_PRELOAD`, and other dangerous environment hooks are stripped before OMNI executes nested commands.
- **Panic Catching:** Proves that even if the distillation engine panics, the `Dispatcher` uses `catch_unwind` to gracefully print the original unedited output rather than crashing the user's workflow.
- **Payload Extremes:** Tests that injecting Null Bytes (`\0`), malformed JSON, and excessively long lines (16MB+) are handled cleanly without memory leaks or crashes.

## 4. End-to-End Hook Simulations (10 Tests)
**Path:** `tests/hook_e2e.rs`  
**Goal:** Simulate how Claude Code and Bash actually interact with the OMNI binary.

- **Pre-Hook Rewriting:** Verifies that commands like `git diff` correctly get prepended with `omni exec` to bypass Claude's auto-truncation.
- **Pipe Mode:** Simulates `cargo test | omni` by writing to `stdin` and asserting that the `Pipe` engine correctly acts as a transparent passthrough without hanging the terminal.

## 5. Performance Latency Assertions (7 Tests)
**Path:** `tests/savings_assertions.rs`  
**Goal:** Hard-coded physical limits preventing performance regressions.

- **Latency `<50ms`:** Fails the build if the entire pipeline (read, classify, score, compose, write) takes longer than 50ms in debug mode (in Release mode, it averages `<2ms`).
- **Token Reduction >50%:** Asserts that given standard noisy outputs (like a raw `npm install` log), OMNI successfully drops at least 50% of the payload size while maintaining context.

## 6. Smoke Tests (Bash Script)
**Path:** `tests/smoke_test.sh`  
**Goal:** Compile the production binary (`cargo build --release`) and physically execute it against a live Bash shell to verify exit codes and output formats.

- Validates `omni doctor`, `omni stats`, `omni session`, and `omni learn`.
- Checks that `omni stats` correctly prints UI charts and terminal ANSI colors.
- Proves that the binary size remains highly optimized (<5MB).

---

> By running `make ci`, any contributor guarantees that their code changes preserve the core OMNI promise: **Lightning fast, 100% context safe, and entirely invisible.**
