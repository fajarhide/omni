const std = @import("std");
const metrics = @import("local_metrics.zig");
const ui = @import("ui.zig");

fn truncateLabel(allocator: std.mem.Allocator, label: []const u8, max_chars: usize) ![]u8 {
    if (ui.visibleLen(label) <= max_chars) return allocator.dupe(u8, label);
    if (max_chars <= 3) return allocator.dupe(u8, "...");

    var out = std.ArrayListUnmanaged(u8){};
    errdefer out.deinit(allocator);

    var visible: usize = 0;
    var i: usize = 0;
    while (i < label.len and visible < max_chars - 3) {
        const start = i;
        const c = label[i];
        if ((c & 0x80) == 0) {
            i += 1;
        } else if ((c & 0xE0) == 0xC0) {
            i += 2;
        } else if ((c & 0xF0) == 0xE0) {
            i += 3;
        } else if ((c & 0xF8) == 0xF0) {
            i += 4;
        } else {
            i += 1;
        }
        try out.appendSlice(allocator, label[start..i]);
        visible += 1;
    }

    try out.appendSlice(allocator, "...");
    return out.toOwnedSlice(allocator);
}

fn joinTokens(allocator: std.mem.Allocator, tokens: []const []const u8, separator: []const u8) ![]u8 {
    var out = std.ArrayListUnmanaged(u8){};
    errdefer out.deinit(allocator);

    for (tokens, 0..) |token, idx| {
        if (idx > 0) try out.appendSlice(allocator, separator);
        try out.appendSlice(allocator, token);
    }

    return out.toOwnedSlice(allocator);
}

fn compactFilterLabel(allocator: std.mem.Allocator, label: []const u8, max_chars: usize) ![]u8 {
    if (std.mem.indexOfScalar(u8, label, '-')) |_| {
        var parts = std.ArrayListUnmanaged([]const u8){};
        defer parts.deinit(allocator);

        var it = std.mem.splitScalar(u8, label, '-');
        while (it.next()) |part| {
            if (part.len > 0) try parts.append(allocator, part);
        }

        if (parts.items.len >= 2) {
            var prefix: []const u8 = "";
            var start_index: usize = 0;
            if (std.mem.eql(u8, parts.items[0], "codex")) {
                prefix = "codex/";
                start_index = 1;
            }

            if (start_index < parts.items.len) {
                const mergeable = [_][]const u8{ "build", "diff", "install" };
                var base_end = start_index + 1;
                if (base_end < parts.items.len) {
                    for (mergeable) |merge_token| {
                        if (std.mem.eql(u8, parts.items[base_end], merge_token)) {
                            base_end += 1;
                            break;
                        }
                    }
                }

                const base = try joinTokens(allocator, parts.items[start_index..base_end], "-");
                defer allocator.free(base);

                var qualifiers = std.ArrayListUnmanaged([]const u8){};
                defer qualifiers.deinit(allocator);
                for (parts.items[base_end..]) |part| {
                    if (std.mem.eql(u8, part, "summary") or std.mem.eql(u8, part, "rich")) continue;
                    try qualifiers.append(allocator, part);
                }

                const qualifiers_joined = if (qualifiers.items.len > 0)
                    try joinTokens(allocator, qualifiers.items, "-")
                else
                    try allocator.dupe(u8, "");
                defer allocator.free(qualifiers_joined);

                const compact = if (qualifiers.items.len > 0)
                    try std.fmt.allocPrint(allocator, "{s}{s}:{s}", .{ prefix, base, qualifiers_joined })
                else
                    try std.fmt.allocPrint(allocator, "{s}{s}", .{ prefix, base });
                defer allocator.free(compact);

                if (ui.visibleLen(compact) <= max_chars) return allocator.dupe(u8, compact);
            }
        }
    }

    return truncateLabel(allocator, label, max_chars);
}

fn normalizeAgentLabel(agent: []const u8) []const u8 {
    if (std.mem.eql(u8, agent, "CLI")) return "native-cli";
    return agent;
}

