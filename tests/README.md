# OMNI Test Suite 🧪

This directory contains the verification and testing tools for OMNI.

## Available Tests

### 1. Semantic Verification Suite (`test-semantic.mjs`)
Validates that the OMNI engine correctly routes and distills text based on signal density thresholds (HIGH, GREY, NOISE). This test uses the Wasm core via Node.js WASI.

## Running Tests

You can run individual tests or the full suite using the following commands:

### Via npm:
```bash
# Run all tests
npm test

# Run only semantic tests
npm run test:semantic
```

### Via Makefile:
```bash
# Run official verification suite
make test
```

## Future Tests
New unit tests for specific filter logic (Zig) or MCP server functionality (TypeScript) should be added to this directory.
