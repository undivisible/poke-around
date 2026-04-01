/// App orchestrator: startup, bridge process management, reconnect watchdog.
const std = @import("std");
const builtin = @import("builtin");

const config = @import("config.zig");
const platform = @import("platform.zig");
const mcp_server = @import("mcp_server.zig");
const agents = @import("agents.zig");

// ── Bridge process management ───────────────────────────────────────────────

const RECONNECT_DELAY_MS: u64 = 15_000;

pub const ansi = struct {
    pub const reset = "\x1b[0m";
    pub const dim = "\x1b[2m";
    pub const green = "\x1b[32m";
    pub const blue = "\x1b[34m";
    pub const yellow = "\x1b[33m";
    pub const red = "\x1b[31m";
    pub const bold = "\x1b[1m";
};

pub const AppRuntime = struct {
    allocator: std.mem.Allocator,
    state: *mcp_server.AppState,
    mcp_port: u16,
    verbose: bool,
    bridge_process: ?std.process.Child,
    bridge_path: []u8,
    reconnect_timer: ?std.Thread,
    stop_flag: std.atomic.Value(bool),

    pub fn deinit(self: *AppRuntime) void {
        self.stop_flag.store(true, .release);
        if (self.bridge_process) |*p| {
            _ = p.kill() catch std.process.Child.Term{ .Signal = 15 };
            _ = p.wait() catch std.process.Child.Term{ .Exited = 0 };
        }
        self.allocator.free(self.bridge_path);
        // Free state and runtime allocations
        self.state.deinit();
        self.allocator.destroy(self.state);
        // Runtime itself is freed by caller with allocator.destroy(runtime)
    }
};

fn log(verbose: bool, comptime fmt: []const u8, args: anytype) void {
    if (!verbose) return;
    const ts = std.time.timestamp();
    const secs = @mod(ts, 86400);
    const h: u64 = @intCast(@divTrunc(secs, 3600));
    const m: u64 = @intCast(@divTrunc(@mod(secs, 3600), 60));
    const s: u64 = @intCast(@mod(secs, 60));
    std.debug.print(ansi.dim ++ "{d:0>2}:{d:0>2}:{d:0>2} │ " ++ ansi.reset ++ fmt ++ "\n", .{ h, m, s } ++ args);
}

pub fn logAlways(comptime fmt: []const u8, args: anytype) void {
    const ts = std.time.timestamp();
    const secs = @mod(ts, 86400);
    const h: u64 = @intCast(@divTrunc(secs, 3600));
    const m: u64 = @intCast(@divTrunc(@mod(secs, 3600), 60));
    const s: u64 = @intCast(@mod(secs, 60));
    std.debug.print(ansi.dim ++ "{d:0>2}:{d:0>2}:{d:0>2} │ " ++ ansi.reset ++ fmt ++ "\n", .{ h, m, s } ++ args);
}

// ── Bridge resolution ────────────────────────────────────────────────────────

/// Find or extract the poke-bridge script.
/// Returns the path to the bridge script (caller must free).
pub fn resolveBridgePath(allocator: std.mem.Allocator) ![]u8 {
    // 1. Look alongside the executable
    const exe_path = try std.fs.selfExePathAlloc(allocator);
    defer allocator.free(exe_path);
    const exe_dir = std.fs.path.dirname(exe_path) orelse ".";

    const candidate = try std.fs.path.join(allocator, &.{ exe_dir, "poke-around-bridge.js" });
    if (std.fs.accessAbsolute(candidate, .{})) |_| {
        return candidate;
    } else |_| {
        allocator.free(candidate);
    }

    // 2. Look in config dir
    const cfg_dir = try config.getConfigDir(allocator);
    defer allocator.free(cfg_dir);
    const cfg_bridge = try std.fs.path.join(allocator, &.{ cfg_dir, "poke-around-bridge.js" });
    if (std.fs.accessAbsolute(cfg_bridge, .{})) |_| {
        return cfg_bridge;
    } else |_| {
        allocator.free(cfg_bridge);
    }

    // 3. Look in project bridge/dist (dev mode)
    const dev = try std.fs.path.join(allocator, &.{ exe_dir, "..", "bridge", "dist", "poke-around-bridge.js" });
    if (std.fs.accessAbsolute(dev, .{})) |_| {
        return dev;
    } else |_| {
        allocator.free(dev);
    }

    // 4. Look for raw bridge/poke-bridge.ts (dev mode, run with bun directly)
    const dev_ts = try std.fs.path.join(allocator, &.{ exe_dir, "..", "bridge", "poke-bridge.ts" });
    if (std.fs.accessAbsolute(dev_ts, .{})) |_| {
        return dev_ts;
    } else |_| {
        allocator.free(dev_ts);
    }

    return error.BridgeNotFound;
}

