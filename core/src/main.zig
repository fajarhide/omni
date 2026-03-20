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
const CatFilter = @import("filters/cat.zig").CatFilter;
const auto_learn = @import("filters/auto_learn.zig");
const monitor = @import("monitor.zig");
const ui = @import("ui.zig");

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
    try filters.append(allocator, CatFilter.filter());

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
            if (args.len > 2 and (std.mem.eql(u8, args[2], "--help") or std.mem.eql(u8, args[2], "-h"))) {
                try printDensityHelp();
                return;
            }
            try handleDensity(allocator, filters.items);
            return;
        } else if (std.mem.eql(u8, cmd, "monitor")) {
            if (args.len > 2 and (std.mem.eql(u8, args[2], "--help") or std.mem.eql(u8, args[2], "-h"))) {
                try printMonitorHelp();
                return;
            }
            if (args.len > 2 and (std.mem.eql(u8, args[2], "scan") or std.mem.eql(u8, args[2], "discover") or std.mem.eql(u8, args[2], "discovery"))) {
                try monitor.handleDiscover(allocator);
                return;
            }
            var opts = monitor.MonitorOptions{};
            for (args[2..]) |arg| {
                if (std.mem.startsWith(u8, arg, "--agent=")) {
                    opts.filter_agent = arg[8..];
                } else if (std.mem.eql(u8, arg, "--trend") or std.mem.eql(u8, arg, "--graph")) {
                    opts.graph = true;
                } else if (std.mem.eql(u8, arg, "--log") or std.mem.eql(u8, arg, "--history")) {
                    opts.history = true;
                } else if (std.mem.eql(u8, arg, "day") or std.mem.eql(u8, arg, "--daily")) {
                    opts.daily = true;
                } else if (std.mem.eql(u8, arg, "week") or std.mem.eql(u8, arg, "--weekly")) {
                    opts.weekly = true;
                } else if (std.mem.eql(u8, arg, "month") or std.mem.eql(u8, arg, "--monthly")) {
                    opts.monthly = true;
                } else if (std.mem.eql(u8, arg, "--by")) {
                    // next arg will be day/week/month, handled above
                } else if (std.mem.eql(u8, arg, "--all")) {
                    opts.all = true;
            } else if (std.mem.eql(u8, arg, "--format=json") or std.mem.eql(u8, arg, "--json")) {
                opts.format_json = true;
            } else if (std.mem.eql(u8, arg, "--prune-noise")) {
                opts.prune_noise = true;
            }
        }
        try monitor.handleMonitor(allocator, opts);
            return;
        } else if (std.mem.eql(u8, cmd, "bench")) {
            var iterations: usize = 100;
            if (args.len > 2) {
                if (std.mem.eql(u8, args[2], "--help") or std.mem.eql(u8, args[2], "-h")) {
                    try handleBench(allocator, 0, filters.items); // 0 as help sentinel
                    return;
                }
                iterations = std.fmt.parseInt(usize, args[2], 10) catch 100;
            }
            try handleBench(allocator, iterations, filters.items);
            return;
        } else if (std.mem.eql(u8, cmd, "learn") or std.mem.eql(u8, cmd, "discover")) {
            if (args.len > 2 and (std.mem.eql(u8, args[2], "--help") or std.mem.eql(u8, args[2], "-h"))) {
                try printLearnHelp();
                return;
            }
            const local_exists = if (std.fs.cwd().access("omni_config.json", .{})) |_| true else |_| false;
            if (local_exists) {
                try handleLearn(allocator, "omni_config.json", false);
            } else {
                const home = std.posix.getenv("HOME") orelse {
                    try handleLearn(allocator, "omni_config.json", false);
                    return;
                };
                const global_path = try std.fmt.allocPrint(allocator, "{s}/.omni/omni_config.json", .{home});
                defer allocator.free(global_path);
                try handleLearn(allocator, global_path, false);
            }
            return;
        } else if (std.mem.eql(u8, cmd, "generate")) {
            const agent = if (args.len > 2) args[2] else "general";
            try handleGenerate(agent);
            return;
        } else if (std.mem.eql(u8, cmd, "doctor")) {
            var fix = false;
            var strict = false;
            for (args[2..]) |arg| {
                if (std.mem.eql(u8, arg, "--fix")) fix = true;
                if (std.mem.eql(u8, arg, "--strict")) strict = true;
            }
            try handleDoctor(allocator, fix, strict);
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
        } else if (std.mem.eql(u8, cmd, "learn")) {
            if (args.len > 2 and (std.mem.eql(u8, args[2], "--help") or std.mem.eql(u8, args[2], "-h"))) {
                try printLearnHelp();
                return;
            }
            // Optional: --config=path override
            var config_path: []const u8 = "omni_config.json";
            var dry_run = false;
            for (args[2..]) |arg| {
                if (std.mem.startsWith(u8, arg, "--config=")) {
                    config_path = arg[9..];
                } else if (std.mem.eql(u8, arg, "--dry-run")) {
                    dry_run = true;
                }
            }
            try handleLearn(allocator, config_path, dry_run);
            return;
        } else if (std.mem.eql(u8, cmd, "examples")) {
            try handleExamples();
            return;
        } else if (std.mem.eql(u8, cmd, "--")) {
            if (args.len > 2) {
                try handleProxy(allocator, args[2..], filters.items);
                return;
            }
        } else {
            const stderr = std.fs.File.stderr().deprecatedWriter();
            try stderr.print(ui.RED ++ " ⓧ " ++ ui.RESET ++ "Error: Unknown subcommand " ++ ui.BOLD ++ "{s}" ++ ui.RESET ++ "\n", .{cmd});
            try printHelp();
            return;
        }
    }

    // Default: Distill from stdin
    try handleDistill(allocator, filters.items);
}

fn printHelp() !void {
    const stdout = std.fs.File.stdout().deprecatedWriter();
    try stdout.print("\n", .{});
    try ui.printHeader(stdout, "OMNI Native Core - Semantic Distillation Engine");
    try ui.row(stdout, ui.BOLD ++ "Usage:" ++ ui.RESET);
    try ui.row(stdout, "  omni [subcommand] [options]");
    try ui.row(stdout, "");
    try ui.row(stdout, ui.BOLD ++ "Subcommands:" ++ ui.RESET);
    try ui.row(stdout, ui.CYAN ++ "  distill   " ++ ui.RESET ++ "Distill input from stdin (default)");
    try ui.row(stdout, ui.CYAN ++ "  density   " ++ ui.RESET ++ "Analyze context density gain");
    try ui.row(stdout, ui.CYAN ++ "  monitor   " ++ ui.RESET ++ "Show unified system & performance metrics");
    try ui.row(stdout, ui.CYAN ++ "  bench     " ++ ui.RESET ++ "Benchmark performance (e.g. omni bench 100)");
    try ui.row(stdout, ui.CYAN ++ "  learn     " ++ ui.RESET ++ "Auto-detect noise patterns and add filters to config");
    try ui.row(stdout, ui.CYAN ++ "  generate  " ++ ui.RESET ++ "Generate configurations (agent, config)");
    try ui.row(stdout, ui.CYAN ++ "  doctor    " ++ ui.RESET ++ "Audit MCP integrations and OMNI filter config");
    try ui.row(stdout, ui.CYAN ++ "  setup     " ++ ui.RESET ++ "Show detailed setup and usage instructions");
    try ui.row(stdout, ui.CYAN ++ "  update    " ++ ui.RESET ++ "Check for the latest version from GitHub");
    try ui.row(stdout, ui.CYAN ++ "  uninstall " ++ ui.RESET ++ "Remove OMNI and clean up all configurations");
    try ui.row(stdout, ui.CYAN ++ "  examples  " ++ ui.RESET ++ "Show real-world study cases and examples");
    try ui.row(stdout, "");
    try ui.row(stdout, ui.BOLD ++ "Examples:" ++ ui.RESET);
    try ui.row(stdout, "  cat log.txt | omni");
    try ui.row(stdout, "  omni density < draft.txt");
    try ui.row(stdout, "  omni generate config     > omni_config.json");
    try ui.row(stdout, "  omni generate claude-code > .omni-input");
    try ui.row(stdout, "  omni generate codex");
    try ui.row(stdout, "  omni doctor --fix");
    try ui.row(stdout, "");
    try ui.row(stdout, ui.DIM ++ "OMNI is designed to be used as a filter in your agentic pipelines." ++ ui.RESET);
    try ui.printFooter(stdout);
    try stdout.print("\n", .{});
}

fn handleDoctor(allocator: std.mem.Allocator, fix: bool, strict: bool) !void {
    const stdout = std.fs.File.stdout().deprecatedWriter();
    const home = std.posix.getenv("HOME") orelse {
        try std.fs.File.stderr().deprecatedWriter().print("Error: HOME environment variable not found.\n", .{});
        return;
    };

    try stdout.print("\n", .{});
    try ui.printHeader(stdout, "🩺 OMNI DOCTOR");
    try ui.row(stdout, "Audit MCP registrations and OMNI filter readiness.");
    try ui.row(stdout, "");

    const omni_dist = try std.fmt.allocPrint(allocator, "{s}/.omni/dist/index.js", .{home});
    defer allocator.free(omni_dist);
    const omni_config = try std.fmt.allocPrint(allocator, "{s}/.omni/omni_config.json", .{home});
    defer allocator.free(omni_config);
    const local_config = "omni_config.json";
    const claude_config = try std.fmt.allocPrint(allocator, "{s}/.claude.json", .{home});
    defer allocator.free(claude_config);
    const codex_config = try std.fmt.allocPrint(allocator, "{s}/.codex/config.toml", .{home});
    defer allocator.free(codex_config);
    const opencode_config = try std.fmt.allocPrint(allocator, "{s}/.config/opencode/opencode.json", .{home});
    defer allocator.free(opencode_config);
    const antigravity_config = try std.fmt.allocPrint(allocator, "{s}/.gemini/antigravity/mcp_config.json", .{home});
    defer allocator.free(antigravity_config);

    const absolute_omni_path = try std.fmt.allocPrint(allocator, "{s}/.omni/dist/index.js", .{home});
    defer allocator.free(absolute_omni_path);

    const dist_ok = if (std.fs.cwd().access(omni_dist, .{})) |_| true else |_| false;
    try printDoctorCheck(allocator, stdout, dist_ok, "OMNI MCP entrypoint", omni_dist);

    var doctor_warnings = std.ArrayList([]u8).empty;
    defer {
        for (doctor_warnings.items) |line| allocator.free(line);
        doctor_warnings.deinit(allocator);
    }

    var config_rules: usize = 0;
    var config_filters: usize = 0;
    const config_ok = blk: {
        const file = std.fs.cwd().openFile(omni_config, .{}) catch break :blk false;
        defer file.close();
        const content = file.readToEndAlloc(allocator, 1024 * 1024) catch break :blk false;
        defer allocator.free(content);
        const parsed = std.json.parseFromSlice(std.json.Value, allocator, content, .{}) catch break :blk false;
        defer parsed.deinit();
        if (parsed.value != .object) break :blk false;
        if (parsed.value.object.get("rules")) |rules_node| {
            if (rules_node == .array) config_rules = rules_node.array.items.len;
        }
        if (parsed.value.object.get("dsl_filters")) |filters_node| {
            if (filters_node == .array) config_filters = filters_node.array.items.len;
        }
        try collectDoctorDslWarnings(allocator, omni_config, parsed.value, &doctor_warnings);
        break :blk true;
    };
    if (config_ok) {
        const detail = try std.fmt.allocPrint(allocator, "{s} ({d} rules, {d} filters)", .{ omni_config, config_rules, config_filters });
        defer allocator.free(detail);
        try printDoctorCheck(allocator, stdout, true, "Global OMNI config", detail);
    } else {
        try printDoctorCheck(allocator, stdout, false, "Global OMNI config", omni_config);
    }

    var local_rules: usize = 0;
    var local_filters: usize = 0;
    const local_config_ok = blk: {
        const file = std.fs.cwd().openFile(local_config, .{}) catch break :blk false;
        defer file.close();
        const content = file.readToEndAlloc(allocator, 1024 * 1024) catch break :blk false;
        defer allocator.free(content);
        const parsed = std.json.parseFromSlice(std.json.Value, allocator, content, .{}) catch break :blk false;
        defer parsed.deinit();
        if (parsed.value != .object) break :blk false;
        if (parsed.value.object.get("rules")) |rules_node| {
            if (rules_node == .array) local_rules = rules_node.array.items.len;
        }
        if (parsed.value.object.get("dsl_filters")) |filters_node| {
            if (filters_node == .array) local_filters = filters_node.array.items.len;
        }
        try collectDoctorDslWarnings(allocator, local_config, parsed.value, &doctor_warnings);
        break :blk true;
    };
    if (local_config_ok) {
        const detail = try std.fmt.allocPrint(allocator, "{s} ({d} rules, {d} filters)", .{ local_config, local_rules, local_filters });
        defer allocator.free(detail);
        try printDoctorCheck(allocator, stdout, true, "Local OMNI config", detail);
    } else {
        try printDoctorCheck(allocator, stdout, false, "Local OMNI config", local_config);
    }

    var claude_ok = fileContains(allocator, claude_config, "\"omni\"");
    var codex_ok = fileContains(allocator, codex_config, "[mcp_servers.omni]");
    var opencode_ok = fileContains(allocator, opencode_config, "\"omni\"");
    var antigravity_ok = fileContains(allocator, antigravity_config, "\"omni\"");

    try printDoctorCheck(allocator, stdout, claude_ok, "Claude Code", claude_config);
    try printDoctorCheck(allocator, stdout, codex_ok, "Codex", codex_config);
    try printDoctorCheck(allocator, stdout, opencode_ok, "OpenCode", opencode_config);
    try printDoctorCheck(allocator, stdout, antigravity_ok, "Antigravity", antigravity_config);

    try ui.row(stdout, "");
    try ui.row(stdout, ui.BOLD ++ "Filter Diagnostics:" ++ ui.RESET);
    if (doctor_warnings.items.len == 0) {
        try ui.row(stdout, ui.GREEN ++ " ● " ++ ui.RESET ++ "No overly generic DSL triggers detected.");
    } else {
        for (doctor_warnings.items) |line| {
            try ui.row(stdout, line);
        }
        if (strict) {
            try ui.row(stdout, "");
            try ui.row(stdout, ui.RED ++ " ⓧ " ++ ui.RESET ++ "Strict mode failed because DSL filter diagnostics reported warnings.");
            std.process.exit(1);
        }
    }

    if (fix) {
        try ui.row(stdout, "");
        try ui.row(stdout, ui.BOLD ++ "Auto Fix:" ++ ui.RESET);
        var changed = false;

        if (!config_ok) {
            const merge_result = try ensureGlobalCodexPolyglotConfig(allocator, home);
            const msg = try std.fmt.allocPrint(allocator, "  Seeded global OMNI config: {s} ({d} rules, {d} filters added)", .{ merge_result.path, merge_result.added_rules, merge_result.added_filters });
            defer allocator.free(msg);
            try ui.row(stdout, msg);
            changed = true;
        }
        if (!claude_ok) {
            try ui.row(stdout, "  Repairing Claude Code integration...");
            try handleGenerate("claude-code");
            claude_ok = true;
            changed = true;
        }
        if (!codex_ok) {
            try ui.row(stdout, "  Repairing Codex integration...");
            try handleGenerate("codex");
            codex_ok = true;
            changed = true;
        }
        if (!opencode_ok) {
            try ui.row(stdout, "  Repairing OpenCode integration...");
            try autoConfigureOpencode(allocator, home, absolute_omni_path);
            opencode_ok = true;
            changed = true;
        }
        if (!antigravity_ok) {
            try ui.row(stdout, "  Repairing Antigravity integration...");
            try autoConfigureAntigravity(allocator, home, absolute_omni_path);
            antigravity_ok = true;
            changed = true;
        }

        if (!changed) {
            try ui.row(stdout, "  No fixes needed. All tracked integrations are already healthy.");
        }

        try ui.row(stdout, "");
        try ui.row(stdout, ui.BOLD ++ "Post-Fix Hint:" ++ ui.RESET);
        try ui.row(stdout, "  Re-run `omni doctor` to confirm all integrations are healthy.");
    }

    try ui.row(stdout, "");
    try ui.row(stdout, ui.BOLD ++ "Suggested Fixes:" ++ ui.RESET);
    try ui.row(stdout, "  omni generate claude-code");
    try ui.row(stdout, "  omni generate codex");
    try ui.row(stdout, "  omni generate opencode");
    try ui.row(stdout, "  omni generate antigravity");
    try ui.row(stdout, "  omni_trust    " ++ ui.DIM ++ "# For repos with local omni_config.json" ++ ui.RESET);
    try ui.printFooter(stdout);
    try stdout.print("\n", .{});
}

