const std = @import("std");
const build_options = @import("build_options");
const compressor = @import("compressor.zig");
const metrics = @import("local_metrics.zig");
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

    // Load Custom Rules (Hierarchy: ~/.omni/omni_config.json + ./omni_config.json)
    const custom_filter = try CustomFilter.init(allocator);
    defer custom_filter.deinit();

    // 1. Try Global Config (~/.omni/omni_config.json)
    if (std.process.getEnvVarOwned(allocator, "HOME")) |home| {
        defer allocator.free(home);
        const global_path = std.fs.path.join(allocator, &[_][]const u8{ home, ".omni", "omni_config.json" }) catch null;
        if (global_path) |gp| {
            defer allocator.free(gp);
            custom_filter.loadFromFile(gp) catch {};
        }
    } else |_| {}

    // 2. Try Local Config (./omni_config.json)
    custom_filter.loadFromFile("omni_config.json") catch {};

    // Add CustomFilter first so user rules take precedence over built-ins
    try filters.append(allocator, custom_filter.filter());

    try filters.append(allocator, GitFilter.filter());
    try filters.append(allocator, BuildFilter.filter());
    try filters.append(allocator, DockerFilter.filter());
    try filters.append(allocator, SqlFilter.filter());
    try filters.append(allocator, NodeFilter.filter());

    const args = try std.process.argsAlloc(allocator);
    defer std.process.argsFree(allocator, args);

    if (args.len > 1) {
        const cmd = args[1];
        if (std.mem.eql(u8, cmd, "-h") or std.mem.eql(u8, cmd, "--help") or std.mem.eql(u8, cmd, "help")) {
            try printHelp();
            return;
        } else if (std.mem.eql(u8, cmd, "-v") or std.mem.eql(u8, cmd, "--version") or std.mem.eql(u8, cmd, "version")) {
            try std.fs.File.stdout().deprecatedWriter().print("OMNI Core {s} (Zig)\n", .{build_options.version});
            return;
        } else if (std.mem.eql(u8, cmd, "density")) {
            try handleDensity(allocator, filters.items);
            return;
        } else if (std.mem.eql(u8, cmd, "report")) {
            var filter_agent: ?[]const u8 = null;
            if (args.len > 2 and std.mem.startsWith(u8, args[2], "--agent=")) {
                filter_agent = args[2][8..];
            }
            try handleReport(allocator, filter_agent);
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
        } else if (std.mem.eql(u8, cmd, "update")) {
            try handleUpdate(allocator);
            return;
        } else if (std.mem.eql(u8, cmd, "uninstall")) {
            try handleUninstall(allocator);
            return;
        } else if (std.mem.eql(u8, cmd, "examples")) {
            try handleExamples();
            return;
        } else if (std.mem.eql(u8, cmd, "--")) {
            if (args.len > 2) {
                try handleProxy(allocator, args[2..], filters.items);
                return;
            }
        }
    }

    // Default: Distill from stdin
    try handleDistill(allocator, filters.items);
}

fn printHelp() !void {
    const help_text =
        \\OMNI Native Core - Semantic Distillation Engine
        \\
        \\Usage:
        \\  omni [subcommand] [options]
        \\
        \\Subcommands:
        \\  distill          Distill input from stdin (default)
        \\  density          Analyze context density gain
        \\  report           Show unified system & performance report
        \\  bench [N]        Benchmark performance (default 100 iterations)
        \\  generate [type]    Generate configurations (agent, config)
        \\  setup            Show detailed setup and usage instructions
        \\  update           Check for the latest version from GitHub
        \\  uninstall        Remove OMNI and clean up all configurations
        \\  examples         Show real-world study cases and examples
        \\
        \\Examples:
        \\  cat log.txt | omni
        \\  omni density < draft.txt
        \\  omni generate config     > omni_config.json
        \\  omni generate claude-code > .omni-input
        \\
        \\OMNI is designed to be used as a filter in your agentic pipelines.
        \\
    ;
    try std.fs.File.stdout().deprecatedWriter().print("{s}", .{help_text});
}

