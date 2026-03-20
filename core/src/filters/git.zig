const std = @import("std");
const Filter = @import("interface.zig").Filter;

pub const GitFilter = struct {
    pub fn filter() Filter {
        return .{
            .name = "git",
            .ptr = undefined,
            .matchFn = match,
            .scoreFn = score,
            .processFn = process,
        };
    }

    fn isCommitLine(line: []const u8) bool {
        const t = std.mem.trim(u8, line, " \t\r");
        // We look for a 7-character hex hash followed by a space
        if (t.len < 8) return false;
        if (t[7] != ' ') return false;
        for (t[0..7]) |c| {
            if (!((c >= '0' and c <= '9') or (c >= 'a' and c <= 'f') or (c >= 'A' and c <= 'F'))) {
                return false;
            }
        }
        return true;
    }

    fn score(_: *anyopaque, input: []const u8) f32 {
        var signals: f32 = 0.0;
        
        // Status signals
        if (std.mem.indexOf(u8, input, "On branch") != null) signals += 3.0;
        if (std.mem.indexOf(u8, input, "nothing to commit") != null) signals += 3.0;
        if (std.mem.indexOf(u8, input, "modified:") != null) signals += 2.0;
        if (std.mem.indexOf(u8, input, "Untracked") != null) signals += 1.0;
        
        // Diff signals
        if (std.mem.indexOf(u8, input, "diff --git") != null) signals += 4.0;
        if (std.mem.indexOf(u8, input, "@@ -") != null) signals += 2.0;
        
        // Add signals (verbose)
        if (std.mem.indexOf(u8, input, "add '") != null) signals += 3.0;

        // Log signals
        var log_count: f32 = 0.0;
        var it = std.mem.splitScalar(u8, input, '\n');
        while (it.next()) |line| {
            if (isCommitLine(line)) log_count += 1.0;
        }
        signals += @min(5.0, log_count * 1.0);

        return @min(1.0, signals / 6.0);
    }

    fn match(_: *anyopaque, input: []const u8) bool {
        if (std.mem.indexOf(u8, input, "On branch") != null or 
            std.mem.indexOf(u8, input, "diff --git") != null or
            std.mem.indexOf(u8, input, "add '") != null) return true;

        var it = std.mem.splitScalar(u8, input, '\n');
        while (it.next()) |line| {
            if (isCommitLine(line)) return true;
        }
        return false;
    }

    fn process(_: *anyopaque, allocator: std.mem.Allocator, input: []const u8) ![]u8 {
        if (std.mem.indexOf(u8, input, "diff --git") != null) {
            return processDiff(allocator, input);
        }
        if (std.mem.indexOf(u8, input, "add '") != null) {
            return processAdd(allocator, input);
        }
        
        // Check if it's a log-heavy input
        var log_lines: usize = 0;
        var it = std.mem.splitScalar(u8, input, '\n');
        while (it.next()) |line| {
            if (isCommitLine(line)) log_lines += 1;
        }
        
        if (log_lines >= 1 and std.mem.indexOf(u8, input, "On branch") == null) {
            return processLog(allocator, input);
        }

        return processStatus(allocator, input);
    }

    fn processStatus(allocator: std.mem.Allocator, input: []const u8) ![]u8 {
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
            } else if (std.mem.startsWith(u8, trimmed, "nothing to commit, working tree clean")) {
                return try std.fmt.allocPrint(allocator, "git: on branch {s} (clean)", .{branch});
            }
        }

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

    fn processDiff(allocator: std.mem.Allocator, input: []const u8) ![]u8 {
        var result = std.ArrayList(u8).empty;
        errdefer result.deinit(allocator);

        var it = std.mem.splitAny(u8, input, "\n\r");
        while (it.next()) |line| {
            const t = std.mem.trimRight(u8, line, " \t\r");
            if (t.len == 0) continue;

            if (std.mem.startsWith(u8, t, "diff --git") or
                std.mem.startsWith(u8, t, "@@") or
                std.mem.startsWith(u8, t, "+") or
                std.mem.startsWith(u8, t, "-")) 
            {
                if (std.mem.startsWith(u8, t, "---") or std.mem.startsWith(u8, t, "+++")) continue;
                
                try result.appendSlice(allocator, t);
                try result.append(allocator, '\n');
            }
        }

        if (result.items.len == 0) return try allocator.dupe(u8, "[Git diff noise distilled]");
        return try result.toOwnedSlice(allocator);
    }

    fn processAdd(allocator: std.mem.Allocator, input: []const u8) ![]u8 {
        var count: usize = 0;
        var it = std.mem.splitAny(u8, input, "\n\r");
        while (it.next()) |line| {
            if (std.mem.indexOf(u8, line, "add '") != null) count += 1;
        }
        return try std.fmt.allocPrint(allocator, "git: added {d} files to index", .{count});
    }

    fn processLog(allocator: std.mem.Allocator, input: []const u8) ![]u8 {
        var it = std.mem.splitScalar(u8, input, '\n');
        var result = std.ArrayList(u8).empty;
        errdefer result.deinit(allocator);

        while (it.next()) |line| {
            const t = std.mem.trimRight(u8, line, " \t\r");
            if (t.len == 0) continue;

            if (isCommitLine(t)) {
                try result.appendSlice(allocator, t[8..]);
                try result.append(allocator, '\n');
            } else {
                try result.appendSlice(allocator, t);
                try result.append(allocator, '\n');
            }
        }

        if (result.items.len == 0) return try allocator.dupe(u8, "[Git log noise distilled]");
        return try result.toOwnedSlice(allocator);
    }
};
