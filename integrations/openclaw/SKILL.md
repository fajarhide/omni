---
id: omni-signal-engine
name: OMNI Semantic Signal Engine
version: 0.5.8
author: Fajar Hidayat
description: Local-only semantic context filtering that saves up to 90% in token costs.
tags: [efficiency, context, cost-saving, terminal, token-efficient]
---

# OMNI Semantic Signal Engine

**Less noise. More signal. Cut your AI token consumption by up to 90%.**

This skill provides a secure proxy to OMNI's powerful semantic distillation engine. It facilitates the filtering of terminal output from noisy commands, ensuring your agent focuses on critical information.

> [!IMPORTANT]
> **Dependency Required**: This skill is a proxy and requires the [OMNI CLI binary](https://github.com/fajarhide/omni) to be installed on your local system path.

## Tools Included

### `omni_cmd`
Execute terminal commands and development tools (npm, git, cargo, etc.) through OMNI. Use this for all your standard development tasks to keep your context window lean and your API bills low.

### `omni_rewind`
Retrieve full archived logs if OMNI's distilled summary was too aggressive.

## Security & Trust (Design Intent)
This skill is designed as a secure bridge to the local OMNI binary. To maintain transparency regarding its security properties:
- **Node-Level Sanitization**: The plugin explicitly strips ~25 dangerous environment variables at the process level before execution (e.g., `LD_PRELOAD`, `NODE_OPTIONS`, `BASH_ENV`).
- **Safe Process Spawning**: Uses `execFile` with an atomic argument array, bypassing shell string parsing and reducing injection risks.
- **Local-Only Architecture**: OMNI is designed as a local-first engine. No terminal data or telemetry is exfiltrated by the engine during normal operation.
- **Data Persistence**: Archives for the `omni_rewind` feature are stored strictly on your local device in a SQLite database (`~/.omni/omni.db`).
- **Audit Invitation**: Both the plugin code and the OMNI binary source are open-source. We encourage security-conscious users to audit the code at the [OMNI GitHub Repository](https://github.com/fajarhide/omni).
