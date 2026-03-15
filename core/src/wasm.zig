const std = @import("std");
const compressor = @import("compressor.zig");
const Filter = @import("filters/interface.zig").Filter;
const GitFilter = @import("filters/git.zig").GitFilter;
const BuildFilter = @import("filters/build.zig").BuildFilter;
const DockerFilter = @import("filters/docker.zig").DockerFilter;
const SqlFilter = @import("filters/sql.zig").SqlFilter;
const NodeFilter = @import("filters/node.zig").NodeFilter;
const CustomFilter = @import("filters/custom.zig").CustomFilter;

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

export fn init_engine() bool {
    if (global_filters != null) return true;
    
    var filters = std.ArrayList(Filter).empty;
    filters.append(allocator, GitFilter.filter()) catch return false;
    filters.append(allocator, BuildFilter.filter()) catch return false;
    filters.append(allocator, DockerFilter.filter()) catch return false;
    filters.append(allocator, SqlFilter.filter()) catch return false;
    filters.append(allocator, NodeFilter.filter()) catch return false;

    // Optional: Load custom rules if config exists in Wasm environment (might need pre-opened file)
    if (CustomFilter.init(allocator, "omni_config.json")) |custom| {
        global_custom_filter = custom;
        filters.append(allocator, custom.filter()) catch {};
    } else |_| {}

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

export fn compress(ptr: [*]u8, len: usize) u64 {
    const input = ptr[0..len];
    _ = init_engine(); // Ensure it's init
    
    const filters = if (global_filters) |f| f.items else &[_]Filter{};
    const result = compressor.compress(allocator, input, filters) catch |err| {
        const err_msg = std.fmt.allocPrint(allocator, "Error: {any}", .{err}) catch "Critical Error";
        return @as(u64, err_msg.len) << 32 | @as(u32, @truncate(@intFromPtr(err_msg.ptr)));
    };

    return @as(u64, result.len) << 32 | @as(u32, @truncate(@intFromPtr(result.ptr)));
}
pub fn main() void {}
