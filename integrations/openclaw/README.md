# OMNI Semantic Signal Engine for OpenClaw

This plugin provides a secure bridge between your OpenClaw agent and the [OMNI Signal Engine](https://github.com/fajarhide/omni). 

> [!IMPORTANT]
> **Dependency Required**: This plugin is a wrapper and requires the `omni` CLI binary to be installed on your local system path.

## Setup

## Prerequisites

- **OMNI** must be installed and available in your PATH.
- **OpenClaw** (Gateway) must be installed.

### Installation

**Option A: via ClawHub (Recommended)**
```bash
clawhub install omni-signal-engine
```

**Option B: Manual Install**
1. Clone or copy the OMNI repository.
2. Navigate to the OMNI directory.
3. Install the plugin into OpenClaw:

```bash
openclaw plugins install ./integrations/openclaw
```

## Configuration (Optional)

**OMNI for OpenClaw is designed to be "Zero Config".** If `omni` is already in your `PATH`, it will work immediately after installation without any additional settings.

If you have a custom setup, you can modify your OpenClaw settings (`~/.openclaw/config.yaml`):

```yaml
plugins:
  omni-signal-engine:
    omniPath: "/usr/local/bin/omni"  # Optional: path to omni binary
    forceDistill: false             # Optional: experimental override
```

## Usage

Once installed, your OpenClaw agent will have access toD dua new tools:

### `omni_cmd`
Use this exactly like the standard `shell` or `bash` tool. 
- **Input**: `{ "command": "npm install" }`
- **Behavior**: Runs the command via `omni exec`, filtering out noise and keeping only the signal (errors, summaries).

### `omni_rewind`
If OMNI omits a large block of text and provides a hash (e.g., `[OMNI: 847 lines omitted — hash: a3f8c2d1]`), the agent can call `omni_rewind` with that hash to see the full output.
- **Input**: `{ "hash": "a3f8c2d1" }`

## Monitoring Savings

You can track your token and cost savings by running:

```bash
omni stats --today
```

## Security & Privacy (Trust Model)

OMNI is built with a **Privacy-First** design intent. This plugin acts as a secure proxy to facilitate safe execution:

- **Node-Level Sanitization**: This plugin explicitly strips ~25 dangerous environment variables (like `LD_PRELOAD`, `NODE_OPTIONS`, `BASH_ENV`) at the process level before execution.
- **Local-Only Architecture**: The OMNI engine is designed for local processing. No terminal output is ever sent to external cloud services by the engine.
- **Local Persistence**: Usage statistics and archived contexts are stored strictly in a local SQLite database at `~/.omni/omni.db`.
- **Trust & Verification**: As OMNI is a tool for developers, we encourage you to audit the full source code and security policies at the [OMNI GitHub Repository](https://github.com/fajarhide/omni/blob/main/SECURITY.md) to verify these claims for yourself.

## Benefits
- **Cheaper Tasks**: Massive savings on API bills for long-running autonomous tasks.
- **Higher Accuracy**: Agents focus on the real errors instead of being distracted by 10,000 lines of build logs.
- **Zero Information Loss**: The agent can always "rewind" to see the full raw logs if needed.