/// Pick the JS/TS runtime to run the bridge.
fn pickRuntime(bridge_path: []const u8) []const u8 {
    const is_ts = std.mem.endsWith(u8, bridge_path, ".ts");
    if (is_ts) {
        // Only bun can run .ts directly
        const bun_paths = [_][]const u8{ "/usr/local/bin/bun", "/opt/homebrew/bin/bun", "bun" };
        for (bun_paths) |p| {
            std.fs.accessAbsolute(p, .{}) catch continue;
            return p;
        }
        return "bun";
    }
    // For .js: try bun first, then node
    const bun_paths = [_][]const u8{ "/usr/local/bin/bun", "/opt/homebrew/bin/bun" };
    for (bun_paths) |p| {
        std.fs.accessAbsolute(p, .{}) catch continue;
        return p;
    }
    return if (builtin.os.tag == .windows) "node.exe" else "node";
}

// ── Bridge process lifecycle ─────────────────────────────────────────────────

const BridgeCtx = struct {
    allocator: std.mem.Allocator,
    runtime: *AppRuntime,
};

pub fn startBridge(runtime: *AppRuntime) !void {
    const mcp_url = try std.fmt.allocPrint(
        runtime.allocator,
        "http://127.0.0.1:{d}/mcp",
        .{runtime.mcp_port},
    );
    defer runtime.allocator.free(mcp_url);

    const rt = pickRuntime(runtime.bridge_path);

    logAlways(ansi.dim ++ "Starting bridge: {s} {s}" ++ ansi.reset, .{ rt, runtime.bridge_path });

    var child = std.process.Child.init(
        &.{ rt, runtime.bridge_path, "tunnel", "--mcp-url", mcp_url },
        runtime.allocator,
    );
    child.stdout_behavior = .Pipe;
    child.stdin_behavior = .Pipe;
    child.stderr_behavior = .Inherit; // let bridge stderr pass through

    try child.spawn();

    // Give the bridge stdin pipe to AppState for sending webhook commands
    runtime.state.bridge_writer_mutex.lock();
    runtime.state.bridge_writer = child.stdin.?;
    runtime.state.bridge_writer_mutex.unlock();

    runtime.bridge_process = child;

    // Read bridge stdout in background thread
    const ctx = try runtime.allocator.create(BridgeCtx);
    ctx.* = .{ .allocator = runtime.allocator, .runtime = runtime };
    const t = try std.Thread.spawn(.{}, bridgeStdoutReader, .{ctx});
    t.detach();
}

fn bridgeStdoutReader(ctx: *BridgeCtx) void {
    defer ctx.allocator.destroy(ctx);
    const runtime = ctx.runtime;

    const stdout = runtime.bridge_process.?.stdout orelse return;
    var buf: [4096]u8 = undefined;
    var line_buf = std.ArrayList(u8).init(runtime.allocator);
    defer line_buf.deinit();

    while (!runtime.stop_flag.load(.acquire)) {
        const n = stdout.read(&buf) catch break;
        if (n == 0) break;

        for (buf[0..n]) |c| {
            if (c == '\n') {
                if (line_buf.items.len > 0) {
                    handleBridgeEvent(runtime, line_buf.items) catch |err| {
                        logAlways("Bridge event error: {}", .{err});
                    };
                    line_buf.clearRetainingCapacity();
                }
            } else {
                line_buf.append(c) catch {};
            }
        }
    }

    // Bridge disconnected
    if (!runtime.stop_flag.load(.acquire)) {
        logAlways("Bridge stdout closed. Scheduling reconnect in {d}s...", .{RECONNECT_DELAY_MS / 1000});
        _ = std.Thread.spawn(.{}, reconnectAfterDelay, .{runtime}) catch {};
    }
}

