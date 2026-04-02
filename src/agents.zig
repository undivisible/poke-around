/// Agent discovery, scheduling, and running.
/// Agents are JS files named <name>.<interval>.js in ~/.config/poke-around/agents/.
const std = @import("std");
const builtin = @import("builtin");
const config = @import("config.zig");

const MIN_INTERVAL_MS: u64 = 10 * 60 * 1000;

pub const Agent = struct {
    name: []u8,
    file: []u8,
    path: []u8,
    interval_token: []u8,
    interval_ms: u64,
    env_file: []u8,
    allocator: std.mem.Allocator,

    pub fn deinit(self: *Agent) void {
        self.allocator.free(self.name);
        self.allocator.free(self.file);
        self.allocator.free(self.path);
        self.allocator.free(self.interval_token);
        self.allocator.free(self.env_file);
    }
};

// ── Interval parsing ────────────────────────────────────────────────────────

/// Parse an interval token like "30m" or "2h" into milliseconds.
/// Returns null if invalid or below the minimum (10m).
pub fn parseInterval(token: []const u8) ?u64 {
    if (token.len < 2) return null;
    const unit = token[token.len - 1];
    if (unit != 'm' and unit != 'h') return null;
    const num = std.fmt.parseInt(u64, token[0 .. token.len - 1], 10) catch return null;
    const ms = if (unit == 'h') num * 60 * 60 * 1000 else num * 60 * 1000;
    if (ms < MIN_INTERVAL_MS) return null;
    return ms;
}

// ── Agent discovery ─────────────────────────────────────────────────────────

/// Discover all valid agents in the agents directory.
/// Caller must free each Agent and the returned slice.
pub fn discoverAgents(allocator: std.mem.Allocator) ![]Agent {
    const agents_dir_path = config.getAgentsDir(allocator) catch |err| {
        if (err == error.FileNotFound) return allocator.alloc(Agent, 0);
        return err;
    };
    defer allocator.free(agents_dir_path);

    // Ensure dir exists
    std.fs.makeDirAbsolute(agents_dir_path) catch |err| switch (err) {
        error.PathAlreadyExists => {},
        else => return allocator.alloc(Agent, 0),
    };

    var dir = std.fs.openDirAbsolute(agents_dir_path, .{ .iterate = true }) catch
        return allocator.alloc(Agent, 0);
    defer dir.close();

    var agents = std.ArrayList(Agent).empty;
    var it = dir.iterate();
    while (try it.next()) |entry| {
        if (entry.kind != .file) continue;
        if (!std.mem.endsWith(u8, entry.name, ".js")) continue;

        // Split on '.' to extract name and interval token
        const stem = entry.name[0 .. entry.name.len - 3]; // strip .js
        const last_dot = std.mem.lastIndexOfScalar(u8, stem, '.') orelse continue;
        const interval_token = stem[last_dot + 1 ..];
        const name = stem[0..last_dot];

        const interval_ms = parseInterval(interval_token) orelse {
            std.log.warn("[agents] Skipping {s}: invalid interval '{s}' (min 10m)", .{ entry.name, interval_token });
            continue;
        };

        const full_path = try std.fs.path.join(allocator, &.{ agents_dir_path, entry.name });
        const env_basename = try std.fmt.allocPrint(allocator, ".env.{s}", .{name});
        const env_file = try std.fs.path.join(allocator, &.{ agents_dir_path, env_basename });
        allocator.free(env_basename);
        // Note: env_file may not exist, that's fine

        try agents.append(allocator, .{
            .name = try allocator.dupe(u8, name),
            .file = try allocator.dupe(u8, entry.name),
            .path = full_path,
            .interval_token = try allocator.dupe(u8, interval_token),
            .interval_ms = interval_ms,
            .env_file = env_file,
            .allocator = allocator,
        });
    }
    return agents.toOwnedSlice(allocator);
}

// ── .env file parsing ────────────────────────────────────────────────────────

