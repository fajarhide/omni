# Installing OMNI

OMNI is designed to be installed locally and integrated into your MCP environment.

## One-Line Installation (Universal)

If you have **Zig 0.15.2** and **Node.js 18+** installed, you can set up OMNI in one step:

```bash
curl -fsSL https://raw.githubusercontent.com/fajarhide/omni/main/install.sh | sh
```

## Homebrew

Install OMNI via Homebrew:

```bash
brew install fajarhide/tap/omni
```

## Manual Installation

If you prefer to install manually:

1. **Clone the Repository**:
   ```bash
   git clone https://github.com/fajarhide/omni.git
   cd omni
   ```

2. **Compile OMNI**:
   OMNI uses the standard Zig build system. Build native CLI + Wasm Edge.
   ```bash
   zig build -Doptimize=ReleaseFast -p 
   ```

3. **Verify via Native CLI**:
   ```bash
   ./bin/omni report
   ./bin/omni setup
   ```

## Integration with AI Agents

OMNI is compatible with any tool that supports the **Model Context Protocol (MCP)**.

### Claude Code / Antigravity
The OMNI CLI is for humans, but **`omni-mcp`** is for your AI. It allows Claude or Antigravity to use OMNI's distillation tools automatically.

To connect OMNI to Claude Code:
```bash
claude config add mcp omni omni-mcp
```

> [!NOTE]
> When starting, you might see an `ExperimentalWarning: WASI`. This is expected! OMNI uses high-performance WebAssembly (WASI) at its core, which Node.js currently labels as experimental. It is completely safe to use.

### Cursor / Windsurf / VS Code Agents
1. Go to **Settings** or **MCP Configuration**.
2. Add a new server with the following details:
   - **Name**: `omni`
   - **Type**: `stdio` or `command`
   - **Command**: `omni-mcp`

### Generic MCP Agents
For any other agent, ensure the `node` environment is available and point the transport to OMNI's entry point: `/path/to/omni/dist/index.js`.

## Dependencies

- **Zig 0.15.2+**: Required for the high-performance core.
- **Node.js 18+**: Required for the MCP gateway.
- **Git**: Required for the installer script.