fn deriveProfileName(filter_name: []const u8) []const u8 {
    if (std.mem.eql(u8, filter_name, "cache")) return "cache";
    if (std.mem.eql(u8, filter_name, "custom")) return "custom";
    if (std.mem.eql(u8, filter_name, "cat")) return "cat";
    if (std.mem.eql(u8, filter_name, "build")) return "build";
    if (std.mem.eql(u8, filter_name, "docker")) return "docker";
    if (std.mem.eql(u8, filter_name, "git")) return "git";
    if (std.mem.eql(u8, filter_name, "node")) return "node";
    if (std.mem.eql(u8, filter_name, "sql")) return "sql";

    if (std.mem.startsWith(u8, filter_name, "codex-") or std.mem.startsWith(u8, filter_name, "codex/")) return "codex";
    if (std.mem.startsWith(u8, filter_name, "claude-") or std.mem.startsWith(u8, filter_name, "claude/")) return "claude";
    if (std.mem.startsWith(u8, filter_name, "opencode-") or std.mem.startsWith(u8, filter_name, "opencode/")) return "opencode";
    if (std.mem.startsWith(u8, filter_name, "antigravity-") or std.mem.startsWith(u8, filter_name, "antigravity/")) return "antigravity";

    if (std.mem.indexOfAny(u8, filter_name, "-/")) |idx| {
        return filter_name[0..idx];
    }

    return filter_name;
}

fn isNoiseFilterName(filter_name: []const u8) bool {
    if (filter_name.len == 0) return true;
    for (filter_name) |c| {
        if (std.mem.eql(u8, filter_name, "")) return true;
        if (c == '-' or c == '/' or c == ':' or c == '_' or c == '.') continue;
        if (std.ascii.isAlphanumeric(c)) continue;
        return true;
    }
    return false;
}
const AGENT_FILTER_SUMMARY_LIMIT: usize = 4;

fn renderAgentFilterBreakdown(
    allocator: std.mem.Allocator,
    stdout: anytype,
    records: []metrics.Record,
    agent_label: []const u8,
) !void {
    var filter_map = std.StringHashMap(metrics.Stats).init(allocator);
    defer filter_map.deinit();

    for (records) |rec| {
        if (!std.mem.eql(u8, rec.agent, agent_label)) continue;
        var entry = try filter_map.getOrPut(rec.filter_name);
        if (!entry.found_existing) entry.value_ptr.* = .{};
        entry.value_ptr.add(rec);
    }

    if (filter_map.count() == 0) {
        try ui.row(stdout, "  Filters: " ++ ui.DIM ++ "none recorded" ++ ui.RESET);
        return;
    }

    var rows = std.ArrayListUnmanaged(metrics.GroupedStats){};
    defer rows.deinit(allocator);
    var it = filter_map.iterator();
    while (it.next()) |entry| try rows.append(allocator, .{ .label = entry.key_ptr.*, .stats = entry.value_ptr.* });

    for (0..rows.items.len) |i| {
        for (0..rows.items.len - i - 1) |j| {
            if (rows.items[j].stats.saved < rows.items[j + 1].stats.saved) {
                const temp = rows.items[j];
                rows.items[j] = rows.items[j + 1];
                rows.items[j + 1] = temp;
            }
        }
    }

    const limit = if (rows.items.len <= AGENT_FILTER_SUMMARY_LIMIT) rows.items.len else AGENT_FILTER_SUMMARY_LIMIT;
    try ui.row(stdout, "  Filters:");

    for (rows.items[0..limit], 0..) |row, idx| {
        const short_label = try compactFilterLabel(allocator, row.label, 24);
        defer allocator.free(short_label);
        const saved_str = try metrics.formatBytes(allocator, row.stats.saved);
        defer allocator.free(saved_str);
        const line = try std.fmt.allocPrint(
            allocator,
            "    {d}. " ++ ui.CYAN ++ "{s}" ++ ui.RESET ++ "  {d} runs  {s} saved",
            .{ idx + 1, short_label, row.stats.cmds, saved_str },
        );
        defer allocator.free(line);
        try ui.row(stdout, line);
    }

    if (rows.items.len > limit) {
        const remaining = rows.items.len - limit;
        const more = try std.fmt.allocPrint(allocator, "    ... and {d} more filter(s)", .{ remaining });
        defer allocator.free(more);
        try ui.row(stdout, more);
    }
}

pub const MonitorOptions = struct {
    filter_agent: ?[]const u8 = null,
    graph: bool = false,
    history: bool = false,
    daily: bool = false,
    weekly: bool = false,
    monthly: bool = false,
    all: bool = false,
    format_json: bool = false,
    prune_noise: bool = false,
};