fn printDoctorCheck(allocator: std.mem.Allocator, stdout: anytype, ok: bool, label: []const u8, detail: []const u8) !void {
    const icon = if (ok) ui.GREEN ++ " ● " ++ ui.RESET else ui.RED ++ " ⓧ " ++ ui.RESET;
    const status = if (ok) "OK" else "Missing";
    const line = try std.fmt.allocPrint(allocator, "{s}{s}: {s}  " ++ ui.DIM ++ "({s})" ++ ui.RESET, .{ icon, label, status, detail });
    defer allocator.free(line);
    try ui.row(stdout, line);
}

fn printDoctorTextMatch(allocator: std.mem.Allocator, stdout: anytype, label: []const u8, path: []const u8, needle: []const u8) !void {
    const ok = fileContains(allocator, path, needle);
    try printDoctorCheck(allocator, stdout, ok, label, path);
}

fn fileContains(allocator: std.mem.Allocator, path: []const u8, needle: []const u8) bool {
    return blk: {
        const file = std.fs.cwd().openFile(path, .{}) catch break :blk false;
        defer file.close();
        const content = file.readToEndAlloc(allocator, 1024 * 1024) catch break :blk false;
        defer allocator.free(content);
        if (std.mem.endsWith(u8, path, ".json")) {
            const parsed = std.json.parseFromSlice(std.json.Value, allocator, content, .{}) catch break :blk std.mem.indexOf(u8, content, needle) != null;
            defer parsed.deinit();
        }
        break :blk std.mem.indexOf(u8, content, needle) != null;
    };
}

fn collectDoctorDslWarnings(allocator: std.mem.Allocator, config_path: []const u8, root: std.json.Value, warnings: *std.ArrayList([]u8)) !void {
    if (root != .object) return;
    const filters_node = root.object.get("dsl_filters") orelse return;
    if (filters_node != .array) return;

    for (filters_node.array.items, 0..) |filter_node, idx| {
        if (filter_node != .object) continue;

        const name = if (filter_node.object.get("name")) |node|
            if (node == .string) node.string else "unnamed"
        else
            "unnamed";
        const trigger = if (filter_node.object.get("trigger")) |node|
            if (node == .string) node.string else ""
        else
            "";

        if (doctorGenericTriggerReason(trigger)) |reason| {
            const line = try std.fmt.allocPrint(
                allocator,
                ui.YELLOW ++ " ⚠ " ++ ui.RESET ++ "{s} #" ++ ui.BOLD ++ "{d}" ++ ui.RESET ++ " `{s}` trigger " ++ ui.DIM ++ "\"{s}\"" ++ ui.RESET ++ " — {s}",
                .{ config_path, idx + 1, name, trigger, reason },
            );
            try warnings.append(allocator, line);
        }
    }
}

fn doctorGenericTriggerReason(trigger: []const u8) ?[]const u8 {
    const trimmed = std.mem.trim(u8, trigger, " \t\r\n");
    if (trimmed.len == 0) return "empty trigger";
    if (trimmed.len < 4) return "too short; likely to match unrelated output";
    if (std.mem.indexOfAny(u8, trimmed, "{}") != null) return "contains placeholder syntax; triggers should be concrete text";

    var alnum_count: usize = 0;
    for (trimmed) |c| {
        if (std.ascii.isAlphanumeric(c)) alnum_count += 1;
    }
    if (alnum_count < 3) return "mostly punctuation; likely too broad";

    const generic_triggers = [_][]const u8{
        "failed",
        "passed",
        "error",
        "ERROR",
        "Done in",
        "Tests:",
        "added ",
        "found ",
        "src/",
        "---",
        "```",
    };
    for (generic_triggers) |generic| {
        if (std.mem.eql(u8, trimmed, generic)) {
            return "too generic; prefer a more specific multi-token trigger";
        }
    }

    return null;
}

fn printLearnHelp() !void {
    const stdout = std.fs.File.stdout().deprecatedWriter();
    try stdout.print("\n", .{});
    try ui.printHeader(stdout, "🧠 OMNI LEARN — Autonomous Filter Discovery");
    try ui.row(stdout, ui.BOLD ++ "Usage:" ++ ui.RESET);
    try ui.row(stdout, "  <tool-output> | omni learn [options]");
    try ui.row(stdout, "");
    try ui.row(stdout, ui.BOLD ++ "Options:" ++ ui.RESET);
    try ui.row(stdout, ui.CYAN ++ "  --config=<path>  " ++ ui.RESET ++ "Target config file (default: ./omni_config.json)");
    try ui.row(stdout, ui.CYAN ++ "  --dry-run        " ++ ui.RESET ++ "Show candidates without writing to disk");
    try ui.row(stdout, "");
    try ui.row(stdout, ui.BOLD ++ "Examples:" ++ ui.RESET);
    try ui.row(stdout, ui.DIM ++ "  docker build . | omni learn" ++ ui.RESET);
    try ui.row(stdout, ui.DIM ++ "  npm install 2>&1 | omni learn --dry-run" ++ ui.RESET);
    try ui.row(stdout, ui.DIM ++ "  kubectl get pods | omni learn --config=~/.omni/omni_config.json" ++ ui.RESET);
    try ui.row(stdout, "");
    try ui.row(stdout, ui.GRAY ++ "OMNI will analyze repetitive patterns, generate DSL filters," ++ ui.RESET);
    try ui.row(stdout, ui.GRAY ++ "and write directly to omni_config.json automatically." ++ ui.RESET);
    try ui.printFooter(stdout);
}

fn handleLearn(allocator: std.mem.Allocator, config_path: []const u8, dry_run: bool) !void {
    const stdout = std.fs.File.stdout().deprecatedWriter();
    const stderr_w = std.fs.File.stderr().deprecatedWriter();

    // Read input from stdin
    const input = std.fs.File.stdin().readToEndAlloc(allocator, 10 * 1024 * 1024) catch |err| {
        try stderr_w.print(ui.RED ++ " ⓧ " ++ ui.RESET ++ "Failed to read stdin: {any}\n", .{err});
        std.process.exit(1);
    };
    defer allocator.free(input);

    if (input.len == 0) {
        try stderr_w.print(ui.RED ++ " ⓧ " ++ ui.RESET ++ "No input provided. Pipe tool output to omni learn.\n", .{});
        try stderr_w.print(ui.DIM ++ "   Example: docker build . | omni learn\n" ++ ui.RESET, .{});
        std.process.exit(1);
    }

    try stdout.print("\n", .{});
    try ui.printHeader(stdout, "🧠 OMNI LEARN — Autonomous Filter Discovery");

    // Discover candidates
    const candidates = auto_learn.discoverCandidates(allocator, input) catch |err| {
        switch (err) {
            auto_learn.LearnError.InsufficientInput => {
                try ui.row(stdout, ui.YELLOW ++ " ⚠ " ++ ui.RESET ++ "Input too short for analysis (min 5 lines).");
            },
            auto_learn.LearnError.NoPatternsFound => {
                try ui.row(stdout, ui.GREEN ++ " ✓ " ++ ui.RESET ++ "No noise patterns discovered — context is already clean!");
            },
            else => {
                try ui.row(stdout, ui.RED ++ " ⓧ " ++ ui.RESET ++ "Analysis failed.");
            },
        }
        try ui.printFooter(stdout);
        return;
    };
    defer auto_learn.freeCandidates(allocator, candidates);

    if (candidates.len == 0) {
        try ui.row(stdout, ui.GREEN ++ " ✓ " ++ ui.RESET ++ "No repetitive noise patterns discovered.");
        try ui.printFooter(stdout);
        return;
    }

    // Tampilkan kandidat yang ditemukan
    {
        const header = try std.fmt.allocPrint(
            allocator,
            ui.BOLD ++ "Found {d} filter candidate(s):" ++ ui.RESET,
            .{candidates.len},
        );
        defer allocator.free(header);
        try ui.row(stdout, header);
    }
    try ui.row(stdout, "");

    for (candidates, 0..) |c, idx| {
        const action_label = switch (c.action) {
            .count => ui.YELLOW ++ "count" ++ ui.RESET,
            .keep => ui.CYAN ++ "keep " ++ ui.RESET,
        };
        const conf_color = if (c.confidence >= 0.8) ui.GREEN else ui.YELLOW;

        const line = try std.fmt.allocPrint(
            allocator,
            "  {d:>2}. {s}[{s}]{s}  trigger={s}\"{s}\"{s}  conf={s}{d:.0}%{s}",
            .{
                idx + 1,
                ui.BOLD,
                action_label,
                ui.RESET,
                ui.CYAN,
                c.trigger,
                ui.RESET,
                conf_color,
                c.confidence * 100.0,
                ui.RESET,
            },
        );
        defer allocator.free(line);
        try ui.row(stdout, line);

        const output_line = try std.fmt.allocPrint(
            allocator,
            "      " ++ ui.DIM ++ "→ {s}" ++ ui.RESET,
            .{c.output_template},
        );
        defer allocator.free(output_line);
        try ui.row(stdout, output_line);
    }

    try ui.row(stdout, "");

    if (dry_run) {
        try ui.row(stdout, ui.YELLOW ++ " ◆ " ++ ui.RESET ++ "Dry-run mode: nothing written to disk.");
        try ui.printFooter(stdout);
        try stdout.print("\n", .{});
        return;
    }

    // Write to config
    const added = auto_learn.writeToConfig(allocator, config_path, candidates) catch |err| {
        const err_msg = try std.fmt.allocPrint(
            allocator,
            ui.RED ++ " ⓧ " ++ ui.RESET ++ "Failed to write to {s}: {any}",
            .{ config_path, err },
        );
        defer allocator.free(err_msg);
        try ui.row(stdout, err_msg);
        try ui.printFooter(stdout);
        return;
    };

    const skipped = candidates.len - added;

    // Ringkasan hasil
    try ui.divider(stdout);

    if (added > 0) {
        const summary = try std.fmt.allocPrint(
            allocator,
            ui.GREEN ++ " ✓ " ++ ui.RESET ++ "{d} new filter(s) added to " ++ ui.CYAN ++ "{s}" ++ ui.RESET,
            .{ added, config_path },
        );
        defer allocator.free(summary);
        try ui.row(stdout, summary);
    }

    if (skipped > 0) {
        const skip_msg = try std.fmt.allocPrint(
            allocator,
            ui.GRAY ++ "   {d} filter(s) skipped (trigger already exists in config)" ++ ui.RESET,
            .{skipped},
        );
        defer allocator.free(skip_msg);
        try ui.row(stdout, skip_msg);
    }

    if (added == 0 and skipped == candidates.len) {
        try ui.row(stdout, ui.GREEN ++ " ✓ " ++ ui.RESET ++ "All patterns already exist in config — no duplicates.");
    } else {
        try ui.row(stdout, "");
        try ui.row(stdout, ui.DIM ++ "Filters will be active on the next distillation." ++ ui.RESET);
        const vmsg = try std.fmt.allocPrint(allocator, ui.DIM ++ "Verification: cat " ++ ui.RESET ++ "{s}", .{config_path});
        defer allocator.free(vmsg);
        try ui.row(stdout, vmsg);
    }

    try ui.printFooter(stdout);
    try stdout.print("\n", .{});
}

