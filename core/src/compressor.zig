const std = @import("std");
const Filter = @import("filters/interface.zig").Filter;

pub const CompressResult = struct {
    output: []const u8,
    filter_name: []const u8,
};

fn categorizeUnknown(input: []const u8) []const u8 {
    const trimmed = std.mem.trim(u8, input, " \n\r\t");
    if (trimmed.len == 0) return "empty";
    
    // JSON check
    if ((std.mem.startsWith(u8, trimmed, "{") and std.mem.endsWith(u8, trimmed, "}")) or
        (std.mem.startsWith(u8, trimmed, "[") and std.mem.endsWith(u8, trimmed, "]"))) {
        return "json";
    }

    // Diff check
    if (std.mem.startsWith(u8, trimmed, "---") or std.mem.startsWith(u8, trimmed, "+++") or std.mem.startsWith(u8, trimmed, "@@ -")) {
        return "diff";
    }

    // Stack trace check
    if (std.mem.indexOf(u8, input, "at ") != null and std.mem.indexOf(u8, input, ":") != null) {
        return "stacktrace";
    }

    // Shell/Log check
    if (std.mem.indexOf(u8, input, "$ ") != null or std.mem.indexOf(u8, input, "> ") != null or std.mem.indexOf(u8, input, "# ") != null) {
        return "shell";
    }

    // Test check
    if (std.mem.indexOf(u8, input, "PASS") != null or std.mem.indexOf(u8, input, "FAIL") != null or std.mem.indexOf(u8, input, "expect(") != null) {
        return "test";
    }

    // Build check
    if (std.mem.indexOf(u8, input, "gcc") != null or std.mem.indexOf(u8, input, "clang") != null or std.mem.indexOf(u8, input, "npm install") != null or std.mem.indexOf(u8, input, "tsc") != null) {
        return "build";
    }
    
    // Markdown check
    if (std.mem.startsWith(u8, trimmed, "#") or std.mem.indexOf(u8, input, "\n# ") != null) {
        return "markdown";
    }

    // Path check
    if (std.mem.indexOf(u8, trimmed, "/") != null and (std.mem.indexOf(u8, trimmed, ".") != null or std.mem.startsWith(u8, trimmed, "/"))) {
        return "path";
    }

    // Metadata check
    if (std.mem.startsWith(u8, trimmed, "[") and std.mem.endsWith(u8, trimmed, "]")) {
        return "metadata";
    }

    // Log check (e.g. 2024-03-20 or 2026-03-20)
    if (input.len > 10 and input[4] == '-' and input[7] == '-') {
        return "log";
    }

    return "unknown";
}

pub fn compress(allocator: std.mem.Allocator, input: []const u8, filters: []const Filter) !CompressResult {
    var best_filter: ?Filter = null;
    var max_score: f32 = -1.0;

    for (filters) |filter| {
        if (filter.match(input)) {
            const s = filter.score(input);
            if (s > max_score) {
                max_score = s;
                best_filter = filter;
            }
        }
    }

    if (best_filter) |filter| {
        if (max_score >= 0.8) {
            // Path A: High Confidence -> Primary Distillation (High Density Signal)
            const processed = try filter.process(allocator, input);
            return CompressResult{ .output = processed, .filter_name = filter.name };
        } else if (max_score >= 0.3) {
            // Path B: Grey Area -> Soft Compression (Context Manifest)
            const processed = try filter.process(allocator, input);
            defer allocator.free(processed);
            const manifest = try std.fmt.allocPrint(allocator, "[OMNI Context Manifest: {s} (Confidence: {d:.2})]\n{s}", .{filter.name, max_score, processed});
            return CompressResult{ .output = manifest, .filter_name = filter.name };
        } else {
            // Path C: Low Confidence/Noise -> Drop
            const dropped = try std.fmt.allocPrint(allocator, "[OMNI: Dropped noisy {s} output (Confidence: {d:.2})]", .{filter.name, max_score});
            return CompressResult{ .output = dropped, .filter_name = filter.name };
        }
    }
    
    // Default: return full input with categorized name
    return CompressResult{ .output = try allocator.dupe(u8, input), .filter_name = categorizeUnknown(input) };
}