pub fn handleMonitor(allocator: std.mem.Allocator, opts: MonitorOptions) !void {
    const stdout = std.fs.File.stdout().deprecatedWriter();

    const home = std.posix.getenv("HOME") orelse return;
    const file_path = try std.fmt.allocPrint(allocator, "{s}/.omni/metrics.csv", .{home});
    defer allocator.free(file_path);

    var global_cmds: usize = 0;
    var global_in: usize = 0;
    var global_out: usize = 0;
    var global_saved: usize = 0;
    var global_ms: u64 = 0;

    var filter_map = std.StringHashMap(metrics.Stats).init(allocator);
    defer filter_map.deinit();

    var agent_map = std.StringHashMap(metrics.Stats).init(allocator);
    defer agent_map.deinit();

    var profile_map = std.StringHashMap(metrics.Stats).init(allocator);
    defer profile_map.deinit();

    var all_records = std.ArrayListUnmanaged(metrics.Record){};
    defer {
        for (all_records.items) |rec| {
            allocator.free(rec.agent);
            allocator.free(rec.filter_name);
        }
        all_records.deinit(allocator);
    }

    if (std.fs.cwd().openFile(file_path, .{})) |file| {
        defer file.close();
        const data = file.readToEndAlloc(allocator, 10 * 1024 * 1024) catch return;
        defer allocator.free(data);

        var it_lines = std.mem.splitSequence(u8, data, "\n");
        while (it_lines.next()) |ln| {
            if (ln.len == 0) continue;
            var rec = metrics.parseCsvLine(allocator, ln) catch continue;

            const agent_label = normalizeAgentLabel(rec.agent);

            if (opts.filter_agent != null and !std.mem.eql(u8, agent_label, opts.filter_agent.?)) {
                allocator.free(rec.agent);
                allocator.free(rec.filter_name);
                continue;
            }

            if (opts.prune_noise and isNoiseFilterName(rec.filter_name)) {
                allocator.free(rec.agent);
                allocator.free(rec.filter_name);
                continue;
            }

            if (!std.mem.eql(u8, rec.agent, agent_label)) {
                allocator.free(rec.agent);
                rec.agent = try allocator.dupe(u8, agent_label);
            }

            try all_records.append(allocator, rec);
            global_cmds += 1;
            global_in += rec.input_bytes;
            global_out += rec.output_bytes;
            if (rec.input_bytes > rec.output_bytes) global_saved += (rec.input_bytes - rec.output_bytes);
            global_ms += rec.ms;

            var f_res = try filter_map.getOrPut(rec.filter_name);
            if (!f_res.found_existing) f_res.value_ptr.* = .{};
            f_res.value_ptr.add(rec);

            var a_res = try agent_map.getOrPut(rec.agent);
            if (!a_res.found_existing) a_res.value_ptr.* = .{};
            a_res.value_ptr.add(rec);

            var p_res = try profile_map.getOrPut(deriveProfileName(rec.filter_name));
            if (!p_res.found_existing) p_res.value_ptr.* = .{};
            p_res.value_ptr.add(rec);
        }
    } else |_| {
        try stdout.print(ui.DIM ++ "  No tracking data yet.\n" ++ ui.RESET, .{});
        return;
    }

    if (global_cmds == 0) {
        try stdout.print(ui.DIM ++ "  No tracking data yet.\n" ++ ui.RESET, .{});
        return;
    }

    // ═══════════════════════════════════════
    // DEFAULT SUMMARY
    // ═══════════════════════════════════════
    if (!opts.daily and !opts.weekly and !opts.monthly and !opts.all and !opts.graph and !opts.history and !opts.format_json) {
        try ui.printHeader(stdout, "OMNI DISTILLATION MONITOR");

        const in_str = try metrics.formatBytes(allocator, global_in);
        const out_str = try metrics.formatBytes(allocator, global_out);
        const s_str = try metrics.formatBytes(allocator, global_saved);
        const avg_ms_str = try metrics.formatMs(allocator, global_ms, global_cmds);
        defer {
            allocator.free(in_str); allocator.free(out_str); allocator.free(s_str); allocator.free(avg_ms_str);
        }

        const avg_pct = if (global_in > 0) (@as(f64, @floatFromInt(global_saved)) / @as(f64, @floatFromInt(global_in))) * 100.0 else 0.0;
        const color = ui.colorForPct(avg_pct);

        try stdout.print("\n", .{});
        try stdout.print("  " ++ ui.WHITE ++ "{d:^12}" ++ ui.RESET ++ " " ++ ui.WHITE ++ "{s:^12}" ++ ui.RESET ++ " " ++ ui.WHITE ++ "{s:^12}" ++ ui.RESET ++ " " ++ ui.WHITE ++ "{s}{d:^12.1}%" ++ ui.RESET ++ " " ++ ui.WHITE ++ "{s:^12}" ++ ui.RESET ++ "\n", .{ global_cmds, in_str, s_str, color, avg_pct, avg_ms_str });
        try stdout.print("  " ++ ui.DIM ++ "{s:^12} {s:^12} {s:^12} {s:^12} {s:^12}" ++ ui.RESET ++ "\n", .{ "runs", "input", "saved", "rate", "avg" });
        try stdout.print("\n", .{});

        const m_buf = try ui.progressBar(allocator, "efficiency", avg_pct, 40);
        defer allocator.free(m_buf);
        try ui.row(stdout, m_buf);
        try stdout.print("\n", .{});
        try ui.divider(stdout);

        // Filter Performance
        if (filter_map.count() > 0) {
            try ui.printHeader(stdout, "FILTER PERFORMANCE");
            
            var rows = std.ArrayListUnmanaged(metrics.GroupedStats){};
            defer rows.deinit(allocator);
            var it = filter_map.iterator();
            while (it.next()) |entry| try rows.append(allocator, .{ .label = entry.key_ptr.*, .stats = entry.value_ptr.* });
            
            for (0..rows.items.len) |i| {
                for (0..rows.items.len - i - 1) |j| {
                    if (rows.items[j].stats.saved < rows.items[j + 1].stats.saved) {
                        const temp = rows.items[j]; rows.items[j] = rows.items[j + 1]; rows.items[j + 1] = temp;
                    }
                }
            }

            try stdout.print("\n", .{});
            for (rows.items, 0..) |r, idx| {
                const s = r.stats;
                const fs = try metrics.formatBytes(allocator, s.saved);
                const display_label = try compactFilterLabel(allocator, r.label, 24);
                defer allocator.free(fs);
                defer allocator.free(display_label);
                const fp = if (s.input > 0) (@as(f64, @floatFromInt(s.saved)) / @as(f64, @floatFromInt(s.input))) * 100.0 else 0.0;
                
                const bar_str = try ui.progressBar(allocator, "", fp, 20);
                defer allocator.free(bar_str);
                
                const rl = try std.fmt.allocPrint(allocator, "{d:>2}. " ++ ui.CYAN ++ "{s:<24}" ++ ui.RESET ++ "{d:>5}x  " ++ ui.WHITE ++ "{s:>8} saved" ++ ui.RESET ++ "  {s}", .{
                    idx + 1, display_label, s.cmds, fs, bar_str,
                });
                defer allocator.free(rl); try ui.row(stdout, rl);
            }
            try stdout.print("\n", .{});
            try ui.divider(stdout);
        }

        if (profile_map.count() > 0) {
            try ui.printHeader(stdout, "PROFILE BREAKDOWN");

            var rows = std.ArrayListUnmanaged(metrics.GroupedStats){};
            defer rows.deinit(allocator);
            var it = profile_map.iterator();
            while (it.next()) |entry| try rows.append(allocator, .{ .label = entry.key_ptr.*, .stats = entry.value_ptr.* });

            for (0..rows.items.len) |i| {
                for (0..rows.items.len - i - 1) |j| {
                    if (rows.items[j].stats.saved < rows.items[j + 1].stats.saved) {
                        const temp = rows.items[j]; rows.items[j] = rows.items[j + 1]; rows.items[j + 1] = temp;
                    }
                }
            }

            try stdout.print("\n", .{});
            for (rows.items, 0..) |r, idx| {
                const s = r.stats;
                const fs = try metrics.formatBytes(allocator, s.saved);
                defer allocator.free(fs);
                const fp = if (s.input > 0) (@as(f64, @floatFromInt(s.saved)) / @as(f64, @floatFromInt(s.input))) * 100.0 else 0.0;

                const bar_str = try ui.progressBar(allocator, "", fp, 20);
                defer allocator.free(bar_str);

                const rl = try std.fmt.allocPrint(allocator, "{d:>2}. " ++ ui.CYAN ++ "{s:<24}" ++ ui.RESET ++ "{d:>5}x  " ++ ui.WHITE ++ "{s:>8} saved" ++ ui.RESET ++ "  {s}", .{
                    idx + 1, r.label, s.cmds, fs, bar_str,
                });
                defer allocator.free(rl);
                try ui.row(stdout, rl);
            }
            try stdout.print("\n", .{});
            try ui.divider(stdout);
        }

        // Agent Breakdown
        if (agent_map.count() > 0) {
            try ui.printHeader(stdout, "AGENT BREAKDOWN");
            try stdout.print("\n", .{});
            var it = agent_map.iterator();
            while (it.next()) |entry| {
                const an = entry.key_ptr.*; const as = entry.value_ptr.*;
                const asv = try metrics.formatBytes(allocator, as.saved);
                const ain = try metrics.formatBytes(allocator, as.input);
                defer { allocator.free(asv); allocator.free(ain); }
                const ap = if (as.input > 0) (@as(f64, @floatFromInt(as.saved)) / @as(f64, @floatFromInt(as.input))) * 100.0 else 0.0;
                const c = ui.colorForPct(ap);

                const row_msg = try std.fmt.allocPrint(allocator, ui.HEX_FULL ++ " " ++ ui.MAGENTA ++ ui.BOLD ++ "{s}" ++ ui.RESET, .{an});
                defer allocator.free(row_msg);
                try ui.row(stdout, row_msg);
                {
                    const l = try std.fmt.allocPrint(allocator, "  Runs: {d}  Input: {s}  Saved: {s} ({s}{d:.1}%" ++ ui.RESET ++ ")", .{ as.cmds, ain, asv, c, ap });
                    defer allocator.free(l); try ui.row(stdout, l);
                }
                try renderAgentFilterBreakdown(allocator, stdout, all_records.items, an);
                try ui.row(stdout, "");
            }
            try ui.divider(stdout);
        }
    }

    if (opts.format_json) {
        try stdout.print("{{\n  \"summary\": {{\n    \"total_commands\": {d},\n    \"input_bytes\": {d},\n    \"output_bytes\": {d},\n    \"saved_bytes\": {d},\n    \"total_time_ms\": {d}\n  }},\n  \"filters\": [\n", .{
            global_cmds, global_in, global_out, global_saved, global_ms,
        });
        var it = filter_map.iterator(); var first = true;
        while (it.next()) |e| {
            if (!first) try stdout.print(",\n", .{});
            try stdout.print("    {{ \"name\": \"{s}\", \"cmds\": {d}, \"saved\": {d} }}", .{ e.key_ptr.*, e.value_ptr.cmds, e.value_ptr.saved });
            first = false;
        }
        try stdout.print("\n  ],\n  \"profiles\": [\n", .{});
        var pit = profile_map.iterator(); first = true;
        while (pit.next()) |e| {
            if (!first) try stdout.print(",\n", .{});
            try stdout.print("    {{ \"name\": \"{s}\", \"cmds\": {d}, \"saved\": {d} }}", .{ e.key_ptr.*, e.value_ptr.cmds, e.value_ptr.saved });
            first = false;
        }
        try stdout.print("\n  ],\n  \"agents\": [\n", .{});
        var ait = agent_map.iterator(); first = true;
        while (ait.next()) |e| {
            if (!first) try stdout.print(",\n", .{});
            try stdout.print("    {{ \"name\": \"{s}\", \"cmds\": {d}, \"saved\": {d} }}", .{ e.key_ptr.*, e.value_ptr.cmds, e.value_ptr.saved });
            first = false;
        }
        try stdout.print("\n  ]\n}}\n", .{}); return;
    }

    const TableRenderer = struct {
        fn render(alloc: std.mem.Allocator, map: *std.StringHashMap(metrics.Stats), title: []const u8, out: anytype, rowTitle: []const u8, g_cmds: usize, g_in: usize, g_out: usize, g_s: usize, g_ms: u64) !void {
            var title_upper = std.ArrayListUnmanaged(u8){};
            defer title_upper.deinit(alloc);
            for (title) |c| try title_upper.append(alloc, std.ascii.toUpper(c));
            
            try ui.printHeader(out, title_upper.items);
            
            const r2 = try std.fmt.allocPrint(alloc, ui.DIM ++ "  {s:<15} {s:>5}  {s:>8}  {s:>8}  {s:>8}  {s:>6}  {s:>7} " ++ ui.RESET, .{ rowTitle, "Cmds", "Input", "Output", "Saved", "Rate", "Time" });
            defer alloc.free(r2);
            try ui.row(out, r2);
            try ui.dividerSolid(out);
            
            var rows = std.ArrayListUnmanaged(metrics.GroupedStats){};
            defer rows.deinit(alloc);
            var it = map.iterator(); while (it.next()) |e| try rows.append(alloc, .{ .label = e.key_ptr.*, .stats = e.value_ptr.* });
            for (0..rows.items.len) |idx| {
                for (0..rows.items.len - idx - 1) |j| {
                    if (std.mem.order(u8, rows.items[j].label, rows.items[j + 1].label) == .gt) {
                        const temp = rows.items[j]; rows.items[j] = rows.items[j + 1]; rows.items[j + 1] = temp;
                    }
                }
            }
            for (rows.items) |r| {
                const s = r.stats; const in_s = try metrics.formatBytes(alloc, s.input); const out_s = try metrics.formatBytes(alloc, s.output); const sv_s = try metrics.formatBytes(alloc, s.saved); const ms_s = try metrics.formatMs(alloc, s.ms, s.cmds); const display_label = try compactFilterLabel(alloc, r.label, 15); defer { alloc.free(in_s); alloc.free(out_s); alloc.free(sv_s); alloc.free(ms_s); alloc.free(display_label); }
                const sp = if (s.input > 0) (@as(f64, @floatFromInt(s.saved)) / @as(f64, @floatFromInt(s.input))) * 100.0 else 0.0; const c = ui.colorForPct(sp);
                const rl = try std.fmt.allocPrint(alloc, "  " ++ ui.CYAN ++ "{s:<15}" ++ ui.RESET ++ " {d:>5}  {s:>8}  {s:>8}  {s:>8}  {s}{d:>5.1}%" ++ ui.RESET ++ "  {s:>7} ", .{ display_label, s.cmds, in_s, out_s, sv_s, c, sp, ms_s });
                defer alloc.free(rl); try ui.row(out, rl);
            }
            try ui.dividerSolid(out);
            const gin = try metrics.formatBytes(alloc, g_in); const gout = try metrics.formatBytes(alloc, g_out); const gs = try metrics.formatBytes(alloc, g_s); const gms = try metrics.formatMs(alloc, g_ms, g_cmds); defer { alloc.free(gin); alloc.free(gout); alloc.free(gs); alloc.free(gms); }
            const gp = if (g_in > 0) (@as(f64, @floatFromInt(g_s)) / @as(f64, @floatFromInt(g_in))) * 100.0 else 0.0;
            const tr = try std.fmt.allocPrint(alloc, ui.BOLD ++ "  {s:<15} {d:>5}  {s:>8}  {s:>8}  {s:>8}  {d:>5.1}%  {s:>7} " ++ ui.RESET, .{ "Total", g_cmds, gin, gout, gs, gp, gms });
            defer alloc.free(tr); try ui.row(out, tr); try out.print("\n", .{});
        }
    };

    if (opts.daily or opts.weekly or opts.monthly or opts.all) {
        var dm = std.StringHashMap(metrics.Stats).init(allocator);
        var wm = std.StringHashMap(metrics.Stats).init(allocator);
        var mm = std.StringHashMap(metrics.Stats).init(allocator);
        defer { dm.deinit(); wm.deinit(); mm.deinit(); }
        for (all_records.items) |rec| {
            const dl = try metrics.toDailyLabel(allocator, rec.timestamp);
            const wl = try metrics.toWeeklyLabel(allocator, rec.timestamp);
            const ml = try metrics.toMonthlyLabel(allocator, rec.timestamp);
            {
                var res = try dm.getOrPut(dl); if (!res.found_existing) res.value_ptr.* = .{} else allocator.free(dl);
                res.value_ptr.add(rec);
            }
            {
                var res = try wm.getOrPut(wl); if (!res.found_existing) res.value_ptr.* = .{} else allocator.free(wl);
                res.value_ptr.add(rec);
            }
            {
                var res = try mm.getOrPut(ml); if (!res.found_existing) res.value_ptr.* = .{} else allocator.free(ml);
                res.value_ptr.add(rec);
            }
        }
        if (opts.all or opts.daily) try TableRenderer.render(allocator, &dm, "Daily Breakdown", stdout, "Date", global_cmds, global_in, global_out, global_saved, global_ms);
        if (opts.all or opts.weekly) try TableRenderer.render(allocator, &wm, "Weekly Breakdown", stdout, "Week", global_cmds, global_in, global_out, global_saved, global_ms);
        if (opts.all or opts.monthly) try TableRenderer.render(allocator, &mm, "Monthly Breakdown", stdout, "Month", global_cmds, global_in, global_out, global_saved, global_ms);
        var it = dm.keyIterator(); while (it.next()) |k| allocator.free(k.*);
        var wit = wm.keyIterator(); while (wit.next()) |k| allocator.free(k.*);
        var mit = mm.keyIterator(); while (mit.next()) |k| allocator.free(k.*);
    }

    if (opts.graph) {
        var dss = std.StringHashMap(usize).init(allocator);
        defer { var it = dss.keyIterator(); while (it.next()) |k| allocator.free(k.*); dss.deinit(); }
        for (all_records.items) |rec| {
            if (rec.input_bytes > rec.output_bytes) {
                const s = rec.input_bytes - rec.output_bytes;
                const dl = try metrics.toDailyLabel(allocator, rec.timestamp);
                const res = try dss.getOrPut(dl);
                if (!res.found_existing) { res.value_ptr.* = s; } else { res.value_ptr.* += s; allocator.free(dl); }
            }
        }
        var darr = std.ArrayListUnmanaged([]const u8){};
        defer darr.deinit(allocator);
        var it = dss.keyIterator(); while (it.next()) |k| try darr.append(allocator, k.*);
        for (0..darr.items.len) |idx| {
            for (0..darr.items.len - idx - 1) |j| {
                if (std.mem.order(u8, darr.items[j], darr.items[j + 1]) == .gt) {
                    const temp = darr.items[j]; darr.items[j] = darr.items[j + 1]; darr.items[j + 1] = temp;
                }
            }
        }
        var maxv: usize = 0; for (darr.items) |d| { const v = dss.get(d) orelse 0; if (v > maxv) maxv = v; }
        try ui.printHeader(stdout, "DISTILLATION TREND");
        for (darr.items) |d| {
            const v = dss.get(d) orelse 0;
            const bl = if (maxv > 0) @as(usize, @intFromFloat((@as(f64, @floatFromInt(v)) / @as(f64, @floatFromInt(maxv))) * 40.0)) else 0;
            const dl = if (d.len >= 10) d[5..10] else d;
            const sv = try metrics.formatBytes(allocator, v); defer allocator.free(sv);
            var gbuf = std.ArrayListUnmanaged(u8){}; defer gbuf.deinit(allocator); const gbw = gbuf.writer(allocator);
            try gbw.print(ui.GRAY ++ "  {s}  " ++ ui.RESET, .{dl});
            if (bl == 0) {
                try gbw.print(ui.DIM ++ "⡀" ++ ui.RESET, .{});
                for (1..40) |_| try gbw.print(" ", .{});
            } else {
                for (0..bl) |_| try gbw.print(ui.GREEN ++ "⣿" ++ ui.RESET, .{});
                for (0..40 - bl) |_| try gbw.print(" ", .{});
            }
            try gbw.print("  " ++ ui.WHITE ++ "{s}" ++ ui.RESET, .{sv});
            try ui.row(stdout, gbuf.items);
        }
        try stdout.print("\n", .{});
        try ui.divider(stdout);
    }

    if (opts.history) {
        try ui.printHeader(stdout, "RECENT DISTILLATIONS");
        const count = if (all_records.items.len > 10) 10 else all_records.items.len;
        for (all_records.items.len - count..all_records.items.len) |idx| {
            const rec = all_records.items[idx];
            const epoch = std.time.epoch.EpochSeconds{ .secs = @as(u64, @intCast(rec.timestamp)) };
            const day = epoch.getEpochDay(); const yd = day.calculateYearDay(); const md = yd.calculateMonthDay();
            const dsecs = epoch.getDaySeconds();
            const saved = if (rec.input_bytes > rec.output_bytes) rec.input_bytes - rec.output_bytes else 0;
            const sv = try metrics.formatBytes(allocator, saved); defer allocator.free(sv);
            const pct = if (rec.input_bytes > 0) (@as(f64, @floatFromInt(saved)) / @as(f64, @floatFromInt(rec.input_bytes))) * 100.0 else 0.0;
            const c = ui.colorForPct(pct); const dot = if (pct >= 50.0) ui.HEX_FULL else if (pct >= 20.0) ui.HEX_EMPTY else "·";
            const hl = try std.fmt.allocPrint(allocator, ui.GRAY ++ "  {d:0>2}-{d:0>2} {d:0>2}:{d:0>2}" ++ ui.RESET ++ "  {s}{s}" ++ ui.RESET ++ "  " ++ ui.CYAN ++ "{s:<15}" ++ ui.RESET ++ "  " ++ ui.GRAY ++ "{s:<12}" ++ ui.RESET ++ "  {s}{d:>4.1}%" ++ ui.RESET ++ " " ++ ui.DIM ++ "({s:>8})" ++ ui.RESET, .{
                md.month.numeric(), md.day_index + 1, dsecs.getHoursIntoDay(), dsecs.getMinutesIntoHour(),
                c, dot, rec.filter_name, rec.agent, c, pct, sv,
            });
            defer allocator.free(hl); try ui.row(stdout, hl);
        }
        try stdout.print("\n", .{});
        try ui.divider(stdout);
    }
}