fn handleExamples() !void {
    const stdout = std.fs.File.stdout().deprecatedWriter();
    try stdout.print("\n", .{});
    try ui.printHeader(stdout, "📚 OMNI STUDY CASES & EXAMPLES");

    try ui.row(stdout, ui.BOLD ++ "1. Git & Code Review" ++ ui.RESET);
    try ui.row(stdout, "   git diff | omni                     " ++ ui.DIM ++ "# Clean diff for LLM" ++ ui.RESET);
    try ui.row(stdout, "   git log -n 5 | omni                 " ++ ui.DIM ++ "# Dense commit history" ++ ui.RESET);
    try ui.row(stdout, "   git show HEAD | omni                " ++ ui.DIM ++ "# Distill single commit" ++ ui.RESET);
    try ui.row(stdout, "");

    try ui.row(stdout, ui.BOLD ++ "2. Containers & Infrastructure" ++ ui.RESET);
    try ui.row(stdout, "   docker build . 2>&1 | omni          " ++ ui.DIM ++ "# Distill layer cache" ++ ui.RESET);
    try ui.row(stdout, "   docker logs <id> | omni             " ++ ui.DIM ++ "# Semantic log summary" ++ ui.RESET);
    try ui.row(stdout, "   terraform plan | omni               " ++ ui.DIM ++ "# Show only infra changes" ++ ui.RESET);
    try ui.row(stdout, "   kubectl describe pod <p> | omni     " ++ ui.DIM ++ "# Distill k8s pod noise" ++ ui.RESET);
    try ui.row(stdout, "");

    try ui.row(stdout, ui.BOLD ++ "3. Build & Dependency Management" ++ ui.RESET);
    try ui.row(stdout, "   npm install | omni                  " ++ ui.DIM ++ "# Clean dependency logs" ++ ui.RESET);
    try ui.row(stdout, "   zig build --summary all | omni      " ++ ui.DIM ++ "# Distill build step noise" ++ ui.RESET);
    try ui.row(stdout, "   cargo build 2>&1 | omni             " ++ ui.DIM ++ "# Rust build distillation" ++ ui.RESET);
    try ui.row(stdout, "");

    try ui.row(stdout, ui.BOLD ++ "4. Database & Queries" ++ ui.RESET);
    try ui.row(stdout, "   cat dump.sql | omni                 " ++ ui.DIM ++ "# Distill SQL schema noise" ++ ui.RESET);
    try ui.row(stdout, "   omni density < logs.txt             " ++ ui.DIM ++ "# Measure token efficiency" ++ ui.RESET);
    try ui.row(stdout, "");

    try ui.row(stdout, ui.BOLD ++ "5. Agentic Workflows" ++ ui.RESET);
    try ui.row(stdout, "   omni generate claude-code           " ++ ui.DIM ++ "# Setup for Claude Code" ++ ui.RESET);
    try ui.row(stdout, "   omni generate antigravity           " ++ ui.DIM ++ "# Setup for Antigravity" ++ ui.RESET);
    try ui.row(stdout, "");

    try ui.row(stdout, ui.BOLD ++ ui.GREEN ++ "▸ Tip: " ++ ui.RESET ++ "OMNI automatically detects the context and applies");
    try ui.row(stdout, "  the right semantic filter for the highest density!");

    try ui.printFooter(stdout);
    try stdout.print("\n", .{});
}

fn handleDistill(allocator: std.mem.Allocator, filters: []const Filter) !void {
    const input = try std.fs.File.stdin().readToEndAlloc(allocator, 10 * 1024 * 1024);
    defer allocator.free(input);

    if (input.len == 0) {
        try std.fs.File.stderr().deprecatedWriter().print("Error: No input provided via stdin.\n", .{});
        std.process.exit(1);
    }

    var timer = try std.time.Timer.start();
    const result = try compressor.compress(allocator, input, filters);
    const elapsed = timer.read() / std.time.ns_per_ms;
    defer allocator.free(result.output);
    try std.fs.File.stdout().deprecatedWriter().print("{s}\n", .{result.output});

    // Log metrics for native CLI usage
    logMetrics(allocator, "native-cli", result.filter_name, input.len, result.output.len, elapsed) catch {};
}

fn logMetrics(allocator: std.mem.Allocator, agent: []const u8, filter_name: []const u8, input_len: usize, output_len: usize, ms: u64) !void {
    const home = std.posix.getenv("HOME") orelse return;
    const omni_dir = try std.fmt.allocPrint(allocator, "{s}/.omni", .{home});
    defer allocator.free(omni_dir);

    std.fs.makeDirAbsolute(omni_dir) catch |err| {
        if (err != error.PathAlreadyExists) return;
    };

    const file_path = try std.fmt.allocPrint(allocator, "{s}/metrics.csv", .{omni_dir});
    defer allocator.free(file_path);

    const file = std.fs.openFileAbsolute(file_path, .{ .mode = .read_write }) catch |err| if (err == error.FileNotFound) blk: {
        break :blk try std.fs.createFileAbsolute(file_path, .{});
    } else return;
    defer file.close();

    try file.seekFromEnd(0);
    const ts = std.time.timestamp();
    const line = try std.fmt.allocPrint(allocator, "{d},{s},{s},{d},{d},{d}\n", .{ ts, agent, filter_name, input_len, output_len, ms });
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
        var timer = try std.time.Timer.start();
        const result = try compressor.compress(allocator, stdout_data, filters);
        const elapsed = timer.read() / std.time.ns_per_ms;
        defer allocator.free(result.output);
        try std.fs.File.stdout().deprecatedWriter().print("{s}\n", .{result.output});
        logMetrics(allocator, "native-cli", result.filter_name, stdout_data.len, result.output.len, elapsed) catch {};
    }

    if (stderr_data.len > 0) {
        var timer = try std.time.Timer.start();
        const result = try compressor.compress(allocator, stderr_data, filters);
        const elapsed = timer.read() / std.time.ns_per_ms;
        defer allocator.free(result.output);
        try std.fs.File.stderr().deprecatedWriter().print("{s}\n", .{result.output});
        logMetrics(allocator, "native-cli", result.filter_name, stderr_data.len, result.output.len, elapsed) catch {};
    }
}

fn handleDensity(allocator: std.mem.Allocator, filters: []const Filter) !void {
    const input = try std.fs.File.stdin().readToEndAlloc(allocator, 10 * 1024 * 1024);
    defer allocator.free(input);

    var timer = try std.time.Timer.start();
    const result = try compressor.compress(allocator, input, filters);
    const elapsed = timer.read() / std.time.ns_per_ms;
    defer allocator.free(result.output);

    logMetrics(allocator, "native-cli", result.filter_name, input.len, result.output.len, elapsed) catch {};

    const in_len = @as(f64, @floatFromInt(input.len));
    const out_len = @as(f64, @floatFromInt(result.output.len));
    const gain = if (out_len > 0) in_len / out_len else 1.0;
    const saving_pct = if (in_len > 0) ((in_len - out_len) / in_len) * 100.0 else 0.0;

    const stdout = std.fs.File.stdout().deprecatedWriter();
    try stdout.print("\n", .{});
    try ui.printHeader(stdout, "🧠 OMNI Context Density Analysis");

    {
        const l = try std.fmt.allocPrint(allocator, ui.GRAY ++ "Filter applied:    " ++ ui.CYAN ++ "{s}" ++ ui.RESET, .{result.filter_name});
        defer allocator.free(l);
        try ui.row(stdout, l);
    }
    {
        const l = try std.fmt.allocPrint(allocator, ui.GRAY ++ "Original Context:  " ++ ui.WHITE ++ "{d} units" ++ ui.RESET, .{input.len});
        defer allocator.free(l);
        try ui.row(stdout, l);
    }
    {
        const l = try std.fmt.allocPrint(allocator, ui.GRAY ++ "Distilled Context: " ++ ui.WHITE ++ "{d} units" ++ ui.RESET, .{result.output.len});
        defer allocator.free(l);
        try ui.row(stdout, l);
    }
    try ui.row(stdout, "");

    const bar = try ui.progressBar(allocator, "Density Gain", saving_pct, 30);
    defer allocator.free(bar);
    try ui.row(stdout, bar);

    {
        const l = try std.fmt.allocPrint(allocator, ui.GREEN ++ "Result: {d:.2}x more token-efficient" ++ ui.RESET, .{gain});
        defer allocator.free(l);
        try ui.row(stdout, l);
    }

    try ui.printFooter(stdout);
    try stdout.print("\n", .{});
}

