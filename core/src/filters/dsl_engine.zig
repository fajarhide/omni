const std = @import("std");
const Filter = @import("interface.zig").Filter;

pub const DslAction = enum {
    keep,
    count,
};

pub const DslRule = struct {
    capture: []const u8,
    action: DslAction,
    as: ?[]const u8 = null,
};

pub const DslFilterConfig = struct {
    name: []const u8,
    trigger: []const u8,
    rules: []DslRule,
    output: []const u8,
    confidence: f32 = 1.0,
};

pub const DslEngine = struct {
    allocator: std.mem.Allocator,
    filters: []DslFilterConfig,

    pub fn init(allocator: std.mem.Allocator, configs: []DslFilterConfig) !*DslEngine {
        const self = try allocator.create(DslEngine);
        self.* = .{
            .allocator = allocator,
            .filters = configs,
        };
        return self;
    }

    pub fn deinit(self: *DslEngine) void {
        self.allocator.destroy(self);
    }

    pub fn getFilters(self: *DslEngine, list: *std.ArrayList(Filter)) !void {
        for (self.filters) |*f| {
            try list.append(self.allocator, .{
                .name = f.name,
                .ptr = f,
                .matchFn = match,
                .scoreFn = score,
                .processFn = process,
            });
        }
    }

    fn score(ptr: *anyopaque, _: []const u8) f32 {
        const config: *DslFilterConfig = @ptrCast(@alignCast(ptr));
        return config.confidence;
    }

    fn match(ptr: *anyopaque, input: []const u8) bool {
        const config: *DslFilterConfig = @ptrCast(@alignCast(ptr));
        return std.mem.indexOf(u8, input, config.trigger) != null;
    }

    fn process(ptr: *anyopaque, allocator: std.mem.Allocator, input: []const u8) ![]u8 {
        const config: *DslFilterConfig = @ptrCast(@alignCast(ptr));
        
        var vars = std.StringHashMap([]const u8).init(allocator);
        defer vars.deinit();
        
        var counters = std.StringHashMap(usize).init(allocator);
        defer counters.deinit();

        var it = std.mem.splitAny(u8, input, "\n\r");
        while (it.next()) |line| {
            const trimmed = std.mem.trim(u8, line, " \t");
            if (trimmed.len == 0) continue;

            for (config.rules) |rule| {
                if (try captureVariables(allocator, trimmed, rule.capture, &vars)) {
                    if (rule.action == .count) {
                        const key = rule.as orelse rule.capture;
                        const entry = try counters.getOrPutValue(key, 0);
                        entry.value_ptr.* += 1;
                    }
                }
            }
        }

        return try formatOutput(allocator, config.output, &vars, &counters);
    }

    fn captureVariables(allocator: std.mem.Allocator, input: []const u8, pattern: []const u8, vars: *std.StringHashMap([]const u8)) !bool {
        _ = allocator;
        var input_idx: usize = 0;
        var pattern_idx: usize = 0;

        while (pattern_idx < pattern.len) {
            if (pattern[pattern_idx] == '{') {
                const end_rel = std.mem.indexOf(u8, pattern[pattern_idx..], "}") orelse return false;
                const end_idx = pattern_idx + end_rel;
                const var_name = pattern[pattern_idx + 1 .. end_idx];
                
                pattern_idx = end_idx + 1;
                
                // If there's more pattern after }, find the next literal part
                if (pattern_idx < pattern.len) {
                    const next_brace = std.mem.indexOf(u8, pattern[pattern_idx..], "{") orelse pattern.len - pattern_idx;
                    const literal_suffix = pattern[pattern_idx .. pattern_idx + next_brace];
                    
                    if (std.mem.indexOf(u8, input[input_idx..], literal_suffix)) |match_idx| {
                        try vars.put(var_name, input[input_idx .. input_idx + match_idx]);
                        input_idx += match_idx + literal_suffix.len;
                        pattern_idx += literal_suffix.len;
                    } else return false;
                } else {
                    // Last variable captures the rest of the input
                    try vars.put(var_name, input[input_idx..]);
                    input_idx = input.len;
                }
            } else {
                // Match literal prefix or parts between variables
                const next_brace = std.mem.indexOf(u8, pattern[pattern_idx..], "{") orelse pattern.len - pattern_idx;
                const literal = pattern[pattern_idx .. pattern_idx + next_brace];
                
                if (!std.mem.startsWith(u8, input[input_idx..], literal)) return false;
                
                input_idx += literal.len;
                pattern_idx += literal.len;
            }
        }
        return true;
    }

    fn formatOutput(allocator: std.mem.Allocator, template: []const u8, vars: *std.StringHashMap([]const u8), counters: *std.StringHashMap(usize)) ![]u8 {
        var result = std.ArrayList(u8).empty;
        errdefer result.deinit(allocator);

        var i: usize = 0;
        while (i < template.len) {
            if (template[i] == '{') {
                if (std.mem.indexOf(u8, template[i..], "}")) |end_rel| {
                    const end_idx = i + end_rel;
                    const raw_key = template[i + 1 .. end_idx];
                    const parsed = parsePlaceholder(raw_key);

                    if (vars.get(parsed.key)) |val| {
                        try result.appendSlice(allocator, val);
                    } else if (counters.get(parsed.key)) |count| {
                        var buf: [32]u8 = undefined;
                        const s = std.fmt.bufPrint(&buf, "{d}", .{count}) catch "???";
                        try result.appendSlice(allocator, s);
                    } else if (parsed.default_value) |default_value| {
                        try result.appendSlice(allocator, default_value);
                    }
                    i = end_idx + 1;
                    continue;
                }
            }
            try result.append(allocator, template[i]);
            i += 1;
        }
        return try result.toOwnedSlice(allocator);
    }

    const ParsedPlaceholder = struct {
        key: []const u8,
        default_value: ?[]const u8,
    };

    fn parsePlaceholder(raw: []const u8) ParsedPlaceholder {
        if (std.mem.indexOfScalar(u8, raw, '?')) |sep| {
            return .{
                .key = raw[0..sep],
                .default_value = raw[sep + 1 ..],
            };
        }

        return .{
            .key = raw,
            .default_value = null,
        };
    }
};