fn handleBridgeEvent(runtime: *AppRuntime, line: []const u8) !void {
    const trimmed = std.mem.trim(u8, line, " \t\r");
    if (trimmed.len == 0) return;

    var arena = std.heap.ArenaAllocator.init(runtime.allocator);
    defer arena.deinit();
    const allocator = arena.allocator();

    const parsed = std.json.parseFromSlice(std.json.Value, allocator, trimmed, .{}) catch {
        // If it's not JSON, it might be auth output from the Poke SDK (like login codes)
        logAlways("{s}", .{trimmed});
        return;
    };
    const obj = switch (parsed.value) {
        .object => |o| o,
        else => return,
    };

    const event_type = switch (obj.get("type") orelse return) {
        .string => |s| s,
        else => return,
    };

    if (std.mem.eql(u8, event_type, "connected")) {
        const conn_id = switch (obj.get("connectionId") orelse std.json.Value{ .string = "?" }) {
            .string => |s| s,
            else => "?",
        };
        logAlways(ansi.green ++ ansi.bold ++ "✔ Tunnel connected ({s})" ++ ansi.reset, .{conn_id});
        logAlways(ansi.green ++ "Ready — your Poke agent can now access this machine." ++ ansi.reset, .{});

        // Save connection ID and notify Poke
        config.setStateField(runtime.allocator, "connectionId", conn_id) catch {};
        notifyPoke(runtime, conn_id) catch |err| logAlways(ansi.red ++ "Notify Poke failed: {}" ++ ansi.reset, .{err});
        agents.startScheduler(runtime.allocator, runtime.verbose) catch |err|
            logAlways(ansi.red ++ "Agent scheduler error: {}" ++ ansi.reset, .{err});

    } else if (std.mem.eql(u8, event_type, "disconnected")) {
        logAlways(ansi.yellow ++ "Tunnel disconnected." ++ ansi.reset, .{});
        agents.stopScheduler();

    } else if (std.mem.eql(u8, event_type, "error")) {
        const msg = switch (obj.get("message") orelse std.json.Value{ .string = "unknown" }) {
            .string => |s| s,
            else => "unknown",
        };
        logAlways(ansi.red ++ "Bridge error: {s}" ++ ansi.reset, .{msg});

    } else if (std.mem.eql(u8, event_type, "tools_synced")) {
        const count = switch (obj.get("count") orelse std.json.Value{ .integer = 0 }) {
            .integer => |n| n,
            else => 0,
        };
        log(runtime.verbose, "Tools synced: {d}", .{count});

    } else if (std.mem.eql(u8, event_type, "user_restart")) {
        logAlways(ansi.yellow ++ "User requested restart..." ++ ansi.reset, .{});
        if (runtime.bridge_process) |*p| {
            _ = p.kill() catch std.process.Child.Term{ .Signal = 15 };
            _ = p.wait() catch std.process.Child.Term{ .Exited = 0 };
        }
        runtime.bridge_process = null;

    } else if (std.mem.eql(u8, event_type, "user_exit")) {
        logAlways(ansi.blue ++ ansi.bold ++ "User requested exit via tray." ++ ansi.reset, .{});
        initiateShutdown();

    } else if (std.mem.eql(u8, event_type, "webhook_ready")) {
        log(runtime.verbose, "Webhook configured.", .{});

    } else if (std.mem.eql(u8, event_type, "webhook_sent")) {
        log(runtime.verbose, "Webhook sent.", .{});

    } else if (std.mem.eql(u8, event_type, "webhook_error")) {
        const msg = switch (obj.get("message") orelse std.json.Value{ .string = "unknown" }) {
            .string => |s| s,
            else => "unknown",
        };
        logAlways("Webhook error: {s}", .{msg});
    }
}

fn reconnectAfterDelay(runtime: *AppRuntime) void {
    std.time.sleep(RECONNECT_DELAY_MS * std.time.ns_per_ms);
    if (runtime.stop_flag.load(.acquire)) return;

    logAlways("Reconnecting bridge...", .{});

    // Kill old bridge process if still alive
    if (runtime.bridge_process) |*p| {
        runtime.state.bridge_writer_mutex.lock();
        runtime.state.bridge_writer = null;
        runtime.state.bridge_writer_mutex.unlock();
        _ = p.kill() catch std.process.Child.Term{ .Signal = 15 };
        _ = p.wait() catch std.process.Child.Term{ .Exited = 0 };
        runtime.bridge_process = null;
    }

    startBridge(runtime) catch |err| {
        logAlways("Failed to restart bridge: {}", .{err});
    };
}

// ── Poke notification ─────────────────────────────────────────────────────────

