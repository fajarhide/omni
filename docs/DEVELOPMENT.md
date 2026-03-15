# OMNI Development Guide 🛠

Welcome to the Project OMNI development guide. This document outlines how to maintain the core engine and expand its semantic filtering capabilities.

## 🏗 Architecture Overview

OMNI consists of three main components:
1.  **Zig Core (Native CLI)**: A high-performance binary (`omni`) providing diagnostic and distillation subcommands.
2.  **Zig Core (Wasm)**: A portable version of the engine for MCP edge integration.
3.  **TypeScript Host**: The MCP gateway that orchestrates Wasm execution with an integrated LRU cache.

## 🌈 Adding a New Filter

To add a new semantic filter:

1.  **Create a Filter Module**: Add a new `.zig` file in `core/src/filters/`.
    ```zig
    const std = @import("std");
    const Filter = @import("interface.zig").Filter;

    pub const MyNewFilter = struct {
        pub fn filter() Filter {
            return .{
                .name = "my_filter",
                .ptr = undefined,
                .matchFn = match,
                .processFn = process,
            };
        }

        fn match(_: *anyopaque, input: []const u8) bool {
            return std.mem.indexOf(u8, input, "keyword") != null;
        }

        fn process(_: *anyopaque, allocator: std.mem.Allocator, input: []const u8) ![]u8 {
            // Your logic here
            return try allocator.dupe(u8, "summarized output");
        }
    };
    ```

2.  **Register the Filter**:
    - Add the import and register in `core/src/main.zig` (for native).
    - Add the import and register in `core/src/wasm.zig` (for WebAssembly).

3.  **Update Interface**: If the filter requires shared state, use the `ptr` field and cast it within your functions.

## 🕸 WebAssembly Bridge

OMNI uses a custom-packed `u64` return to communicate between Zig and the JavaScript host.
- **High 32 bits**: Length of the result.
- **Low 32 bits**: Memory pointer (relative to Wasm memory).

When modifying the `compress` export in `wasm.zig`, ensure that both memory and string encodings are correctly handled on the TypeScript side (`src/index.ts`).

## 🧪 Testing

Run native engine unit tests:
```bash
zig build test
```

Verify CLI performance and stability:
```bash
./bin/omni bench 1000
./bin/omni report
```

This will produce a small, optimized `.wasm` binary suitable for edge distribution.

## 🚀 Official Release Workflow

To release a new version of OMNI and update the Homebrew tap:

1.  **Update `CHANGELOG.md`**: Add the new version and its changes.
2.  **Run the Release Script**:
    ```bash
    ./scripts/omni-release.sh 0.2.1
    ```
    This script will:
    - Update the version and SHA256 in `omni.rb`.
    - Tag and push the current commit.
    - Fetch the new archive and update the checksum.
    - Sync the changes to the `homebrew-omni` repository.

3.  **Manual Check**: Verify the release at [GitHub Releases](https://github.com/fajarhide/omni/releases).
