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
            .mask = std.posix.empty_sigset,
            .flags = 0,
        };
        std.posix.sigaction(std.posix.SIG.INT, &sa, null);
        std.posix.sigaction(std.posix.SIG.TERM, &sa, null);
    }
    try app.runDaemon(allocator, mode_str, verbose);
}

var shutdown_flag: std.atomic.Value(bool) = std.atomic.Value(bool).init(false);

fn handleSignal(_: c_int) callconv(.C) void {
    shutdown_flag.store(true, .release);
    std.debug.print("\nShutting down...\n", .{});
    std.process.exit(0);
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
        var line_buf: [1024]u8 = undefined;
        const stdin = std.io.getStdIn().reader();
        const line = try stdin.readUntilDelimiterOrEof(&line_buf, '\n') orelse return error.NoInput;
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
    const bun_paths = [_][]const u8{ "/usr/local/bin/bun", "/opt/homebrew/bin/bun" };
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