fn buildAccessModeMessage(allocator: std.mem.Allocator, mode: mcp_server.PermissionMode) ![]u8 {
    return switch (mode) {
        .limited => allocator.dupe(u8,
            "Access mode: Limited. You can read files, list directories, and run safe read-only commands. " ++
                "File writes, screenshots, and other commands are disabled."),
        .sandbox => allocator.dupe(u8,
            "Access mode: Sandbox. You can run commands like brew, node, python, ffmpeg, curl, and more. " ++
                "File writes are restricted to ~/Downloads and /tmp. Screenshots are disabled."),
        .full => allocator.dupe(u8,
            "Access mode: Full. You can run any shell command, read/write files, take screenshots, " ++
                "and check system info. Only destructive actions require a one-time approval per session."),
    };
}

fn notifyPoke(runtime: *AppRuntime, connection_id: []const u8) !void {
    const mode_msg = try buildAccessModeMessage(runtime.allocator, runtime.state.permission_mode);
    defer runtime.allocator.free(mode_msg);

    const msg = try std.fmt.allocPrint(
        runtime.allocator,
        "Hey! I've connected my computer to you via Poke Around (tunnel: {s}). {s} " ++
            "Just use the tools whenever I ask you to do something on my computer. " ++
            "Now reply me with \"now I am connected to your computer\" but everytime write those replies in most creative fun way.",
        .{ connection_id, mode_msg },
    );
    defer runtime.allocator.free(msg);

    // Use send-message directly (same as `poke-around notify` CLI).
    // This avoids depending on the webhook being set up in the bridge.
    var child = std.process.Child.init(
        &.{ pickRuntime(runtime.bridge_path), runtime.bridge_path, "send-message", "--message", msg },
        runtime.allocator,
    );
    child.stdout_behavior = .Ignore;
    child.stderr_behavior = .Inherit;
    child.stdin_behavior = .Ignore;
    try child.spawn();
    _ = child.wait() catch {};

    logAlways(ansi.dim ++ "Notified Poke agent about connection." ++ ansi.reset, .{});
}

// ── Stale connection cleanup ──────────────────────────────────────────────────

fn cleanupStaleConnections(allocator: std.mem.Allocator) void {
    const raw = config.readStateJson(allocator) catch return;
    defer allocator.free(raw);

    const parsed = std.json.parseFromSlice(std.json.Value, allocator, raw, .{}) catch return;
    defer parsed.deinit();

    const obj = switch (parsed.value) {
        .object => |o| o,
        else => return,
    };

    // Read connection IDs to clean up
    var ids = std.ArrayList([]const u8).init(allocator);
    defer ids.deinit();

    if (obj.get("connectionId")) |v| {
        if (v == .string) ids.append(v.string) catch {};
    }
    if (obj.get("connectionHistory")) |v| {
        if (v == .array) {
            for (v.array.items) |item| {
                if (item == .string) ids.append(item.string) catch {};
            }
        }
    }

    if (ids.items.len == 0) return;

    // Read Poke token from poke SDK credentials
    const token = readPokeToken(allocator) catch return;
    defer allocator.free(token);

    const base_url = std.process.getEnvVarOwned(allocator, "POKE_API") catch
        allocator.dupe(u8, "https://poke.com/api/v1") catch return;
    defer allocator.free(base_url);

    logAlways(ansi.dim ++ "Cleaning up {d} old connection(s)..." ++ ansi.reset, .{ids.items.len});

    for (ids.items) |id| {
        const url = std.fmt.allocPrint(allocator, "{s}/mcp/connections/{s}", .{ base_url, id }) catch continue;
        defer allocator.free(url);

        const auth_hdr = std.fmt.allocPrint(allocator, "Authorization: Bearer {s}", .{token}) catch continue;
        defer allocator.free(auth_hdr);
        const del_argv = [_][]const u8{ "curl", "-fsSL", "-X", "DELETE", "-H", auth_hdr, url };
        var child = std.process.Child.init(&del_argv, allocator);
        child.stdout_behavior = .Ignore;
        child.stderr_behavior = .Ignore;
        child.stdin_behavior = .Ignore;
        child.spawn() catch continue;
        _ = child.wait() catch std.process.Child.Term{ .Exited = 1 };
    }
}

/// Read the Poke SDK auth token from ~/.config/poke/credentials.json
pub fn readPokeToken(allocator: std.mem.Allocator) ![]u8 {
    const cred_path = try pokeCredentialsPath(allocator);
    defer allocator.free(cred_path);

    const file = try std.fs.openFileAbsolute(cred_path, .{});
    defer file.close();
    const content = try file.readToEndAlloc(allocator, 64 * 1024);
    defer allocator.free(content);

    const parsed = try std.json.parseFromSlice(std.json.Value, allocator, content, .{});
    defer parsed.deinit();

    const obj = switch (parsed.value) {
        .object => |o| o,
        else => return error.InvalidCredentials,
    };
    const token = switch (obj.get("token") orelse return error.NoToken) {
        .string => |s| s,
        else => return error.NoToken,
    };
    return allocator.dupe(u8, token);
}

