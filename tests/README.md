# OMNI Test Suite 🧪

Welcome to the OMNI testing infrastructure. This suite ensures the reliability of the Zig core (via WASM), the TypeScript MCP server, and the semantic distillation logic.

## Directory Structure

```text
tests/
├── fixtures/          # Realistic CLI outputs (Git, Docker, SQL, etc.)
├── filters/           # Unit tests for individual distillation filters
├── mcp/               # Integration tests for the MCP server
├── test-helper.js     # Wasm loading and memory management utility
└── test-semantic.mjs  # Core semantic routing verification
```

## Available Tests

### 1. Filter Unit Tests (`tests/filters/`)
These tests verify that each native Zig filter correctly matches, scores, and processes its target CLI output. They use the `test-helper.js` to run the actual Wasm binary.

### 2. MCP Integration Tests (`tests/mcp/`)
Verifies that the MCP server starts correctly and handles tool calls as expected.

### 3. Semantic Core Tests (`test-semantic.mjs`)
Validates the high-level routing logic (HIGH/GREY/NOISE) of the OMNI engine.

## Running Tests

### The Standard Way (Recommended)
This runs all filter, MCP, and semantic tests in one command.
```bash
make test
```

### via npm
```bash
# Run all unit and integration tests
npm test

# Run only semantic tests
npm run test:semantic
```

## Adding New Tests

1.  **Fixtures**: Add a new `.txt` file to `tests/fixtures/` with the raw CLI output you want to test.
2.  **Filter Test**: Create a new `.test.js` file in `tests/filters/`. Use `createOmniEngine()` from `../test-helper.js` to load the engine.
3.  **Run**: Execute `npm test` to verify your new test.

---
*Note: These tests require `node >= 20.0.0` for the native test runner and `zig` for building the Wasm core.*
