const std = @import("std");
const build_options = @import("build_options");
const compressor = @import("compressor.zig");
const Filter = @import("filters/interface.zig").Filter;
const GitFilter = @import("filters/git.zig").GitFilter;
const BuildFilter = @import("filters/build.zig").BuildFilter;
const DockerFilter = @import("filters/docker.zig").DockerFilter;
const SqlFilter = @import("filters/sql.zig").SqlFilter;
const NodeFilter = @import("filters/node.zig").NodeFilter;
const CustomFilter = @import("filters/custom.zig").CustomFilter;

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();

    // Initialize Filter Registry
    var filters = std.ArrayList(Filter).empty;
    defer filters.deinit(allocator);

    try filters.append(allocator, GitFilter.filter());
    try filters.append(allocator, BuildFilter.filter());
    try filters.append(allocator, DockerFilter.filter());
    try filters.append(allocator, SqlFilter.filter());
    try filters.append(allocator, NodeFilter.filter());

    // Load Custom Rules
    var custom_filter_to_deinit: ?*CustomFilter = null;
    defer if (custom_filter_to_deinit) |c| c.deinit();

    const custom_init = CustomFilter.init(allocator, "omni_config.json");
    if (custom_init) |custom| {
        custom_filter_to_deinit = custom;
        try filters.append(allocator, custom.filter());
    } else |_| {}

    const args = try std.process.argsAlloc(allocator);
    defer std.process.argsFree(allocator, args);

    if (args.len > 1) {
        const cmd = args[1];
        if (std.mem.eql(u8, cmd, "-h") or std.mem.eql(u8, cmd, "--help") or std.mem.eql(u8, cmd, "help")) {
            try printHelp();
            return;
        } else if (std.mem.eql(u8, cmd, "-v") or std.mem.eql(u8, cmd, "--version") or std.mem.eql(u8, cmd, "version")) {
            try std.fs.File.stdout().deprecatedWriter().print("OMNI Core v{s} (Zig)\n", .{build_options.version});
            return;
        } else if (std.mem.eql(u8, cmd, "density")) {
            try handleDensity(allocator, filters.items);
            return;
        } else if (std.mem.eql(u8, cmd, "report")) {
            try handleReport(allocator, filters.items);
            return;
        } else if (std.mem.eql(u8, cmd, "bench")) {
            var iterations: usize = 100;
            if (args.len > 2) {
                iterations = std.fmt.parseInt(usize, args[2], 10) catch 100;
            }
            try handleBench(allocator, iterations, filters.items);
            return;
        } else if (std.mem.eql(u8, cmd, "generate")) {
            const agent = if (args.len > 2) args[2] else "general";
            try handleGenerate(agent);
            return;
        } else if (std.mem.eql(u8, cmd, "setup")) {
            try handleSetup();
            return;
        }
    }

    // Default: Distill from stdin
    try handleDistill(allocator, filters.items);
}

fn printHelp() !void {
    const help_text =
        \\OMNI Native Core - Semantic Distillation Engine 🌌
        \\
        \\Usage:
        \\  omni [subcommand] [options]
        \\
        \\Subcommands:
        \\  distill          Distill input from stdin (default)
        \\  density          Analyze context density gain
        \\  report           Show unified system & performance report
        \\  bench [N]        Benchmark performance (default 100 iterations)
        \\  generate [agent] Generate template input_file for AI agents
        \\  setup            Show detailed setup and usage instructions
        \\
        \\Examples:
        \\  cat log.txt | omni
        \\  omni density < draft.txt
        \\  omni generate claude-code > .omni-input
        \\
        \\OMNI is designed to be used as a filter in your agentic pipelines.
        \\
    ;
    try std.fs.File.stdout().deprecatedWriter().print("{s}", .{help_text});
}

fn handleDistill(allocator: std.mem.Allocator, filters: []const Filter) !void {
    const input = try std.fs.File.stdin().readToEndAlloc(allocator, 10 * 1024 * 1024);
    defer allocator.free(input);

    if (input.len == 0) {
        try std.fs.File.stderr().deprecatedWriter().print("Error: No input provided via stdin.\n", .{});
        std.process.exit(1);
    }

    const compressed = try compressor.compress(allocator, input, filters);
    defer allocator.free(compressed);
    try std.fs.File.stdout().deprecatedWriter().print("{s}\n", .{compressed});
}

fn handleDensity(allocator: std.mem.Allocator, filters: []const Filter) !void {
    const input = try std.fs.File.stdin().readToEndAlloc(allocator, 10 * 1024 * 1024);
    defer allocator.free(input);

    const compressed = try compressor.compress(allocator, input, filters);
    defer allocator.free(compressed);

    const in_len = @as(f64, @floatFromInt(input.len));
    const out_len = @as(f64, @floatFromInt(compressed.len));
    const gain = if (out_len > 0) in_len / out_len else 1.0;

    const stdout = std.fs.File.stdout().deprecatedWriter();
    try stdout.print("\n\x1b[0;35m🧠 OMNI Context Density Analysis\x1b[0m\n", .{});
    try stdout.print("════════════════════════════════════════\n", .{});
    try stdout.print("Original Context:  {d} units\n", .{input.len});
    try stdout.print("Distilled Context: {d} units\n", .{compressed.len});
    try stdout.print("\x1b[0;32mContext Density Gain: {d:.2}x\x1b[0m\n", .{gain});
}

