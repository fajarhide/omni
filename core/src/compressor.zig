const std = @import("std");
const Filter = @import("filters/interface.zig").Filter;

pub fn compress(allocator: std.mem.Allocator, input: []const u8, filters: []const Filter) ![]u8 {
    var best_filter: ?Filter = null;
    var max_score: f32 = -1.0;

    for (filters) |filter| {
        // std.debug.print("Checking filter {d}: {s}\n", .{ i, filter.name });
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
            return try filter.process(allocator, input);
        } else if (max_score >= 0.3) {
            // Path B: Grey Area -> Soft Compression (Context Manifest)
            const processed = try filter.process(allocator, input);
            defer allocator.free(processed);
            return try std.fmt.allocPrint(allocator, "[OMNI Context Manifest: {s} (Confidence: {d:.2})]\n{s}", .{filter.name, max_score, processed});
        } else {
            // Path C: Low Confidence/Noise -> Drop
            return try std.fmt.allocPrint(allocator, "[OMNI: Dropped noisy {s} output (Confidence: {d:.2})]", .{filter.name, max_score});
        }
    }
    
    // Default: return full input if no filter matched
    return try allocator.dupe(u8, input);
}
