/// Platform-specific screenshot capture.
/// Returns base64-encoded PNG bytes (caller must free).
const std = @import("std");
const builtin = @import("builtin");

pub const ScreenshotError = error{
    Unsupported,
    CaptureFailed,
    ReadFailed,
    OutOfMemory,
};

/// Captures a screenshot and returns it as base64-encoded PNG.
/// Caller must free the returned slice.
pub fn captureBase64(allocator: std.mem.Allocator) ![]u8 {
    const png = try capturePng(allocator);
    defer allocator.free(png);

    const encoded_len = std.base64.standard.Encoder.calcSize(png.len);
    const out = try allocator.alloc(u8, encoded_len);
    _ = std.base64.standard.Encoder.encode(out, png);
    return out;
}

/// Captures a screenshot and returns the raw PNG bytes (caller must free).
fn capturePng(allocator: std.mem.Allocator) ![]u8 {
    // Write to a temp file, read it back, delete it.
    const tmp_path = try makeTmpPath(allocator);
    defer allocator.free(tmp_path);

    try captureToFile(allocator, tmp_path);
    defer std.fs.deleteFileAbsolute(tmp_path) catch {};

    const file = std.fs.openFileAbsolute(tmp_path, .{}) catch return error.ReadFailed;
    defer file.close();
    return file.readToEndAlloc(allocator, 50 * 1024 * 1024) catch error.ReadFailed;
}

/// Creates a temp file path for a PNG (caller must free).
fn makeTmpPath(allocator: std.mem.Allocator) ![]u8 {
    const ts = std.time.milliTimestamp();
    return switch (builtin.os.tag) {
        .windows => std.fmt.allocPrint(allocator, "C:\\Windows\\Temp\\poke-shot-{d}.png", .{ts}),
        else => std.fmt.allocPrint(allocator, "/tmp/poke-shot-{d}.png", .{ts}),
    };
}

/// Captures a screenshot to the given file path using the platform's tool.
fn captureToFile(allocator: std.mem.Allocator, path: []const u8) !void {
    switch (builtin.os.tag) {
        .macos => try captureMacos(allocator, path),
        .linux => try captureLinux(allocator, path),
        .windows => try captureWindows(allocator, path),
        else => return error.Unsupported,
    }
}

// ── macOS ──────────────────────────────────────────────────────────────────

fn captureMacos(allocator: std.mem.Allocator, path: []const u8) !void {
    var child = std.process.Child.init(
        &.{ "/usr/sbin/screencapture", "-x", path },
        allocator,
    );
    child.stdin_behavior = .Ignore;
    child.stdout_behavior = .Ignore;
    child.stderr_behavior = .Ignore;
    try child.spawn();
    const result = try child.wait();
    if (result != .Exited or result.Exited != 0) return error.CaptureFailed;
}

// ── Linux ──────────────────────────────────────────────────────────────────

fn captureLinux(allocator: std.mem.Allocator, path: []const u8) !void {
    // Detect available screenshot tool
    const search_dirs = [_][]const u8{
        "/usr/bin", "/usr/local/bin", "/bin", "/snap/bin",
    };

    const tools_ordered = [_]struct { exe: []const u8, build_argv: *const fn (std.mem.Allocator, []const u8) anyerror![]const []const u8 }{
        .{ .exe = "scrot", .build_argv = buildScrotArgv },
        .{ .exe = "gnome-screenshot", .build_argv = buildGnomeArgv },
        .{ .exe = "import", .build_argv = buildImportArgv },
        .{ .exe = "spectacle", .build_argv = buildSpectacleArgv },
        .{ .exe = "xwd", .build_argv = buildXwdArgv },
    };

    for (tools_ordered) |tool| {
        for (search_dirs) |dir| {
            const full = std.fs.path.join(allocator, &.{ dir, tool.exe }) catch continue;
            defer allocator.free(full);
            std.fs.accessAbsolute(full, .{}) catch continue;

            const argv = tool.build_argv(allocator, path) catch continue;
            defer {
                for (argv) |a| allocator.free(@constCast(a));
                allocator.free(argv);
            }

            var child = std.process.Child.init(argv, allocator);
            child.stdin_behavior = .Ignore;
            child.stdout_behavior = .Ignore;
            child.stderr_behavior = .Ignore;
            child.spawn() catch continue;
            const result = child.wait() catch continue;
            if (result == .Exited and result.Exited == 0) return;
        }
    }
    return error.CaptureFailed;
}