/// Parse a .env file into a map of key→value pairs.
/// Caller must call envMapDeinit on the returned map.
pub fn parseEnvFile(allocator: std.mem.Allocator, path: []const u8) !std.StringHashMap([]u8) {
    var map = std.StringHashMap([]u8).init(allocator);

    const file = std.fs.openFileAbsolute(path, .{}) catch |err| switch (err) {
        error.FileNotFound => return map,
        else => return err,
    };
    defer file.close();

    const content = try file.readToEndAlloc(allocator, 1024 * 1024);
    defer allocator.free(content);

    var iter = std.mem.splitScalar(u8, content, '\n');
    while (iter.next()) |raw_line| {
        const line = std.mem.trim(u8, raw_line, " \t\r");
        if (line.len == 0 or line[0] == '#') continue;
        const eq = std.mem.indexOfScalar(u8, line, '=') orelse continue;
        const key = std.mem.trim(u8, line[0..eq], " \t");
        var val = std.mem.trim(u8, line[eq + 1 ..], " \t");
        // Strip surrounding quotes
        if (val.len >= 2 and ((val[0] == '"' and val[val.len - 1] == '"') or
            (val[0] == '\'' and val[val.len - 1] == '\'')))
        {
            val = val[1 .. val.len - 1];
        }
        const k = try allocator.dupe(u8, key);
        const v = try allocator.dupe(u8, val);
        try map.put(k, v);
    }
    return map;
}

pub fn envMapDeinit(allocator: std.mem.Allocator, map: *std.StringHashMap([]u8)) void {
    var it = map.iterator();
    while (it.next()) |entry| {
        allocator.free(entry.key_ptr.*);
        allocator.free(entry.value_ptr.*);
    }
    map.deinit();
}

// ── Running agents ───────────────────────────────────────────────────────────

const AGENT_TIMEOUT_MS: u64 = 5 * 60 * 1000; // 5 minutes

const AgentKillCtx = struct {
    child: *std.process.Child,
    done: std.atomic.Value(bool),
};

fn agentKillThread(ctx: *AgentKillCtx) void {
    std.Thread.sleep(AGENT_TIMEOUT_MS * std.time.ns_per_ms);
    if (!ctx.done.load(.acquire)) {
        _ = ctx.child.kill() catch {};
    }
}

/// Run an agent process. Blocks until complete (or 5-minute timeout).
pub fn runAgentProcess(allocator: std.mem.Allocator, agent: *const Agent, log_enabled: bool) void {
    if (log_enabled) {
        std.log.info("[agents] Running agent: {s} ({s})", .{ agent.name, agent.file });
    }

    const home_dir = config.getHomeDir(allocator) catch "";
    defer if (home_dir.len > 0) allocator.free(home_dir);

    // Parse .env file
    var env_map = parseEnvFile(allocator, agent.env_file) catch std.StringHashMap([]u8).init(allocator);
    defer envMapDeinit(allocator, &env_map);

    // Build environment: inherit current env + agent-specific vars
    var env = std.process.EnvMap.init(allocator);
    defer env.deinit();

    // Copy current env
    var cur_env = std.process.getEnvMap(allocator) catch {
        runNodeScript(allocator, agent, &env, log_enabled, home_dir);
        return;
    };
    defer cur_env.deinit();
    var cur_it = cur_env.iterator();
    while (cur_it.next()) |entry| {
        env.put(entry.key_ptr.*, entry.value_ptr.*) catch {};
    }

    // Overlay agent env
    var env_it = env_map.iterator();
    while (env_it.next()) |entry| {
        env.put(entry.key_ptr.*, entry.value_ptr.*) catch {};
    }

    runNodeScript(allocator, agent, &env, log_enabled, home_dir);
}

