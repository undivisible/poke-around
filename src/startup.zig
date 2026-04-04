/// OS-specific install / uninstall of the poke-around login item.
///
/// macOS   – LaunchAgent plist in ~/Library/LaunchAgents/
/// Linux   – systemd user service in ~/.config/systemd/user/
/// Windows – registry key via reg.exe (HKCU\...\Run)
const std = @import("std");
const builtin = @import("builtin");
const config = @import("config.zig");
const mac = if (builtin.os.tag == .macos) @cImport({
    @cInclude("CoreFoundation/CoreFoundation.h");
    @cInclude("ApplicationServices/ApplicationServices.h");
    @cInclude("CoreGraphics/CoreGraphics.h");
}) else struct {};

/// Returns true when poke-around is registered to start at login.
pub fn isEnabled(allocator: std.mem.Allocator) bool {
    if (comptime builtin.os.tag == .macos) return isMacosEnabled(allocator);
    if (comptime builtin.os.tag == .linux) return isLinuxEnabled(allocator);
    if (comptime builtin.os.tag == .windows) return isWindowsEnabled(allocator);
    return false;
}

/// Register the current executable as a login item and start it via the OS
/// service manager. Safe to call while already running — the daemon's singleton
/// guard handles any overlap.
pub fn enable(allocator: std.mem.Allocator) !void {
    const exe = try std.fs.selfExePathAlloc(allocator);
    defer allocator.free(exe);
    if (comptime builtin.os.tag == .macos) return enableMacos(allocator, exe);
    if (comptime builtin.os.tag == .linux) return enableLinux(allocator, exe);
    if (comptime builtin.os.tag == .windows) return enableWindows(allocator, exe);
}

/// Remove the login-item entry. Does not stop the currently running daemon.
pub fn disable(allocator: std.mem.Allocator) !void {
    if (comptime builtin.os.tag == .macos) return disableMacos(allocator);
    if (comptime builtin.os.tag == .linux) return disableLinux(allocator);
    if (comptime builtin.os.tag == .windows) return disableWindows(allocator);
}

/// On macOS, ensure the LaunchAgent is installed and prompt the permissions the
/// app uses at runtime.
pub fn ensureMacosPersistenceAndPermissions(allocator: std.mem.Allocator) !void {
    if (comptime builtin.os.tag != .macos) return;

    if (!isMacosEnabled(allocator)) {
        enable(allocator) catch |err| {
            std.debug.print("poke-around: could not install launch at login (LaunchAgent): {}\n", .{err});
            return err;
        };
    }
    requestMacPermissions();
}

// ── path helpers ──────────────────────────────────────────────────────────────

fn launchAgentPath(allocator: std.mem.Allocator) ![]u8 {
    const home = try config.getHomeDir(allocator);
    defer allocator.free(home);
    return std.fs.path.join(allocator, &.{
        home, "Library", "LaunchAgents", "com.poke-around.plist",
    });
}

fn systemdServicePath(allocator: std.mem.Allocator) ![]u8 {
    const home = try config.getHomeDir(allocator);
    defer allocator.free(home);
    return std.fs.path.join(allocator, &.{
        home, ".config", "systemd", "user", "poke-around.service",
    });
}

/// Create a directory and all missing parents (like mkdir -p).
fn ensureDirAbsolute(abs_path: []const u8) !void {
    std.fs.makeDirAbsolute(abs_path) catch |err| switch (err) {
        error.PathAlreadyExists => {},
        error.FileNotFound => {
            // parent missing — recurse
            const parent = std.fs.path.dirname(abs_path) orelse return err;
            try ensureDirAbsolute(parent);
            try std.fs.makeDirAbsolute(abs_path);
        },
        else => return err,
    };
}

// ── macOS ─────────────────────────────────────────────────────────────────────

fn isMacosEnabled(allocator: std.mem.Allocator) bool {
    const path = launchAgentPath(allocator) catch return false;
    defer allocator.free(path);
    std.fs.accessAbsolute(path, .{}) catch return false;
    return true;
}

fn requestMacPermissions() void {
    if (!mac.CGPreflightScreenCaptureAccess()) {
        _ = mac.CGRequestScreenCaptureAccess();
    }

    if (mac.AXIsProcessTrusted() == 0) {
        const key = @as(mac.CFTypeRef, @ptrCast(mac.kAXTrustedCheckOptionPrompt));
        const value = @as(mac.CFTypeRef, @ptrCast(mac.kCFBooleanTrue));
        const keys = [1]mac.CFTypeRef{key};
        const values = [1]mac.CFTypeRef{value};
        const opts = mac.CFDictionaryCreate(
            mac.kCFAllocatorDefault,
            @ptrCast(@constCast(&keys)),
            @ptrCast(@constCast(&values)),
            1,
            &mac.kCFTypeDictionaryKeyCallBacks,
            &mac.kCFTypeDictionaryValueCallBacks,
        );
        if (opts != null) {
            defer mac.CFRelease(opts);
            _ = mac.AXIsProcessTrustedWithOptions(opts);
        }
    }
}

const PLIST_TEMPLATE =
    \\<?xml version="1.0" encoding="UTF-8"?>
    \\<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
    \\<plist version="1.0">
    \\<dict>
    \\    <key>Label</key>
    \\    <string>com.poke-around</string>
    \\    <key>ProgramArguments</key>
    \\    <array>
    \\        <string>{s}</string>
    \\        <string>--daemon-worker</string>
    \\    </array>
    \\    <key>RunAtLoad</key>
    \\    <true/>
    \\    <key>KeepAlive</key>
    \\    <true/>
    \\    <key>StandardOutPath</key>
    \\    <string>/tmp/poke-around.log</string>
    \\    <key>StandardErrorPath</key>
    \\    <string>/tmp/poke-around-error.log</string>
    \\</dict>
    \\</plist>
