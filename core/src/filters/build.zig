const std = @import("std");
const Filter = @import("interface.zig").Filter;

pub const BuildFilter = struct {
    pub fn filter() Filter {
        return .{
            .name = "build",
            .ptr = undefined,
            .matchFn = match,
            .processFn = process,
        };
    }

    fn match(_: *anyopaque, input: []const u8) bool {
        return std.mem.indexOf(u8, input, "error:") != null or std.mem.indexOf(u8, input, "warning:") != null;
    }

    fn process(_: *anyopaque, allocator: std.mem.Allocator, input: []const u8) ![]u8 {
        var it = std.mem.splitScalar(u8, input, '\n');
        var result = std.ArrayList(u8).empty;
        errdefer result.deinit(allocator);

        var shown_context: bool = false;

        while (it.next()) |line| {
            const t = std.mem.trim(u8, line, " \t\r");
            if (t.len == 0) continue;

            const is_issue = std.mem.indexOf(u8, t, "error:") != null or 
                             std.mem.indexOf(u8, t, "warning:") != null or
                             std.mem.indexOf(u8, t, "Error:") != null;

            const is_important = std.mem.indexOf(u8, t, "succeeded") != null or
                                std.mem.indexOf(u8, t, "failed") != null or
                                std.mem.indexOf(u8, t, "Build Summary") != null;

            if (is_issue or is_important) {
                try result.appendSlice(allocator, t);
                try result.append(allocator, '\n');
                shown_context = true;
            } else if (std.mem.startsWith(u8, t, "Compiling") or std.mem.startsWith(u8, t, "Building")) {
                // Ignore individual file compilation lines
            }
        }

        if (!shown_context) {
            return try allocator.dupe(u8, "[Build output distilled]");
        }

        return try result.toOwnedSlice(allocator);
    }
};