fn runNodeScript(
    allocator: std.mem.Allocator,
    agent: *const Agent,
    env: *std.process.EnvMap,
    log_enabled: bool,
    home_dir: []const u8,
) void {
    var node_buf: [512]u8 = undefined;
    const node_exe = pickNodeExe(home_dir, &node_buf);
    const argv = [_][]const u8{ node_exe, agent.path };
    const agents_dir = std.fs.path.dirname(agent.path) orelse ".";

    var child = std.process.Child.init(&argv, allocator);
    child.cwd = agents_dir;
    child.env_map = env;
    child.stdout_behavior = .Pipe;
    child.stderr_behavior = .Pipe;
    child.stdin_behavior = .Ignore;

    child.spawn() catch |err| {
        if (log_enabled) std.log.err("[agents] Failed to spawn {s}: {}", .{ agent.name, err });
        return;
    };

    // 5-minute hard timeout watchdog
    var kill_ctx = AgentKillCtx{
        .child = &child,
        .done = std.atomic.Value(bool).init(false),
    };
    const watchdog = std.Thread.spawn(.{}, agentKillThread, .{&kill_ctx}) catch null;

    const stdout = child.stdout.?.readToEndAlloc(allocator, 1024 * 1024) catch "";
    defer if (stdout.len > 0) allocator.free(stdout);
    const stderr = child.stderr.?.readToEndAlloc(allocator, 10 * 1024) catch "";
    defer if (stderr.len > 0) allocator.free(stderr);

    kill_ctx.done.store(true, .release);
    if (watchdog) |t| t.detach();

    const result = child.wait() catch |err| {
        if (log_enabled) std.log.err("[agents] Wait failed for {s}: {}", .{ agent.name, err });
        return;
    };

    if (log_enabled) {
        if (stdout.len > 0) std.log.info("[agents] [{s}] {s}", .{ agent.name, std.mem.trim(u8, stdout, " \n\r\t") });
        if (stderr.len > 0) std.log.warn("[agents] [{s}] stderr: {s}", .{ agent.name, std.mem.trim(u8, stderr, " \n\r\t") });
        switch (result) {
            .Exited => |code| if (code != 0) std.log.warn("[agents] [{s}] exited with code {d}", .{ agent.name, code }),
            else => std.log.warn("[agents] [{s}] exited abnormally", .{agent.name}),
        }
    }
}

fn pickNodeExe(home_dir: []const u8, buf: []u8) []const u8 {
    // Try ~/.bun/bin/bun first — most common on Linux/WSL2 user installs.
    const home_bun = std.fmt.bufPrint(buf, "{s}/.bun/bin/bun", .{home_dir}) catch return "bun";
    std.fs.accessAbsolute(home_bun, .{}) catch {
        const static_paths = [_][]const u8{ "/usr/local/bin/bun", "/opt/homebrew/bin/bun" };
        for (static_paths) |p| {
            std.fs.accessAbsolute(p, .{}) catch continue;
            return p;
        }
        return if (builtin.os.tag == .windows) "node.exe" else "node";
    };
    return home_bun;
}

// ── Scheduler ────────────────────────────────────────────────────────────────

const SchedulerCtx = struct {
    agent: Agent,
    log_enabled: bool,
    stop: std.atomic.Value(bool),
};

var scheduler_running: std.atomic.Value(bool) = std.atomic.Value(bool).init(false);
var scheduler_threads: std.ArrayList(std.Thread) = std.ArrayList(std.Thread).empty;
var scheduler_ctxs: std.ArrayList(*SchedulerCtx) = std.ArrayList(*SchedulerCtx).empty;
var scheduler_allocator: std.mem.Allocator = undefined;

/// Start the agent scheduler. Safe to call only once at a time.
pub fn startScheduler(allocator: std.mem.Allocator, log_enabled: bool) !void {
    if (scheduler_running.load(.acquire)) return;
    scheduler_running.store(true, .release);

    scheduler_allocator = allocator;
    scheduler_threads = std.ArrayList(std.Thread).empty;
    scheduler_ctxs = std.ArrayList(*SchedulerCtx).empty;

    const agents = try discoverAgents(allocator);
    defer {
        for (agents) |*a| @constCast(a).deinit();
        allocator.free(agents);
    }

    if (agents.len == 0) {
        std.log.info("[agents] No agents found. Add scripts to ~/.config/poke-around/agents/", .{});
        return;
    }

    std.log.info("[agents] Found {d} agent(s):", .{agents.len});
    for (agents) |*a| {
        std.log.info("[agents]   {s} (every {s})", .{ a.name, a.interval_token });
    }

    for (agents) |ag| {
        const ctx = try allocator.create(SchedulerCtx);
        ctx.* = .{
            .agent = .{
                .name = try allocator.dupe(u8, ag.name),
                .file = try allocator.dupe(u8, ag.file),
                .path = try allocator.dupe(u8, ag.path),
                .interval_token = try allocator.dupe(u8, ag.interval_token),
                .interval_ms = ag.interval_ms,
                .env_file = try allocator.dupe(u8, ag.env_file),
                .allocator = allocator,
            },
            .log_enabled = log_enabled,
            .stop = std.atomic.Value(bool).init(false),
        };
        try scheduler_ctxs.append(allocator, ctx);
        const t = try std.Thread.spawn(.{}, schedulerLoop, .{ctx});
        try scheduler_threads.append(allocator, t);
    }
}

