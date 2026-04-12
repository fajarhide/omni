# OMNI Integration for OpenClaw

This plugin integrates **OMNI** into the **OpenClaw** agent framework, allowing OpenClaw agents to intelligently filter terminal output, reducing token consumption by up to 90% while improving context quality.

## Prerequisites

- **OMNI** must be installed and available in your PATH.
- **OpenClaw** (Gateway) must be installed.

## Installation

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

Once installed, your OpenClaw agent will have access to two new tools:

### `omni_shell`
Use this exactly like the standard `shell` or `bash` tool. 
- **Input**: `{ "command": "npm install" }`
- **Behavior**: Runs the command via `omni exec`, filtering out noise and keeping only the signal (errors, summaries).

### `omni_rewind`
If OMNI omits a large block of text and provides a hash (e.g., `[OMNI: 847 lines omitted — hash: a3f8c2d1]`), the agent can call `omni_rewind` with that hash to see the full output.
- **Input**: `{ "hash": "a3f8c2d1" }`

## Monitoring Savings

You can track how many tokens and how much money the OpenClaw plugin is saving you by running the following command in your terminal:

```bash
omni stats --today
```

This will show a detailed breakdown of all commands processed for your OpenClaw session, including the signal reduction percentage.

## Benefits
- **Cheaper Tasks**: Massive savings on API bills for long-running autonomous tasks.
- **Higher Accuracy**: Agents focus on the real errors instead of being distracted by 10,000 lines of build logs.
- **Zero Information Loss**: The agent can always "rewind" to see the full raw logs if needed.
