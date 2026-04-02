const std = @import("std");
const builtin = @import("builtin");

pub const Os = enum { macos, linux, windows, other };

pub const current_os: Os = switch (builtin.os.tag) {
    .macos => .macos,
    .linux => .linux,
    .windows => .windows,
    else => .other,
};

// ── shell ──────────────────────────────────────────────────────────────────

/// Returns the shell args to run a command string (caller must free the inner slice).
pub fn shellArgs(allocator: std.mem.Allocator, command: []const u8) ![]const []const u8 {
    return switch (current_os) {
        .windows => blk: {
            const argv = try allocator.alloc([]const u8, 3);
            argv[0] = "cmd.exe";
            argv[1] = "/c";
            argv[2] = command;
            break :blk argv;
        },
        else => blk: {
            // prefer zsh, fall back to bash
            const shell = if (shellExists("/bin/zsh")) "/bin/zsh" else "/bin/bash";
            const argv = try allocator.alloc([]const u8, 3);
            argv[0] = shell;
            argv[1] = "-lc";
            argv[2] = command;
            break :blk argv;
        },
    };
}

fn shellExists(path: []const u8) bool {
    std.fs.accessAbsolute(path, .{}) catch return false;
    return true;
}

// ── sandbox ────────────────────────────────────────────────────────────────

/// Returns a sandboxed command string (macOS: sandbox-exec, Linux: bwrap if available).
/// Caller must free the returned slice.
pub fn wrapSandbox(allocator: std.mem.Allocator, command: []const u8, home: []const u8) !struct {
    cmd: []const u8,
    applied: bool,
    note: ?[]const u8,
} {
    switch (current_os) {
        .macos => {
            const sandbox_exec = "/usr/bin/sandbox-exec";
            std.fs.accessAbsolute(sandbox_exec, .{}) catch {
                return .{ .cmd = command, .applied = false, .note = "sandbox-exec unavailable" };
            };
            const profile = try buildMacosSandboxProfile(allocator, home);
            defer allocator.free(profile);
            const escaped_profile = try singleQuote(allocator, profile);
            defer allocator.free(escaped_profile);
            const escaped_cmd = try singleQuote(allocator, command);
            defer allocator.free(escaped_cmd);
            const wrapped = try std.fmt.allocPrint(
                allocator,
                "{s} -p {s} /bin/zsh -lc {s}",
                .{ sandbox_exec, escaped_profile, escaped_cmd },
            );
            return .{ .cmd = wrapped, .applied = true, .note = null };
        },
        .linux => {
            // Try bwrap (bubblewrap)
            const bwrap = findExecutable(allocator, "bwrap") catch null;
            if (bwrap) |bw| {
                defer allocator.free(bw);
                const downloads = try std.fs.path.join(allocator, &.{ home, "Downloads" });
                defer allocator.free(downloads);
                const wrapped = try std.fmt.allocPrint(
                    allocator,
                    "{s} --ro-bind / / --bind {s} {s} --bind /tmp /tmp --dev /dev " ++
                        "--proc /proc --unshare-pid --unshare-net -- /bin/sh -c {s}",
                    .{ bw, downloads, downloads, command },
                );
                return .{ .cmd = wrapped, .applied = true, .note = null };
            }
            return .{
                .cmd = command,
                .applied = false,
                .note = "bwrap not found; running without sandbox",
            };
        },
        else => return .{ .cmd = command, .applied = false, .note = "no sandbox on this platform" },
    }
}

fn buildMacosSandboxProfile(allocator: std.mem.Allocator, home: []const u8) ![]u8 {
    const downloads = try std.fs.path.join(allocator, &.{ home, "Downloads" });
    defer allocator.free(downloads);
    return std.fmt.allocPrint(
        allocator,
        \\(version 1)
        \\(deny default)
        \\(import "system.sb")
        \\(allow process-exec)
        \\(allow process-fork)
        \\(allow file-read*)
        \\(allow network-outbound)
        \\(allow sysctl-read)
        \\(allow file-write*
        \\  (subpath "{s}")
        \\  (subpath "/private/tmp")
        \\  (subpath "/tmp")
        \\)
    ,
        .{downloads},
    );
}