fn handleBench(allocator: std.mem.Allocator, iterations: usize, filters: []const Filter) !void {
    const stdout = std.fs.File.stdout().deprecatedWriter();

    if (iterations == 0) { // Sentinel for help
        try ui.printHeader(stdout, "⚡ OMNI BENCHMARK HELP");
        try ui.row(stdout, ui.BOLD ++ "Usage:" ++ ui.RESET);
        try ui.row(stdout, "  omni bench [iterations]");
        try ui.row(stdout, "");
        try ui.row(stdout, "Measures the latency and throughput of the OMNI engine.");
        try ui.row(stdout, "Example: " ++ ui.CYAN ++ "omni bench 1000" ++ ui.RESET);
        try ui.printFooter(stdout);
        return;
    }

    try stdout.print("\n", .{});
    try ui.printHeader(stdout, "⚡ OMNI Performance Benchmark");

    const status = try std.fmt.allocPrint(allocator, "Running {d} iterations...", .{iterations});
    defer allocator.free(status);
    try ui.row(stdout, status);
    try ui.row(stdout, "");

    const sample = "git status\nOn branch main\nChanges not staged for commit:\n  (use \"git add <file>...\" to update what will be committed)";

    var timer = try std.time.Timer.start();
    for (0..iterations) |_| {
        const res = try compressor.compress(allocator, sample, filters);
        allocator.free(res.output);
    }
    const elapsed = timer.read();

    const total_ms = @as(f64, @floatFromInt(elapsed)) / 1_000_000.0;
    const avg_ms = total_ms / @as(f64, @floatFromInt(iterations));
    const ops_sec = 1000.0 / avg_ms;

    {
        const l = try std.fmt.allocPrint(allocator, ui.GRAY ++ "Total Time:   " ++ ui.WHITE ++ "{d:.2}ms" ++ ui.RESET, .{total_ms});
        defer allocator.free(l);
        try ui.row(stdout, l);
    }
    {
        const l = try std.fmt.allocPrint(allocator, ui.GRAY ++ "Avg Latency:  " ++ ui.WHITE ++ "{d:.4}ms per request" ++ ui.RESET, .{avg_ms});
        defer allocator.free(l);
        try ui.row(stdout, l);
    }

    try ui.row(stdout, "");

    // Throughput bar (Cap at 100,000 ops/sec for visual 100%)
    const tp_pct = @min((ops_sec / 100000.0) * 100.0, 100.0);
    const bar = try ui.progressBar(allocator, "Throughput", tp_pct, 30);
    defer allocator.free(bar);
    try ui.row(stdout, bar);

    {
        const l = try std.fmt.allocPrint(allocator, ui.GREEN ++ "Benchmark Result: {d:.0} ops/sec" ++ ui.RESET, .{ops_sec});
        defer allocator.free(l);
        try ui.row(stdout, l);
    }

    try ui.printFooter(stdout);
    try stdout.print("\n", .{});
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

    if (std.mem.eql(u8, agent, "--help") or std.mem.eql(u8, agent, "-h")) {
        try ui.printHeader(stdout, "📦 OMNI GENERATE HELP");
        try ui.row(stdout, ui.BOLD ++ "Usage:" ++ ui.RESET);
        try ui.row(stdout, "  omni generate [agent|config]");
        try ui.row(stdout, "");
        try ui.row(stdout, ui.BOLD ++ "Arguments:" ++ ui.RESET);
        try ui.row(stdout, ui.CYAN ++ "  claude-code " ++ ui.RESET ++ "Auto-register OMNI with Claude Code");
        try ui.row(stdout, ui.CYAN ++ "  codex       " ++ ui.RESET ++ "Auto-register OMNI with Codex CLI");
        try ui.row(stdout, ui.CYAN ++ "  antigravity " ++ ui.RESET ++ "Auto-register OMNI with Antigravity");
        try ui.row(stdout, ui.CYAN ++ "  opencode    " ++ ui.RESET ++ "Auto-register OMNI with OpenCode AI");
        try ui.row(stdout, ui.CYAN ++ "  config      " ++ ui.RESET ++ "Generate a template omni_config.json");
        try ui.printFooter(stdout);
        return;
    }

    if (std.mem.eql(u8, agent, "claude-code")) {
        try stdout.print("\n", .{});
        try ui.printHeader(stdout, "🤖 OMNI MCP CLAUDE INTEGRATION");
        try ui.row(stdout, ui.BOLD ++ "Target: " ++ ui.RESET ++ "Claude Code / Claude CLI");
        try ui.row(stdout, "");
        try ui.row(stdout, "Registering OMNI as an MCP server...");
        try ui.row(stdout, "");

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
            const merge_result = try ensureGlobalCodexPolyglotConfig(alloc, home);
            try ui.row(stdout, ui.GREEN ++ " ● " ++ ui.RESET ++ "Successfully registered with Claude Code!");
            {
                const l = try std.fmt.allocPrint(alloc, "   MCP: " ++ ui.DIM ++ "claude mcp add-json omni ..." ++ ui.RESET, .{});
                defer alloc.free(l);
                try ui.row(stdout, l);
            }
            {
                const l = try std.fmt.allocPrint(alloc, "   Filters: " ++ ui.DIM ++ "{s}" ++ ui.RESET ++ " ({d} rules, {d} filters added)", .{ merge_result.path, merge_result.added_rules, merge_result.added_filters });
                defer alloc.free(l);
                try ui.row(stdout, l);
            }
        } else {
            try ui.row(stdout, ui.RED ++ " ⓧ " ++ ui.RESET ++ "Failed to register with Claude Code.");
            const err_msg = try std.fmt.allocPrint(alloc, "Error: {s}", .{run_result.stderr});
            defer alloc.free(err_msg);
            if (err_msg.len > 0) try ui.row(stdout, err_msg);
            try ui.row(stdout, "");
            try ui.row(stdout, ui.DIM ++ "# Manual fallback command:" ++ ui.RESET);
            const fb = try std.fmt.allocPrint(alloc, "claude mcp add-json omni '{s}'", .{command_json});
            defer alloc.free(fb);
            try ui.row(stdout, fb);
        }

        try ui.row(stdout, "");
        try ui.row(stdout, ui.BOLD ++ "To Verify:" ++ ui.RESET);
        try ui.row(stdout, "  claude mcp list");
        try ui.row(stdout, "");
        try ui.row(stdout, ui.BOLD ++ "Recommended Next Steps:" ++ ui.RESET);
        try ui.row(stdout, "  1. Use `omni_execute`, `omni_read_file`, and `omni_list_dir` when Claude would otherwise read noisy output.");
        try ui.row(stdout, "  2. OMNI already merged the polyglot coding filter bundle into your global config.");
        try ui.row(stdout, "  3. Run `omni_trust` in repos with a local omni_config.json.");
        try ui.printFooter(stdout);
        try stdout.print("\n", .{});
    } else if (std.mem.eql(u8, agent, "codex")) {
        try stdout.print("\n", .{});
        try ui.printHeader(stdout, "🤖 OMNI MCP CODEX INTEGRATION");
        try ui.row(stdout, ui.BOLD ++ "Target: " ++ ui.RESET ++ "Codex CLI");
        try ui.row(stdout, "");
        try ui.row(stdout, "Registering OMNI as an MCP server...");
        try ui.row(stdout, "");

        const expected_path = absolute_omni_path;
        const expected_agent_flag = "--agent=codex";

        const get_argv = [_][]const u8{ "codex", "mcp", "get", "omni", "--json" };
        const get_result = std.process.Child.run(.{
            .allocator = alloc,
            .argv = &get_argv,
        }) catch |err| {
            try stdout.print("❌ Failed to inspect Codex MCP config: {any}\n", .{err});
            try stdout.print("\n# Manual fallback command:\ncodex mcp add omni -- node {s} --agent=codex\n", .{absolute_omni_path});
            return;
        };

        if (get_result.term == .Exited and get_result.term.Exited == 0) {
            const already_matches =
                std.mem.indexOf(u8, get_result.stdout, expected_path) != null and
                std.mem.indexOf(u8, get_result.stdout, expected_agent_flag) != null;

            if (already_matches) {
                const merge_result = try ensureGlobalCodexPolyglotConfig(alloc, home);
                try ui.row(stdout, ui.GREEN ++ " ● " ++ ui.RESET ++ "Codex is already configured to use OMNI.");
                {
                    const msg = try std.fmt.allocPrint(alloc, " ● Global OMNI config ready at {s} ({d} filters added)", .{ merge_result.path, merge_result.added_filters });
                    defer alloc.free(msg);
                    try ui.row(stdout, msg);
                }
                try ui.row(stdout, "");
                try ui.row(stdout, ui.BOLD ++ "To Verify:" ++ ui.RESET);
                try ui.row(stdout, "  codex mcp list");
                try ui.row(stdout, "");
                try ui.row(stdout, ui.BOLD ++ "Recommended Next Steps:" ++ ui.RESET);
                try ui.row(stdout, "  1. In Codex, use the OMNI MCP tools instead of raw shell/file reads when possible.");
                try ui.row(stdout, "  2. Apply `codex-polyglot` or `codex-advanced` in your OMNI config for denser test/build summaries.");
                try ui.row(stdout, "  3. If this repo uses local config, run `omni_trust` so Codex can benefit from project filters.");
                try ui.printFooter(stdout);
                try stdout.print("\n", .{});
                return;
            }

            try ui.row(stdout, ui.YELLOW ++ " ○ " ++ ui.RESET ++ "Existing Codex MCP entry found. Updating it...");
            const remove_argv = [_][]const u8{ "codex", "mcp", "remove", "omni" };
            const remove_result = std.process.Child.run(.{
                .allocator = alloc,
                .argv = &remove_argv,
            }) catch |err| {
                try stdout.print("❌ Failed to remove existing Codex MCP server: {any}\n", .{err});
                try stdout.print("\n# Manual fallback commands:\ncodex mcp remove omni\ncodex mcp add omni -- node {s} --agent=codex\n", .{absolute_omni_path});
                return;
            };

            if (!(remove_result.term == .Exited and remove_result.term.Exited == 0)) {
                try ui.row(stdout, ui.RED ++ " ⓧ " ++ ui.RESET ++ "Failed to remove existing Codex MCP server.");
                const remove_err = try std.fmt.allocPrint(alloc, "Error: {s}", .{remove_result.stderr});
                defer alloc.free(remove_err);
                if (remove_err.len > 0) try ui.row(stdout, remove_err);
                try ui.row(stdout, "");
                try ui.row(stdout, ui.DIM ++ "# Manual fallback commands:" ++ ui.RESET);
                const fb = try std.fmt.allocPrint(alloc, "codex mcp remove omni\ncodex mcp add omni -- node {s} --agent=codex", .{absolute_omni_path});
                defer alloc.free(fb);
                try ui.row(stdout, fb);
                try ui.printFooter(stdout);
                try stdout.print("\n", .{});
                return;
            }
        }

        const argv = [_][]const u8{ "codex", "mcp", "add", "omni", "--", "node", absolute_omni_path, expected_agent_flag };

        const run_result = std.process.Child.run(.{
            .allocator = alloc,
            .argv = &argv,
        }) catch |err| {
            try stdout.print("❌ Failed to register with Codex: {any}\n", .{err});
            try stdout.print("\n# Manual fallback command:\ncodex mcp add omni -- node {s} --agent=codex\n", .{absolute_omni_path});
            return;
        };

        if (run_result.term == .Exited and run_result.term.Exited == 0) {
            const merge_result = try ensureGlobalCodexPolyglotConfig(alloc, home);
            try ui.row(stdout, ui.GREEN ++ " ● " ++ ui.RESET ++ "Successfully registered with Codex!");
            {
                const msg = try std.fmt.allocPrint(alloc, " ● Global OMNI config ready at {s} ({d} filters added)", .{ merge_result.path, merge_result.added_filters });
                defer alloc.free(msg);
                try ui.row(stdout, msg);
            }
        } else {
            try ui.row(stdout, ui.RED ++ " ⓧ " ++ ui.RESET ++ "Failed to register with Codex.");
            const err_msg = try std.fmt.allocPrint(alloc, "Error: {s}", .{run_result.stderr});
            defer alloc.free(err_msg);
            if (err_msg.len > 0) try ui.row(stdout, err_msg);
            try ui.row(stdout, "");
            try ui.row(stdout, ui.DIM ++ "# Manual fallback command:" ++ ui.RESET);
            const fb = try std.fmt.allocPrint(alloc, "codex mcp add omni -- node {s} --agent=codex", .{absolute_omni_path});
            defer alloc.free(fb);
            try ui.row(stdout, fb);
        }

        try ui.row(stdout, "");
        try ui.row(stdout, ui.BOLD ++ "To Verify:" ++ ui.RESET);
        try ui.row(stdout, "  codex mcp list");
        try ui.row(stdout, "");
        try ui.row(stdout, ui.BOLD ++ "Recommended Next Steps:" ++ ui.RESET);
        try ui.row(stdout, "  1. Open Codex and prefer `omni_execute`, `omni_read_file`, and `omni_list_dir` for noisy context.");
        try ui.row(stdout, "  2. Apply `codex-polyglot` for JS/TS, Python, Rust, Go, Zig, and pnpm workflows.");
        try ui.row(stdout, "  3. If you only need TS/JS summaries, use `codex-advanced` instead.");
        try ui.row(stdout, "  4. Run `omni_trust` inside project repos that have a local omni_config.json.");
        try ui.printFooter(stdout);
        try stdout.print("\n", .{});
    } else if (std.mem.eql(u8, agent, "antigravity")) {
        try autoConfigureAntigravity(alloc, home, absolute_omni_path);
    } else if (std.mem.eql(u8, agent, "config")) {
        try handleGenerateConfig();
    } else if (std.mem.eql(u8, agent, "opencode")) {
        try autoConfigureOpencode(alloc, home, absolute_omni_path);
    } else {
        try stdout.print("\n", .{});
        try ui.printHeader(stdout, "\xf0\x9f\x93\xa6 OMNI GENERATE");
        try ui.row(stdout, "Generate a ready-to-use MCP configuration for your AI agent.");
        try ui.row(stdout, "");
        try ui.row(stdout, ui.BOLD ++ "Usage:" ++ ui.RESET);
        try ui.row(stdout, "  omni generate [agent|config]");
        try ui.row(stdout, "");
        try ui.row(stdout, ui.BOLD ++ "Available Targets:" ++ ui.RESET);
        try ui.row(stdout, ui.CYAN ++ "  claude-code  " ++ ui.RESET ++ "Auto-register with Claude Code / CLI");
        try ui.row(stdout, ui.CYAN ++ "  codex        " ++ ui.RESET ++ "Auto-register with Codex CLI");
        try ui.row(stdout, ui.CYAN ++ "  antigravity  " ++ ui.RESET ++ "Auto-register with Google Antigravity");
        try ui.row(stdout, ui.CYAN ++ "  opencode     " ++ ui.RESET ++ "Auto-register with OpenCode AI");
        try ui.row(stdout, ui.CYAN ++ "  config       " ++ ui.RESET ++ "Generate a template omni_config.json");
        try ui.row(stdout, "");
        try ui.row(stdout, ui.DIM ++ "Or run the full interactive setup guide: omni setup" ++ ui.RESET);
        try ui.printFooter(stdout);
        try stdout.print("\n", .{});
    }
}

