/// CLI entry point — subcommand dispatch for poke-around.
const std = @import("std");

const app = @import("app.zig");
const agents = @import("agents.zig");
const config = @import("config.zig");
const mcp_server = @import("mcp_server.zig");

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();

    const args = try std.process.argsAlloc(allocator);
    defer std.process.argsFree(allocator, args);

    const argv = args[1..]; // skip executable name

    // Global flags
    const verbose = hasFlag(argv, "--verbose") or hasFlag(argv, "-v");
    const mode_str = getFlagValue(argv, "--mode");

    if (argv.len == 0 or (argv.len == 1 and (std.mem.eql(u8, argv[0], "--verbose") or std.mem.eql(u8, argv[0], "-v")))) {
        // Default: daemon mode
        try runDaemon(allocator, mode_str, verbose);
        return;
    }

    const subcmd = argv[0];

    if (std.mem.eql(u8, subcmd, "--mode")) {
        // poke-around --mode <mode> [--verbose]
        try runDaemon(allocator, mode_str, verbose);
        return;
    }

    if (std.mem.eql(u8, subcmd, "run-agent")) {
        const name = if (argv.len > 1) argv[1] else {
            std.debug.print("Usage: poke-around run-agent <name>\n", .{});
            std.process.exit(1);
        };
        try agents.runAgentByName(allocator, name);
        return;
    }

    if (std.mem.eql(u8, subcmd, "agent")) {
        if (argv.len < 2) {
            printAgentUsage();
            std.process.exit(1);
        }
        const agent_subcmd = argv[1];

        if (std.mem.eql(u8, agent_subcmd, "get")) {
            const name = if (argv.len > 2) argv[2] else {
                std.debug.print("Usage: poke-around agent get <name>\n", .{});
                std.process.exit(1);
            };
            try agents.downloadAgent(allocator, name);
            return;
        }

        if (std.mem.eql(u8, agent_subcmd, "create")) {
            const prompt_idx = indexOfFlag(argv, "--prompt");
            const prompt = if (prompt_idx != null and prompt_idx.? + 1 < argv.len)
                argv[prompt_idx.? + 1]
            else if (argv.len > 2)
                argv[2]
            else
                null;
            try runAgentCreate(allocator, prompt);
            return;
        }

        printAgentUsage();
        std.process.exit(1);
    }

    if (std.mem.eql(u8, subcmd, "take-screenshot")) {
        try runTakeScreenshot(allocator);
        return;
    }

    if (std.mem.eql(u8, subcmd, "status")) {
        try runStatus(allocator);
        return;
    }

    if (std.mem.eql(u8, subcmd, "notify")) {
        try runNotify(allocator);
        return;
    }

    if (std.mem.eql(u8, subcmd, "restart")) {
        try runRestart(allocator);
        return;
    }

    if (std.mem.eql(u8, subcmd, "set-mode")) {
        const new_mode = if (argv.len > 1) argv[1] else {
            std.debug.print("Usage: poke-around set-mode <full|limited|sandbox>\n", .{});
            std.process.exit(1);
        };
        const valid = std.mem.eql(u8, new_mode, "full") or
            std.mem.eql(u8, new_mode, "limited") or
            std.mem.eql(u8, new_mode, "sandbox");
        if (!valid) {
            std.debug.print("Invalid mode '{s}'. Use: full, limited, sandbox\n", .{new_mode});
            std.process.exit(1);
        }
        try config.savePermissionMode(allocator, new_mode);
        std.debug.print("Permission mode set to: {s}\n", .{new_mode});
        return;
    }

    if (std.mem.eql(u8, subcmd, "--help") or std.mem.eql(u8, subcmd, "-h") or std.mem.eql(u8, subcmd, "help")) {
        printHelp();
        return;
    }

    std.debug.print("Unknown command: {s}\nRun 'poke-around --help' for usage.\n", .{subcmd});
    std.process.exit(1);
}

// ── Subcommand implementations ─────────────────────────────────────────────

fn runDaemon(allocator: std.mem.Allocator, mode_str: ?[]const u8, verbose: bool) !void {
    // Signal handling
    if (@import("builtin").os.tag != .windows) {
        const sa = std.posix.Sigaction{
            .handler = .{ .handler = handleSignal },
            .mask = std.posix.sigemptyset(),
            .flags = 0,
        };
        std.posix.sigaction(std.posix.SIG.INT, &sa, null);
        std.posix.sigaction(std.posix.SIG.TERM, &sa, null);
    }
    try app.runDaemon(allocator, mode_str, verbose);
}

