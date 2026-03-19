const std = @import("std");
const Filter = @import("interface.zig").Filter;

pub const SqlFilter = struct {
    pub fn filter() Filter {
        return .{
            .name = "sql",
            .ptr = undefined,
            .matchFn = match,
            .scoreFn = score,
            .processFn = process,
        };
    }

    fn score(_: *anyopaque, input: []const u8) f32 {
        var count: f32 = 0.0;
        if (std.mem.indexOf(u8, input, "SELECT") != null) count += 2.0;
        if (std.mem.indexOf(u8, input, "INSERT") != null) count += 2.0;
        if (std.mem.indexOf(u8, input, "CREATE") != null) count += 2.0;
        if (std.mem.indexOf(u8, input, "rows returned") != null) count += 2.0;
        return @min(1.0, count / 4.0);
    }

    fn match(_: *anyopaque, input: []const u8) bool {
        const lower = std.mem.indexOf(u8, input, "SELECT") != null or 
                     std.mem.indexOf(u8, input, "INSERT") != null or
                     std.mem.indexOf(u8, input, "CREATE TABLE") != null;
        return lower;
    }

    fn process(_: *anyopaque, allocator: std.mem.Allocator, input: []const u8) ![]u8 {
        var result = std.ArrayList(u8).empty;
        errdefer result.deinit(allocator);

        var in_comment: bool = false;
        var it = std.mem.splitScalar(u8, input, ' ');
        
        while (it.next()) |word| {
            if (word.len == 0) continue;
            
            if (std.mem.startsWith(u8, word, "--")) {
                // Ignore the rest of the input for this line-based distillation
                break;
            }

            if (std.mem.startsWith(u8, word, "/*")) {
                in_comment = true;
                if (std.mem.endsWith(u8, word, "*/")) {
                    in_comment = false;
                    continue;
                }
            }
            
            if (!in_comment) {
                try result.appendSlice(allocator, word);
                try result.append(allocator, ' ');
            }
            
            if (std.mem.endsWith(u8, word, "*/")) {
                in_comment = false;
            }
        }

        const trimmed = std.mem.trim(u8, result.items, " ");
        return try allocator.dupe(u8, trimmed);
    }
};
