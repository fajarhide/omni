const std = @import("std");
const Filter = @import("interface.zig").Filter;

pub const DockerFilter = struct {
    pub fn filter() Filter {
        return .{
            .name = "docker",
            .ptr = undefined,
            .matchFn = match,
            .scoreFn = score,
            .processFn = process,
        };
    }

    fn score(_: *anyopaque, input: []const u8) f32 {
        var count: f32 = 0.0;
        if (std.mem.indexOf(u8, input, "FROM") != null) count += 2.0;
        if (std.mem.indexOf(u8, input, "Step") != null) count += 2.0;
        if (std.mem.indexOf(u8, input, "Successfully built") != null) count += 3.0;
        if (std.mem.indexOf(u8, input, "CACHED") != null) count += 1.0;
        return @min(1.0, count / 5.0);
    }

    fn match(_: *anyopaque, input: []const u8) bool {
        return std.mem.indexOf(u8, input, "Step ") != null or std.mem.indexOf(u8, input, "CACHED") != null;
    }

    fn process(_: *anyopaque, allocator: std.mem.Allocator, input: []const u8) ![]u8 {
        var it = std.mem.splitScalar(u8, input, '\n');
        var result = std.ArrayList(u8).empty;
        errdefer result.deinit(allocator);

        while (it.next()) |line| {
            const t = std.mem.trim(u8, line, " \t\r");
            if (t.len == 0) continue;

            // Keep Step definitions, CACHED indicators, and actual errors
            if (std.mem.startsWith(u8, t, "Step ") or 
                std.mem.indexOf(u8, t, "CACHED") != null or
                std.mem.indexOf(u8, t, "ERROR") != null or
                std.mem.indexOf(u8, t, "failed") != null or
                std.mem.startsWith(u8, t, "Successfully built") or
                std.mem.startsWith(u8, t, "Successfully tagged")) 
            {
                try result.appendSlice(allocator, t);
                try result.append(allocator, '\n');
            } else if (std.mem.startsWith(u8, t, " ---> ")) {
                // Keep intermediate layer IDs as they are short and provide context
                try result.appendSlice(allocator, t);
                try result.append(allocator, '\n');
            }
        }

        if (result.items.len == 0) {
            return try allocator.dupe(u8, "[Docker noise distilled]");
        }

        return try result.toOwnedSlice(allocator);
    }
};
