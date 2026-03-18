// OMNI Local Metrics Subsystem
// Tracks your session stats locally for 'omni report' — no data ever leaves your machine.

const std = @import("std");

pub const Record = struct {
    timestamp: i64,
    agent: []const u8,
    filter_name: []const u8,
    input_bytes: usize,
    output_bytes: usize,
    ms: u64,
};

pub const Stats = struct {
    cmds: usize = 0,
    input: usize = 0,
    output: usize = 0,
    saved: usize = 0,
    ms: u64 = 0,

    pub fn add(self: *Stats, rec: Record) void {
        self.cmds += 1;
        self.input += rec.input_bytes;
        self.output += rec.output_bytes;
        if (rec.input_bytes > rec.output_bytes) {
            self.saved += rec.input_bytes - rec.output_bytes;
        }
        self.ms += rec.ms;
    }
};

pub const GroupedStats = struct {
    label: []const u8,
    stats: Stats,
};

pub fn parseCsvLine(allocator: std.mem.Allocator, line: []const u8) !Record {
    var it = std.mem.splitSequence(u8, line, ",");
    
    const ts_str = it.next() orelse return error.InvalidFormat;
    const agent_str = it.next() orelse return error.InvalidFormat;
    const filter_str = it.next() orelse "unknown"; // Fallback for old metrics
    
    // For old metrics where filter_name is missing, the third part might be input_bytes
    // So we need to handle potential legacy formats gracefully.
    // If filter_str.len > 0 and it parses to an integer, it's an old format.
    var in_str: []const u8 = undefined;
    var actual_filter: []const u8 = filter_str;
    
    if (std.fmt.parseInt(usize, filter_str, 10)) |_| {
        actual_filter = "unknown";
        in_str = filter_str;
    } else |_| {
        in_str = it.next() orelse return error.InvalidFormat;
    }

    const out_str = it.next() orelse return error.InvalidFormat;
    const ms_str = it.next() orelse return error.InvalidFormat;

    return Record{
        .timestamp = try std.fmt.parseInt(i64, ts_str, 10),
        .agent = try allocator.dupe(u8, agent_str),
        .filter_name = try allocator.dupe(u8, actual_filter),
        .input_bytes = try std.fmt.parseInt(usize, in_str, 10),
        .output_bytes = try std.fmt.parseInt(usize, out_str, 10),
        .ms = try std.fmt.parseInt(u64, ms_str, 10),
    };
}

pub fn formatBytes(allocator: std.mem.Allocator, bytes: usize) ![]u8 {
    if (bytes < 1000) {
        return std.fmt.allocPrint(allocator, "{d}", .{bytes});
    } else if (bytes < 1_000_000) {
        const val = @as(f64, @floatFromInt(bytes)) / 1000.0;
        return std.fmt.allocPrint(allocator, "{d:.1}K", .{val});
    } else if (bytes < 1_000_000_000) {
        const val = @as(f64, @floatFromInt(bytes)) / 1_000_000.0;
        return std.fmt.allocPrint(allocator, "{d:.1}M", .{val});
    } else {
        const val = @as(f64, @floatFromInt(bytes)) / 1_000_000_000.0;
        return std.fmt.allocPrint(allocator, "{d:.1}G", .{val});
    }
}

pub fn formatMs(allocator: std.mem.Allocator, total_ms: u64, cmds: usize) ![]u8 {
    if (cmds == 0) return std.fmt.allocPrint(allocator, "0ms", .{});
    const avg = total_ms / cmds;
    if (avg < 1000) {
        return std.fmt.allocPrint(allocator, "{d}ms", .{avg});
    } else {
        const s = @as(f64, @floatFromInt(avg)) / 1000.0;
        return std.fmt.allocPrint(allocator, "{d:.1}s", .{s});
    }
}

// Convert timestamp to YYYY-MM-DD string
pub fn toDailyLabel(allocator: std.mem.Allocator, ts: i64) ![]u8 {
    const epoch_seconds = @as(u64, @intCast(ts));
    const epoch = std.time.epoch.EpochSeconds{ .secs = epoch_seconds };
    const day = epoch.getEpochDay();
    const year_day = day.calculateYearDay();
    const month_day = year_day.calculateMonthDay();
    return std.fmt.allocPrint(allocator, "{d:0>4}-{d:0>2}-{d:0>2}", .{
        year_day.year,
        month_day.month.numeric(),
        month_day.day_index + 1,
    });
}

// Convert timestamp to YYYY-MM string
pub fn toMonthlyLabel(allocator: std.mem.Allocator, ts: i64) ![]u8 {
    const epoch_seconds = @as(u64, @intCast(ts));
    const epoch = std.time.epoch.EpochSeconds{ .secs = epoch_seconds };
    const day = epoch.getEpochDay();
    const year_day = day.calculateYearDay();
    const month_day = year_day.calculateMonthDay();
    return std.fmt.allocPrint(allocator, "{d:0>4}-{d:0>2}", .{
        year_day.year,
        month_day.month.numeric(),
    });
}

// Returns "MM-DD ~ MM-DD" week range (Monday to Sunday)
pub fn toWeeklyLabel(allocator: std.mem.Allocator, ts: i64) ![]u8 {
    const epoch_seconds = @as(u64, @intCast(ts));
    const epoch = std.time.epoch.EpochSeconds{ .secs = epoch_seconds };
    const day = epoch.getEpochDay();

    // 1970-01-01 was Thursday. day_of_week: 0=Mon..6=Sun
    const dow = (day.day + 3) % 7;
    const monday_day = day.day - dow;
    const sunday_day = monday_day + 6;

    const mon_epoch: std.time.epoch.EpochDay = .{ .day = monday_day };
    const mon_yd = mon_epoch.calculateYearDay();
    const mon_md = mon_yd.calculateMonthDay();

    const sun_epoch: std.time.epoch.EpochDay = .{ .day = sunday_day };
    const sun_yd = sun_epoch.calculateYearDay();
    const sun_md = sun_yd.calculateMonthDay();

    return std.fmt.allocPrint(allocator, "{d:0>2}-{d:0>2} ~ {d:0>2}-{d:0>2}", .{
        mon_md.month.numeric(), mon_md.day_index + 1,
        sun_md.month.numeric(), sun_md.day_index + 1,
    });
}