fn handleExamples() !void {
    const stdout = std.fs.File.stdout().deprecatedWriter();
    const examples_text =
        \\
        \\📚 OMNI STUDY CASES & EXAMPLES
        \\══════════════════════════════════════════════════════════
        \\
        \\   1. Git & Code Review
        \\   git diff | omni                     # Clean diff for LLM
        \\   git log -n 5 | omni                 # Dense commit history
        \\   git show HEAD | omni                # Distill single commit noise
        \\
        \\🐳 2. Containers & Infrastructure
        \\   docker build . 2>&1 | omni          # Distill layer cache & noise
        \\   docker logs <id> | omni             # Semantic log summary
        \\   terraform plan | omni               # Show only infra changes
        \\   kubectl describe pod <p> | omni     # Distill k8s pod noise
        \\
        \\📦 3. Build & Dependency Management
        \\   npm install | omni                  # Clean dependency logs
        \\   zig build --summary all | omni      # Distill build step noise
        \\   cargo build 2>&1 | omni             # Rust build distillation
        \\
        \\📊 4. Database & Queries
        \\   cat dump.sql | omni                 # Distill SQL schema noise
        \\   omni density < logs.txt             # Measure token efficiency
        \\
        \\🤖 5. Agentic Workflows
        \\   omni generate claude-code           # Setup for Claude Code
        \\   omni generate antigravity            # Setup for Antigravity
        \\
        \\💡 Tip: OMNI automatically detects the context and applies 
        \\   the right semantic filter for the highest density!
        \\
        \\══════════════════════════════════════════════════════════
        \\
    ;
    try stdout.print("{s}", .{examples_text});
}

fn handleDistill(allocator: std.mem.Allocator, filters: []const Filter) !void {
    const input = try std.fs.File.stdin().readToEndAlloc(allocator, 10 * 1024 * 1024);
    defer allocator.free(input);

    if (input.len == 0) {
        try std.fs.File.stderr().deprecatedWriter().print("Error: No input provided via stdin.\n", .{});
        std.process.exit(1);
    }

    var timer = try std.time.Timer.start();
    const compressed = try compressor.compress(allocator, input, filters);
    const elapsed = timer.read() / std.time.ns_per_ms;
    defer allocator.free(compressed);
    try std.fs.File.stdout().deprecatedWriter().print("{s}\n", .{compressed});
    
    // Log metrics for native CLI usage
    logMetrics(allocator, "CLI", input.len, compressed.len, elapsed) catch {};
}

fn logMetrics(allocator: std.mem.Allocator, agent: []const u8, input_len: usize, output_len: usize, ms: u64) !void {
    const home = std.posix.getenv("HOME") orelse return;
    const omni_dir = try std.fmt.allocPrint(allocator, "{s}/.omni", .{home});
    defer allocator.free(omni_dir);
    
    std.fs.cwd().makeDir(omni_dir) catch {};
    
    const file_path = try std.fmt.allocPrint(allocator, "{s}/metrics.csv", .{omni_dir});
    defer allocator.free(file_path);

    const file = std.fs.cwd().openFile(file_path, .{ .mode = .read_write }) catch |err| switch (err) {
        error.FileNotFound => try std.fs.cwd().createFile(file_path, .{}),
        else => return,
    };
    defer file.close();

    try file.seekFromEnd(0);
    const ts = std.time.timestamp();
    const line = try std.fmt.allocPrint(allocator, "{d},{s},{d},{d},{d}\n", .{ ts, agent, input_len, output_len, ms });
    defer allocator.free(line);
    try file.writeAll(line);
}

