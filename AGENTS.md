# AGENTS.md — Omni Multi-Agent Coordination Rules

Welcome to the OMNI project! If you are an AI Agent (Claude, Cursor, Copilot, or any other LLM), you MUST read and follow this document. This file acts as the primary synchronization point for multi-agent workflows and ensures that all AI systems adhere to the same development standards.

## 1. Core Architecture Principles

OMNI is a "Context Operating System" that intercepts, analyzes, and distills terminal outputs into high-signal information for AI Agents.

When contributing to this project, adhere strictly to the following principles:
- **Low Latency**: All hooks (`pre_tool`, `post_tool`, `pre_compact`, etc.) must execute in under 10ms. OMNI intercepts every command; overhead must be imperceptible.
- **Fail Open**: If an OMNI hook panics or errors, it must fail silently or gracefully degrade, allowing the original command execution or context gathering to proceed uninterrupted.
- **High Signal-to-Noise**: Distillers must ruthlessly trim noise while preserving the absolute minimum context required to diagnose an issue.

## 2. Development Gates (v1.0)

All code changes must pass the strict OMNI Development Gates before being considered complete:

1. **Compilation**: `cargo check` and `cargo build` must succeed without warnings.
2. **Formatting**: `cargo fmt --all -- --check` must pass.
3. **Linting**: `cargo clippy --all-targets --all-features -- -D warnings` must pass. **Zero warnings allowed**.
4. **Testing**: `cargo test --all` must pass. Snapshot tests (`cargo insta`) must be updated if output structures change intentionally.

Before finalizing any task, run `make fmt && make clippy && cargo test --all` to verify compliance.

## 3. Omni Context Lifecycle Management

OMNI implements advanced context management features. As an AI working on OMNI, you are subject to the rules you are helping build:
- **Context Pressure System**: OMNI actively estimates the token budget of the session. If the token count exceeds 65% (Warning) or 82% (Critical), OMNI will inject pressure warnings into your context. **Do NOT ignore these warnings**. Summarize your tasks and avoid unnecessary data retrieval when under pressure.
- **Critical File Pinning**: Files like `AGENTS.md` and `CLAUDE.md` are automatically pinned to the session context to ensure you never lose sight of project instructions during aggressive session compaction.
- **File Re-Read Guard**: Do not run redundant `cat` or `grep` commands on files you have already read. OMNI tracks hot files and will warn you if you attempt redundant reads or mutate a highly accessed file blindly.

## 4. Multi-Agent Awareness

OMNI is developed concurrently by multiple agents using the **Grandmaster v4.0** blueprint. 
- **Modularity**: Do not couple independent modules. The pipeline, distillers, and store should remain isolated via well-defined interfaces.
- **Telemetry**: OMNI records agent execution paths. Do not attempt to bypass `store::sqlite` or `store::transcript` logging mechanisms.
- **Workflow Usage**: Use `/slash-commands` to invoke workflows located in `.agents/workflows/` when handling specific development or release tasks.

## 5. Artifacts and Tool Calling

- **Tool Selection**: Always use the most specific tool available (e.g., use `replace_file_content` over `run_command` with `sed`).
- **Artifacts**: Use markdown artifacts to present structured plans, analysis, or test results when the response is lengthy. 

## Remember: 
You are developing a tool meant to make AI agents (like yourself) exponentially more efficient. Keep the codebase clean, the binary fast, and the signal pure.
