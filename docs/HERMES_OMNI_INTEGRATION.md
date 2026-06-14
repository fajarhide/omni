# OMNI + Hermes Agent — Integration Best Practices

> Authoritative source for getting maximum value from OMNI while running
> Hermes Agent.  
> Complements `omni init --hermes` by documenting the *why*, the recommended
> defaults, how to verify, and the edge cases that trip users up.

---

## 1. What this integration actually does

| Layer | Mechanism | What changes |
|---|---|---|
| **Hooks** | `~/.hermes/plugins/omni-signal-engine/__init__.py` → `omni --pre-hook`, `--post-hook`, `--session-start` | Terminal tool outputs are distilled *before* they enter Hermes context. |
| **MCP** | `mcp_servers.omni = ["/opt/homebrew/bin/omni", "--mcp"]` | 27 OMNI tools (`omni_insight`, `omni_retrieve`, `omni_history`, …) become first-class Hermes tools. |
| **Shared state** | SQLite store at `~/.omni/omni.db` | Cross-session knowledge, rewind breadcrumbs, token savings telemetry. |

Hermes itself is not fork-modified; the bridge is purely via the plugin entry
point and MCP stdio transport.

---

## 2. Prerequisites

```bash
# 1. OMNI binary
brew install fajarhide/tap/omni

# 2. Verify
omni --version          # expect v0.6.0+
omni doctor             # all green

# 3. Hermes venv python (needed for `hermes plugins enable`)
export HERMES_VENV="${HERMES_HOME:-$HOME/.hermes}/hermes-agent/venv"
export HERMES_PY="$HERMES_VENV/bin/python"
"$HERMES_PY" --version   # expect 3.11+
```

---

## 3. Installation (idempotent)

`omni init --hermes` now performs a fully automated setup. It will:
- install the `omni-signal-engine` plugin scaffold in `~/.hermes/plugins/omni-signal-engine/`,
- register the OMNI MCP server in `~/.hermes/config.yaml` if it is not already present and enable Hermes `compression` when safe to do so,
- write Hermes-optimized OMNI defaults to `~/.omni/config.toml` (`mode = "efficient"`, pinned files, etc.) **without overwriting an existing OMNI config**.

```bash
# Single command setup
omni init --hermes

# Enable the plugin (required once)
hermes plugins enable omni-signal-engine

# Restart Hermes to load the plugin and MCP server
hermes gateway restart
```

Finally, install the Hermes entry point into the Hermes venv so the plugin resolves:

```bash
"$HERMES_PY" -m pip install hermes-omni-plugin
```

> Use either `hermes-omni-plugin` or the `omni init --hermes` scaffold, not both at once,
> to avoid duplicate plugin registrations.

---

## 4. Recommended Hermes config additions

```yaml
# ~/.hermes/config.yaml

# 4a. Make sure the plugin is enabled
plugins:
  enabled:
    - omni-signal-engine

# 4b. Register OMNI MCP server (paste hasil `omni init --hermes` / `hermes mcp add`)
mcp_servers:
  omni:
    command: "/opt/homebrew/bin/omni"
    args: ["--mcp"]
    env:
      OMNI_AGENT_ID: "hermes"

# 4c. Enable Hermes context compression so OMNI's pressure warnings line up
compression:
  enabled: true
  threshold: 0.50    # compress at 50 % context usage
  target_ratio: 0.20 # keep 20 % of original
```

Minimal config checklist:
- ✅ `plugins.enabled` contains `omni-signal-engine`
- ✅ `mcp_servers.omni` points to the real `omni` binary
- ✅ `compression.enabled = true`

---

## 5. Verification checklist

```bash
# 1. OMNI happy path
omni doctor

# 2. Plugin is loaded
hermes plugins list | grep omni
# expect: omni-signal-engine enabled

# 3. Hermes sees 27 OMNI tools (after restart)
hermes tools list | grep mcp_omni_

# 4. Quick functional test
cat /Users/fajarhide/project/06_PERSONAL_WORKSPACE/token-efficient/omni/tests/fixtures/cargo_test_500.txt | \
  omni --post-hook 2>&1 | head -20
# expect: passaing test lines stripped, failures preserved
```

For a full live test, start a fresh Hermes session and run a noisy command
through Hermes' `terminal` tool:

```text
Run a command that produces > 5 000 tokens of output:
  terminal("npm install", timeout=120)
Then inspect the session; tool result size should be materially smaller than
raw npm output.  Confirm with:
  omni stats
```

---

## 6. Best practices

### 6a. Lean on OMNI for *high-noise* tool calls only

| Typical noise | Typical signal | Action |
|---|---|---|
| `npm install`, `cargo build`, `docker build` | Dependency warnings, cache hits, progress bars | OMNI shines — expect 70–99 % savings |
| `cargo test` / `pytest` passing suite | 1 failing test buried in 10 000 lines | OMNI surfaces failure + stack, drops the rest |
| `kubectl get pods`, `terraform plan` | Healthy rows vs CrashLoopBackOff | OMNI keeps unhealthy rows, drops healthy |
| `cat src/` (file dumps) | Imports, API shapes, risk markers | OMNI reshape → outline; do *cat* through agent that calls `read_file` since OMNI only hooks terminal stdout |

> OMNI is not a replacement for `read_file` / `grep`; it is a terminal-output
> conditioner. If you structured information through files, rely on Hermes'
> built-in file tools and let OMNI handle shell noise.

### 6b. Use the MCP tools as Hermes' "OMNI controls"

After MCP is registered, prompt Hermes to:

- `omni_insight` — recurring errors across the session
- `omni_history` — what got compressed and why
- `omni_retrieve <hash>` — pull the raw unfiltered output of a previous tool call
- `omni_session --status` — hot files, active errors, token pressure
- `omni_stop` analog: set env `OMNI_PASSTHROUGH=1` for one command when you need 100 % raw terminal output