fn handleProxy(allocator: std.mem.Allocator, cmd_args: []const [:0]u8, filters: []const Filter) !void {
    var child = std.process.Child.init(cmd_args, allocator);
    child.stdout_behavior = .Pipe;
    child.stderr_behavior = .Pipe;

    try child.spawn();

    const stdout_data = try child.stdout.?.readToEndAlloc(allocator, 10 * 1024 * 1024);
    const stderr_data = try child.stderr.?.readToEndAlloc(allocator, 10 * 1024 * 1014);
    defer allocator.free(stdout_data);
    defer allocator.free(stderr_data);

    _ = try child.wait();

    if (stdout_data.len > 0) {
        const distilled = try compressor.compress(allocator, stdout_data, filters);
        defer allocator.free(distilled);
        try std.fs.File.stdout().deprecatedWriter().print("{s}\n", .{distilled});
    }

    if (stderr_data.len > 0) {
        const distilled_err = try compressor.compress(allocator, stderr_data, filters);
        defer allocator.free(distilled_err);
        try std.fs.File.stderr().deprecatedWriter().print("{s}\n", .{distilled_err});
    }
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

fn handleReport(allocator: std.mem.Allocator, filter_agent: ?[]const u8) !void {
    const stdout = std.fs.File.stdout().deprecatedWriter();
    
    // Parse Local Metrics Data
    const home = std.posix.getenv("HOME") orelse return;
    const file_path = try std.fmt.allocPrint(allocator, "{s}/.omni/metrics.csv", .{home});
    defer allocator.free(file_path);

    var daily_map = std.StringHashMap(metrics.Stats).init(allocator);
    var weekly_map = std.StringHashMap(metrics.Stats).init(allocator);
    var monthly_map = std.StringHashMap(metrics.Stats).init(allocator);
    defer daily_map.deinit();
    defer weekly_map.deinit();
    defer monthly_map.deinit();

    var global_cmds: usize = 0;
    var global_in: usize = 0;
    var global_out: usize = 0;
    var global_saved: usize = 0;
    var global_ms: u64 = 0;

    if (std.fs.cwd().openFile(file_path, .{})) |file| {
        defer file.close();
        
        const data = file.readToEndAlloc(allocator, 10 * 1024 * 1024) catch return;
        defer allocator.free(data);

        var it_lines = std.mem.splitSequence(u8, data, "\n");
        while (it_lines.next()) |line| {
            if (line.len == 0) continue;
            const rec = metrics.parseCsvLine(allocator, line) catch continue;
            defer allocator.free(rec.agent);

            if (filter_agent != null and !std.mem.eql(u8, rec.agent, filter_agent.?)) {
                continue;
            }

            global_cmds += 1;
            global_in += rec.input_bytes;
            global_out += rec.output_bytes;
            if (rec.input_bytes > rec.output_bytes) global_saved += (rec.input_bytes - rec.output_bytes);
            global_ms += rec.ms;

            const d_lbl = try metrics.toDailyLabel(allocator, rec.timestamp);
            const w_lbl = try metrics.toWeeklyLabel(allocator, rec.timestamp);
            const m_lbl = try metrics.toMonthlyLabel(allocator, rec.timestamp);
            defer allocator.free(d_lbl);
            defer allocator.free(w_lbl);
            defer allocator.free(m_lbl);

            var d_res = try daily_map.getOrPut(d_lbl);
            if (!d_res.found_existing) {
                d_res.value_ptr.* = .{};
                d_res.key_ptr.* = try allocator.dupe(u8, d_lbl);
            }
            d_res.value_ptr.add(rec);

            var w_res = try weekly_map.getOrPut(w_lbl);
            if (!w_res.found_existing) {
                w_res.value_ptr.* = .{};
                w_res.key_ptr.* = try allocator.dupe(u8, w_lbl);
            }
            w_res.value_ptr.add(rec);

            var m_res = try monthly_map.getOrPut(m_lbl);
            if (!m_res.found_existing) {
                m_res.value_ptr.* = .{};
                m_res.key_ptr.* = try allocator.dupe(u8, m_lbl);
            }
            m_res.value_ptr.add(rec);
        }
    } else |_| {}

    // Function to render a single table
    const renderTable = struct {
        fn do(alloc: std.mem.Allocator, map: *std.StringHashMap(metrics.Stats), title: []const u8, out: anytype, rowTitle: []const u8, g_cmds: usize, g_in: usize, g_out: usize, g_s: usize, g_ms: u64) !void {
            try out.print("\n\x1b[1m📅 {s} ({d} entries)\x1b[0m\n", .{ title, map.count() });
            try out.print("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n", .{});
            try out.print("{s:<15} {s:>6} {s:>10} {s:>10} {s:>10} {s:>7} {s:>7}\n", .{ rowTitle, "Cmds", "Input", "Output", "Saved", "Save%", "Time" });
            try out.print("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n", .{});

            var iter = map.iterator();
            // Collect and sort
            var rows: std.ArrayList(metrics.GroupedStats) = .empty;
            defer rows.deinit(alloc);
            while (iter.next()) |entry| {
                try rows.append(alloc, .{ .label = entry.key_ptr.*, .stats = entry.value_ptr.* });
            }

            // Simple bubble sort for demonstration based on label logic (assume string sort matches chronologically here)
            for (0..rows.items.len) |i| {
                for (0..rows.items.len - i - 1) |j| {
                    if (std.mem.order(u8, rows.items[j].label, rows.items[j + 1].label) == .gt) {
                        const temp = rows.items[j];
                        rows.items[j] = rows.items[j + 1];
                        rows.items[j + 1] = temp;
                    }
                }
            }

            for (rows.items) |row| {
                const s = row.stats;
                const in_str = try metrics.formatBytes(alloc, s.input);
                const out_str = try metrics.formatBytes(alloc, s.output);
                const s_str = try metrics.formatBytes(alloc, s.saved);
                const ms_str = try metrics.formatMs(alloc, s.ms, s.cmds);
                defer alloc.free(in_str);
                defer alloc.free(out_str);
                defer alloc.free(s_str);
                defer alloc.free(ms_str);

                const save_pct = if (s.input > 0)
                    (@as(f64, @floatFromInt(s.saved)) / @as(f64, @floatFromInt(s.input))) * 100.0
                else
                    0.0;

                try out.print("{s:<15} {d:>6} {s:>10} {s:>10} {s:>10} {d:>5.1}% {s:>7}\n", .{
                    row.label, s.cmds, in_str, out_str, s_str, save_pct, ms_str,
                });
            }

            try out.print("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n", .{});
            
            const gin_str = try metrics.formatBytes(alloc, g_in);
            const gout_str = try metrics.formatBytes(alloc, g_out);
            const gs_str = try metrics.formatBytes(alloc, g_s);
            const gms_str = try metrics.formatMs(alloc, g_ms, g_cmds);
            defer alloc.free(gin_str);
            defer alloc.free(gout_str);
            defer alloc.free(gs_str);
            defer alloc.free(gms_str);
            
            const g_pct = if (g_in > 0)
                    (@as(f64, @floatFromInt(g_s)) / @as(f64, @floatFromInt(g_in))) * 100.0
                else
                    0.0;

            try out.print("\x1b[1m{s:<15} {d:>6} {s:>10} {s:>10} {s:>10} {d:>5.1}% {s:>7}\x1b[0m\n", .{
                "TOTAL", g_cmds, gin_str, gout_str, gs_str, g_pct, gms_str,
            });
        }
    }.do;

    try stdout.print("\n\x1b[0;35m\x1b[1mOMNI Local Metrics Report\x1b[0m\n", .{});
    if (filter_agent) |ag| {
        try stdout.print("Filtering by Agent: \x1b[0;32m{s}\x1b[0m\n", .{ag});
    } else {
        try stdout.print("Aggregate across all agents and CLI usages\n", .{});
    }

    try renderTable(allocator, &daily_map, "Daily Breakdown", stdout, "Date", global_cmds, global_in, global_out, global_saved, global_ms);
    try renderTable(allocator, &weekly_map, "Weekly Breakdown", stdout, "Week", global_cmds, global_in, global_out, global_saved, global_ms);
    try renderTable(allocator, &monthly_map, "Monthly Breakdown", stdout, "Month", global_cmds, global_in, global_out, global_saved, global_ms);
    try stdout.print("\n", .{});

    // Cleanup keys
    var it = daily_map.keyIterator();
    while (it.next()) |k| allocator.free(k.*);
    var wit = weekly_map.keyIterator();
    while (wit.next()) |k| allocator.free(k.*);
    var mit = monthly_map.keyIterator();
    while (mit.next()) |k| allocator.free(k.*);
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
    
    // Get absolute home path for Claude and Antigravity
    const home = std.posix.getenv("HOME") orelse {
        try std.fs.File.stderr().deprecatedWriter().print("Error: HOME environment variable not found.\n", .{});
        return;
    };
    
    var arena = std.heap.ArenaAllocator.init(std.heap.page_allocator);
    defer arena.deinit();
    const alloc = arena.allocator();
    
    const absolute_omni_path = try std.fmt.allocPrint(alloc, "{s}/.omni/dist/index.js", .{home});

    if (std.mem.eql(u8, agent, "claude-code")) {
        try stdout.print(
            \\# ─── OMNI MCP Config for Claude Code ───
            \\#
            \\# Registering OMNI as an MCP server with Claude Code...
            \\
        , .{});

        const command_json = try std.fmt.allocPrint(alloc, "{{\"type\":\"stdio\",\"command\":\"node\",\"args\":[\"{s}\", \"--agent=claude-code\"]}}", .{absolute_omni_path});
        const argv = [_][]const u8{ "claude", "mcp", "add-json", "omni", command_json };
        
        const run_result = std.process.Child.run(.{
            .allocator = alloc,
            .argv = &argv,
        }) catch |err| {
            try stdout.print("❌ Failed to register with Claude Code: {any}\n", .{err});
            try stdout.print("\n# Manual fallback command:\nclaude mcp add-json omni '{s}'\n", .{command_json});
            return;
        };

        if (run_result.term == .Exited and run_result.term.Exited == 0) {
            try stdout.print("✅ Successfully registered with Claude Code!\n", .{});
        } else {
            try stdout.print("❌ Registration command returned error: {s}\n", .{run_result.stderr});
            try stdout.print("\n# Manual fallback command:\nclaude mcp add-json omni '{s}'\n", .{command_json});
        }
        
        try stdout.print(
            \\
            \\# Verify:
            \\#   claude mcp list
            \\
        , .{});
    } else if (std.mem.eql(u8, agent, "antigravity")) {
        try autoConfigureAntigravity(alloc, home, absolute_omni_path);
    } else if (std.mem.eql(u8, agent, "config")) {
        try handleGenerateConfig();
    } else {
        try stdout.print(
            \\# ─── OMNI MCP Setup ───
            \\#
            \\# Generate a ready-to-use MCP configuration for your AI agent:
            \\#
            \\#   omni generate claude-code     → Claude Code / Claude CLI (Absolute Path)
            \\#   omni generate antigravity      → Google Antigravity (Auto-Merge)
            \\#   omni generate config           → Starter omni_config.json template
            \\#
            \\# Or run the full interactive setup guide:
            \\#   omni setup
            \\
        , .{});
    }
}

fn handleGenerateConfig() !void {
    const stdout = std.fs.File.stdout().deprecatedWriter();
    try stdout.print(
        \\{{
        \\  "//": "OMNI Configuration Template",
        \\  "//": "Place this in ~/.omni/omni_config.json (Global) or ./omni_config.json (Local)",
        \\  
        \\  "rules": [
        \\    {{
        \\      "name": "mask-passwords",
        \\      "match": "password:",
        \\      "action": "mask"
        \\    }},
        \\    {{
        \\      "name": "remove-noise",
        \\      "match": "Checking for updates...",
        \\      "action": "remove"
        \\    }}
        \\  ],
        \\  
        \\  "dsl_filters": [
        \\    {{
        \\      "name": "git-status",
        \\      "trigger": "On branch",
        \\      "rules": [
        \\        {{ "capture": "On branch {{branch}}", "action": "keep" }},
        \\        {{ "capture": "modified: {{file}}", "action": "count", "as": "mod" }}
        \\      ],
        \\      "output": "git({{branch}}) | {{mod}} files modified"
        \\    }}
        \\  ]
        \\}}
        \\
    , .{});
}

fn autoConfigureAntigravity(alloc: std.mem.Allocator, home: []const u8, absolute_omni_path: []const u8) !void {
    const stdout = std.fs.File.stdout().deprecatedWriter();
    const config_path = try std.fmt.allocPrint(alloc, "{s}/.gemini/antigravity/mcp_config.json", .{home});
    
    // Ensure parent directories exist
    if (std.fs.path.dirname(config_path)) |dir| {
        std.fs.cwd().makePath(dir) catch {};
    }

    var file_content: []u8 = undefined;
    var parsed_json: std.json.Parsed(std.json.Value) = undefined;
    var root_obj: std.json.ObjectMap = undefined;
    var mcp_servers_obj: std.json.ObjectMap = undefined;

    // Try reading existing config
    const file_or_err = std.fs.cwd().openFile(config_path, .{});
    if (file_or_err) |file| {
        defer file.close();
        file_content = file.readToEndAlloc(alloc, 1024 * 1024) catch std.fmt.allocPrint(alloc, "{{}}", .{}) catch unreachable;
        parsed_json = std.json.parseFromSlice(std.json.Value, alloc, file_content, .{}) catch |e| {
            try stdout.print("❌ Failed to parse existing mcp_config.json: {any}\n", .{e});
            return;
        };
        if (parsed_json.value != .object) {
            root_obj = std.json.ObjectMap.init(alloc);
        } else {
            root_obj = parsed_json.value.object;
        }
    } else |_| {
        root_obj = std.json.ObjectMap.init(alloc);
    }

    // Get or create "mcpServers"
    if (root_obj.get("mcpServers")) |mcp_node| {
        if (mcp_node == .object) {
            mcp_servers_obj = mcp_node.object;
        } else {
            mcp_servers_obj = std.json.ObjectMap.init(alloc);
        }
    } else {
        mcp_servers_obj = std.json.ObjectMap.init(alloc);
    }

    // Create OMNI server block
    var omni_obj = std.json.ObjectMap.init(alloc);
    try omni_obj.put("command", std.json.Value{ .string = "node" });
    
    var args_array_val = std.json.Array.init(alloc);
    try args_array_val.append(std.json.Value{ .string = absolute_omni_path });
    try args_array_val.append(std.json.Value{ .string = "--agent=antigravity" });
    try omni_obj.put("args", std.json.Value{ .array = args_array_val });

    // Inject into mcpServers and root
    try mcp_servers_obj.put("omni", std.json.Value{ .object = omni_obj });
    try root_obj.put("mcpServers", std.json.Value{ .object = mcp_servers_obj });

    // Write back to file
    const out_file = try std.fs.cwd().createFile(config_path, .{ .truncate = true });
    defer out_file.close();
    
    var write_buf: [4096]u8 = undefined;
    var file_writer = out_file.writer(&write_buf);
    try std.json.Stringify.value(std.json.Value{ .object = root_obj }, .{ .whitespace = .indent_2 }, &file_writer.interface);
    try file_writer.end();

    try stdout.print(
        \\# ─── OMNI MCP Config for Antigravity ───
        \\
        \\✅ Successfully merged configuration into:
        \\   {s}
        \\
        \\OMNI is now registered as an Antigravity MCP server.
        \\Please restart Antigravity or reload your configuration to apply changes.
        \\
    , .{config_path});
}

fn handleSetup() !void {
    if (std.posix.getenv("HOME")) |home| {
        var arena = std.heap.ArenaAllocator.init(std.heap.page_allocator);
        defer arena.deinit();
        const alloc = arena.allocator();

        const omni_dir = std.fmt.allocPrint(alloc, "{s}/.omni", .{home}) catch null;
        const omni_dist_dir = std.fmt.allocPrint(alloc, "{s}/.omni/dist", .{home}) catch null;
        
        if (omni_dir != null and omni_dist_dir != null) {
            std.fs.cwd().makeDir(omni_dir.?) catch {};
            std.fs.cwd().makeDir(omni_dist_dir.?) catch {};

            var buffer: [std.fs.max_path_bytes]u8 = undefined;
            if (std.fs.selfExeDirPath(&buffer)) |exe_dir_raw| {
                var exe_dir = exe_dir_raw;
                
                // --- HOMEBREW STABILITY FIX ---
                // If running from Cellar (e.g., /opt/homebrew/Cellar/omni/0.3.9/bin),
                // transform to stable opt path (e.g., /opt/homebrew/opt/omni/bin)
                // so symlinks don't break on upgrade.
                const cellar_marker = std.fs.path.sep_str ++ "Cellar" ++ std.fs.path.sep_str ++ "omni" ++ std.fs.path.sep_str;
                if (std.mem.indexOf(u8, exe_dir, cellar_marker)) |cellar_idx| {
                    const prefix = exe_dir[0..cellar_idx];
                    const suffix_start = std.mem.indexOfPos(u8, exe_dir, cellar_idx + cellar_marker.len, std.fs.path.sep_str) orelse exe_dir.len;
                    const suffix = exe_dir[suffix_start..];
                    
                    exe_dir = std.fmt.allocPrint(alloc, "{s}" ++ std.fs.path.sep_str ++ "opt" ++ std.fs.path.sep_str ++ "omni{s}", .{prefix, suffix}) catch exe_dir;
                    // Note: We don't free prefix/suffix as they are slices of exe_dir_raw (stack-based buffer)
                }
                
                // Search candidate paths for index.js
                const candidates = [_]?[]const u8{
                    std.fs.path.join(alloc, &.{ exe_dir, "..", "dist", "index.js" }) catch null,
                    std.fs.path.join(alloc, &.{ exe_dir, "..", "libexec", "dist", "index.js" }) catch null,
                    std.fs.path.join(alloc, &.{ exe_dir, "..", "libexec", "src", "index.js" }) catch null,
                    std.fs.path.join(alloc, &.{ exe_dir, "..", "src", "index.js" }) catch null,
                };
                
                var real_src_dist: ?[]const u8 = null;
                for (&candidates) |candidate| {
                    if (candidate) |c| {
                        if (std.fs.cwd().access(c, .{})) |_| {
                            real_src_dist = c;
                            break;
                        } else |_| {}
                    }
                }

                if (real_src_dist != null) {
                    const dst_dist = std.fmt.allocPrint(alloc, "{s}/index.js", .{omni_dist_dir.?}) catch null;
                    if (dst_dist != null) {
                        // Skip if source and destination are already same path
                        if (!std.mem.eql(u8, real_src_dist.?, dst_dist.?)) {
                            // Remove stale symlink if exists
                            std.posix.unlink(dst_dist.?) catch {};
                            std.posix.symlink(real_src_dist.?, dst_dist.?) catch {};
                        }
                    }
                }

                // Initialize Global Config if it doesn't exist
                const global_config_path = std.fmt.allocPrint(alloc, "{s}/omni_config.json", .{omni_dir.?}) catch null;
                if (global_config_path) |path| {
                    const config_file_check = std.fs.cwd().openFile(path, .{});
                    if (config_file_check) |file| {
                        file.close();
                    } else |_| {
                        // Create default config
                        const default_config = 
                            \\{
                            \\  "rules": [],
                            \\  "dsl_filters": []
                            \\}
                            \\
                        ;
                        const f = std.fs.cwd().createFile(path, .{}) catch null;
                        if (f) |file| {
                            _ = file.write(default_config) catch {};
                            file.close();
                        }
                    }
                }
            } else |_| {}
        }
    }

    const help_text =
        \\
        \\🌌 OMNI SETUP & INTEGRATION GUIDE
        \\══════════════════════════════════════════════════════════
        \\
        \\📍 Step 1: Verify Installation
        \\   omni --version              # Should print OMNI Core vX.X.X
        \\   omni report                 # Check engine status
        \\
        \\📍 Step 2: Choose Your Agent
        \\
        \\   ┌─────────────────────────────────────────────────────┐
        \\   │  CLAUDE CODE / CLAUDE CLI                           │
        \\   │                                                     │
        \\   │  Run:                                               │
        \\   │  claude mcp add-json omni \                         │
        \\   │    '{"type":"stdio","command":"node",               │
        \\   │     "args":["$HOME/.omni/dist/index.js"]}'          │
        \\   │                                                     │
        \\   │  Verify: claude mcp list                            │
        \\   └─────────────────────────────────────────────────────┘
        \\
        \\   ┌─────────────────────────────────────────────────────┐
        \\   │  ANTIGRAVITY (Google)                               │
        \\   │                                                     │
        \\   │  Add to ~/.gemini/antigravity/mcp_config.json:      │
        \\   │                                                     │
        \\   │  {                                                  │
        \\   │    "mcpServers": {                                  │
        \\   │      "omni": {                                      │
        \\   │        "command": "node",                           │
        \\   │        "args": ["$HOME/.omni/dist/index.js"]        │
        \\   │      }                                              │
        \\   │    }                                                │
        \\   │  }                                                  │
        \\   └─────────────────────────────────────────────────────┘
        \\
        \\📍 Step 3: Generate Config Automatically
        \\   omni generate claude-code   # Copy-paste config for Claude
        \\   omni generate antigravity   # Copy-paste config for Antigravity
        \\
        \\📍 Step 4: Use OMNI Everywhere
        \\   git diff | omni                     # Distill git output
        \\   docker build . 2>&1 | omni          # Distill docker output
        \\   omni density < logs.txt             # Analyze token density
        \\   omni bench 1000                     # Benchmark performance
        \\
        \\══════════════════════════════════════════════════════════
        \\OMNI is mission-ready.
        \\
    ;
    try std.fs.File.stdout().deprecatedWriter().print("{s}", .{help_text});
}

