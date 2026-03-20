const std = @import("std");

pub const Route = enum {
    keep,
    compress,
    drop,
};

pub const Filter = struct {
    name: []const u8,
    ptr: *anyopaque,
    matchFn: *const fn (ptr: *anyopaque, input: []const u8) bool,
    scoreFn: *const fn (ptr: *anyopaque, input: []const u8) f32,
    processFn: *const fn (ptr: *anyopaque, allocator: std.mem.Allocator, input: []const u8) anyerror![]u8,
    labelFn: ?*const fn (ptr: *anyopaque) []const u8 = null,

    pub fn match(self: Filter, input: []const u8) bool {
        return self.matchFn(self.ptr, input);
    }

    pub fn score(self: Filter, input: []const u8) f32 {
        return self.scoreFn(self.ptr, input);
    }

    pub fn process(self: Filter, allocator: std.mem.Allocator, input: []const u8) ![]u8 {
        return self.processFn(self.ptr, allocator, input);
    }

    pub fn label(self: Filter) []const u8 {
        if (self.labelFn) |label_fn| return label_fn(self.ptr);
        return self.name;
    }
};