fn singleQuote(allocator: std.mem.Allocator, s: []const u8) ![]u8 {
    // Replace every ' with '"'"' for shell-safe single quoting
    var buf = std.ArrayList(u8).empty;
    try buf.append(allocator, '\'');
    for (s) |c| {
        if (c == '\'') {
            try buf.appendSlice(allocator, "'\"'\"'");
        } else {
            try buf.append(allocator, c);
        }
    }
    try buf.append(allocator, '\'');
    return buf.toOwnedSlice(allocator);
}

fn findExecutable(allocator: std.mem.Allocator, name: []const u8) ![]u8 {
    const paths = [_][]const u8{ "/usr/bin", "/usr/local/bin", "/bin", "/opt/homebrew/bin" };
    for (paths) |dir| {
        const full = try std.fs.path.join(allocator, &.{ dir, name });
        std.fs.accessAbsolute(full, .{}) catch {
            allocator.free(full);
            continue;
        };
        return full;
    }
    return error.FileNotFound;
}

// ── process management ─────────────────────────────────────────────────────

/// Kill other running poke-around instances (best effort, ignores errors).
pub fn killExistingInstances(allocator: std.mem.Allocator) void {
    const my_pid = if (builtin.os.tag != .windows)
        std.os.linux.getpid()
    else
        0;

    switch (current_os) {
        .macos, .linux => {
            var child = std.process.Child.init(
                &.{ "pgrep", "-f", "poke-around" },
                allocator,
            );
            child.stdout_behavior = .Pipe;
            child.stderr_behavior = .Ignore;
            child.spawn() catch return;

            const out = child.stdout.?.readToEndAlloc(allocator, 64 * 1024) catch {
                _ = child.wait() catch {};
                return;
            };
            defer allocator.free(out);
            _ = child.wait() catch {};

            var iter = std.mem.splitScalar(u8, std.mem.trim(u8, out, &std.ascii.whitespace), '\n');
            while (iter.next()) |line| {
                const trimmed = std.mem.trim(u8, line, &std.ascii.whitespace);
                if (trimmed.len == 0) continue;
                const pid = std.fmt.parseInt(i32, trimmed, 10) catch continue;
                if (pid == my_pid or pid == 0) continue;
                // SIGTERM
                const kill_argv = [_][]const u8{ "kill", "-TERM", trimmed };
                var kc = std.process.Child.init(&kill_argv, allocator);
                kc.stdin_behavior = .Ignore;
                kc.stdout_behavior = .Ignore;
                kc.stderr_behavior = .Ignore;
                kc.spawn() catch continue;
                _ = kc.wait() catch {};
            }
        },
        .windows => {
            // tasklist + taskkill
            const query = "wmic process where \"name='poke-around.exe'\" get ProcessId /format:value";
            var child = std.process.Child.init(
                &.{ "cmd.exe", "/c", query },
                allocator,
            );
            child.stdout_behavior = .Pipe;
            child.stderr_behavior = .Ignore;
            child.spawn() catch return;
            const out = child.stdout.?.readToEndAlloc(allocator, 64 * 1024) catch {
                _ = child.wait() catch {};
                return;
            };
            defer allocator.free(out);
            _ = child.wait() catch {};

            var iter = std.mem.splitScalar(u8, out, '\n');
            while (iter.next()) |line| {
                const trimmed = std.mem.trim(u8, line, &std.ascii.whitespace);
                if (!std.mem.startsWith(u8, trimmed, "ProcessId=")) continue;
                const pid_str = trimmed["ProcessId=".len..];
                const pid = std.fmt.parseInt(u32, pid_str, 10) catch continue;
                _ = pid;
                const kill_argv = [_][]const u8{ "taskkill", "/F", "/PID", pid_str };
                var kc = std.process.Child.init(&kill_argv, allocator);
                kc.stdin_behavior = .Ignore;
                kc.stdout_behavior = .Ignore;
                kc.stderr_behavior = .Ignore;
                kc.spawn() catch continue;
                _ = kc.wait() catch {};
            }
        },
        else => {},
    }
}