fn pokeCredentialsPath(allocator: std.mem.Allocator) ![]u8 {
    if (std.process.getEnvVarOwned(allocator, "XDG_CONFIG_HOME")) |xdg| {
        defer allocator.free(xdg);
        return std.fs.path.join(allocator, &.{ xdg, "poke", "credentials.json" });
    } else |_| {}
    const home = try config.getHomeDir(allocator);
    defer allocator.free(home);
    return std.fs.path.join(allocator, &.{ home, ".config", "poke", "credentials.json" });
}

// ── Main daemon entry point ───────────────────────────────────────────────────

var global_runtime: ?*AppRuntime = null;

pub fn initiateShutdown() void {
    if (global_runtime) |rt| {
        rt.stop_flag.store(true, .release);
    }
}

pub fn runDaemon(allocator: std.mem.Allocator, mode_str: ?[]const u8, verbose: bool) !void {
    // Kill existing instances on this machine
    platform.killExistingInstances(allocator);

    // Determine permission mode (env > CLI arg > config file)
    const mode = blk: {
        const from_env = std.process.getEnvVarOwned(allocator, "POKE_GATE_PERMISSION_MODE") catch null;
        defer if (from_env) |s| allocator.free(s);
        if (from_env) |s| break :blk mcp_server.parsePermissionMode(s);
        if (mode_str) |s| break :blk mcp_server.parsePermissionMode(s);
        const saved = config.readPermissionMode(allocator) catch null;
        defer if (saved) |s| allocator.free(s);
        if (saved) |s| break :blk mcp_server.parsePermissionMode(s);
        break :blk mcp_server.PermissionMode.full;
    };

    logAlways(ansi.blue ++ ansi.bold ++ "▶ poke-around starting..." ++ ansi.reset, .{});
    logAlways(ansi.dim ++ "Access mode: {s}" ++ ansi.reset, .{@tagName(mode)});

    // Resolve bridge path
    const bridge_path = resolveBridgePath(allocator) catch |err| {
        logAlways(ansi.red ++ "ERROR: Could not find poke-around-bridge.js: {}" ++ ansi.reset, .{err});
        logAlways(ansi.dim ++ "Run: cd bridge && bun install && bun build poke-bridge.ts --bundle --outfile dist/poke-around-bridge.js" ++ ansi.reset, .{});
        return err;
    };
    errdefer allocator.free(bridge_path);

    // Initialize shared state
    const state = try allocator.create(mcp_server.AppState);
    errdefer allocator.destroy(state);
    state.* = try mcp_server.AppState.init(allocator, mode, verbose);
    errdefer state.deinit();

    // Start MCP HTTP server
    const port = try mcp_server.startMcpServer(allocator, state);
    logAlways(ansi.dim ++ "MCP server on port {d}" ++ ansi.reset, .{port});
    const pid_text = try std.fmt.allocPrint(allocator, "{d}", .{std.os.linux.getpid()});
    defer allocator.free(pid_text);
    const port_text = try std.fmt.allocPrint(allocator, "{d}", .{port});
    defer allocator.free(port_text);
    config.setStateField(allocator, "pid", pid_text) catch {};
    config.setStateField(allocator, "port", port_text) catch {};

    // Cleanup stale Poke connections from this machine
    cleanupStaleConnections(allocator);

    // Create runtime
    const runtime = try allocator.create(AppRuntime);
    runtime.* = .{
        .allocator = allocator,
        .state = state,
        .mcp_port = port,
        .verbose = verbose,
        .bridge_process = null,
        .bridge_path = bridge_path,
        .reconnect_timer = null,
        .stop_flag = std.atomic.Value(bool).init(false),
    };

    // Start bridge
    try startBridge(runtime);

    global_runtime = runtime;

    // Block main thread (signal handling)
    while (!runtime.stop_flag.load(.acquire)) {
        std.time.sleep(1 * std.time.ns_per_s);
    }

    logAlways(ansi.blue ++ ansi.bold ++ "Shutting down..." ++ ansi.reset, .{});
    runtime.deinit();
    allocator.destroy(runtime);
}