fn handleReport(allocator: std.mem.Allocator, filters: []const Filter) !void {
    const stdout = std.fs.File.stdout().deprecatedWriter();
    try stdout.print("\n\x1b[0;35m\x1b[1m🌌 OMNI Unified Intelligence Report\x1b[0m\n", .{});
    try stdout.print("══════════════════════════════════════════════════════════\n", .{});

    // 1. System Status
    try stdout.print("\n\x1b[0;34m\x1b[1m🔧 [1/3] SYSTEM STATUS\x1b[0m\n", .{});
    try stdout.print("  Native Engine:   \x1b[0;32mONLINE\x1b[0m (Zig)\n", .{});
    try stdout.print("  Active Filters:  {d} (Git, Build, Docker, SQL, Node, Custom)\n", .{filters.len});

    // 2. Sample Performance
    const sample = "Step 1/5 : FROM node:18\n ---> 1234\nCACHED\nStep 2/5 : RUN npm install\n[DEBUG] trace...\nSuccessfully built";
    const compressed = try compressor.compress(allocator, sample, filters);
    defer allocator.free(compressed);

    const reduction = (1.0 - (@as(f32, @floatFromInt(compressed.len)) / @as(f32, @floatFromInt(sample.len)))) * 100.0;
    try stdout.print("\n\x1b[0;34m\x1b[1m🧠 [2/3] PERFORMANCE METRICS\x1b[0m\n", .{});
    try stdout.print("  Sample Reduction: {d:.1}% (Signal: High)\n", .{reduction});

    // 3. Recommendation
    try stdout.print("\n\x1b[0;34m\x1b[1m🚀 [3/3] PROJECT STATUS\x1b[0m\n", .{});
    try stdout.print("  Status:          \x1b[0;32mMission-Ready\x1b[0m\n", .{});
    try stdout.print("══════════════════════════════════════════════════════════\n\n", .{});
}

fn handleBench(allocator: std.mem.Allocator, iterations: usize, filters: []const Filter) !void {
    const stdout = std.fs.File.stdout().deprecatedWriter();
    try stdout.print("\n\x1b[0;35m\x1b[1m⚡ OMNI Performance Benchmark\x1b[0m\n", .{});
    try stdout.print("Running {d} iterations...\n", .{iterations});

    const sample = "git status\nOn branch main\nChanges not staged for commit:\n  (use \"git add <file>...\" to update what will be committed)";
    
    var timer = try std.time.Timer.start();
    for (0..iterations) |_| {
        const res = try compressor.compress(allocator, sample, filters);
        allocator.free(res);
    }
    const elapsed = timer.read();
    
    const total_ms = @as(f64, @floatFromInt(elapsed)) / 1_000_000.0;
    const avg_ms = total_ms / @as(f64, @floatFromInt(iterations));

    try stdout.print("Total Time:   {d:.2}ms\n", .{total_ms});
    try stdout.print("Avg Latency:  {d:.4}ms per request\n", .{avg_ms});
    try stdout.print("\x1b[0;32mThroughput:   {d:.0} ops/sec\x1b[0m\n", .{1000.0 / avg_ms});
}

fn handleGenerate(agent: []const u8) !void {
    const stdout = std.fs.File.stdout().deprecatedWriter();
    if (std.mem.eql(u8, agent, "claude-code")) {
        try stdout.print(
            \\# OMNI Template for Claude Code
            \\# Use this to distill your context before sending to Claude
            \\---
            \\project: my-awesome-app
            \\focus: semantic-purity
            \\---
            \\[PASTE YOUR VERBOSE LOGS/DIFFS HERE]
            \\
        , .{});
    } else if (std.mem.eql(u8, agent, "antigravity")) {
        try stdout.print(
            \\# OMNI Template for Antigravity
            \\# Optimized for high-density semantic streams
            \\{{
            \\  "agent": "antigravity",
            \\  "mode": "distill",
            \\  "content": "[PASTE CONTENT HERE]"
            \\}}
            \\
        , .{});
    } else {
        try stdout.print(
            \\# OMNI General Template
            \\---
            \\# Distill any verbose output:
            \\# cat your_output.log | omni
            \\
        , .{});
    }
}

fn handleSetup() !void {
    const help_text =
        \\🌌 OMNI SETUP & USAGE GUIDE
        \\════════════════════════════════════════
        \\1. Compilation:
        \\   zig build -Doptimize=ReleaseFast
        \\
        \\2. Binary Location:
        \\   Your binary is at ./zig-out/bin/omni
        \\
        \\3. Fast Setup (Alias):
        \\   echo "alias omni='$(pwd)/zig-out/bin/omni'" >> ~/.zshrc
        \\
        \\4. Integration:
        \\   # Git:
        \\   git diff | omni
        \\
        \\   # Docker:
        \\   docker build . 2>&1 | omni
        \\
        \\   # SQL:
        \\   pg_dump --schema-only mydb | omni
        \\
    ;
    try std.fs.File.stdout().deprecatedWriter().print("{s}", .{help_text});
}

test "compressor integration" {
    const gpa = std.testing.allocator;
    const input = "On branch main\nChanges not staged for commit:";
    const filters = [_]Filter{GitFilter.filter()};
    const result = try compressor.compress(gpa, input, &filters);
    defer gpa.free(result);
    try std.testing.expect(std.mem.indexOf(u8, result, "git: on branch main") != null);
}