// ── dangerous pattern detection ────────────────────────────────────────────

/// Returns true if the command contains a dangerous shell pattern.
pub fn hasDangerousPattern(command: []const u8) bool {
    const patterns = [_][]const u8{
        "sudo ",
        "rm -rf",
        "rm -fr",
        "rm -r -f",
        "diskutil erase",
        "mkfs.",
        "mkfs ",
        "shutdown",
        "reboot",
        "launchctl bootout",
        "chmod 777",
    };
    for (patterns) |p| {
        if (std.ascii.indexOfIgnoreCasePos(command, 0, p) != null) return true;
    }
    // curl ... | sh/bash/zsh
    if (std.mem.indexOf(u8, command, "| sh") != null or
        std.mem.indexOf(u8, command, "| bash") != null or
        std.mem.indexOf(u8, command, "| zsh") != null) return true;
    return false;
}

/// Returns true if the command contains a destructive write/rm pattern (full mode check).
pub fn isDestructiveCommand(command: []const u8) bool {
    const patterns = [_][]const u8{ "rm ", "rmdir ", "unlink ", "mkfs", "diskutil erase", "> /" };
    for (patterns) |p| {
        if (std.mem.indexOf(u8, command, p) != null) return true;
    }
    return false;
}

/// Extracts the base executable name from a command segment.
pub fn extractExecutable(allocator: std.mem.Allocator, segment: []const u8) ![]u8 {
    const trimmed = std.mem.trimLeft(u8, segment, " \t()");
    // strip leading sudo
    const no_sudo = if (std.mem.startsWith(u8, trimmed, "sudo "))
        std.mem.trimLeft(u8, trimmed["sudo ".len..], " \t")
    else
        trimmed;

    var i: usize = 0;
    while (i < no_sudo.len and (std.ascii.isAlphanumeric(no_sudo[i]) or
        no_sudo[i] == '_' or no_sudo[i] == '-' or no_sudo[i] == '.' or
        no_sudo[i] == '/')) : (i += 1)
    {}
    const exe_path = no_sudo[0..i];
    if (exe_path.len == 0) return allocator.dupe(u8, "");

    // Take the basename
    const basename = std.fs.path.basename(exe_path);
    return allocator.dupe(u8, basename);
}

/// Split a command into segments on &&, ||, ;, newline.
pub fn splitCommandSegments(allocator: std.mem.Allocator, command: []const u8) ![][]const u8 {
    var segments = std.ArrayList([]const u8).empty;
    var i: usize = 0;
    var start: usize = 0;
    while (i < command.len) {
        if (command[i] == '\n' or command[i] == ';') {
            const seg = std.mem.trim(u8, command[start..i], " \t");
            if (seg.len > 0) try segments.append(allocator, seg);
            start = i + 1;
            i = start;
        } else if (i + 1 < command.len and
            (std.mem.eql(u8, command[i .. i + 2], "&&") or
            std.mem.eql(u8, command[i .. i + 2], "||")))
        {
            const seg = std.mem.trim(u8, command[start..i], " \t");
            if (seg.len > 0) try segments.append(allocator, seg);
            start = i + 2;
            i = start;
        } else {
            i += 1;
        }
    }
    if (start < command.len) {
        const seg = std.mem.trim(u8, command[start..], " \t");
        if (seg.len > 0) try segments.append(allocator, seg);
    }
    return segments.toOwnedSlice(allocator);
}