fn handleUpdate(allocator: std.mem.Allocator) !void {
    try std.fs.File.stdout().deprecatedWriter().print(" Checking for updates...\n", .{});

    const repo_url = "https://api.github.com/repos/fajarhide/omni/releases/latest";
    const result = std.process.Child.run(.{
        .allocator = allocator,
        .argv = &[_][]const u8{ "curl", "-s", "-H", "Accept: application/vnd.github.v3+json", repo_url },
    }) catch |err| {
        try std.fs.File.stderr().deprecatedWriter().print("Error: Failed to run curl. Please ensure curl is installed.\n({any})\n", .{err});
        return;
    };
    defer allocator.free(result.stdout);
    defer allocator.free(result.stderr);

    if (result.stdout.len == 0) {
        try std.fs.File.stderr().deprecatedWriter().print("Error: Received empty response from GitHub.\n", .{});
        return;
    }

    // Simple parsing for "tag_name": "vX.X.X"
    const tag_marker = "\"tag_name\":";
    if (std.mem.indexOf(u8, result.stdout, tag_marker)) |idx| {
        const start = std.mem.indexOfPos(u8, result.stdout, idx + tag_marker.len, "\"") orelse return;
        const end = std.mem.indexOfPos(u8, result.stdout, start + 1, "\"") orelse return;
        const latest_tag = result.stdout[start + 1 .. end];

        // Remove 'v' prefix if present for comparison
        const latest_version = if (std.mem.startsWith(u8, latest_tag, "v")) latest_tag[1..] else latest_tag;
        const current_version = build_options.version;

        if (std.mem.eql(u8, latest_version, current_version)) {
            try std.fs.File.stdout().deprecatedWriter().print("✨ OMNI is up to date (v{s}).\n", .{current_version});
        } else {
            try std.fs.File.stdout().deprecatedWriter().print(" A new version of OMNI is available: {s} (current: v{s})\n", .{ latest_tag, current_version });

            // Detect How to Update (Homebrew vs Installer)
            var buffer: [std.fs.max_path_bytes]u8 = undefined;
            if (std.fs.selfExePath(&buffer)) |exe_path| {
                if (std.mem.indexOf(u8, exe_path, "Cellar") != null or std.mem.indexOf(u8, exe_path, "homebrew") != null) {
                    try std.fs.File.stdout().deprecatedWriter().print("\nTo update, run:\n  brew upgrade fajarhide/tap/omni\n", .{});
                } else {
                    try std.fs.File.stdout().deprecatedWriter().print("\nTo update, run the installer:\n  curl -fsSL https://raw.githubusercontent.com/fajarhide/omni/main/install.sh | sh\n", .{});
                }
            } else |_| {}
        }
    } else {
        try std.fs.File.stderr().deprecatedWriter().print("Error: Could not find version tag in GitHub response.\n", .{});
    }
}