fn handleSignal(_: c_int) callconv(.c) void {
    app.initiateShutdown();
}

/// Create an agent via the Poke bridge (sends a message to the Poke agent).
fn runAgentCreate(allocator: std.mem.Allocator, prompt: ?[]const u8) !void {
    const SYSTEM_PROMPT =
        \\Generate a Poke Around agent based on my description below.
        \\Write the COMPLETE JavaScript code using the write_file tool to save it directly to the agents folder.
        \\RULES:
        \\- Save to: ~/.config/poke-around/agents/<name>.<interval>.js
        \\- Valid ES module with imports.
        \\- Start with JSDoc frontmatter: @agent, @name, @description, @interval, @author.
        \\- Use: import { Poke, getToken } from "poke";
        \\- Auth: const token = getToken(); const poke = new Poke({ apiKey: token });
        \\- Send results: await poke.sendMessage("...");
        \\- State files: ~/.config/poke-around/agents/.<agent-name>-state.json
        \\- Only send to Poke when something changed (use state files).
        \\- Handle errors with try/catch. Keep under 100 lines.
        \\- Intervals: 10m, 30m, 1h, 2h, 6h, 12h, 24h.
        \\
        \\Now write the agent. My request:
    ;

    const description = if (prompt) |p| p else blk: {
        std.debug.print("\n  Describe the agent you want to create:\n  > ", .{});
        var stdin_buf: [1024]u8 = undefined;
        var stdin = std.fs.File.stdin().reader(&stdin_buf);
        const line = (try stdin.interface.takeDelimiter('\n')) orelse return error.NoInput;
        break :blk std.mem.trim(u8, line, " \t\r\n");
    };

    if (description.len == 0) {
        std.debug.print("  No description provided.\n", .{});
        std.process.exit(1);
    }

    const full_prompt = try std.fmt.allocPrint(allocator, "{s}{s}", .{ SYSTEM_PROMPT, description });
    defer allocator.free(full_prompt);

    // Find bridge and send message via it
    const bridge_path = try app.resolveBridgePath(allocator);
    defer allocator.free(bridge_path);

    std.debug.print("\n  Sending request to Poke...\n", .{});

    var child = std.process.Child.init(
        &.{ pickRuntime(bridge_path), bridge_path, "send-message", "--message", full_prompt },
        allocator,
    );
    child.stdout_behavior = .Pipe;
    child.stderr_behavior = .Inherit;
    child.stdin_behavior = .Ignore;
    try child.spawn();
    const result = try child.wait();
    _ = result;

    std.debug.print("  Request sent! Poke will write the agent file to:\n", .{});
    const agents_dir = try config.getAgentsDir(allocator);
    defer allocator.free(agents_dir);
    std.debug.print("  {s}/<name>.<interval>.js\n\n", .{agents_dir});
    std.debug.print("  Watch for Poke's confirmation in your chat.\n", .{});
}

/// Capture a screenshot and send it to Poke.
fn runTakeScreenshot(allocator: std.mem.Allocator) !void {
    const screenshot = @import("screenshot.zig");

    std.debug.print("Capturing screenshot...\n", .{});
    const b64 = screenshot.captureBase64(allocator) catch |err| {
        std.debug.print("Screenshot failed: {}.\nMake sure screen recording/display access is granted.\n", .{err});
        std.process.exit(1);
    };
    defer allocator.free(b64);

    std.debug.print("Screenshot captured ({d} KB). Sending to Poke...\n", .{b64.len * 3 / 4 / 1024});

    // Send via bridge in one-shot mode
    const bridge_path = try app.resolveBridgePath(allocator);
    defer allocator.free(bridge_path);

    const msg = try std.fmt.allocPrint(
        allocator,
        "Here is a screenshot of my screen right now. Reply me with the image.\n\n```\ndata:image/png;base64,{s}\n```",
        .{b64},
    );
    defer allocator.free(msg);

    var child = std.process.Child.init(
        &.{ pickRuntime(bridge_path), bridge_path, "send-message", "--message", msg },
        allocator,
    );
    child.stdout_behavior = .Ignore;
    child.stderr_behavior = .Inherit;
    child.stdin_behavior = .Ignore;
    try child.spawn();
    _ = try child.wait();

    std.debug.print("Screenshot sent to Poke.\n", .{});
}