pub fn handleDiscover(allocator: std.mem.Allocator) !void {
    const stdout = std.fs.File.stdout().deprecatedWriter();
    try ui.printHeader(stdout, "SAVINGS OPPORTUNITY SCANNER");
    const home = std.posix.getenv("HOME") orelse return; var found: usize = 0;
    const history_files = [_][]const u8{ ".zsh_history", ".bash_history" };
    for (history_files) |hf| {
        const fp = try std.fmt.allocPrint(allocator, "{s}/{s}", .{ home, hf }); defer allocator.free(fp);
        if (std.fs.cwd().openFile(fp, .{})) |file| {
            defer file.close(); const data = try file.readToEndAlloc(allocator, 5 * 1024 * 1024); defer allocator.free(data);
            var it = std.mem.splitSequence(u8, data, "\n");
            var missed = std.ArrayListUnmanaged([]const u8){}; defer missed.deinit(allocator);
            while (it.next()) |ln| {
                if (ln.len == 0) continue;
                if ((std.mem.indexOf(u8, ln, "git diff") != null or std.mem.indexOf(u8, ln, "git log") != null or std.mem.indexOf(u8, ln, "npm install") != null) and std.mem.indexOf(u8, ln, "omni") == null) {
                    var clean = ln; if (std.mem.startsWith(u8, ln, ": ")) { if (std.mem.indexOf(u8, ln, ";")) |idx| { if (idx + 1 < ln.len) clean = ln[idx + 1 ..]; } }
                    try missed.append(allocator, clean);
                }
            }
            const maxs = if (missed.items.len > 10) 10 else missed.items.len;
            for (missed.items.len - maxs..missed.items.len) |idx| {
                const cmd = missed.items[idx]; const tr = if (cmd.len > 68) cmd[0..65] else cmd; const dots_c = if (cmd.len > 68) "..." else "";
                const row_msg = try std.fmt.allocPrint(allocator, ui.DIM ++ "  " ++ ui.HEX_EMPTY ++ ui.RESET ++ " " ++ ui.WHITE ++ "{s}{s}" ++ ui.RESET, .{ tr, dots_c });
                defer allocator.free(row_msg); try ui.row(stdout, row_msg);
                try ui.row(stdout, "    " ++ ui.DIM ++ "→ Pipe with `| omni` for ~60% reduction" ++ ui.RESET); 
                try stdout.print("\n", .{});
                found += 1;
            }
        } else |_| {}
    }
    try ui.divider(stdout);
    if (found == 0) { try ui.row(stdout, "  " ++ ui.GREEN ++ ui.HEX_FULL ++ " All clear!" ++ ui.RESET ++ " No missed opportunities found.\n"); } else {
        const fmsg = try std.fmt.allocPrint(allocator, ui.YELLOW ++ "  {d} commands could benefit from OMNI distillation\n" ++ ui.RESET, .{found});
        defer allocator.free(fmsg); try ui.row(stdout, fmsg);
    }
}
