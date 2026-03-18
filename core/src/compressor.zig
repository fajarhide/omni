const std = @import("std");
const Filter = @import("filters/interface.zig").Filter;

pub const CompressResult = struct {
    output: []const u8,
    filter_name: []const u8,
};

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
    
    // Default: return full input if no filter matched
    return CompressResult{ .output = try allocator.dupe(u8, input), .filter_name = "unknown" };
}
