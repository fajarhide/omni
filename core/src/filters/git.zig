const std = @import("std");
const Filter = @import("interface.zig").Filter;

pub const GitFilter = struct {
    pub fn filter() Filter {
        return .{
            .name = "git",
            .ptr = undefined,
            .matchFn = match,
            .processFn = process,
        };
    }

    fn match(_: *anyopaque, input: []const u8) bool {
        return std.mem.indexOf(u8, input, "On branch") != null;
    }

    fn process(_: *anyopaque, allocator: std.mem.Allocator, input: []const u8) ![]u8 {
        var branch: []const u8 = "unknown";
        var modified: usize = 0;
        var deleted: usize = 0;
        var untracked: usize = 0;
        var staged: usize = 0;

        var it = std.mem.splitAny(u8, input, "\n\r");
        while (it.next()) |line| {
            const trimmed = std.mem.trim(u8, line, " \t");
            if (trimmed.len == 0) continue;

            if (std.mem.indexOf(u8, trimmed, "On branch ")) |idx| {
                branch = trimmed[idx + 10 ..];
            } else if (std.mem.indexOf(u8, trimmed, "modified:")) |_| {
                modified += 1;
            } else if (std.mem.indexOf(u8, trimmed, "deleted:")) |_| {
                deleted += 1;
            } else if (std.mem.startsWith(u8, trimmed, "new file:")) {
                staged += 1;
            } else if (std.mem.startsWith(u8, trimmed, "use \"git add\" to include in what will be committed")) {
                // This indicates we are in the untracked section
            } else if (std.mem.startsWith(u8, trimmed, "nothing to commit, working tree clean")) {
                return try std.fmt.allocPrint(allocator, "git: on branch {s} (clean)", .{branch});
            }
        }

        // Quick heuristic for untracked files count in git status
        var untracked_it = std.mem.splitSequence(u8, input, "Untracked files:");
        if (untracked_it.next() != null) {
            if (untracked_it.next()) |untracked_block| {
                var lines = std.mem.splitAny(u8, untracked_block, "\n\r");
                while (lines.next()) |l| {
                    const t = std.mem.trim(u8, l, " \t");
                    if (t.len > 0 and !std.mem.startsWith(u8, t, "(") and !std.mem.startsWith(u8, t, "git add")) {
                        untracked += 1;
                    }
                }
            }
        }

        return try std.fmt.allocPrint(allocator, 
            "git: on {s} | {d} staged, {d} mod, {d} del, {d} untracked", 
            .{branch, staged, modified, deleted, untracked});
    }
};
