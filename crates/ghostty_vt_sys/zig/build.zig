const std = @import("std");

pub fn build(b: *std.Build) void {
    const optimize = b.standardOptimizeOption(.{});
    const target = b.standardTargetOptions(.{});

    // Get uucode dependency and its generated tables
    const uucode_dep = b.dependency("uucode", .{
        .build_config_path = b.path("ghostty_src/build/uucode_config.zig"),
    });
    const uucode_tables = uucode_dep.namedLazyPath("tables.zig");

    const props_exe = b.addExecutable(.{
        .name = "props-unigen",
        .root_module = b.createModule(.{
            .root_source_file = b.path("ghostty_src/unicode/props_uucode.zig"),
            .target = b.graph.host,
            .optimize = optimize,
        }),
        .use_llvm = true,
    });

    const symbols_exe = b.addExecutable(.{
        .name = "symbols-unigen",
        .root_module = b.createModule(.{
            .root_source_file = b.path("ghostty_src/unicode/symbols_uucode.zig"),
            .target = b.graph.host,
            .optimize = optimize,
        }),
        .use_llvm = true,
    });

    // Add uucode import to both generators
    if (b.lazyDependency("uucode", .{
        .target = b.graph.host,
        .tables_path = uucode_tables,
        .build_config_path = b.path("ghostty_src/build/uucode_config.zig"),
    })) |dep| {
        inline for (&.{ props_exe, symbols_exe }) |exe| {
            exe.root_module.addImport("uucode", dep.module("uucode"));
        }
    }

    const props_run = b.addRunArtifact(props_exe);
    const symbols_run = b.addRunArtifact(symbols_exe);

    // Generated Zig files have to end with .zig
    const wf = b.addWriteFiles();
    const props_output = wf.addCopyFile(props_run.captureStdOut(), "props.zig");
    const symbols_output = wf.addCopyFile(symbols_run.captureStdOut(), "symbols.zig");

    const lib = b.addLibrary(.{
        .name = "ghostty_vt",
        .root_module = b.createModule(.{
            .root_source_file = b.path("lib.zig"),
            .target = target,
            .optimize = optimize,
        }),
        .linkage = .static,
    });
    lib.linkLibC();

    // Add terminal_options build options (matching ghostty's lib-vt config)
    const terminal_opts = b.addOptions();
    terminal_opts.addOption(
        @import("ghostty_src/terminal/build_options.zig").Artifact,
        "artifact",
        .lib,
    );
    terminal_opts.addOption(bool, "c_abi", false);
    terminal_opts.addOption(bool, "oniguruma", false);
    terminal_opts.addOption(bool, "simd", false);
    terminal_opts.addOption(bool, "slow_runtime_safety", false);
    terminal_opts.addOption(bool, "kitty_graphics", false);
    terminal_opts.addOption(bool, "tmux_control_mode", false);
    lib.root_module.addOptions("terminal_options", terminal_opts);

    // Add uucode import to the library module
    if (b.lazyDependency("uucode", .{
        .target = target,
        .tables_path = uucode_tables,
        .build_config_path = b.path("ghostty_src/build/uucode_config.zig"),
    })) |dep| {
        lib.root_module.addImport("uucode", dep.module("uucode"));
    }

    props_output.addStepDependencies(&lib.step);
    lib.root_module.addAnonymousImport("unicode_tables", .{
        .root_source_file = props_output,
    });
    symbols_output.addStepDependencies(&lib.step);
    lib.root_module.addAnonymousImport("symbols_tables", .{
        .root_source_file = symbols_output,
    });

    const include_step = b.addInstallHeaderFile(
        b.path("../include/ghostty_vt.h"),
        "ghostty_vt.h",
    );

    const lib_install = b.addInstallLibFile(lib.getEmittedBin(), "libghostty_vt.a");
    b.getInstallStep().dependOn(&include_step.step);
    b.getInstallStep().dependOn(&lib_install.step);
}