;

fn enableMacos(allocator: std.mem.Allocator, exe: []const u8) !void {
    const path = try launchAgentPath(allocator);
    defer allocator.free(path);

    // ~/Library/LaunchAgents almost always exists; create it just in case.
    const dir = std.fs.path.dirname(path) orelse return error.BadPath;
    std.fs.makeDirAbsolute(dir) catch |err| switch (err) {
        error.PathAlreadyExists => {},
        else => return err,
    };

    const plist = try std.fmt.allocPrint(allocator, PLIST_TEMPLATE, .{exe});
    defer allocator.free(plist);
    {
        const f = try std.fs.createFileAbsolute(path, .{});
        defer f.close();
        try f.writeAll(plist);
    }

    // load -w registers the job and starts it immediately; the daemon's
    // singleton guard handles any overlap with an already-running instance.
    const load_result = try std.process.Child.run(.{
        .allocator = allocator,
        .argv = &.{ "launchctl", "load", "-w", path },
    });
    defer allocator.free(load_result.stdout);
    defer allocator.free(load_result.stderr);
    if (load_result.term.Exited != 0) {
        if (load_result.stderr.len > 0) {
            std.debug.print("{s}", .{load_result.stderr});
            if (!std.mem.endsWith(u8, load_result.stderr, "\n")) std.debug.print("\n", .{});
        }
        return error.LaunchctlLoadFailed;
    }
}

fn disableMacos(allocator: std.mem.Allocator) !void {
    const path = try launchAgentPath(allocator);
    defer allocator.free(path);
    _ = std.process.Child.run(.{
        .allocator = allocator,
        .argv = &.{ "launchctl", "unload", "-w", path },
    }) catch {};
    std.fs.deleteFileAbsolute(path) catch |err| switch (err) {
        error.FileNotFound => {},
        else => return err,
    };
}

// ── Linux ─────────────────────────────────────────────────────────────────────

fn isLinuxEnabled(allocator: std.mem.Allocator) bool {
    const path = systemdServicePath(allocator) catch return false;
    defer allocator.free(path);
    std.fs.accessAbsolute(path, .{}) catch return false;
    return true;
}

const SERVICE_TEMPLATE =
    \\[Unit]
    \\Description=Poke Around — expose your machine to your Poke AI assistant
    \\After=network-online.target
    \\Wants=network-online.target
    \\
    \\[Service]
    \\Type=simple
    \\ExecStart={s} --daemon-worker
    \\Restart=on-failure
    \\RestartSec=10
    \\
    \\[Install]
    \\WantedBy=default.target
;

fn enableLinux(allocator: std.mem.Allocator, exe: []const u8) !void {
    const path = try systemdServicePath(allocator);
    defer allocator.free(path);
    const dir = std.fs.path.dirname(path) orelse return error.BadPath;
    try ensureDirAbsolute(dir);

    const svc = try std.fmt.allocPrint(allocator, SERVICE_TEMPLATE, .{exe});
    defer allocator.free(svc);
    {
        const f = try std.fs.createFileAbsolute(path, .{});
        defer f.close();
        try f.writeAll(svc);
    }

    _ = std.process.Child.run(.{
        .allocator = allocator,
        .argv = &.{ "systemctl", "--user", "daemon-reload" },
    }) catch {};
    // --now also starts the service immediately
    _ = std.process.Child.run(.{
        .allocator = allocator,
        .argv = &.{ "systemctl", "--user", "enable", "--now", "poke-around.service" },
    }) catch {};
}

fn disableLinux(allocator: std.mem.Allocator) !void {
    // --now also stops the service
    _ = std.process.Child.run(.{
        .allocator = allocator,
        .argv = &.{ "systemctl", "--user", "disable", "--now", "poke-around.service" },
    }) catch {};

    const path = try systemdServicePath(allocator);
    defer allocator.free(path);
    std.fs.deleteFileAbsolute(path) catch |err| switch (err) {
        error.FileNotFound => {},
        else => return err,
    };
    _ = std.process.Child.run(.{
        .allocator = allocator,
        .argv = &.{ "systemctl", "--user", "daemon-reload" },
    }) catch {};
}

// ── Windows ──────────────────────────────────────────────────────────────────

const WIN_REG_KEY = "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const WIN_REG_NAME = "PokeAround";

fn isWindowsEnabled(allocator: std.mem.Allocator) bool {
    const result = std.process.Child.run(.{
        .allocator = allocator,
        .argv = &.{ "reg", "query", WIN_REG_KEY, "/v", WIN_REG_NAME },
    }) catch return false;
    defer allocator.free(result.stdout);
    defer allocator.free(result.stderr);
    return result.term.Exited == 0;
}

fn enableWindows(allocator: std.mem.Allocator, exe: []const u8) !void {
    const result = try std.process.Child.run(.{
        .allocator = allocator,
        .argv = &.{ "reg", "add", WIN_REG_KEY, "/v", WIN_REG_NAME, "/t", "REG_SZ", "/d", exe, "/f" },
    });
    defer allocator.free(result.stdout);
    defer allocator.free(result.stderr);
    if (result.term.Exited != 0) return error.RegistryWriteFailed;
}

fn disableWindows(allocator: std.mem.Allocator) !void {
    const result = std.process.Child.run(.{
        .allocator = allocator,
        .argv = &.{ "reg", "delete", WIN_REG_KEY, "/v", WIN_REG_NAME, "/f" },
    }) catch return;
    allocator.free(result.stdout);
    allocator.free(result.stderr);
}
