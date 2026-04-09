# Security Policy

## Supported Versions

Security updates are applied to the latest release on the `main` branch.

| Version | Supported          |
| ------- | ------------------ |
| v0.5.x  | :white_check_mark: |
| v0.4.x  | :x:                |
| v0.3.x  | :x:                |
| v0.2.x  | :x:                |

## Reporting a Vulnerability

We take the security of OMNI seriously. If you discover a vulnerability, you can report it to us through one of the following channels:

1. **GitHub Issues (Recommended)**: Create an issue at [https://github.com/fajarhide/omni/issues](https://github.com/fajarhide/omni/issues). *(If the vulnerability is critical, you can also use GitHub's private vulnerability reporting feature if enabled on the repository).*
2. **Email us**: Send a detailed report to [security@weekndlabs.com](mailto:security@weekndlabs.com).

For either method, please include:
- A description of the issue.
- Steps to reproduce.
- Potential impact.

We will acknowledge your report within 48 hours and provide a timeline for a fix.

## Security Considerations

- **Local-only processing**: OMNI processes all data locally. No data is sent to external servers during distillation.
- **Local SQLite Persistence**: Usage stats and archived contexts are stored locally in the SQLite database `~/.omni/omni.db`. **No data ever leaves your machine.**
- **MCP Server**: The MCP server runs locally via `stdio` transport and does not expose any network ports.
- **`omni update`**: Only reads the public GitHub Releases API (no authentication required) to download the latest binary. No data is uploaded.

---

## 1. Project Trust Boundary

OMNI will **not** load project-local configurations or custom TOML filters (inside `.omni/filters/`) until you explicitly trust the project. This prevents a malicious repository from injecting custom filter rules that could hide important output from your AI agent.

### How it Works

OMNI uses `omni_config.json` as the trust anchor for a repository. 

```
 Your Project/
 ├── omni_config.json   ← OMNI sees this but WON'T load filters unless trusted
 ├── .omni/filters/     ← Local custom rules
 └── ...

 ~/.omni/
 └── trusted-projects.json  ← Trust registry (path + SHA-256 hash)
```

1. OMNI detects project-local configurations.
2. It checks `~/.omni/trusted-projects.json` for the project path **and** a matching SHA-256 hash of the `omni_config.json` anchor file.
3. If not found or hash doesn't match → **local configs & filters are skipped**, OMNI logs a warning.
4. If trusted and hash matches → configs and local `.omni/filters/` are loaded normally.

### Quick Start

**Trust a project for the first time:**
```bash
omni trust
```
Or call the `omni_trust` MCP tool manually from Claude Code.

The tool will:
- Display the config contents for your review.
- Show the SHA-256 fingerprint.
- Add the project to `~/.omni/trusted-projects.json`.

**After editing your local config:**
```bash
omni trust
```
Run it again to re-verify and update the hash.

> [!IMPORTANT]
> If you modify `omni_config.json` after trusting, OMNI will **stop loading project filters** until you re-trust. This protects against silent repo tampering.

### Trust Flow

| Scenario | OMNI Behavior |
| :--- | :--- |
| No local config exists | Global and Built-in filters only (normal) |
| Local config exists, **not trusted** | Skipped. Logs: `⚠ Local config not trusted. Run omni trust to review and trust.` |
| Local config exists, **trusted** | Loaded and merged with global configs |
| Local config **modified** after trust | Skipped. Logs: `⚠ Local config modified since last trust. Run omni trust to re-verify.` |

---

## 2. Sandbox Environment Denylist

OMNI **strips ~25 dangerous environment variables** from child processes it manages (e.g., when routing commands through `omni exec`). This prevents environment-based attacks where malicious env vars could hijack command execution.

### Why This Matters

Some environment variables can inject code into any process that reads them:

| Variable | Risk |
| :--- | :--- |
| `BASH_ENV` / `ENV` | Shell runs this file **before** executing any command |
| `NODE_OPTIONS` | Injects flags/code into every Node.js process |
| `LD_PRELOAD` | Loads a shared library into **every** process (Linux) |
| `DYLD_INSERT_LIBRARIES` | Same as `LD_PRELOAD` (macOS) |
| `PYTHONSTARTUP` | Python executes this file on startup |
| `JAVA_TOOL_OPTIONS` | Injects JVM arguments into every Java process |

### What OMNI Blocks

All commands that are wrapped by OMNI (e.g., `omni exec <cmd>`) receive a **sanitized** copy of `process.env` with these categories removed:

- **Shell injection**: `BASH_ENV`, `ENV`, `ZDOTDIR`, `BASH_PROFILE`, `PROMPT_COMMAND`, `IFS`, etc.
- **Runtime hijacking**: `NODE_OPTIONS`, `PYTHONSTARTUP`, `RUBYOPT`, `JAVA_TOOL_OPTIONS`
- **Dynamic linker**: `LD_PRELOAD`, `LD_LIBRARY_PATH`, `DYLD_INSERT_LIBRARIES`, `DYLD_FORCE_FLAT_NAMESPACE`
- **Path manipulation**: `PYTHONPATH`, `PYTHONHOME`, `RUBYLIB`
- **Git injection**: `GIT_ASKPASS`, `GIT_EXEC_PATH`, `GIT_TEMPLATE_DIR`

> [!NOTE]
> This is transparent — you don't need to configure anything. OMNI automatically sanitizes the environment to protect command executions.

---

## Security Tools Summary

| Tool | Purpose | When to Use |
| :--- | :--- | :--- |
| `omni trust` | Trust a project's local configurations | After cloning a repo with custom filters, or after editing the config anchor |

---

Thank you for helping keep OMNI secure!
