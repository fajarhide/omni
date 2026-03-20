const std = @import("std");
const Filter = @import("interface.zig").Filter;

pub const CatFilter = struct {
    pub fn filter() Filter {
        return .{
            .name = "cat",
            .ptr = undefined,
            .matchFn = match,
            .scoreFn = score,
            .processFn = process,
        };
    }

    fn score(_: *anyopaque, input: []const u8) f32 {
        // If it looks like a document with headers, give it high confidence.
        if (std.mem.indexOf(u8, input, "\n# ") != null or std.mem.startsWith(u8, input, "# ")) return 0.85;

        // Count lines to distinguish between a small noise snippet and a real "cat" output.
        var line_count: usize = 0;
        var non_empty_lines: usize = 0;
        var total_trimmed_len: usize = 0;
        var long_lines: usize = 0;
        var it = std.mem.splitAny(u8, input, "\n\r");
        while (it.next()) |line| {
            line_count += 1;
            const trimmed = std.mem.trim(u8, line, " \t\r");
            if (trimmed.len == 0) continue;
            non_empty_lines += 1;
            total_trimmed_len += trimmed.len;
            if (trimmed.len >= 40) long_lines += 1;
        }

        // If it's just a single short line and no headers, give it very low confidence.
        // This allows specialized filters (even low-confidence "noise" ones) to take precedence.
        if (line_count <= 1 and input.len < 100) return 0.1;

        // Plain text documents without explicit headers should still be treated as
        // high-confidence cat output when they have enough multiline prose signal.
        if (non_empty_lines >= 8) {
            const avg_len = if (non_empty_lines == 0) 0 else total_trimmed_len / non_empty_lines;
            if (avg_len >= 32 and long_lines * 2 >= non_empty_lines) return 0.82;
        }

        // Default catch-all score for multi-line or longer raw content.
        return 0.35;
    }

    fn match(_: *anyopaque, _: []const u8) bool {
        // Broad match to catch anything that doesn't trigger other filters.
        return true;
    }

    fn process(_: *anyopaque, allocator: std.mem.Allocator, input: []const u8) ![]u8 {
        var it = std.mem.splitAny(u8, input, "\n\r");
        var line_count: usize = 0;
        var result = std.ArrayList(u8).empty;
        errdefer result.deinit(allocator);
        var first_content_line: ?[]const u8 = null;

        var header_count: usize = 0;
        while (it.next()) |line| {
            const trimmed = std.mem.trim(u8, line, " \t\r");
            if (trimmed.len == 0) continue;
            line_count += 1;
            if (first_content_line == null) first_content_line = trimmed;

            // Simple header/list detection for documents
            if (std.mem.startsWith(u8, trimmed, "#") or 
                std.mem.startsWith(u8, trimmed, "##") or 
                std.mem.startsWith(u8, trimmed, "###")) 
            {
                try result.appendSlice(allocator, trimmed);
                try result.append(allocator, '\n');
                header_count += 1;
            } else if (header_count < 10 and (std.mem.startsWith(u8, trimmed, "- ") or std.mem.startsWith(u8, trimmed, "* "))) {
                // Keep top-level list items if they appear early
                try result.appendSlice(allocator, trimmed);
                try result.append(allocator, '\n');
            }
        }

        if (result.items.len == 0 or line_count > 100) {
            if (result.items.len > 0) {
                const summary = try std.fmt.allocPrint(allocator, "[cat distilled {d} lines, kept {d} headers]\n{s}", .{line_count, header_count, result.items});
                return summary;
            }
            if (first_content_line) |content_line| {
                const preview_len = @min(content_line.len, 64);
                const preview = content_line[0..preview_len];
                if (content_line.len > preview_len) {
                    return try std.fmt.allocPrint(allocator, "[cat distilled {d} lines of raw content] {s}...", .{line_count, preview});
                }
                return try std.fmt.allocPrint(allocator, "[cat distilled {d} lines of raw content] {s}", .{line_count, preview});
            }
            return try std.fmt.allocPrint(allocator, "[cat distilled {d} lines of raw content]", .{line_count});
        }

        return try result.toOwnedSlice(allocator);
    }
};