Treat those tools as observability into OMNI itself; they exist so you don't
have to wonder "did OMNI just drop something important?".

### 6c. Tune by project, not globally

```yaml
# ~/.omni/config.toml (Hermes projects tend to read large logs and test outputs)

[global]
aggressiveness = "balanced"

[agents.hermes]
aggressiveness = "aggressive"   # Hermes sessions are long; lean into compression
enable_readfile_distillation = true
```

`aggressiveness` accepted values: `conservative`, `balanced`, `aggressive`.

### 6d. Pin the files OMNI keeps warm

```yaml
# optional: ~/.omni/config.toml
[pinned_files]
paths = [
  "AGENTS.md",
  ".omni/CONTEXT.md",
  "CLAUDE.md",
]
```

Pinned files are **not** dropped during aggressive session compaction in
Hermes. Use this for project rules the agent must never lose.

### 6e. Watch context pressure, don't ignore it

When OMNI emits `[omni:context pressure: WARNING]` or `[omni:context pressure: CRITICAL]`:

1. Pause new tool calls that dump large outputs (`npm install`, full-file reads).
2. Use Hermes existing slash `omni_session --status` or ask the agent to call `omni_session "context"`.
3. Do not re-run the same heavy command hoping for different output; OMNI's
   `active_errors` already tracks *why* the last iteration failed.

Hermes' own `compression` feature kicks in at `threshold: 0.50`, but OMNI
reports pressure independently. Keeping both on gives early warnings + the
actual compaction, rather than a single tripwire.

### 6f. Multi-agent workflows (Hermes + Claude Code or Cursor side-by-side)

```bash
# Each agent sets OMNI_AGENT_ID on its own hook/MCP registration:
#   Hermes: OMNI_AGENT_ID=hermes        (via plugin + mcp env)
#   Claude: OMNI_AGENT_ID=claude        (via `omni init --claude`)
#   Cursor: OMNI_AGENT_ID=cursor        (via `omni init --cursor`)
```

Result: one `~/.omni/omni.db`, but session attribution + active-error lists
are per-agent, and `omni_agents` shows who else is active on the same
workspace.

### 6g. Learning from Hermes usage

```bash
# Feed Hermes terminal traces into OMNI's auto-learn
omni learn
```

Output is TOML filter proposals for `~/.omni/signals/`. Review and copy into
`~/.omni/signals/` rather than editing built-in signals. This is where
project-specific commands (internal CLIs, bespoke deploy scripts) get tuned.

---

## 7. Common pitfalls and how to avoid them

| Symptom | Probable cause | Fix |
|---|---|---|
| Hermes tools output is *longer* after enabling OMNI | `omni doctor --fix` to trust project `.omni/filters/`; otherwise project-level signals override global | Run `omni doctor --fix`, restart Hermes |
| `[OMNI: omitted X lines]` shows up but tool result is empty/short | Input was under the 95 % reduction guardrail; OMNI passthrough raw | Expected — harmless; do not double-filter |
| `omni_retrieve` returns empty | Raw output not in RewindStore (streaming distillers skip rewind by default) | Fall back to `omni_history` for the same command |
| Gateway restart doesn't load plugin | Plugin directory is `~/.hermes/plugins/…` *without* trailing slash; or ~/.hermes ownership mismatch | `hermes plugins enable omni-signal-engine` fixes directory; then `hermes gateway restart` |
| `hermes mcp add` times out | `connect_timeout` too short; switch to YAML config instead | See §4b |
| `OMNI_PASSTHROUGH=1` has no effect in Hermes terminal tool | Hermes cleans env before tool calls; inject via `terminal(env_passthrough=[OMNI_PASSTHROUGH], ...)` | Toggle per-call instead of global export |

---

## 8. Performance budget

Guaranteed by OMNI's Rust release profile on modern Mac/Linux:

- Pipeline per-tool latency: **< 100 ms** (including binary startup)
- Streaming pipeline: **memory flat** even for 10 000-line logs
- Fail-open: any hook error is swallowed; raw terminal output passes through

Do not add heavy I/O *inside* Hermes' `post_tool_call` plugin handler
— `subprocess.run(..., capture_output=True)` is already the cheapest possible
boundary. Avoid reading files, network calls, or LLM calls in the plugin.

---

## 9. Ops checklist after Hermes updates

1. `omni doctor` — confirm hooks still registered
2. `hermes plugins list` — `omni-signal-engine` still enabled
3. `omni stats` — savings telemetry is still recording
4. If a Hermes release changes hook lifecycles, re-run `omni init --hermes --hook`
   to rewrite the plugin scaffold without touching MCP or config.

---

## 10. Decision tree

```text
Q: Should I keep OMNI on?
├─ Output is pure noise (npm install, cargo build)      → ALWAYS ON
├─ Output contains secrets / tokens (env, kubeconfig)   → ALWAYS ON + OMNI_PASSTHROUGH=0
├─ Output is sequential logs I read top-to-bottom       → ALWAYS ON
├─ Output is JSON / YAML I want intact                  → OMNI may downsample; set env per-call
└─ Output is small (< 2 KB)                             → OMNI bypasses automatically (no overhead)
```

---

## 11. References

- OMNI docs: `docs/HOW_TO_USE.md`, `docs/LOOP_ENGINEERING.md`
- Hermes skill: `skill_view(name="hermes-agent")`
- Hermes native MCP: `skill_view(name="hermes-agent", file_path="references/native-mcp.md")`
- OMNI source: `src/hooks/pipe.rs`, `src/agents/hermes.rs`