fn buildScrotArgv(allocator: std.mem.Allocator, path: []const u8) ![]const []const u8 {
    const argv = try allocator.alloc([]const u8, 2);
    argv[0] = try allocator.dupe(u8, "scrot");
    argv[1] = try allocator.dupe(u8, path);
    return argv;
}

fn buildGnomeArgv(allocator: std.mem.Allocator, path: []const u8) ![]const []const u8 {
    const argv = try allocator.alloc([]const u8, 3);
    argv[0] = try allocator.dupe(u8, "gnome-screenshot");
    argv[1] = try allocator.dupe(u8, "-f");
    argv[2] = try allocator.dupe(u8, path);
    return argv;
}

fn buildImportArgv(allocator: std.mem.Allocator, path: []const u8) ![]const []const u8 {
    const argv = try allocator.alloc([]const u8, 4);
    argv[0] = try allocator.dupe(u8, "import");
    argv[1] = try allocator.dupe(u8, "-window");
    argv[2] = try allocator.dupe(u8, "root");
    argv[3] = try allocator.dupe(u8, path);
    return argv;
}

fn buildSpectacleArgv(allocator: std.mem.Allocator, path: []const u8) ![]const []const u8 {
    const argv = try allocator.alloc([]const u8, 5);
    argv[0] = try allocator.dupe(u8, "spectacle");
    argv[1] = try allocator.dupe(u8, "-b");
    argv[2] = try allocator.dupe(u8, "-n");
    argv[3] = try allocator.dupe(u8, "-o");
    argv[4] = try allocator.dupe(u8, path);
    return argv;
}

fn buildXwdArgv(allocator: std.mem.Allocator, path: []const u8) ![]const []const u8 {
    // xwd -root -silent | convert - png:<path>
    const cmd = try std.fmt.allocPrint(
        allocator,
        "xwd -root -silent | convert - png:{s}",
        .{path},
    );
    defer allocator.free(cmd);
    const argv = try allocator.alloc([]const u8, 3);
    argv[0] = try allocator.dupe(u8, "/bin/sh");
    argv[1] = try allocator.dupe(u8, "-c");
    argv[2] = try allocator.dupe(u8, cmd);
    return argv;
}

// ── Windows ────────────────────────────────────────────────────────────────

fn captureWindows(allocator: std.mem.Allocator, path: []const u8) !void {
    const ps_script = try std.fmt.allocPrint(allocator,
        \\Add-Type -AssemblyName System.Windows.Forms,System.Drawing
        \\$s = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
        \\$bmp = New-Object System.Drawing.Bitmap($s.Width, $s.Height)
        \\$g = [System.Drawing.Graphics]::FromImage($bmp)
        \\$g.CopyFromScreen($s.Location, [System.Drawing.Point]::Empty, $s.Size)
        \\$bmp.Save('{s}', [System.Drawing.Imaging.ImageFormat]::Png)
        \\$g.Dispose(); $bmp.Dispose()
    , .{path});
    defer allocator.free(ps_script);

    var child = std.process.Child.init(
        &.{ "powershell.exe", "-NoProfile", "-NonInteractive", "-Command", ps_script },
        allocator,
    );
    child.stdin_behavior = .Ignore;
    child.stdout_behavior = .Ignore;
    child.stderr_behavior = .Ignore;
    try child.spawn();
    const result = try child.wait();
    if (result != .Exited or result.Exited != 0) return error.CaptureFailed;
}
