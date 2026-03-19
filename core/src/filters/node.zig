const std = @import("std");
const Filter = @import("interface.zig").Filter;

pub const NodeFilter = struct {
    pub fn filter() Filter {
        return .{
            .name = "node",
            .ptr = undefined,
            .matchFn = match,
            .scoreFn = score,
            .processFn = process,
        };
    }

    fn score(_: *anyopaque, input: []const u8) f32 {
        var count: f32 = 0.0;
        if (std.mem.indexOf(u8, input, "added") != null) count += 2.0;
        if (std.mem.indexOf(u8, input, "removed") != null) count += 2.0;
        if (std.mem.indexOf(u8, input, "updated") != null) count += 2.0;
        if (std.mem.indexOf(u8, input, "vulnerabilities") != null) count += 3.0;
        if (std.mem.indexOf(u8, input, "Done in") != null) count += 2.0;
        return @min(1.0, count / 5.0);
    }

    fn match(_: *anyopaque, input: []const u8) bool {
        return std.mem.indexOf(u8, input, "added ") != null or 
               std.mem.indexOf(u8, input, "packages") != null or
               std.mem.indexOf(u8, input, "vulnerabilities") != null or
               std.mem.indexOf(u8, input, "Done in ") != null;
    }

    fn process(_: *anyopaque, allocator: std.mem.Allocator, input: []const u8) ![]u8 {
        var it = std.mem.splitScalar(u8, input, '\n');
        var result = std.ArrayList(u8).empty;
        errdefer result.deinit(allocator);

        while (it.next()) |line| {
            const t = std.mem.trim(u8, line, " \t\r");
            if (t.len == 0) continue;

            // Keep key summary lines
            if (std.mem.indexOf(u8, t, "added ") != null or
                std.mem.indexOf(u8, t, "removed ") != null or
                std.mem.indexOf(u8, t, "updated ") != null or
                std.mem.indexOf(u8, t, "packages") != null or
                (std.mem.indexOf(u8, t, "found ") != null and std.mem.indexOf(u8, t, "vulnerabilities") != null) or
                std.mem.startsWith(u8, t, "Done in ") or
                std.mem.startsWith(u8, t, "error ") or
                std.mem.startsWith(u8, t, "ERR! "))
            {
                try result.appendSlice(allocator, t);
                try result.append(allocator, '\n');
            }
        }

        if (result.items.len == 0) {
            return try allocator.dupe(u8, "[Node/NPM noise distilled]");
        }

        return try result.toOwnedSlice(allocator);
    }
};