fn schedulerLoop(ctx: *SchedulerCtx) void {
    // Run immediately on start
    if (!ctx.stop.load(.acquire)) {
        runAgentProcess(ctx.agent.allocator, &ctx.agent, ctx.log_enabled);
    }
    while (!ctx.stop.load(.acquire)) {
        // Sleep in 1s increments so we can check stop flag
        var slept: u64 = 0;
        while (slept < ctx.agent.interval_ms and !ctx.stop.load(.acquire)) {
            std.Thread.sleep(1 * std.time.ns_per_s);
            slept += 1000;
        }
        if (!ctx.stop.load(.acquire)) {
            runAgentProcess(ctx.agent.allocator, &ctx.agent, ctx.log_enabled);
        }
    }
}

/// Stop all scheduled agents.
pub fn stopScheduler() void {
    if (!scheduler_running.load(.acquire)) return;
    scheduler_running.store(false, .release);

    for (scheduler_ctxs.items) |ctx| ctx.stop.store(true, .release);
    // Threads will exit within ~1s naturally; we don't join to avoid blocking.
    std.log.info("[agents] Agent scheduler stopped.", .{});
}

// ── One-shot agent run ────────────────────────────────────────────────────────

/// Run a specific agent by name immediately (blocking). Used by CLI `run-agent`.
pub fn runAgentByName(allocator: std.mem.Allocator, name: []const u8) !void {
    const agents = try discoverAgents(allocator);
    defer {
        for (agents) |*a| @constCast(a).deinit();
        allocator.free(agents);
    }

    for (agents) |*agent| {
        if (std.mem.eql(u8, agent.name, name)) {
            runAgentProcess(allocator, agent, true);
            return;
        }
    }

    // Fallback: look for any file starting with name.
    const agents_dir = try config.getAgentsDir(allocator);
    defer allocator.free(agents_dir);

    var dir = try std.fs.openDirAbsolute(agents_dir, .{ .iterate = true });
    defer dir.close();

    var it = dir.iterate();
    while (try it.next()) |entry| {
        if (!std.mem.endsWith(u8, entry.name, ".js")) continue;
        const prefix = try std.fmt.allocPrint(allocator, "{s}.", .{name});
        defer allocator.free(prefix);
        if (!std.mem.startsWith(u8, entry.name, prefix)) continue;

        const stem = entry.name[0 .. entry.name.len - 3];
        const last_dot = std.mem.lastIndexOfScalar(u8, stem, '.') orelse continue;
        const interval_token = stem[last_dot + 1 ..];
        const interval_ms = parseInterval(interval_token) orelse 0;
        const full_path = try std.fs.path.join(allocator, &.{ agents_dir, entry.name });
        const env_basename = try std.fmt.allocPrint(allocator, ".env.{s}", .{name});
        const env_file = try std.fs.path.join(allocator, &.{ agents_dir, env_basename });
        allocator.free(env_basename);

        var ag = Agent{
            .name = try allocator.dupe(u8, name),
            .file = try allocator.dupe(u8, entry.name),
            .path = full_path,
            .interval_token = try allocator.dupe(u8, interval_token),
            .interval_ms = interval_ms,
            .env_file = env_file,
            .allocator = allocator,
        };
        defer ag.deinit();
        runAgentProcess(allocator, &ag, true);
        return;
    }

    std.log.err("[agents] Agent '{s}' not found in {s}", .{ name, agents_dir });
    return error.AgentNotFound;
}

// ── Download agent from GitHub ────────────────────────────────────────────────

const REPO_BASE = "https://raw.githubusercontent.com/undivisible/poke-around/main/examples/agents";