fn handleGenerateConfig() !void {
    const stdout = std.fs.File.stdout().deprecatedWriter();
    try stdout.print("\n", .{});
    try ui.printHeader(stdout, "\xe2\x9a\x99\xef\xb8\x8f  OMNI CONFIGURATION TEMPLATE");
    try ui.row(stdout, ui.DIM ++ "Save to ~/.omni/omni_config.json (Global)" ++ ui.RESET);
    try ui.row(stdout, ui.DIM ++ "or ./omni_config.json (Local, higher priority)" ++ ui.RESET);
    try ui.row(stdout, "");
    try stdout.print(
        \\{{
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
    try stdout.print("\n", .{});
    try ui.row(stdout, ui.DIM ++ "Redirect to file: omni generate config > omni_config.json" ++ ui.RESET);
    try ui.printFooter(stdout);
    try stdout.print("\n", .{});
}

fn printDensityHelp() !void {
    const stdout = std.fs.File.stdout().deprecatedWriter();
    try ui.printHeader(stdout, "\xf0\x9f\xa7\xa0 OMNI DENSITY HELP");
    try ui.row(stdout, ui.BOLD ++ "Usage:" ++ ui.RESET);
    try ui.row(stdout, "  omni density < input.txt");
    try ui.row(stdout, "  cat file.log | omni density");
    try ui.row(stdout, "");
    try ui.row(stdout, "Analyzes input from stdin and shows the context density");
    try ui.row(stdout, "gain — how many tokens OMNI saves.");
    try ui.row(stdout, "");
    try ui.row(stdout, ui.BOLD ++ "Output Includes:" ++ ui.RESET);
    try ui.row(stdout, "  " ++ ui.CYAN ++ "\xe2\x97\x8f" ++ ui.RESET ++ " Original vs Distilled size");
    try ui.row(stdout, "  " ++ ui.CYAN ++ "\xe2\x97\x8f" ++ ui.RESET ++ " Token saving percentage bar");
    try ui.row(stdout, "  " ++ ui.CYAN ++ "\xe2\x97\x8f" ++ ui.RESET ++ " Density gain multiplier (e.g. 2.5x)");
    try ui.printFooter(stdout);
}

fn printMonitorHelp() !void {
    const stdout = std.fs.File.stdout().deprecatedWriter();
    try ui.printHeader(stdout, "\xf0\x9f\x93\x8a OMNI MONITOR HELP");
    try ui.row(stdout, ui.BOLD ++ "Usage:" ++ ui.RESET);
    try ui.row(stdout, "  omni monitor [options]");
    try ui.row(stdout, "");
    try ui.row(stdout, "Shows unified system & performance metrics.");
    try ui.row(stdout, "");
    try ui.row(stdout, ui.BOLD ++ "Options:" ++ ui.RESET);
    try ui.row(stdout, ui.CYAN ++ "  --agent=<name>  " ++ ui.RESET ++ "Filter metrics by agent");
    try ui.row(stdout, ui.CYAN ++ "  --trend         " ++ ui.RESET ++ "Show savings trend chart");
    try ui.row(stdout, ui.CYAN ++ "  --log           " ++ ui.RESET ++ "Show recent distillation log");
    try ui.row(stdout, ui.CYAN ++ "  --by day        " ++ ui.RESET ++ "Breakdown by day");
    try ui.row(stdout, ui.CYAN ++ "  --by week       " ++ ui.RESET ++ "Breakdown by week");
    try ui.row(stdout, ui.CYAN ++ "  --by month      " ++ ui.RESET ++ "Breakdown by month");
    try ui.row(stdout, ui.CYAN ++ "  --all           " ++ ui.RESET ++ "Show all time ranges");
    try ui.row(stdout, ui.CYAN ++ "  --json          " ++ ui.RESET ++ "Output in JSON format");
    try ui.row(stdout, ui.CYAN ++ "  --prune-noise   " ++ ui.RESET ++ "Hide noisy shorthand filters");
    try ui.row(stdout, "");
    try ui.row(stdout, ui.BOLD ++ "Subcommands:" ++ ui.RESET);
    try ui.row(stdout, ui.CYAN ++ "  scan            " ++ ui.RESET ++ "Scan for missed savings opportunities");
    try ui.printFooter(stdout);
}

const CODEX_POLYGLOT_TEMPLATE_JSON =
    \\{
    \\  "rules": [],
    \\  "dsl_filters": [
    \\    {
    \\      "name": "codex-tsc-summary",
    \\      "trigger": "error TS",
    \\      "confidence": 0.98,
    \\      "rules": [
    \\        { "capture": "{location}: error TS{code}: {message}", "action": "keep" },
    \\        { "capture": "{ts_location}: error TS{ts_code}: {ts_message}", "action": "count", "as": "diagnostics" }
    \\      ],
    \\      "output": "tsc: {diagnostics} diagnostics | last TS{code}: {message}"
    \\    },
    \\    {
    \\      "name": "codex-eslint-summary-plural",
    \\      "trigger": "problems (",
    \\      "confidence": 0.97,
    \\      "rules": [
    \\        { "capture": "✖ {problems} problems ({errors} errors, {warnings} warnings)", "action": "keep" }
    \\      ],
    \\      "output": "eslint: {problems} problems | {errors} errors | {warnings} warnings"
    \\    },
    \\    {
    \\      "name": "codex-eslint-summary-singular",
    \\      "trigger": "problem (",
    \\      "confidence": 0.97,
    \\      "rules": [
    \\        { "capture": "✖ {problems} problem ({errors} error, {warnings} warning)", "action": "keep" }
    \\      ],
    \\      "output": "eslint: {problems} problem | {errors} error | {warnings} warning"
    \\    },
    \\    {
    \\      "name": "codex-jest-summary-fail",
    \\      "trigger": "failed, ",
    \\      "confidence": 0.96,
    \\      "rules": [
    \\        { "capture": "Tests:       {failed} failed, {passed} passed, {total} total", "action": "keep" }
    \\      ],
    \\      "output": "jest: {passed} passed | {failed} failed | {total} total"
    \\    },
    \\    {
    \\      "name": "codex-jest-summary-pass",
    \\      "trigger": "Tests:",
    \\      "confidence": 0.95,
    \\      "rules": [
    \\        { "capture": "Tests:       {passed} passed, {total} total", "action": "keep" }
    \\      ],
    \\      "output": "jest: {passed} passed | {total} total"
    \\    },
    \\    {
    \\      "name": "codex-vitest-summary",
    \\      "trigger": "Test Files",
    \\      "confidence": 0.95,
    \\      "rules": [
    \\        { "capture": "Test Files {files} passed", "action": "keep" },
    \\        { "capture": "Tests {tests} passed", "action": "keep" }
    \\      ],
    \\      "output": "vitest: {files} files passed | {tests} tests passed"
    \\    },
    \\    {
    \\      "name": "pytest-summary-fail",
    \\      "trigger": " failed, ",
    \\      "confidence": 0.95,
    \\      "rules": [
    \\        { "capture": "{failed} failed, {passed} passed in {duration}", "action": "keep" }
    \\      ],
    \\      "output": "pytest: {passed} passed | {failed} failed | {duration}"
    \\    },
    \\    {
    \\      "name": "pytest-summary-pass",
    \\      "trigger": " passed in ",
    \\      "confidence": 0.94,
    \\      "rules": [
    \\        { "capture": "{passed} passed in {duration}", "action": "keep" }
    \\      ],
    \\      "output": "pytest: {passed} passed | {duration}"
    \\    },
    \\    {
    \\      "name": "ruff-summary-pass",
    \\      "trigger": "All checks passed!",
    \\      "confidence": 0.99,
    \\      "rules": [],
    \\      "output": "ruff: all checks passed"
    \\    },
    \\    {
    \\      "name": "ruff-summary-errors-plural",
    \\      "trigger": "Found ",
    \\      "confidence": 0.96,
    \\      "rules": [
    \\        { "capture": "Found {errors} errors.", "action": "keep" }
    \\      ],
    \\      "output": "ruff: {errors} errors"
    \\    },
    \\    {
    \\      "name": "ruff-summary-errors-singular",
    \\      "trigger": "Found 1 error.",
    \\      "confidence": 0.96,
    \\      "rules": [
    \\        { "capture": "Found {error} error.", "action": "keep" }
    \\      ],
    \\      "output": "ruff: {error} error"
    \\    },
    \\    {
    \\      "name": "cargo-test-summary-pass",
    \\      "trigger": "test result: ok.",
    \\      "confidence": 0.97,
    \\      "rules": [
    \\        { "capture": "test result: ok. {passed} passed; {failed} failed; {ignored} ignored; {measured} measured; {filtered} filtered out; finished in {duration}", "action": "keep" }
    \\      ],
    \\      "output": "cargo test: {passed} passed | {failed} failed | {duration}"
    \\    },
    \\    {
    \\      "name": "cargo-test-summary-fail",
    \\      "trigger": "test result: FAILED.",
    \\      "confidence": 0.97,
    \\      "rules": [
    \\        { "capture": "test result: FAILED. {passed} passed; {failed} failed; {ignored} ignored; {measured} measured; {filtered} filtered out; finished in {duration}", "action": "keep" }
    \\      ],
    \\      "output": "cargo test: {passed} passed | {failed} failed | {duration}"
    \\    },
    \\    {
    \\      "name": "pnpm-install-summary",
    \\      "trigger": "Progress: resolved",
    \\      "confidence": 0.93,
    \\      "rules": [
    \\        { "capture": "Progress: resolved {resolved}, reused {reused}, downloaded {downloaded}, added {added}, done", "action": "keep" },
    \\        { "capture": "Done in {duration}", "action": "keep" }
    \\      ],
    \\      "output": "pnpm: resolved {resolved} | reused {reused} | downloaded {downloaded} | added {added} | {duration}"
    \\    },
    \\    {
    \\      "name": "zig-test-summary-pass",
    \\      "trigger": "tests passed.",
    \\      "confidence": 0.92,
    \\      "rules": [
    \\        { "capture": "All {passed} tests passed.", "action": "keep" }
    \\      ],
    \\      "output": "zig test: {passed} passed"
    \\    },
    \\    {
    \\      "name": "go-test-summary-pass",
    \\      "trigger": "ok\t",
    \\      "confidence": 0.9,
    \\      "rules": [
    \\        { "capture": "ok\t{pkg}\t{duration}", "action": "keep" },
    \\        { "capture": "ok\t{counted_pkg}\t{counted_duration}", "action": "count", "as": "passed_packages" }
    \\      ],
    \\      "output": "go test: {passed_packages} packages passed | last {pkg} | {duration}"
    \\    },
    \\    {
    \\      "name": "go-test-summary-fail",
    \\      "trigger": "FAIL\t",
    \\      "confidence": 0.9,
    \\      "rules": [
    \\        { "capture": "FAIL\t{failed_pkg}\t{failed_duration}", "action": "keep" },
    \\        { "capture": "FAIL\t{counted_failed_pkg}\t{counted_failed_duration}", "action": "count", "as": "failed_packages" }
    \\      ],
    \\      "output": "go test: {failed_packages} packages failed | last {failed_pkg} | {failed_duration}"
    \\    },
    \\    { "name": "codex-tsc-success", "trigger": "Found 0 errors", "confidence": 0.99, "rules": [], "output": "tsc: 0 errors" },
    \\    { "name": "codex-eslint-fix", "trigger": "Fixed {count} files", "confidence": 0.96, "rules": [{ "capture": "Fixed {count} files", "action": "keep" }], "output": "eslint: fixed {count} files" },
    \\    { "name": "codex-bun-test", "trigger": " bun v", "confidence": 0.95, "rules": [{ "capture": "{passed} passed | {failed} failed", "action": "keep" }], "output": "bun test: {passed}/{failed}" },
    \\    { "name": "codex-cypress", "trigger": "Running:", "confidence": 0.95, "rules": [{ "capture": "{specs} specs {passed} passed {failed} failed", "action": "keep" }], "output": "cypress: {passed}/{specs}" },
    \\    { "name": "codex-playwright", "trigger": "Test Suites:", "confidence": 0.95, "rules": [{ "capture": "Test Suites: {suites} passed, {failed} failed", "action": "keep" }], "output": "playwright: {suites} suites" },
    \\    { "name": "codex-npm-install", "trigger": "added {d} packages", "confidence": 0.95, "rules": [{ "capture": "added {d} packages in {duration}", "action": "keep" }], "output": "npm: {d} packages" },
    \\    { "name": "codex-yarn-install", "trigger": "Done in", "confidence": 0.94, "rules": [{ "capture": "Done in {duration}", "action": "keep" }], "output": "yarn: {duration}" },
    \\    { "name": "codex-pnpm-summary", "trigger": "Done in", "confidence": 0.93, "rules": [{ "capture": "Done in {duration}", "action": "keep" }], "output": "pnpm: {duration}" },
    \\    { "name": "codex-bun-install", "trigger": "built @", "confidence": 0.93, "rules": [{ "capture": "bun install v{version}", "action": "keep" }], "output": "bun: v{version}" },
    \\    { "name": "codex-vite-build", "trigger": "built in", "confidence": 0.95, "rules": [{ "capture": "built in {duration}", "action": "keep" }], "output": "vite: {duration}" },
    \\    { "name": "codex-webpack", "trigger": "compiled successfully", "confidence": 0.97, "rules": [{ "capture": "compiled successfully in {duration}ms", "action": "keep" }], "output": "webpack: {duration}ms" },
    \\    { "name": "codex-webpack-errors", "trigger": "ERROR in", "confidence": 0.98, "rules": [{ "capture": "ERROR in {file}", "action": "keep" }], "output": "webpack: ERROR" },
    \\    { "name": "codex-nextjs", "trigger": "Route (app)", "confidence": 0.96, "rules": [], "output": "next.js: build complete" },
    \\    { "name": "codex-mypy", "trigger": "Success: no issues", "confidence": 0.98, "rules": [], "output": "mypy: no issues" },
    \\    { "name": "codex-mypy-errors", "trigger": "error:", "confidence": 0.97, "rules": [{ "capture": "Found {count} errors", "action": "keep" }], "output": "mypy: {count} errors" },
    \\    { "name": "codex-black", "trigger": "reformatted", "confidence": 0.90, "rules": [{ "capture": "reformatted {files} file", "action": "keep" }], "output": "black: reformatted" },
    \\    { "name": "codex-cargo-build", "trigger": "Finished", "confidence": 0.97, "rules": [{ "capture": "Finished dev target(s) in {duration}", "action": "keep" }], "output": "cargo: {duration}" },
    \\    { "name": "codex-rustfmt", "trigger": "Rustfiles unchanged", "confidence": 0.95, "rules": [], "output": "rustfmt: unchanged" },
    \\    { "name": "codex-clippy", "trigger": "Checks passed", "confidence": 0.97, "rules": [], "output": "clippy: passed" },
    \\    { "name": "codex-go-build", "trigger": "go build", "confidence": 0.95, "rules": [], "output": "go build: success" },
    \\    { "name": "codex-zig-build", "trigger": "Build Summary", "confidence": 0.96, "rules": [], "output": "zig build: success" },
    \\    { "name": "codex-docker-build", "trigger": "Successfully built", "confidence": 0.97, "rules": [{ "capture": "Successfully built {hash}", "action": "keep" }], "output": "docker: {hash}" },
    \\    { "name": "codex-docker-compose", "trigger": "Container started", "confidence": 0.93, "rules": [{ "capture": "Container {name} started", "action": "keep" }], "output": "docker-compose: {name}" },
    \\    { "name": "codex-kubectl", "trigger": "kubectl", "confidence": 0.85, "rules": [{ "capture": "pod/{name} {status}", "action": "keep" }], "output": "kubectl: {name} {status}" },
    \\    { "name": "codex-terraform-plan", "trigger": "Plan:", "confidence": 0.95, "rules": [{ "capture": "Plan: {to_add} to add, {to_change} to change, {to_destroy} to destroy", "action": "keep" }], "output": "terraform: +{to_add} ~{to_change} -{to_destroy}" },
    \\    { "name": "codex-terraform-apply", "trigger": "Apply complete", "confidence": 0.97, "rules": [], "output": "terraform: apply complete" },
    \\    { "name": "codex-helm", "trigger": "release", "confidence": 0.92, "rules": [{ "capture": "release {name} upgraded", "action": "keep" }], "output": "helm: {name}" },
    \\    { "name": "codex-gradle", "trigger": "BUILD SUCCESSFUL", "confidence": 0.97, "rules": [{ "capture": "BUILD SUCCESSFUL in {duration}", "action": "keep" }], "output": "gradle: {duration}" },
    \\    { "name": "codex-semgrep", "trigger": "Scan complete", "confidence": 0.95, "rules": [{ "capture": "Ran {rules} rules and found {issues} issues", "action": "keep" }], "output": "semgrep: {issues} issues" },
    \\    { "name": "codex-trivy", "trigger": "Total:", "confidence": 0.92, "rules": [{ "capture": "Total: {count}", "action": "keep" }], "output": "trivy: {count} vulns" },
    \\    { "name": "codex-gitleaks", "trigger": "no leaks", "confidence": 0.95, "rules": [], "output": "gitleaks: no leaks" },
    \\    { "name": "codex-nx", "trigger": "NX   Running target", "confidence": 0.94, "rules": [{ "capture": "NX   Successfully ran target {target}", "action": "keep" }], "output": "nx: {target}" },
    \\    { "name": "codex-make", "trigger": "make:", "confidence": 0.85, "rules": [{ "capture": "make `{target}`", "action": "keep" }], "output": "make: {target}" }
    \\  ]
    \\}
;

const ANTIGRAVITY_TEMPLATE_JSON =
    \\{
    \\  "rules": [
    \\    {
    \\      "name": "k8s_uid",
    \\      "match": "uid:",
    \\      "action": "mask"
    \\    },
    \\    {
    \\      "name": "k8s_managed_fields",
    \\      "match": "managedFields:",
    \\      "action": "remove"
    \\    },
    \\    {
    \\      "name": "tf_refresh",
    \\      "match": "Refreshing state...",
    \\      "action": "remove"
    \\    },
    \\    {
    \\      "name": "tf_no_changes",
    \\      "match": "No changes. Your infrastructure matches the configuration.",
    \\      "action": "mask"
    \\    },
    \\    {
    \\      "name": "docker_hash",
    \\      "match": "sha256:",
    \\      "action": "mask"
    \\    }
    \\  ],
    \\  "dsl_filters": []
    \\}
;

const OPENCODE_COMPLETE_TEMPLATE_JSON =
    \\{
    \\  "rules": [],
    \\  "dsl_filters": [
    \\    {
    \\      "name": "opencode-npm-install-summary",
    \\      "trigger": "added {d} packages",
    \\      "confidence": 0.95,
    \\      "rules": [
    \\        { "capture": "added {d} packages in {duration}", "action": "keep" }
    \\      ],
    \\      "output": "npm: {d} packages added | {duration}"
    \\    },
    \\    {
    \\      "name": "opencode-npm-audit-summary",
    \\      "trigger": "found {d} vulnerabilities",
    \\      "confidence": 0.97,
    \\      "rules": [
    \\        { "capture": "found {vulns} vulnerabilities ({high} high, {medium} moderate)", "action": "keep" }
    \\      ],
    \\      "output": "npm audit: {vulns} vulnerabilities | {high} high | {medium} moderate"
    \\    },
    \\    {
    \\      "name": "opencode-yarn-install-summary",
    \\      "trigger": "Done in",
    \\      "confidence": 0.94,
    \\      "rules": [
    \\        { "capture": "Done in {duration}", "action": "keep" },
    \\        { "capture": "{count} packages added", "action": "keep" }
    \\      ],
    \\      "output": "yarn: {count} packages | {duration}"
    \\    },
    \\    {
    \\      "name": "opencode-pnpm-install-summary",
    \\      "trigger": "Done in",
    \\      "confidence": 0.95,
    \\      "rules": [
    \\        { "capture": "Done in {duration}", "action": "keep" }
    \\      ],
    \\      "output": "pnpm: {duration}"
    \\    },
    \\    {
    \\      "name": "opencode-bun-install-summary",
    \\      "trigger": "built @",
    \\      "confidence": 0.93,
    \\      "rules": [
    \\        { "capture": "bun install v{version}", "action": "keep" }
    \\      ],
    \\      "output": "bun: {version}"
    \\    },
    \\    {
    \\      "name": "opencode-tsc-errors",
    \\      "trigger": "error TS",
    \\      "confidence": 0.98,
    \\      "rules": [
    \\        { "capture": "error TS{code}: {msg} ({file}:{line}:{col})", "action": "keep" },
    \\        { "capture": "Found {count} errors.", "action": "keep" }
    \\      ],
    \\      "output": "tsc: {count} errors | last: TS{code} {msg}"
    \\    },
    \\    {
    \\      "name": "opencode-tsc-success",
    \\      "trigger": "Found 0 errors",
    \\      "confidence": 0.99,
    \\      "rules": [],
    \\      "output": "tsc: 0 errors"
    \\    },
    \\    {
    \\      "name": "opencode-eslint-errors",
    \\      "trigger": "problems",
    \\      "confidence": 0.97,
    \\      "rules": [
    \\        { "capture": "{problems} problems ({errors} errors, {warnings} warnings)", "action": "keep" }
    \\      ],
    \\      "output": "eslint: {problems} problems | {errors} errors | {warnings} warnings"
    \\    },
    \\    {
    \\      "name": "opencode-prettier-format",
    \\      "trigger": "src/",
    \\      "confidence": 0.80,
    \\      "rules": [
    \\        { "capture": "Formatting {files} files", "action": "keep" }
    \\      ],
    \\      "output": "prettier: formatted"
    \\    },
    \\    {
    \\      "name": "opencode-nextjs-build",
    \\      "trigger": "Route (app)",
    \\      "confidence": 0.96,
    \\      "rules": [
    \\        { "capture": "Route {route} {size} {first_load}", "action": "keep" }
    \\      ],
    \\      "output": "next.js build complete"
    \\    },
    \\    {
    \\      "name": "opencode-nextjs-error",
    \\      "trigger": "Error: ",
    \\      "confidence": 0.98,
    \\      "rules": [
    \\        { "capture": "Error: {message}", "action": "keep" }
    \\      ],
    \\      "output": "next.js error: {message}"
    \\    },
    \\    {
    \\      "name": "opencode-vite-build",
    \\      "trigger": "built in",
    \\      "confidence": 0.95,
    \\      "rules": [
    \\        { "capture": "built in {duration}", "action": "keep" }
    \\      ],
    \\      "output": "vite: built in {duration}"
    \\    },
    \\    {
    \\      "name": "opencode-webpack-build",
    \\      "trigger": "compiled successfully",
    \\      "confidence": 0.97,
    \\      "rules": [
    \\        { "capture": "compiled successfully in {duration}ms", "action": "keep" }
    \\      ],
    \\      "output": "webpack: compiled in {duration}ms"
    \\    },
    \\    {
    \\      "name": "opencode-webpack-errors",
    \\      "trigger": "ERROR in",
    \\      "confidence": 0.98,
    \\      "rules": [
    \\        { "capture": "ERROR in {file}", "action": "keep" },
    \\        { "capture": "Module not found: {module}", "action": "keep" }
    \\      ],
    \\      "output": "webpack: module error"
    \\    },
    \\    {
    \\      "name": "opencode-jest-pass",
    \\      "trigger": "Tests:",
    \\      "confidence": 0.96,
    \\      "rules": [
    \\        { "capture": "Tests: {passed} passed, {total} total", "action": "keep" }
    \\      ],
    \\      "output": "jest: {passed}/{total} passed"
    \\    },
    \\    {
    \\      "name": "opencode-jest-fail",
    \\      "trigger": "failed",
    \\      "confidence": 0.97,
    \\      "rules": [
    \\        { "capture": "Tests: {failed} failed, {passed} passed", "action": "keep" }
    \\      ],
    \\      "output": "jest: {failed} failed | {passed} passed"
    \\    },
    \\    {
    \\      "name": "opencode-vitest-pass",
    \\      "trigger": "passed",
    \\      "confidence": 0.95,
    \\      "rules": [
    \\        { "capture": "Test Files  {passed}", "action": "keep" }
    \\      ],
    \\      "output": "vitest: {passed} passed"
    \\    },
    \\    {
    \\      "name": "opencode-cypress-run",
    \\      "trigger": "Running:",
    \\      "confidence": 0.95,
    \\      "rules": [
    \\        { "capture": "{specs} specs {passed} passed {failed} failed", "action": "keep" }
    \\      ],
    \\      "output": "cypress: {passed}/{specs} passed | {failed} failed"
    \\    },
    \\    {
    \\      "name": "opencode-playwright",
    \\      "trigger": "Test Suites:",
    \\      "confidence": 0.95,
    \\      "rules": [
    \\        { "capture": "Test Suites: {suites} passed, {failed} failed", "action": "keep" }
    \\      ],
    \\      "output": "playwright: {suites} suites | {failed} failed"
    \\    },
    \\    {
    \\      "name": "opencode-pytest-pass",
    \\      "trigger": " passed",
    \\      "confidence": 0.95,
    \\      "rules": [
    \\        { "capture": "{passed} passed in {duration}", "action": "keep" }
    \\      ],
    \\      "output": "pytest: {passed} passed | {duration}"
    \\    },
    \\    {
    \\      "name": "opencode-pytest-fail",
    \\      "trigger": " failed",
    \\      "confidence": 0.96,
    \\      "rules": [
    \\        { "capture": "{failed} failed, {passed} passed", "action": "keep" }
    \\      ],
    \\      "output": "pytest: {passed} passed | {failed} failed"
    \\    },
    \\    {
    \\      "name": "opencode-mypy-check",
    \\      "trigger": "Success: no issues",
    \\      "confidence": 0.98,
    \\      "rules": [],
    \\      "output": "mypy: no issues"
    \\    },
    \\    {
    \\      "name": "opencode-mypy-errors",
    \\      "trigger": "error:",
    \\      "confidence": 0.97,
    \\      "rules": [
    \\        { "capture": "Found {count} errors", "action": "keep" }
    \\      ],
    \\      "output": "mypy: {count} errors"
    \\    },
    \\    {
    \\      "name": "opencode-black-format",
    \\      "trigger": "reformatted",
    \\      "confidence": 0.90,
    \\      "rules": [
    \\        { "capture": "reformatted {files} file", "action": "keep" }
    \\      ],
    \\      "output": "black: reformatted {files} file"
    \\    },
    \\    {
    \\      "name": "opencode-isort",
    \\      "trigger": "ERROR",
    \\      "confidence": 0.85,
    \\      "rules": [
    \\        { "capture": "ERROR: {message}", "action": "keep" }
    \\      ],
    \\      "output": "isort: error - {message}"
    \\    },
    \\    {
    \\      "name": "opencode-pip-install",
    \\      "trigger": "Successfully installed",
    \\      "confidence": 0.95,
    \\      "rules": [
    \\        { "capture": "Successfully installed {packages}", "action": "keep" }
    \\      ],
    \\      "output": "pip: {packages}"
    \\    },
    \\    {
    \\      "name": "opencode-poetry-install",
    \\      "trigger": "Installing dependencies",
    \\      "confidence": 0.92,
    \\      "rules": [
    \\        { "capture": "Installing dependencies from lock file", "action": "keep" }
    \\      ],
    \\      "output": "poetry: installing dependencies"
    \\    },
    \\    {
    \\      "name": "opencode-cargo-build",
    \\      "trigger": "Finished",
    \\      "confidence": 0.97,
    \\      "rules": [
    \\        { "capture": "Finished dev [optimized + debuginfo] target(s) in {duration}", "action": "keep" }
    \\      ],
    \\      "output": "cargo: finished | {duration}"
    \\    },
    \\    {
    \\      "name": "opencode-cargo-test",
    \\      "trigger": "test result:",
    \\      "confidence": 0.98,
    \\      "rules": [
    \\        { "capture": "test result: {result}. {passed} passed; {failed} failed", "action": "keep" }
    \\      ],
    \\      "output": "cargo test: {passed} passed | {failed} failed"
    \\    },
    \\    {
    \\      "name": "opencode-go-build",
    \\      "trigger": "go build",
    \\      "confidence": 0.95,
    \\      "rules": [
    \\        { "capture": "go build -o {binary} {pkg}", "action": "keep" }
    \\      ],
    \\      "output": "go build: {binary}"
    \\    },
    \\    {
    \\      "name": "opencode-go-test",
    \\      "trigger": "ok  ",
    \\      "confidence": 0.95,
    \\      "rules": [
    \\        { "capture": "ok  {pkg}  {duration}", "action": "keep" }
    \\      ],
    \\      "output": "go test: {pkg} ok"
    \\    },
    \\    {
    \\      "name": "opencode-zig-build",
    \\      "trigger": "Build Summary",
    \\      "confidence": 0.96,
    \\      "rules": [
    \\        { "capture": "success", "action": "keep" }
    \\      ],
    \\      "output": "zig build: success"
    \\    },
    \\    {
    \\      "name": "opencode-docker-build",
    \\      "trigger": "Successfully built",
    \\      "confidence": 0.97,
    \\      "rules": [
    \\        { "capture": "Successfully built {hash}", "action": "keep" }
    \\      ],
    \\      "output": "docker: built {hash}"
    \\    },
    \\    {
    \\      "name": "opencode-docker-compose",
    \\      "trigger": "Container started",
    \\      "confidence": 0.93,
    \\      "rules": [
    \\        { "capture": "Container {name} started", "action": "keep" }
    \\      ],
    \\      "output": "docker-compose: {name} started"
    \\    },
    \\    {
    \\      "name": "opencode-kubectl",
    \\      "trigger": "kubectl",
    \\      "confidence": 0.85,
    \\      "rules": [
    \\        { "capture": "pod/{name} {status}", "action": "keep" }
    \\      ],
    \\      "output": "kubectl: {name} {status}"
    \\    },
    \\    {
    \\      "name": "opencode-terraform-plan",
    \\      "trigger": "Plan:",
    \\      "confidence": 0.95,
    \\      "rules": [
    \\        { "capture": "Plan: {to_add} to add, {to_change} to change, {to_destroy} to destroy", "action": "keep" }
    \\      ],
    \\      "output": "terraform: {to_add} add | {to_change} change | {to_destroy} destroy"
    \\    },
    \\    {
    \\      "name": "opencode-terraform-apply",
    \\      "trigger": "Apply complete",
    \\      "confidence": 0.97,
    \\      "rules": [
    \\        { "capture": "Apply complete! Resources: {count} added", "action": "keep" }
    \\      ],
    \\      "output": "terraform: apply complete"
    \\    },
    \\    {
    \\      "name": "opencode-ansible",
    \\      "trigger": "PLAY RECAP",
    \\      "confidence": 0.95,
    \\      "rules": [
    \\        { "capture": "ok={ok} changed={changed} unreachable={unreachable} failed={failed}", "action": "keep" }
    \\      ],
    \\      "output": "ansible: ok={ok} changed={changed} failed={failed}"
    \\    },
    \\    {
    \\      "name": "opencode-gradle-build",
    \\      "trigger": "BUILD SUCCESSFUL",
    \\      "confidence": 0.97,
    \\      "rules": [
    \\        { "capture": "BUILD SUCCESSFUL in {duration}", "action": "keep" }
    \\      ],
    \\      "output": "gradle: BUILD SUCCESSFUL | {duration}"
    \\    },
    \\    {
    \\      "name": "opencode-gradle-fail",
    \\      "trigger": "BUILD FAILED",
    \\      "confidence": 0.98,
    \\      "rules": [
    \\        { "capture": "FAILURE: {message}", "action": "keep" }
    \\      ],
    \\      "output": "gradle: BUILD FAILED"
    \\    },
    \\    {
    \\      "name": "opencode-android-build",
    \\      "trigger": "BUILD SUCCESSFUL",
    \\      "confidence": 0.96,
    \\      "rules": [
    \\        { "capture": "BUILD SUCCESSFUL", "action": "keep" }
    \\      ],
    \\      "output": "android: BUILD SUCCESSFUL"
    \\    },
    \\    {
    \\      "name": "opencode-flutter-build",
    \\      "trigger": "Running Gradle",
    \\      "confidence": 0.90,
    \\      "rules": [
    \\        { "capture": "Built build/app/outputs/flutter-apk/{apk}", "action": "keep" }
    \\      ],
    \\      "output": "flutter: built {apk}"
    \\    },
    \\    {
    \\      "name": "opencode-react-native",
    \\      "trigger": "BUILD SUCCESSFUL",
    \\      "confidence": 0.95,
    \\      "rules": [],
    \\      "output": "react-native: BUILD SUCCESSFUL"
    \\    },
    \\    {
    \\      "name": "opencode-composer-install",
    \\      "trigger": "Installing from cache",
    \\      "confidence": 0.92,
    \\      "rules": [
    \\        { "capture": "Installing {count} packages", "action": "keep" }
    \\      ],
    \\      "output": "composer: {count} packages"
    \\    },
    \\    {
    \\      "name": "opencode-bundle-install",
    \\      "trigger": "Bundle complete",
    \\      "confidence": 0.95,
    \\      "rules": [
    \\        { "capture": "Bundle complete! {count} Gemfile dependencies", "action": "keep" }
    \\      ],
    \\      "output": "bundle: {count} gems"
    \\    },
    \\    {
    \\      "name": "opencode-dotnet-build",
    \\      "trigger": "Build succeeded",
    \\      "confidence": 0.97,
    \\      "rules": [
    \\        { "capture": "Build succeeded. {warnings} Warning(s)", "action": "keep" }
    \\      ],
    \\      "output": "dotnet: Build succeeded | {warnings} warnings"
    \\    },
    \\    {
    \\      "name": "opencode-dotnet-test",
    \\      "trigger": "Passed!  - Failed",
    \\      "confidence": 0.96,
    \\      "rules": [
    \\        { "capture": "Passed!  - Failed:     0, Passed: {passed}, Skipped: {skipped}", "action": "keep" }
    \\      ],
    \\      "output": "dotnet test: {passed} passed"
    \\    },
    \\    {
    \\      "name": "opencode-helm",
    \\      "trigger": "release my-chart",
    \\      "confidence": 0.92,
    \\      "rules": [
    \\        { "capture": "release my-chart {version} has been upgraded", "action": "keep" }
    \\      ],
    \\      "output": "helm: my-chart upgraded"
    \\    },
    \\    {
    \\      "name": "opencode-packer",
    \\      "trigger": "Build finished",
    \\      "confidence": 0.93,
    \\      "rules": [
    \\        { "capture": "Build 'template' finished", "action": "keep" }
    \\      ],
    \\      "output": "packer: build finished"
    \\    },
    \\    {
    \\      "name": "opencode-terragrunt",
    \\      "trigger": "Running tgfmt",
    \\      "confidence": 0.88,
    \\      "rules": [
    \\        { "capture": "Terraform initialized", "action": "keep" }
    \\      ],
    \\      "output": "terragrunt: initialized"
    \\    },
    \\    {
    \\      "name": "opencode-make",
    \\      "trigger": "make:",
    \\      "confidence": 0.85,
    \\      "rules": [
    \\        { "capture": "make `{target}`", "action": "keep" }
    \\      ],
    \\      "output": "make: {target}"
    \\    },
    \\    {
    \\      "name": "opencode-eslint-fix",
    \\      "trigger": "Fixed {count} files",
    \\      "confidence": 0.96,
    \\      "rules": [
    \\        { "capture": "Fixed {count} files", "action": "keep" }
    \\      ],
    \\      "output": "eslint: fixed {count} files"
    \\    },
    \\    {
    \\      "name": "opencode-prettier-check",
    \\      "trigger": "Checking format...",
    \\      "confidence": 0.90,
    \\      "rules": [
    \\        { "capture": "Matched {count} files", "action": "keep" }
    \\      ],
    \\      "output": "prettier: {count} files checked"
    \\    },
    \\    {
    \\      "name": "opencode-nx-build",
    \\      "trigger": "NX   Running target build",
    \\      "confidence": 0.94,
    \\      "rules": [
    \\        { "capture": "NX   Successfully ran target build", "action": "keep" }
    \\      ],
    \\      "output": "nx: build complete"
    \\    },
    \\    {
    \\      "name": "opencode-nx-affected",
    \\      "trigger": "NX   Affected criteria changed",
    \\      "confidence": 0.92,
    \\      "rules": [
    \\        { "capture": "NX   Ran {count} tasks", "action": "keep" }
    \\      ],
    \\      "output": "nx: {count} tasks affected"
    \\    },
    \\    {
    \\      "name": "opencode-bun-test",
    \\      "trigger": " bun v",
    \\      "confidence": 0.95,
    \\      "rules": [
    \\        { "capture": "{passed} passed | {failed} failed", "action": "keep" }
    \\      ],
    \\      "output": "bun test: {passed} passed | {failed} failed"
    \\    },
    \\    {
    \\      "name": "opencode-rustfmt",
    \\      "trigger": "Rustfiles unchanged",
    \\      "confidence": 0.95,
    \\      "rules": [],
    \\      "output": "rustfmt: unchanged"
    \\    },
    \\    {
    \\      "name": "opencode-clippy",
    \\      "trigger": "Checks passed",
    \\      "confidence": 0.97,
    \\      "rules": [],
    \\      "output": "clippy: checks passed"
    \\    },
    \\    {
    \\      "name": "opencode-gitleaks",
    \\      "trigger": "no leaks",
    \\      "confidence": 0.95,
    \\      "rules": [],
    \\      "output": "gitleaks: no leaks detected"
    \\    },
    \\    {
    \\      "name": "opencode-semgrep",
    \\      "trigger": "Scan complete",
    \\      "confidence": 0.95,
    \\      "rules": [
    \\        { "capture": "Ran {rules} rules and found {issues} issues", "action": "keep" }
    \\      ],
    \\      "output": "semgrep: {issues} issues | {rules} rules"
    \\    },
    \\    {
    \\      "name": "opencode-snyk",
    \\      "trigger": "Snyk policy report",
    \\      "confidence": 0.93,
    \\      "rules": [
    \\        { "capture": "Package manager:  {pm}", "action": "keep" }
    \\      ],
    \\      "output": "snyk: scan complete"
    \\    },
    \\    {
    \\      "name": "opencode-trivy",
    \\      "trigger": "Total: {count}",
    \\      "confidence": 0.92,
    \\      "rules": [
    \\        { "capture": "Total: {count} (UNKNOWN:{unknown}, LOW:{low}, MEDIUM:{medium}, HIGH:{high}, CRITICAL:{critical})", "action": "keep" }
    \\      ],
    \\      "output": "trivy: {count} vulnerabilities | {critical} critical"
    \\    },
    \\    {
    \\      "name": "opencode-hadolint",
    \\      "trigger": "hadolint",
    \\      "confidence": 0.90,
    \\      "rules": [
    \\        { "capture": "Dockerfile parsed successfully", "action": "keep" }
    \\      ],
    \\      "output": "hadolint: Dockerfile OK"
    \\    },
    \\    {
    \\      "name": "opencode-kubesec",
    \\      "trigger": "kubesec",
    \\      "confidence": 0.88,
    \\      "rules": [
    \\        { "capture": "Failed to score", "action": "keep" }
    \\      ],
    \\      "output": "kubesec: scan complete"
    \\    },
    \\    {
    \\      "name": "opencode-skaffold",
    \\      "trigger": "Deployments stabilized",
    \\      "confidence": 0.92,
    \\      "rules": [
    \\        { "capture": "deployments stabilized in {duration}", "action": "keep" }
    \\      ],
    \\      "output": "skaffold: deployed | {duration}"
    \\    },
    \\    {
    \\      "name": "opencode-argocd",
    \\      "trigger": "ArgoCD",
    \\      "confidence": 0.88,
    \\      "rules": [
    \\        { "capture": "health status = {status}", "action": "keep" }
    \\      ],
    \\      "output": "argocd: {status}"
    \\    },
    \\    {
    \\      "name": "opencode-dagger",
    \\      "trigger": "✔",
    \\      "confidence": 0.85,
    \\      "rules": [
    \\        { "capture": "{step}  {duration}", "action": "keep" }
    \\      ],
    \\      "output": "dagger: {step}"
    \\    }
    \\  ]
    \\}
;

const ConfigMergeResult = struct {
    path: []const u8,
    added_rules: usize,
    added_filters: usize,
};

fn ensureGlobalConfigFromTemplate(alloc: std.mem.Allocator, home: []const u8, template_json: []const u8) !ConfigMergeResult {
    const omni_dir = try std.fmt.allocPrint(alloc, "{s}/.omni", .{home});
    std.fs.cwd().makePath(omni_dir) catch {};

    const config_path = try std.fmt.allocPrint(alloc, "{s}/omni_config.json", .{omni_dir});

    var root_obj = std.json.ObjectMap.init(alloc);
    const file_or_err = std.fs.cwd().openFile(config_path, .{});
    if (file_or_err) |file| {
        defer file.close();
        const content = try file.readToEndAlloc(alloc, 1024 * 1024);
        const parsed = try std.json.parseFromSlice(std.json.Value, alloc, content, .{});
        if (parsed.value == .object) {
            root_obj = parsed.value.object;
        }
    } else |_| {}

    var rules_array = if (root_obj.get("rules")) |node|
        if (node == .array) node.array else std.json.Array.init(alloc)
    else
        std.json.Array.init(alloc);

    var dsl_filters_array = if (root_obj.get("dsl_filters")) |node|
        if (node == .array) node.array else std.json.Array.init(alloc)
    else
        std.json.Array.init(alloc);

    const template_parsed = try std.json.parseFromSlice(std.json.Value, alloc, template_json, .{});
    var added_rules: usize = 0;
    var added_filters: usize = 0;

    if (template_parsed.value == .object) {
        if (template_parsed.value.object.get("rules")) |template_rules_node| {
            if (template_rules_node == .array) {
                for (template_rules_node.array.items) |rule_node| {
                    if (rule_node != .object) continue;
                    const name_node = rule_node.object.get("name") orelse continue;
                    if (name_node != .string) continue;

                    var exists = false;
                    for (rules_array.items) |existing_node| {
                        if (existing_node != .object) continue;
                        const existing_name_node = existing_node.object.get("name") orelse continue;
                        if (existing_name_node != .string) continue;
                        if (std.mem.eql(u8, existing_name_node.string, name_node.string)) {
                            exists = true;
                            break;
                        }
                    }

                    if (!exists) {
                        try rules_array.append(rule_node);
                        added_rules += 1;
                    }
                }
            }
        }

        if (template_parsed.value.object.get("dsl_filters")) |template_filters_node| {
            if (template_filters_node == .array) {
                for (template_filters_node.array.items) |filter_node| {
                    if (filter_node != .object) continue;
                    const name_node = filter_node.object.get("name") orelse continue;
                    if (name_node != .string) continue;

                    var exists = false;
                    for (dsl_filters_array.items) |existing_node| {
                        if (existing_node != .object) continue;
                        const existing_name_node = existing_node.object.get("name") orelse continue;
                        if (existing_name_node != .string) continue;
                        if (std.mem.eql(u8, existing_name_node.string, name_node.string)) {
                            exists = true;
                            break;
                        }
                    }

                    if (!exists) {
                        try dsl_filters_array.append(filter_node);
                        added_filters += 1;
                    }
                }
            }
        }
    }

    try root_obj.put("rules", std.json.Value{ .array = rules_array });
    try root_obj.put("dsl_filters", std.json.Value{ .array = dsl_filters_array });

    const out_file = try std.fs.cwd().createFile(config_path, .{ .truncate = true });
    defer out_file.close();

    var write_buf: [4096]u8 = undefined;
    var file_writer = out_file.writer(&write_buf);
    try std.json.Stringify.value(std.json.Value{ .object = root_obj }, .{ .whitespace = .indent_2 }, &file_writer.interface);
    try file_writer.end();

    return .{ .path = config_path, .added_rules = added_rules, .added_filters = added_filters };
}

fn ensureGlobalCodexPolyglotConfig(alloc: std.mem.Allocator, home: []const u8) !ConfigMergeResult {
    return ensureGlobalConfigFromTemplate(alloc, home, CODEX_POLYGLOT_TEMPLATE_JSON);
}

fn ensureGlobalOpencodeFiltersConfig(alloc: std.mem.Allocator, home: []const u8) !ConfigMergeResult {
    return ensureGlobalConfigFromTemplate(alloc, home, OPENCODE_COMPLETE_TEMPLATE_JSON);
}

fn ensureGlobalAntigravityConfig(alloc: std.mem.Allocator, home: []const u8) !ConfigMergeResult {
    return ensureGlobalConfigFromTemplate(alloc, home, ANTIGRAVITY_TEMPLATE_JSON);
}

fn autoConfigureOpencode(alloc: std.mem.Allocator, home: []const u8, absolute_omni_path: []const u8) !void {
    const stdout = std.fs.File.stdout().deprecatedWriter();
    const config_path = try std.fmt.allocPrint(alloc, "{s}/.config/opencode/opencode.json", .{home});

    if (std.fs.path.dirname(config_path)) |dir| {
        std.fs.cwd().makePath(dir) catch {};
    }

    var root_obj: std.json.ObjectMap = undefined;
    var mcp_obj: std.json.ObjectMap = undefined;

    const file_or_err = std.fs.cwd().openFile(config_path, .{});
    if (file_or_err) |file| {
        defer file.close();
        const content = try file.readToEndAlloc(alloc, 1024 * 1024);
        const parsed_json = std.json.parseFromSlice(std.json.Value, alloc, content, .{}) catch |e| {
            try stdout.print("Failed to parse existing opencode.json: {any}\n", .{e});
            root_obj = std.json.ObjectMap.init(alloc);
            mcp_obj = std.json.ObjectMap.init(alloc);
            return;
        };
        if (parsed_json.value == .object) {
            root_obj = parsed_json.value.object;
        } else {
            root_obj = std.json.ObjectMap.init(alloc);
        }
    } else |_| {
        root_obj = std.json.ObjectMap.init(alloc);
    }

    if (root_obj.get("mcp")) |mcp_node| {
        if (mcp_node == .object) {
            mcp_obj = mcp_node.object;
        } else {
            mcp_obj = std.json.ObjectMap.init(alloc);
        }
    } else {
        mcp_obj = std.json.ObjectMap.init(alloc);
    }

    var omni_server_obj = std.json.ObjectMap.init(alloc);
    try omni_server_obj.put("type", std.json.Value{ .string = "local" });

    var command_array = std.json.Array.init(alloc);
    try command_array.append(std.json.Value{ .string = "node" });
    try command_array.append(std.json.Value{ .string = absolute_omni_path });
    try command_array.append(std.json.Value{ .string = "--agent=opencode" });
    try omni_server_obj.put("command", std.json.Value{ .array = command_array });

    try omni_server_obj.put("enabled", std.json.Value{ .bool = true });

    try mcp_obj.put("omni", std.json.Value{ .object = omni_server_obj });
    try root_obj.put("mcp", std.json.Value{ .object = mcp_obj });

    try ui.printHeader(stdout, "🤖 OMNI MCP OPENCODE INTEGRATION");
    try ui.row(stdout, ui.BOLD ++ "Target: " ++ ui.RESET ++ "OpenCode AI");
    try ui.row(stdout, "");

    const out_file = try std.fs.cwd().createFile(config_path, .{ .truncate = true });
    defer out_file.close();

    var write_buf: [4096]u8 = undefined;
    var file_writer = out_file.writer(&write_buf);
    try std.json.Stringify.value(std.json.Value{ .object = root_obj }, .{ .whitespace = .indent_2 }, &file_writer.interface);
    try file_writer.end();

    try ui.row(stdout, ui.GREEN ++ " ● " ++ ui.RESET ++ "Successfully configured OMNI for OpenCode.");
    {
        const l = try std.fmt.allocPrint(alloc, "   Path: " ++ ui.DIM ++ "{s}" ++ ui.RESET, .{config_path});
        defer alloc.free(l);
        try ui.row(stdout, l);
    }

    const omni_config_result = try ensureGlobalOpencodeFiltersConfig(alloc, home);

    try ui.row(stdout, "");
    {
        const l = try std.fmt.allocPrint(alloc, ui.GREEN ++ " ● " ++ ui.RESET ++ "Added {d} AI Coding filters to omni config.", .{omni_config_result.added_filters});
        defer alloc.free(l);
        try ui.row(stdout, l);
    }
    {
        const l = try std.fmt.allocPrint(alloc, "   Filters: " ++ ui.DIM ++ "{s}" ++ ui.RESET, .{omni_config_result.path});
        defer alloc.free(l);
        try ui.row(stdout, l);
    }

    try ui.row(stdout, "");
    try ui.row(stdout, ui.BOLD ++ "Token-Efficient AI Coding Setup Complete!" ++ ui.RESET);
    try ui.row(stdout, "");
    try ui.row(stdout, "  " ++ ui.CYAN ++ "1." ++ ui.RESET ++ " Restart OpenCode to use OMNI MCP");
    try ui.row(stdout, "  " ++ ui.CYAN ++ "2." ++ ui.RESET ++ " Run: opencode mcp list  " ++ ui.DIM ++ "# Verify OMNI is registered" ++ ui.RESET);
    try ui.row(stdout, "  " ++ ui.CYAN ++ "3." ++ ui.RESET ++ " Use: git diff | omni   " ++ ui.DIM ++ "# Test distillation" ++ ui.RESET);
    try ui.row(stdout, "");
    try ui.row(stdout, ui.DIM ++ "Supported: npm, yarn, pnpm, tsc, eslint, jest, vitest," ++ ui.RESET);
    try ui.row(stdout, ui.DIM ++ "          pytest, ruff, mypy, cargo, go, docker," ++ ui.RESET);
    try ui.row(stdout, ui.DIM ++ "          kubectl, terraform, gradle, and 50+ more!" ++ ui.RESET);
    try ui.printFooter(stdout);
    try stdout.print("\n", .{});
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

    try ui.printHeader(stdout, "🤖 OMNI MCP ANTIGRAVITY INTEGRATION");
    try ui.row(stdout, ui.BOLD ++ "Target: " ++ ui.RESET ++ "Google Antigravity");
    try ui.row(stdout, "");

    // Write back to file
    const out_file = try std.fs.cwd().createFile(config_path, .{ .truncate = true });
    defer out_file.close();

    var write_buf: [4096]u8 = undefined;
    var file_writer = out_file.writer(&write_buf);
    try std.json.Stringify.value(std.json.Value{ .object = root_obj }, .{ .whitespace = .indent_2 }, &file_writer.interface);
    try file_writer.end();

    const merge_result = try ensureGlobalAntigravityConfig(alloc, home);

    try ui.row(stdout, ui.GREEN ++ " ● " ++ ui.RESET ++ "Successfully merged configuration.");
    {
        const l = try std.fmt.allocPrint(alloc, "   Path: " ++ ui.DIM ++ "{s}" ++ ui.RESET, .{config_path});
        defer alloc.free(l);
        try ui.row(stdout, l);
    }
    {
        const l = try std.fmt.allocPrint(alloc, "   Filters: " ++ ui.DIM ++ "{s}" ++ ui.RESET ++ " ({d} rules, {d} filters added)", .{ merge_result.path, merge_result.added_rules, merge_result.added_filters });
        defer alloc.free(l);
        try ui.row(stdout, l);
    }
    try ui.row(stdout, "");
    try ui.row(stdout, "OMNI is now registered as an Antigravity MCP server.");
    try ui.row(stdout, ui.CYAN ++ "▸" ++ ui.RESET ++ " Please restart Antigravity to apply changes.");
    try ui.row(stdout, ui.CYAN ++ "▸" ++ ui.RESET ++ " Cloud-native filters for Kubernetes, Terraform, and Docker layers are now in your global OMNI config.");
    try ui.printFooter(stdout);
    try stdout.print("\n", .{});
}

fn handleSetup() !void {
    const stdout = std.fs.File.stdout().deprecatedWriter();
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

                    exe_dir = std.fmt.allocPrint(alloc, "{s}" ++ std.fs.path.sep_str ++ "opt" ++ std.fs.path.sep_str ++ "omni{s}", .{ prefix, suffix }) catch exe_dir;
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

    try stdout.print("\n", .{});
    try ui.printHeader(stdout, "🌌 OMNI QUICKSTART");

    try ui.row(stdout, ui.BOLD ++ "Step 1: Verify Installation" ++ ui.RESET);
    try ui.row(stdout, "  omni --version              " ++ ui.DIM ++ "# Should print OMNI Core vX.X.X" ++ ui.RESET);
    try ui.row(stdout, "  omni monitor                " ++ ui.DIM ++ "# Check engine status" ++ ui.RESET);
    try ui.row(stdout, "");

    try ui.row(stdout, ui.BOLD ++ "Step 2: Generate Config Automatically" ++ ui.RESET);
    try ui.row(stdout, "  omni generate claude-code   " ++ ui.DIM ++ "# Auto-config for Claude" ++ ui.RESET);
    try ui.row(stdout, "  omni generate codex         " ++ ui.DIM ++ "# Auto-config for Codex" ++ ui.RESET);
    try ui.row(stdout, "  omni generate antigravity   " ++ ui.DIM ++ "# Auto-config for Antigravity" ++ ui.RESET);
    try ui.row(stdout, "  omni generate opencode      " ++ ui.DIM ++ "# Auto-config for OpenCode" ++ ui.RESET);
    try ui.row(stdout, "");

    try ui.row(stdout, ui.BOLD ++ "Step 3: Use OMNI Everywhere" ++ ui.RESET);
    try ui.row(stdout, "  git diff | omni                     " ++ ui.DIM ++ "# Distill git output" ++ ui.RESET);
    try ui.row(stdout, "  docker build . 2>&1 | omni          " ++ ui.DIM ++ "# Distill docker output" ++ ui.RESET);
    try ui.row(stdout, "  omni_apply_template(\"node-verbose\")  " ++ ui.DIM ++ "# Compact tsc/eslint/jest output" ++ ui.RESET);
    try ui.row(stdout, "  omni_apply_template(\"codex-polyglot\")" ++ ui.DIM ++ "# Add summaries (30+ languages)" ++ ui.RESET);
    try ui.row(stdout, "  omni density < logs.txt             " ++ ui.DIM ++ "# Analyze token density" ++ ui.RESET);
    try ui.row(stdout, "");

    try ui.row(stdout, ui.BOLD ++ ui.GREEN ++ "OMNI is mission-ready." ++ ui.RESET);
    try ui.printFooter(stdout);
    try stdout.print("\n", .{});
}

fn handleUpdate(allocator: std.mem.Allocator) !void {
    try std.fs.File.stdout().deprecatedWriter().print(ui.CYAN ++ " ▸ " ++ ui.RESET ++ "Checking for updates...\n", .{});

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
            try std.fs.File.stdout().deprecatedWriter().print(ui.GREEN ++ " ● " ++ ui.RESET ++ "OMNI is up to date (v{s}).\n", .{current_version});
        } else {
            try std.fs.File.stdout().deprecatedWriter().print(ui.YELLOW ++ " ○ " ++ ui.RESET ++ "A new version of OMNI is available: " ++ ui.BOLD ++ "{s}" ++ ui.RESET ++ " (current: v{s})\n", .{ latest_tag, current_version });

            // Detect How to Update (Homebrew vs Installer)
            var buffer: [std.fs.max_path_bytes]u8 = undefined;
            if (std.fs.selfExePath(&buffer)) |exe_path| {
                if (std.mem.indexOf(u8, exe_path, "Cellar") != null or std.mem.indexOf(u8, exe_path, "homebrew") != null) {
                    try std.fs.File.stdout().deprecatedWriter().print("\nTo update, run:\n  " ++ ui.CYAN ++ "brew upgrade fajarhide/tap/omni" ++ ui.RESET ++ "\n", .{});
                } else {
                    try std.fs.File.stdout().deprecatedWriter().print("\nTo update, run the installer:\n  " ++ ui.CYAN ++ "curl -fsSL https://raw.githubusercontent.com/fajarhide/omni/main/install.sh | sh" ++ ui.RESET ++ "\n", .{});
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

    try std.fs.File.stdout().deprecatedWriter().print(ui.MAGENTA ++ " ▸ " ++ ui.RESET ++ "Starting OMNI Uninstall...\n", .{});

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

    try std.fs.File.stdout().deprecatedWriter().print("\n" ++ ui.GREEN ++ " ● " ++ ui.RESET ++ "OMNI has been successfully uninstalled.\n", .{});
    try std.fs.File.stdout().deprecatedWriter().print(ui.DIM ++ "Note: If you installed via Homebrew, also run: brew uninstall omni" ++ ui.RESET ++ "\n", .{});
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
