const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    const main_module = b.createModule(.{
        .root_source_file = b.path("src/main.zig"),
        .target = target,
        .optimize = optimize,
    });
    // Read version from package.json; fall back to "dev" if unavailable.
    const version: []const u8 = blk: {
        const content = std.fs.cwd().readFileAlloc(b.allocator, "package.json", 8192) catch break :blk "dev";
        defer b.allocator.free(content);
        const parsed = std.json.parseFromSlice(std.json.Value, b.allocator, content, .{}) catch break :blk "dev";
        defer parsed.deinit();
        const ver = switch (parsed.value) {
            .object => |obj| obj.get("version") orelse break :blk "dev",
            else => break :blk "dev",
        };
        break :blk switch (ver) {
            .string => |s| b.dupe(s),
            else => "dev",
        };
    };
    const options = b.addOptions();
    options.addOption([]const u8, "version", b.option([]const u8, "version", version) orelse version);
    main_module.addOptions("build_options", options);

    // ── Main executable ──────────────────────────────────────────────────────
    const exe = b.addExecutable(.{
        .name = "poke-around",
        .root_module = main_module,
    });

    if (target.result.os.tag == .macos) {
        const objc_dep = b.dependency("zig_objc", .{
            .target = target,
            .optimize = optimize,
        });
        exe.root_module.addImport("objc", objc_dep.module("objc"));
        exe.linkFramework("AppKit");
        exe.linkFramework("Foundation");
        exe.linkFramework("ApplicationServices");
        exe.linkFramework("CoreGraphics");
        exe.linkSystemLibrary("objc");
        exe.linkLibC();
    }

    b.installArtifact(exe);

    // Also install the bridge alongside the binary
    const install_bridge = b.addInstallFile(
        b.path("bridge/dist/poke-around-bridge.js"),
        "bin/poke-around-bridge.js",
    );
    b.getInstallStep().dependOn(&install_bridge.step);

    if (target.result.os.tag == .linux) {
        const install_helper = b.addInstallFile(
            b.path("src/menubar_linux.py"),
            "bin/menubar_linux.py",
        );
        b.getInstallStep().dependOn(&install_helper.step);
    }

    // ── Run step ─────────────────────────────────────────────────────────────
    const run_cmd = b.addRunArtifact(exe);
    run_cmd.step.dependOn(b.getInstallStep());
    if (b.args) |args| run_cmd.addArgs(args);

    const run_step = b.step("run", "Run poke-around daemon");
    run_step.dependOn(&run_cmd.step);

    // ── Build bridge (requires bun in PATH) ──────────────────────────────────
    const bridge_step_desc = b.step("bridge", "Bundle the poke-bridge.ts with bun");
    {
        const mkdir = b.addSystemCommand(&.{ "mkdir", "-p", "bridge/dist" });
        const bun_build = b.addSystemCommand(&.{
            "bun", "build", "bridge/poke-bridge.ts",
            "--bundle", "--target=node",
            "--outfile", "bridge/dist/poke-around-bridge.js",
        });
        bun_build.setCwd(b.path("."));
        bun_build.step.dependOn(&mkdir.step);
        bridge_step_desc.dependOn(&bun_build.step);
    }

    // ── Cross-compilation release targets ────────────────────────────────────
    const release_step = b.step("release-all", "Build release binaries for all platforms");

    const release_targets = [_]struct {
        cpu: std.Target.Cpu.Arch,
        os: std.Target.Os.Tag,
        abi: std.Target.Abi,
        name: []const u8,
    }{
        .{ .cpu = .x86_64, .os = .linux, .abi = .gnu, .name = "linux-x86_64" },
        .{ .cpu = .aarch64, .os = .linux, .abi = .gnu, .name = "linux-aarch64" },
        .{ .cpu = .x86_64, .os = .windows, .abi = .gnu, .name = "windows-x86_64" },
        .{ .cpu = .x86_64, .os = .macos, .abi = .none, .name = "macos-x86_64" },
        .{ .cpu = .aarch64, .os = .macos, .abi = .none, .name = "macos-aarch64" },
    };

    for (release_targets) |rt| {
        const cross_target = b.resolveTargetQuery(.{
            .cpu_arch = rt.cpu,
            .os_tag = rt.os,
            .abi = rt.abi,
        });
        const cross_module = b.createModule(.{
            .root_source_file = b.path("src/main.zig"),
            .target = cross_target,
            .optimize = .ReleaseSafe,
        });
        const cross_exe = b.addExecutable(.{
            .name = "poke-around",
            .root_module = cross_module,
        });
        const dest_dir = b.fmt("release/{s}", .{rt.name});
        const install_cross = b.addInstallArtifact(cross_exe, .{
            .dest_dir = .{ .override = .{ .custom = dest_dir } },
        });
        // Also copy bridge to each release dir
        const install_bridge_cross = b.addInstallFile(
            b.path("bridge/dist/poke-around-bridge.js"),
            b.fmt("{s}/poke-around-bridge.js", .{dest_dir}),
        );
        if (rt.os == .linux) {
            const install_helper_cross = b.addInstallFile(
                b.path("src/menubar_linux.py"),
                b.fmt("{s}/menubar_linux.py", .{dest_dir}),
            );
            release_step.dependOn(&install_helper_cross.step);
        }
        release_step.dependOn(&install_cross.step);
        release_step.dependOn(&install_bridge_cross.step);
    }

    // ── Test step ────────────────────────────────────────────────────────────
    const test_step = b.step("test", "Run unit tests");
    const test_module = b.createModule(.{
        .root_source_file = b.path("src/main.zig"),
        .target = target,
        .optimize = optimize,
    });
    const unit_tests = b.addTest(.{
        .root_module = test_module,
    });
    const run_tests = b.addRunArtifact(unit_tests);
    test_step.dependOn(&run_tests.step);
}
