const root = @import("root.zig");

export fn ktrap() callconv(.c) noreturn {
    @call(.always_inline, root.trap, .{});
}

export fn kmain() callconv(.c) noreturn {
    @call(.always_inline, root.main, .{});
}
