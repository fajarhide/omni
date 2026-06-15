# OMNI + Hermes Autonomous Maker-Checker Loop

This guide demonstrates how to configure Hermes Agent to execute a Level-4 Autonomous Loop (Maker-Checker pattern) natively utilizing OMNI's advanced MCP tools (`omni_loop_memory`, `omni_knowledge`, `omni_retrieve`, `omni_learn`).

## 1. Prerequisites

Ensure OMNI is installed and Hermes integration is active:
```bash
omni init --hermes
hermes plugins enable omni-signal-engine
hermes gateway restart
```

## 2. Launching Hermes with Loop Control

When executing an autonomous loop, you should explicitly set the loop tracking variables so that OMNI's SessionStart and PreCompact hooks can embed checkpointing information directly into the Context Pressure warnings.

```bash
export OMNI_LOOP_ID="hermes-maker-checker-001"
export OMNI_LOOP_GOAL="Refactor the authentication module and fix all related tests."
export OMNI_LOOP_BUDGET=500000

# Start Hermes session
hermes session start
```

## 3. The Hermes System Prompt (Agent Rules)

In your Hermes project settings (or initial prompt), instruct the agent on how to use the MCP tools to govern its own lifecycle.

```markdown
You are an autonomous Maker-Checker agent. You will execute tasks in a continuous loop until the goal is achieved. OMNI Context OS is attached to your session to optimize your context.

### Loop Management Rules:
1. **Checkpointing:** At the start of a major phase (e.g., "Starting backend refactor"), call the `mcp_omni_loop_memory` tool with `action="set"` to record your current intent. This memory will survive if your context is compacted.
2. **Context Pressure:** If you see `[omni:context pressure: WARNING]` injected into the output, you must stop reading new large files and immediately call `mcp_omni_session` with `action="status"` to review active errors.
3. **Retrieving Details:** If you run a noisy test command and see `[OMNI: omitted X lines]`, and you need the exact stack trace, use `mcp_omni_retrieve` with the provided Hash.
4. **Learning Project Patterns:** If a custom internal script (e.g., `./bin/deploy.sh`) generates extreme noise that OMNI doesn't filter perfectly, call `mcp_omni_learn` to ask OMNI to generate a project-specific TOML filter.
5. **Knowledge Transfer:** When you successfully deduce the architecture of a complex module, use `mcp_omni_knowledge` with `action="store"` to save it globally.
```

## 4. Execution Trace Example

1. **Agent decides to run tests:**
   - *Hermes:* `terminal("cargo test --all")`
   - *OMNI Hook:* Intercepts output, strips 50,000 lines of passing tests, returns 1 failure.
   
2. **Agent encounters Context Pressure:**
   - *OMNI Hook:* Detects budget usage > 80%. Returns `[omni:context pressure: WARNING]`.
   - *Hermes Plugin:* Catches the warning and automatically calls `ctx.compact()` to compress history.
   - *OMNI PreCompact Hook:* Saves `OMNI_LOOP_GOAL` and current `loop_memory` as an Engram so Hermes doesn't lose its train of thought.

3. **Agent utilizes `omni_retrieve`:**
   - *OMNI:* `... [OMNI: omitted 500 lines of Webpack build noise. Hash: a1b2c3d4] ...`
   - *Hermes:* Needs to see the specific Webpack warning. Calls `mcp_omni_retrieve(hash="a1b2c3d4")`.
   - *OMNI:* Returns the raw 500 lines directly via the MCP channel.

By following this workflow, Hermes remains highly autonomous while OMNI acts as its intelligent Context OS, preventing context window bloat during loops > 100 iterations.
