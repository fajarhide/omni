const std = @import("std");
const compressor = @import("compressor.zig");
const Filter = @import("filters/interface.zig").Filter;
const GitFilter = @import("filters/git.zig").GitFilter;
const BuildFilter = @import("filters/build.zig").BuildFilter;
const DockerFilter = @import("filters/docker.zig").DockerFilter;
const SqlFilter = @import("filters/sql.zig").SqlFilter;
const NodeFilter = @import("filters/node.zig").NodeFilter;
const CustomFilter = @import("filters/custom.zig").CustomFilter;
const DslEngine = @import("filters/dsl_engine.zig").DslEngine;
const DslFilterConfig = @import("filters/dsl_engine.zig").DslFilterConfig;
const CatFilter = @import("filters/cat.zig").CatFilter;

var gpa = std.heap.GeneralPurposeAllocator(.{}){};
const allocator = gpa.allocator();

/// Result structure for Wasm interaction.
/// Must be extern for Wasm ABI compatibility.
pub const CompressResult = extern struct {
    ptr: [*]u8,
    len: usize,
};

var global_filters: ?std.ArrayList(Filter) = null;
var global_custom_filter: ?*CustomFilter = null;
var global_dsl_engine: ?*DslEngine = null;

export fn init_engine() bool {
    if (global_filters != null) return true;
    return init_engine_with_config(null, 0);
}

export fn init_engine_with_config(config_ptr: ?[*]u8, config_len: usize) bool {
    if (global_filters != null) return true;

    var filters = std.ArrayList(Filter).empty;
    errdefer filters.deinit(allocator);

    filters.append(allocator, GitFilter.filter()) catch return false;
    filters.append(allocator, BuildFilter.filter()) catch return false;
    filters.append(allocator, DockerFilter.filter()) catch return false;
    filters.append(allocator, SqlFilter.filter()) catch return false;
    filters.append(allocator, NodeFilter.filter()) catch return false;
    filters.append(allocator, CatFilter.filter()) catch return false;

    var config_content: ?[]const u8 = null;

    if (config_ptr) |ptr| {
        config_content = ptr[0..config_len];
    } else {
        if (std.fs.cwd().readFileAlloc(allocator, "omni_config.json", 1024 * 1024)) |content| {
            config_content = content;
        } else |_| {}
    }

    if (config_content) |raw_json| {
        // Load Legacy Custom Rules
        if (CustomFilter.init(allocator)) |custom| {
            global_custom_filter = custom;
            custom.loadFromContent(raw_json) catch {};
            filters.append(allocator, custom.filter()) catch {};
        } else |_| {}

        // Load DSL Filters
        const FullConfig = struct { dsl_filters: []DslFilterConfig = &[_]DslFilterConfig{} };
        if (std.json.parseFromSlice(FullConfig, allocator, raw_json, .{ .ignore_unknown_fields = true })) |parsed| {
            if (DslEngine.init(allocator, parsed.value.dsl_filters)) |engine| {
                global_dsl_engine = engine;
                _ = engine.getFilters(&filters) catch {};
            } else |_| {}
        } else |_| {}
    }

    global_filters = filters;
    return true;
}

export fn alloc(len: usize) ?[*]u8 {
    const slice = allocator.alloc(u8, len) catch return null;
    return slice.ptr;
}

export fn free(ptr: [*]u8, len: usize) void {
    allocator.free(ptr[0..len]);
}

var last_filter_name_ptr: [*]const u8 = "";
var last_filter_name_len: usize = 0;

export fn get_last_filter_name_ptr() [*]const u8 {
    return last_filter_name_ptr;
}

export fn get_last_filter_name_len() usize {
    return last_filter_name_len;
}

const auto_learn = @import("filters/auto_learn.zig");

export fn discover(ptr: [*]u8, len: usize) u64 {
    const input = ptr[0..len];
    const candidates = auto_learn.discoverCandidates(allocator, input) catch |err| {
        const err_msg = std.fmt.allocPrint(allocator, "Discovery Error: {any}", .{err}) catch "Discovery Error";
        return @as(u64, err_msg.len) << 32 | @as(u32, @truncate(@intFromPtr(err_msg.ptr)));
    };
    defer auto_learn.freeCandidates(allocator, candidates);

    if (candidates.len == 0) {
        const empty = allocator.dupe(u8, "[]") catch "[]";
        return @as(u64, empty.len) << 32 | @as(u32, @truncate(@intFromPtr(empty.ptr)));
    }

    var list = std.ArrayList(u8).empty;
    defer list.deinit(allocator);

    list.appendSlice(allocator, "[") catch {};
    for (candidates, 0..) |c, i| {
        const action_str = if (c.action == .count) "count" else "keep";
        const entry = std.fmt.allocPrint(allocator,
            \\{{"name":"{s}","trigger":"{s}","pattern":"{s}","action":"{s}","output":"{s}","confidence":{d:.2}}}
        , .{
            c.name,
            c.trigger,
            c.pattern,
            action_str,
            c.output_template,
            c.confidence,
        }) catch "";
        defer allocator.free(entry);
        
        list.appendSlice(allocator, entry) catch {};
        if (i < candidates.len - 1) list.appendSlice(allocator, ",") catch {};
    }
    list.appendSlice(allocator, "]") catch {};

    const result = list.toOwnedSlice(allocator) catch "[]";
    return @as(u64, result.len) << 32 | @as(u32, @truncate(@intFromPtr(result.ptr)));
}

export fn compress(ptr: [*]u8, len: usize) u64 {
    const input = ptr[0..len];
    _ = init_engine(); // Ensure it's init
    
    const filters = if (global_filters) |f| f.items else &[_]Filter{};
    const result = compressor.compress(allocator, input, filters) catch |err| {
        const err_msg = std.fmt.allocPrint(allocator, "Error: {any}", .{err}) catch "Critical Error";
        last_filter_name_ptr = "error";
        last_filter_name_len = 5;
        return @as(u64, err_msg.len) << 32 | @as(u32, @truncate(@intFromPtr(err_msg.ptr)));
    };

    last_filter_name_ptr = result.filter_name.ptr;
    last_filter_name_len = result.filter_name.len;

    return @as(u64, result.output.len) << 32 | @as(u32, @truncate(@intFromPtr(result.output.ptr)));
}
pub fn main() void {}