fn pickRuntime(bridge_path: []const u8) []const u8 {
    const is_ts = std.mem.endsWith(u8, bridge_path, ".ts");
    if (is_ts) return "bun";
    const bun_paths = [_][]const u8{ "/home/undivisible/.bun/bin/bun", "/usr/local/bin/bun", "/opt/homebrew/bin/bun" };
    for (bun_paths) |p| {
        std.fs.accessAbsolute(p, .{}) catch continue;
        return p;
    }
    return if (@import("builtin").os.tag == .windows) "node.exe" else "node";
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn hasFlag(argv: []const []const u8, flag: []const u8) bool {
    for (argv) |a| if (std.mem.eql(u8, a, flag)) return true;
    return false;
}

fn getFlagValue(argv: []const []const u8, flag: []const u8) ?[]const u8 {
    for (argv, 0..) |a, i| {
        if (std.mem.eql(u8, a, flag) and i + 1 < argv.len) return argv[i + 1];
    }
    return null;
}

fn indexOfFlag(argv: []const []const u8, flag: []const u8) ?usize {
    for (argv, 0..) |a, i| if (std.mem.eql(u8, a, flag)) return i;
    return null;
}

fn printHelp() void {
    std.debug.print(
        \\poke-around — expose your machine to your Poke AI assistant
        \\
        \\USAGE:
        \\  poke-around [--mode <mode>] [--verbose]
        \\  poke-around status
        \\  poke-around notify
        \\  poke-around restart
        \\  poke-around run-agent <name>
        \\  poke-around agent get <name>
        \\  poke-around agent create [--prompt "<description>"]
        \\  poke-around take-screenshot
        \\  poke-around set-mode <full|limited|sandbox>
        \\
        \\OPTIONS:
        \\  --mode <full|limited|sandbox>   Set access permission mode
        \\  --verbose, -v                    Enable verbose logging
        \\
        \\ACCESS MODES:
        \\  full      All tools available; destructive actions need one-time approval
        \\  limited   Read-only tools + safe commands (ls, cat, grep, curl, jq...)
        \\  sandbox   Broader commands; file writes restricted to ~/Downloads and /tmp
        \\
    , .{});
}

fn printAgentUsage() void {
    std.debug.print(
        \\Usage:
        \\  poke-around agent get <name>              Download agent from GitHub
        \\  poke-around agent create [--prompt "..."] Create agent with AI assistance
        \\
    , .{});
}

/// Show the status of poke-around daemon
fn runStatus(allocator: std.mem.Allocator) !void {
    const state_json = config.readStateJson(allocator) catch {
        std.debug.print("\npoke-around is {s}not running{s}\n\n", .{ app.ansi.red, app.ansi.reset });
        std.debug.print("Start it with: {s}poke-around{s} or your service manager\n\n", .{ app.ansi.dim, app.ansi.reset });
        return;
    };
    defer allocator.free(state_json);

    const parsed = std.json.parseFromSlice(std.json.Value, allocator, state_json, .{}) catch {
        std.debug.print("poke-around state file corrupted\n", .{});
        return;
    };
    defer parsed.deinit();

    const obj = switch (parsed.value) {
        .object => |o| o,
        else => {
            std.debug.print("Invalid state format\n", .{});
            return;
        },
    };

    const is_running = blk: {
        const pid_val = obj.get("pid") orelse break :blk false;
        const pid = switch (pid_val) {
            .integer => |i| i,
            .string => |s| std.fmt.parseInt(std.posix.pid_t, s, 10) catch break :blk false,
            else => break :blk false,
        };
        // Check if PID exists
        const pid_text = std.fmt.allocPrint(allocator, "{d}", .{pid}) catch break :blk false;
        defer allocator.free(pid_text);
        const result = std.process.Child.run(.{
            .allocator = allocator,
            .argv = &.{ "kill", "-0", pid_text },
        }) catch break :blk false;
        defer allocator.free(result.stdout);
        defer allocator.free(result.stderr);
        break :blk result.term.Exited == 0;
    };

    const is_starting = !is_running and obj.get("port") == null and obj.get("connectionId") == null;

    std.debug.print("\n", .{});
    std.debug.print("  {s}●{s} poke-around {s}{s}{s}\n\n", .{
        if (is_running) app.ansi.green else if (is_starting) app.ansi.yellow else app.ansi.red,
        app.ansi.reset,
        if (is_running) app.ansi.green else if (is_starting) app.ansi.yellow else app.ansi.red,
        if (is_running) "running" else if (is_starting) "starting" else "stopped",
        app.ansi.reset,
    });

    if (obj.get("connectionId")) |conn| {
        const conn_str = switch (conn) {
            .string => |s| s,
            else => "unknown",
        };
        std.debug.print("  Connection ID: {s}{s}{s}\n", .{ app.ansi.dim, conn_str, app.ansi.reset });
    }

    const mode = config.readPermissionMode(allocator) catch null;
    defer if (mode) |owned_mode| allocator.free(owned_mode);
    std.debug.print("  Access mode:   {s}{s}{s}\n", .{ app.ansi.dim, mode orelse "full", app.ansi.reset });

    if (obj.get("port")) |port_val| {
        const port = switch (port_val) {
            .integer => |p| p,
            .string => |s| std.fmt.parseInt(u16, s, 10) catch 0,
            else => 0,
        };
        std.debug.print("  MCP server:    {s}http://127.0.0.1:{d}/mcp{s}\n", .{ app.ansi.dim, port, app.ansi.reset });
    }

    std.debug.print("\n", .{});
    std.debug.print("  Use {s}journalctl --user -u poke-around -f{s} to see logs\n", .{ app.ansi.dim, app.ansi.reset });
    std.debug.print("\n", .{});
}

/// Restart the poke-around user service.
fn runRestart(allocator: std.mem.Allocator) !void {
    std.debug.print("Restarting poke-around.service...\n", .{});
    const result = std.process.Child.run(.{
        .allocator = allocator,
        .argv = &.{ "systemctl", "--user", "restart", "poke-around.service" },
    }) catch |err| {
        std.debug.print("Failed to restart service: {}\n", .{err});
        std.process.exit(1);
    };
    defer allocator.free(result.stdout);
    defer allocator.free(result.stderr);

    if (result.term.Exited != 0) {
        if (result.stderr.len > 0) {
            std.debug.print("{s}\n", .{result.stderr});
        }
        std.process.exit(1);
    }

    std.debug.print("Restart requested. Use 'poke-around status' to confirm it came back.\n", .{});
}

/// Send the startup-style notification again without restarting the daemon.
fn runNotify(allocator: std.mem.Allocator) !void {
    const state_json = config.readStateJson(allocator) catch {
        std.debug.print("No state file found. Start poke-around first.\n", .{});
        std.process.exit(1);
    };
    defer allocator.free(state_json);

    const parsed = std.json.parseFromSlice(std.json.Value, allocator, state_json, .{}) catch {
        std.debug.print("State file is corrupted.\n", .{});
        std.process.exit(1);
    };
    defer parsed.deinit();

    const obj = switch (parsed.value) {
        .object => |o| o,
        else => {
            std.debug.print("Invalid state format.\n", .{});
            std.process.exit(1);
        },
    };

    const conn_id = if (obj.get("connectionId")) |v| switch (v) {
        .string => |s| s,
        else => {
            std.debug.print("No connectionId found in state yet.\n", .{});
            std.process.exit(1);
        },
    } else {
        std.debug.print("No connectionId found in state yet.\n", .{});
        std.process.exit(1);
    };

    const bridge_path = try app.resolveBridgePath(allocator);
    defer allocator.free(bridge_path);

    const mode = config.readPermissionMode(allocator) catch null;
    defer if (mode) |owned_mode| allocator.free(owned_mode);

    const mode_msg = if (mode) |m| blk: {
        if (std.mem.eql(u8, m, "limited")) break :blk "Access mode: Limited. You can read files, list directories, and run safe read-only commands. File writes, screenshots, and other commands are disabled.";
        if (std.mem.eql(u8, m, "sandbox")) break :blk "Access mode: Sandbox. You can run commands like brew, node, python, ffmpeg, curl, and more. File writes are restricted to ~/Downloads and /tmp. Screenshots are disabled.";
        break :blk "Access mode: Full. You can run any shell command, read/write files, take screenshots, and check system info. Only destructive actions require a one-time approval per session.";
    } else "Access mode: Full. You can run any shell command, read/write files, take screenshots, and check system info. Only destructive actions require a one-time approval per session.";

    const msg = try std.fmt.allocPrint(
        allocator,
        "Hey! I've connected my computer to you via Poke Around (tunnel: {s}). {s} Just use the tools whenever I ask you to do something on my computer. Now reply me with \"now I am connected to your computer\" but everytime write those replies in most creative fun way.",
        .{ conn_id, mode_msg },
    );
    defer allocator.free(msg);

    var child = std.process.Child.init(
        &.{ pickRuntime(bridge_path), bridge_path, "send-message", "--message", msg },
        allocator,
    );
    child.stdout_behavior = .Ignore;
    child.stderr_behavior = .Inherit;
    child.stdin_behavior = .Ignore;
    try child.spawn();
    _ = try child.wait();

    std.debug.print("Notification sent.\n", .{});
}