fn handleUninstall(allocator: std.mem.Allocator) !void {
    const home = std.posix.getenv("HOME") orelse {
        try std.fs.File.stderr().deprecatedWriter().print("Error: HOME environment variable not set.\n", .{});
        return;
    };

    try std.fs.File.stdout().deprecatedWriter().print("\xf0\x9f\x8c\x8c Starting OMNI Uninstall...\n", .{});

    // 1. Clean up known Agent MCP Configs using Node.js (guaranteed available)
    const agent_configs = [_]struct { rel: []const u8, label: []const u8 }{
        .{ .rel = ".gemini/antigravity/mcp_config.json", .label = "Antigravity (Google)" },
        .{ .rel = ".claude/mcp_config.json", .label = "Claude Code CLI" },
        .{ .rel = "Library/Application Support/Claude/claude_desktop_config.json", .label = "Claude Desktop" },
    };

    for (agent_configs) |cfg| {
        const full_path = std.fs.path.join(allocator, &.{ home, cfg.rel }) catch continue;
        defer allocator.free(full_path);

        // Check if file exists and contains "omni"
        const file_content = blk: {
            const f = std.fs.openFileAbsolute(full_path, .{}) catch continue;
            defer f.close();
            break :blk f.readToEndAlloc(allocator, 1024 * 1024) catch continue;
        };
        defer allocator.free(file_content);

        if (std.mem.indexOf(u8, file_content, "\"omni\"") == null) continue;

        // Use node to safely remove the "omni" key from mcpServers
        const node_script = std.fmt.allocPrint(allocator,
            \\const fs=require('fs');
            \\try{{const p='{s}';const c=JSON.parse(fs.readFileSync(p,'utf8'));
            \\if(c.mcpServers&&c.mcpServers.omni){{delete c.mcpServers.omni;
            \\fs.writeFileSync(p,JSON.stringify(c,null,2)+'\n');
            \\process.stdout.write('ok')}}}}catch(e){{}}
        , .{full_path}) catch continue;
        defer allocator.free(node_script);

        const result = std.process.Child.run(.{
            .allocator = allocator,
            .argv = &.{ "node", "-e", node_script },
        }) catch continue;
        defer allocator.free(result.stdout);
        defer allocator.free(result.stderr);

        if (std.mem.eql(u8, result.stdout, "ok")) {
            try std.fs.File.stdout().deprecatedWriter().print("\xe2\x9c\x85 Removed 'omni' from {s}\n", .{cfg.label});
        }
    }

    // 2. Remove ~/.omni directory
    const omni_dir = std.fs.path.join(allocator, &.{ home, ".omni" }) catch null;
    if (omni_dir) |dir| {
        defer allocator.free(dir);
        std.fs.deleteTreeAbsolute(dir) catch |err| {
            if (err != error.FileNotFound) {
                try std.fs.File.stderr().deprecatedWriter().print("Warn: Failed to delete {s} ({any})\n", .{ dir, err });
            }
        };
        try std.fs.File.stdout().deprecatedWriter().print("\xe2\x9c\x85 Cleaned up ~/.omni directory\n", .{});
    }

    try std.fs.File.stdout().deprecatedWriter().print("\n\xe2\x9c\xa8 OMNI has been successfully uninstalled.\n", .{});
    try std.fs.File.stdout().deprecatedWriter().print("Note: If you installed via Homebrew, also run: brew uninstall omni\n", .{});
}


test "compressor integration" {
    const gpa = std.testing.allocator;
    const input = "On branch main\nChanges not staged for commit:";
    const filters = [_]Filter{GitFilter.filter()};
    const result = try compressor.compress(gpa, input, &filters);
    defer gpa.free(result);
    // Git filter now outputs compact summary format: "git: on <branch> | ..."
    try std.testing.expect(std.mem.indexOf(u8, result, "git: on main") != null);
}
