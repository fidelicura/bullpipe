const std = @import("std");

pub fn build(b: *std.Build) void {
    const optimize = b.standardOptimizeOption(.{});

    const features = [_]std.Target.riscv.Feature{
        std.Target.riscv.Feature.@"32bit",
        std.Target.riscv.Feature.i,
        std.Target.riscv.Feature.m,
        std.Target.riscv.Feature.c,
    };
    const set = std.Target.riscv.featureSet(&features);

    const target = b.standardTargetOptions(.{
        .default_target = .{
            .abi = .ilp32,
            .cpu_arch = .riscv32,
            .os_tag = .freestanding,
            .cpu_features_add = set,
        },
    });

    const kernel = b.addExecutable(.{
        .name = "raftor.elf",
        .root_module = b.createModule(.{
            .code_model = .medlow,
            .optimize = optimize,
            .root_source_file = b.path("main/kernel.zig"),
            .single_threaded = true,
            .target = target,
        }),
    });
    kernel.setLinkerScript(b.path("main/linker.ld"));
    kernel.addCSourceFiles(.{
        .files = &.{"main/boot.s"},
        .flags = &.{ "-x", "assembler-with-cpp" },
    });
    b.installArtifact(kernel);
}