pub fn downloadAgent(allocator: std.mem.Allocator, name: []const u8) !void {
    const agents_dir = try config.ensureAgentsDir(allocator);
    defer allocator.free(agents_dir);

    std.debug.print("Fetching agent \"{s}\" from GitHub...\n", .{name});

    // Try common intervals
    const intervals = [_][]const u8{ "10m", "30m", "1h", "2h", "6h", "12h", "24h" };
    const direct_names = [_][]const u8{ name, try std.fmt.allocPrint(allocator, "{s}.js", .{name}) };

    var js_file_name: ?[]u8 = null;
    var js_content: ?[]u8 = null;

    // Try direct filename
    for (direct_names) |try_name| {
        const url = try std.fmt.allocPrint(allocator, "{s}/{s}", .{ REPO_BASE, try_name });
        defer allocator.free(url);
        if (try fetchUrl(allocator, url)) |body| {
            js_file_name = try allocator.dupe(u8, try_name);
            js_content = body;
            break;
        }
    }

    // Try with intervals
    if (js_content == null) {
        for (intervals) |iv| {
            const fname = try std.fmt.allocPrint(allocator, "{s}.{s}.js", .{ name, iv });
            const url = try std.fmt.allocPrint(allocator, "{s}/{s}", .{ REPO_BASE, fname });
            defer allocator.free(url);
            if (try fetchUrl(allocator, url)) |body| {
                js_file_name = fname;
                js_content = body;
                break;
            }
            allocator.free(fname);
        }
    }

    const content = js_content orelse {
        std.debug.print("Agent \"{s}\" not found.\nBrowse: https://github.com/undivisible/poke-around/tree/main/examples/agents\n", .{name});
        return error.AgentNotFound;
    };
    defer allocator.free(content);

    const fname = js_file_name orelse unreachable;
    defer allocator.free(fname);

    const dest = try std.fs.path.join(allocator, &.{ agents_dir, fname });
    defer allocator.free(dest);
    {
        const f = try std.fs.createFileAbsolute(dest, .{});
        defer f.close();
        try f.writeAll(content);
    }
    std.debug.print("  Saved: {s}\n", .{dest});

    // Try to fetch .env template
    const base_name = name_without_interval(name);
    const env_dest = try std.fs.path.join(allocator, &.{
        agents_dir,
        try std.fmt.allocPrint(allocator, ".env.{s}", .{base_name}),
    });
    defer allocator.free(env_dest);

    // Skip if .env already exists
    std.fs.accessAbsolute(env_dest, .{}) catch {
        const env_url = try std.fmt.allocPrint(allocator, "{s}/.env.{s}", .{ REPO_BASE, base_name });
        defer allocator.free(env_url);
        if (try fetchUrl(allocator, env_url)) |env_content| {
            defer allocator.free(env_content);
            const ef = try std.fs.createFileAbsolute(env_dest, .{});
            defer ef.close();
            try ef.writeAll(env_content);
            std.debug.print("  Saved: {s}\n", .{env_dest});
        }
    };

    std.debug.print("\n  Test it: poke-around run-agent {s}\n", .{base_name});
}

fn name_without_interval(name: []const u8) []const u8 {
    // If name contains '.', take the first part
    return if (std.mem.indexOfScalar(u8, name, '.')) |dot| name[0..dot] else name;
}

/// Fetch a URL and return its body (caller must free), or null on error/non-200.
fn fetchUrl(allocator: std.mem.Allocator, url: []const u8) !?[]u8 {
    var child = std.process.Child.init(
        &.{ "curl", "-fsSL", "--max-time", "10", url },
        allocator,
    );
    child.stdout_behavior = .Pipe;
    child.stderr_behavior = .Ignore;
    child.stdin_behavior = .Ignore;
    child.spawn() catch return null;
    const body = child.stdout.?.readToEndAlloc(allocator, 1024 * 1024) catch {
        _ = child.wait() catch {};
        return null;
    };
    const result = child.wait() catch {
        allocator.free(body);
        return null;
    };
    switch (result) {
        .Exited => |code| if (code != 0) {
            allocator.free(body);
            return null;
        },
        else => {
            allocator.free(body);
            return null;
        },
    }
    if (body.len == 0) {
        allocator.free(body);
        return null;
    }
    return body;
}
