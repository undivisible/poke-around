/// MCP HTTP server — JSON-RPC 2.0 over HTTP, with all 9 tools.
const std = @import("std");
const builtin = @import("builtin");

const config = @import("config.zig");
const platform = @import("platform.zig");
const permission = @import("permission.zig");
const screenshot = @import("screenshot.zig");
const agents = @import("agents.zig");

// ── Permission mode ─────────────────────────────────────────────────────────

pub const PermissionMode = enum { full, limited, sandbox };

pub fn parsePermissionMode(s: []const u8) PermissionMode {
    if (std.mem.eql(u8, s, "limited")) return .limited;
    if (std.mem.eql(u8, s, "sandbox")) return .sandbox;
    return .full;
}

// ── Loop guard (run_command deduplication) ──────────────────────────────────

const LOOP_SUPPRESS_MS: i64 = 60_000;

pub const LoopGuard = struct {
    in_flight: std.StringHashMap(void),
    recent_failures: std.StringHashMap(i64), // fingerprint → suppress_until_ms
    mutex: std.Thread.Mutex,
    allocator: std.mem.Allocator,

    pub fn init(allocator: std.mem.Allocator) LoopGuard {
        return .{
            .in_flight = std.StringHashMap(void).init(allocator),
            .recent_failures = std.StringHashMap(i64).init(allocator),
            .mutex = .{},
            .allocator = allocator,
        };
    }

    pub fn deinit(self: *LoopGuard) void {
        var it = self.in_flight.keyIterator();
        while (it.next()) |k| self.allocator.free(k.*);
        self.in_flight.deinit();
        var it2 = self.recent_failures.keyIterator();
        while (it2.next()) |k| self.allocator.free(k.*);
        self.recent_failures.deinit();
    }

    /// Try to acquire in-flight slot. Returns false if suppressed.
    pub fn tryAcquire(self: *LoopGuard, fingerprint: []const u8) bool {
        self.mutex.lock();
        defer self.mutex.unlock();
        if (self.in_flight.contains(fingerprint)) return false;
        const now = std.time.milliTimestamp();
        if (self.recent_failures.get(fingerprint)) |until| {
            if (now < until) return false;
        }
        const key = self.allocator.dupe(u8, fingerprint) catch return false;
        self.in_flight.put(key, {}) catch {
            self.allocator.free(key);
            return false;
        };
        return true;
    }

    /// Release in-flight slot and record success or failure.
    pub fn release(self: *LoopGuard, fingerprint: []const u8, success: bool) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        if (self.in_flight.fetchRemove(fingerprint)) |entry| {
            self.allocator.free(entry.key);
        }
        if (!success) {
            const until = std.time.milliTimestamp() + LOOP_SUPPRESS_MS;
            if (self.recent_failures.getPtr(fingerprint)) |ptr| {
                ptr.* = until;
            } else {
                const key = self.allocator.dupe(u8, fingerprint) catch return;
                self.recent_failures.put(key, until) catch self.allocator.free(key);
            }
        } else {
            if (self.recent_failures.fetchRemove(fingerprint)) |entry| {
                self.allocator.free(entry.key);
            }
        }
    }
};

// ── App state (shared across connection threads) ────────────────────────────

pub const AppState = struct {
    allocator: std.mem.Allocator,
    permission_mode: PermissionMode,
    perm_svc: permission.PermissionService,
    loop_guard: LoopGuard,
    auto_approve: std.StringHashMap(void), // session_id → auto-approve all risky
    auto_approve_mutex: std.Thread.Mutex,
    log_enabled: bool,
    bridge_writer: ?std.fs.File,
    bridge_writer_mutex: std.Thread.Mutex,
    home_dir: []u8,

    pub fn init(allocator: std.mem.Allocator, mode: PermissionMode, log_enabled: bool) !AppState {
        var rand_secret: [32]u8 = undefined;
        std.crypto.random.bytes(&rand_secret);
        const secret_hex = std.fmt.bytesToHex(rand_secret, .lower);

        // Override from env if set
        const final_secret = if (std.process.getEnvVarOwned(allocator, "POKE_GATE_HMAC_SECRET")) |s| blk: {
            break :blk s;
        } else |_| blk: {
            break :blk try allocator.dupe(u8, &secret_hex);
        };
        defer allocator.free(final_secret);

        const home = try config.getHomeDir(allocator);
        errdefer allocator.free(home);

        return .{
            .allocator = allocator,
            .permission_mode = mode,
            .perm_svc = try permission.PermissionService.init(allocator, final_secret),
            .loop_guard = LoopGuard.init(allocator),
            .auto_approve = std.StringHashMap(void).init(allocator),
            .auto_approve_mutex = .{},
            .log_enabled = log_enabled,
            .bridge_writer = null,
            .bridge_writer_mutex = .{},
            .home_dir = home,
        };
    }

    pub fn deinit(self: *AppState) void {
        self.perm_svc.deinit();
        self.loop_guard.deinit();
        var it = self.auto_approve.keyIterator();
        while (it.next()) |k| self.allocator.free(k.*);
        self.auto_approve.deinit();
        self.allocator.free(self.home_dir);
    }

    pub fn sendToBridge(self: *AppState, json: []const u8) void {
        self.bridge_writer_mutex.lock();
        defer self.bridge_writer_mutex.unlock();
        const w = self.bridge_writer orelse return;
        w.writeAll(json) catch {};
        w.writeAll("\n") catch {};
    }

    pub fn isAutoApprove(self: *AppState, session_id: []const u8) bool {
        self.auto_approve_mutex.lock();
        defer self.auto_approve_mutex.unlock();
        return self.auto_approve.contains(session_id);
    }

    pub fn setAutoApprove(self: *AppState, session_id: []const u8) void {
        self.auto_approve_mutex.lock();
        defer self.auto_approve_mutex.unlock();
        const key = self.allocator.dupe(u8, session_id) catch return;
        self.auto_approve.put(key, {}) catch self.allocator.free(key);
    }
};

// ── HTTP server ─────────────────────────────────────────────────────────────

const SrvCtx = struct {
    server: std.net.Server,
    state: *AppState,
};

pub fn startMcpServer(allocator: std.mem.Allocator, state: *AppState) !u16 {
    const address = try std.net.Address.parseIp("127.0.0.1", 0);
    const ctx = try allocator.create(SrvCtx);
    errdefer allocator.destroy(ctx);
    ctx.server = try address.listen(.{ .reuse_address = true });
    errdefer ctx.server.deinit();
    ctx.state = state;
    const port = ctx.server.listen_address.getPort();
    const t = try std.Thread.spawn(.{}, serverLoop, .{ctx});
    t.detach();
    return port;
}

fn serverLoop(ctx: *SrvCtx) void {
    defer ctx.server.deinit();
    defer ctx.state.allocator.destroy(ctx);
    while (true) {
        const conn = ctx.server.accept() catch |err| {
            std.log.err("[mcp] accept error: {}", .{err});
            continue;
        };
        const t = std.Thread.spawn(.{}, handleConnection, .{ conn, ctx.state }) catch {
            conn.stream.close();
            continue;
        };
        t.detach();
    }
}

// ── HTTP connection handler ─────────────────────────────────────────────────

const HttpRequest = struct {
    method: []const u8,
    path: []const u8,
    session_id: []const u8,
    body: []const u8,
};

fn handleConnection(conn: std.net.Server.Connection, state: *AppState) void {
    defer conn.stream.close();

    var arena = std.heap.ArenaAllocator.init(state.allocator);
    defer arena.deinit();
    const allocator = arena.allocator();

    const req = parseHttpRequest(allocator, conn.stream) catch |err| {
        const err_body = std.fmt.allocPrint(allocator, "{{\"error\":\"{}\"}}", .{err}) catch return;
        writeHttpResponse(conn.stream, 400, err_body) catch {};
        return;
    };
    const response = buildHttpResponse(allocator, req, state) catch |err| {
        const err_body = std.fmt.allocPrint(allocator, "{{\"error\":\"{}\"}}", .{err}) catch return;
        writeHttpResponse(conn.stream, 500, err_body) catch {};
        return;
    };
    writeHttpResponse(conn.stream, response.status, response.body) catch {};
}

fn parseHttpRequest(allocator: std.mem.Allocator, stream: std.net.Stream) !HttpRequest {
    var recv_buf: [8192]u8 = undefined;
    var reader = stream.reader(&recv_buf);
    const br = reader.interface();

    // Request line
    const req_line = (try br.takeDelimiter('\n')) orelse return error.EmptyRequest;
    const req_trimmed = std.mem.trimRight(u8, req_line, "\r");
    var parts = std.mem.splitScalar(u8, req_trimmed, ' ');
    const method = try allocator.dupe(u8, parts.next() orelse "");
    _ = parts.next(); // skip path for now
    const path_raw = parts.next() orelse "";
    _ = path_raw;

    // Re-parse to get actual path
    var parts2 = std.mem.splitScalar(u8, req_trimmed, ' ');
    _ = parts2.next(); // method
    const path = try allocator.dupe(u8, parts2.next() orelse "/");

    // Headers — 8 KiB covers Authorization: Bearer <jwt> and any realistic header
    var content_length: usize = 0;
    var session_id: []const u8 = "default";
    while (true) {
        const hline = (try br.takeDelimiter('\n')) orelse break;
        const ht = std.mem.trimRight(u8, hline, "\r");
        if (ht.len == 0) break;
        const colon = std.mem.indexOfScalar(u8, ht, ':') orelse continue;
        const hname = std.mem.trim(u8, ht[0..colon], " ");
        const hval = std.mem.trim(u8, ht[colon + 1 ..], " ");
        if (std.ascii.eqlIgnoreCase(hname, "content-length")) {
            content_length = std.fmt.parseInt(usize, hval, 10) catch 0;
        } else if (std.ascii.eqlIgnoreCase(hname, "mcp-session-id")) {
            const s = std.mem.trim(u8, hval, &std.ascii.whitespace);
            if (s.len > 0) session_id = try allocator.dupe(u8, s);
        }
    }

    // Body
    const body = try allocator.alloc(u8, content_length);
    if (content_length > 0) {
        try br.readSliceAll(body);
    }

    return .{
        .method = method,
        .path = path,
        .session_id = session_id,
        .body = body,
    };
}

const HttpResponse = struct { status: u16, body: []const u8 };

fn buildHttpResponse(allocator: std.mem.Allocator, req: HttpRequest, state: *AppState) !HttpResponse {
    // CORS preflight
    if (std.mem.eql(u8, req.method, "OPTIONS")) {
        return .{ .status = 204, .body = "" };
    }
    // Health check
    if (std.mem.eql(u8, req.path, "/health")) {
        return .{ .status = 200, .body = "{\"status\":\"ok\"}" };
    }
    // MCP endpoint
    if (std.mem.eql(u8, req.path, "/mcp") and std.mem.eql(u8, req.method, "POST")) {
        if (req.body.len == 0) {
            return .{ .status = 400, .body = "{\"error\":\"empty body\"}" };
        }
        const resp = handleJsonRpcBody(allocator, req.body, req.session_id, state) catch |err| {
            const msg = try std.fmt.allocPrint(allocator, "{{\"jsonrpc\":\"2.0\",\"error\":{{\"code\":-32700,\"message\":\"{}\"}},\"id\":null}}", .{err});
            return .{ .status = 400, .body = msg };
        };
        return .{ .status = 200, .body = resp };
    }
    return .{ .status = 404, .body = "Not found" };
}

fn writeHttpResponse(stream: std.net.Stream, status: u16, body: []const u8) !void {
    const status_text = switch (status) {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        else => "OK",
    };
    var write_buf: [4096]u8 = undefined;
    var writer = stream.writer(&write_buf);
    try writer.interface.print(
        "HTTP/1.1 {d} {s}\r\n" ++
            "Content-Type: application/json\r\n" ++
            "Connection: close\r\n" ++
            "Access-Control-Allow-Origin: *\r\n" ++
            "Access-Control-Allow-Methods: GET, POST, DELETE, OPTIONS\r\n" ++
            "Access-Control-Allow-Headers: Content-Type, Authorization, Mcp-Session-Id, Accept\r\n" ++
            "Access-Control-Expose-Headers: Mcp-Session-Id\r\n" ++
            "Content-Length: {d}\r\n\r\n",
        .{ status, status_text, body.len },
    );
    if (body.len > 0) try writer.interface.writeAll(body);
}

// ── JSON-RPC dispatch ───────────────────────────────────────────────────────

fn handleJsonRpcBody(
    allocator: std.mem.Allocator,
    body: []const u8,
    session_id: []const u8,
    state: *AppState,
) ![]const u8 {
    const parsed = try std.json.parseFromSlice(std.json.Value, allocator, body, .{});
    defer parsed.deinit();

    switch (parsed.value) {
        .array => |arr| {
            var results = std.ArrayList(u8).empty;
            try results.appendSlice(allocator, "[");
            var first = true;
            for (arr.items) |msg| {
                const r = handleSingleRpc(allocator, msg, session_id, state) catch continue;
                if (r == null) continue;
                if (!first) try results.appendSlice(allocator, ",");
                try results.appendSlice(allocator, r.?);
                first = false;
            }
            try results.appendSlice(allocator, "]");
            return results.toOwnedSlice(allocator);
        },
        else => {
            const r = try handleSingleRpc(allocator, parsed.value, session_id, state);
            return r orelse try allocator.dupe(u8, "");
        },
    }
}

fn handleSingleRpc(
    allocator: std.mem.Allocator,
    msg: std.json.Value,
    session_id: []const u8,
    state: *AppState,
) !?[]const u8 {
    const obj = switch (msg) {
        .object => |o| o,
        else => return null,
    };

    const id_val = obj.get("id");
    const method_val = obj.get("method") orelse return null;
    const method = switch (method_val) {
        .string => |s| s,
        else => return null,
    };

    // Encode id
    const id_json = try encodeId(allocator, id_val);

    if (std.mem.eql(u8, method, "initialize")) {
        const proto = blk: {
            const params = obj.get("params") orelse break :blk "2024-11-05";
            const p_obj = switch (params) {
                .object => |o| o,
                else => break :blk "2024-11-05",
            };
            const pv = p_obj.get("protocolVersion") orelse break :blk "2024-11-05";
            break :blk switch (pv) {
                .string => |s| s,
                else => "2024-11-05",
            };
        };
        _ = proto;
        const result =
            \\{"protocolVersion":"2024-11-05","capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":"poke-around","version":"0.3.2"},"instructions":"This server gives you access to the user's machine. Use tools to help the user with OS-level tasks."}
        ;
        return try std.fmt.allocPrint(allocator, "{{\"jsonrpc\":\"2.0\",\"id\":{s},\"result\":{s}}}", .{ id_json, result });
    }

    if (std.mem.eql(u8, method, "notifications/initialized")) return null;

    if (std.mem.eql(u8, method, "ping")) {
        return try std.fmt.allocPrint(allocator, "{{\"jsonrpc\":\"2.0\",\"id\":{s},\"result\":{{}}}}", .{id_json});
    }

    if (std.mem.eql(u8, method, "tools/list")) {
        return try std.fmt.allocPrint(allocator, "{{\"jsonrpc\":\"2.0\",\"id\":{s},\"result\":{{\"tools\":{s}}}}}", .{ id_json, TOOLS_JSON });
    }

    if (std.mem.eql(u8, method, "tools/call")) {
        const params = switch (obj.get("params") orelse return null) {
            .object => |o| o,
            else => return null,
        };
        const tool_name = switch (params.get("name") orelse return null) {
            .string => |s| s,
            else => return null,
        };
        const tool_args = params.get("arguments") orelse std.json.Value{ .object = std.json.ObjectMap.init(allocator) };

        const result_json = try handleToolCall(allocator, tool_name, tool_args, session_id, state);
        return try std.fmt.allocPrint(allocator, "{{\"jsonrpc\":\"2.0\",\"id\":{s},\"result\":{s}}}", .{ id_json, result_json });
    }

    if (id_val == null) return null;
    return try std.fmt.allocPrint(
        allocator,
        "{{\"jsonrpc\":\"2.0\",\"id\":{s},\"error\":{{\"code\":-32601,\"message\":\"Method not found: {s}\"}}}}",
        .{ id_json, method },
    );
}

fn encodeId(allocator: std.mem.Allocator, id: ?std.json.Value) ![]const u8 {
    const v = id orelse return allocator.dupe(u8, "null");
    return switch (v) {
        .integer => |n| std.fmt.allocPrint(allocator, "{d}", .{n}),
        .string => |s| blk: {
            break :blk try std.json.Stringify.valueAlloc(allocator, s, .{});
        },
        .null => allocator.dupe(u8, "null"),
        else => allocator.dupe(u8, "null"),
    };
}

// ── Tool schemas ─────────────────────────────────────────────────────────────

const TOOLS_JSON =
    \\[
    \\  {"name":"run_command","description":"Execute a shell command on the user's machine and return stdout, stderr, and exit code.","inputSchema":{"type":"object","properties":{"command":{"type":"string"},"cwd":{"type":"string"},"approval_token":{"type":"string"},"approve":{"type":"boolean"},"remember_in_session":{"type":"boolean"},"remember_all_risky":{"type":"boolean"}},"required":["command"]}},
    \\  {"name":"network_speed","description":"Run a built-in internet speed test and return download/upload Mbps.","inputSchema":{"type":"object","properties":{"tests":{"type":"string","enum":["download","upload","both"]}}}},
    \\  {"name":"read_file","description":"Read the contents of a file on the user's machine.","inputSchema":{"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}},
    \\  {"name":"write_file","description":"Write content to a file on the user's machine.","inputSchema":{"type":"object","properties":{"path":{"type":"string"},"content":{"type":"string"},"approval_token":{"type":"string"},"approve":{"type":"boolean"},"remember_all_risky":{"type":"boolean"}},"required":["path","content"]}},
    \\  {"name":"list_directory","description":"List files and directories at a given path.","inputSchema":{"type":"object","properties":{"path":{"type":"string"}}}},
    \\  {"name":"system_info","description":"Get system information: OS, hostname, architecture, uptime, memory.","inputSchema":{"type":"object","properties":{}}},
    \\  {"name":"read_image","description":"Read an image or binary file and return it as base64-encoded data.","inputSchema":{"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}},
    \\  {"name":"run_agent","description":"Run a Poke Around agent by name.","inputSchema":{"type":"object","properties":{"name":{"type":"string"}},"required":["name"]}},
    \\  {"name":"take_screenshot","description":"Take a screenshot of the user's screen.","inputSchema":{"type":"object","properties":{"approval_token":{"type":"string"},"approve":{"type":"boolean"},"remember_all_risky":{"type":"boolean"}}}},
    \\  {"name":"edit_file","description":"Surgically replace an exact string in a file. Fails if old_string is not found or is ambiguous (appears more than once).","inputSchema":{"type":"object","properties":{"path":{"type":"string"},"old_string":{"type":"string"},"new_string":{"type":"string"},"approval_token":{"type":"string"},"approve":{"type":"boolean"},"remember_all_risky":{"type":"boolean"}},"required":["path","old_string","new_string"]}},
    \\  {"name":"web_fetch","description":"Fetch the text content of a URL and return it (uses curl). Optionally truncate to max_chars.","inputSchema":{"type":"object","properties":{"url":{"type":"string"},"max_chars":{"type":"integer"}},"required":["url"]}},
    \\  {"name":"http_request","description":"Make an HTTP request with custom method, headers and body. Returns status code and response body.","inputSchema":{"type":"object","properties":{"method":{"type":"string","enum":["GET","POST","PUT","DELETE","PATCH","HEAD","OPTIONS"]},"url":{"type":"string"},"headers":{"type":"object"},"body":{"type":"string"}},"required":["method","url"]}},
    \\  {"name":"git_operations","description":"Run a git operation in the current directory or cwd. Read operations (status, diff, log, show) are always allowed; write operations (commit, add, checkout, stash, reset) require approval in full mode.","inputSchema":{"type":"object","properties":{"operation":{"type":"string","enum":["status","diff","log","show","commit","add","checkout","branch","stash","reset","rev-parse"]},"args":{"type":"array","items":{"type":"string"}},"cwd":{"type":"string"},"approval_token":{"type":"string"},"approve":{"type":"boolean"},"remember_all_risky":{"type":"boolean"}},"required":["operation"]}}
    \\]
;

// ── Access policy ────────────────────────────────────────────────────────────

const LIMITED_COMMANDS = [_][]const u8{
    "curl", "yt-dlp", "youtube-dl", "ls", "pwd", "cat", "grep", "find",
    "head", "tail", "wc",  "sed",    "awk", "which", "command", "echo",
    "stat", "du",   "df",  "ps",     "uname", "sw_vers", "whoami", "jq",
    "diff",
};

const SANDBOX_COMMANDS = [_][]const u8{
    "yt-dlp", "youtube-dl", "ffmpeg", "ffprobe", "brew", "node", "python",
    "python3", "curl",      "dd",     "rm",       "mktemp", "mkdir",  "cp",
    "mv",      "touch",     "jq",     "diff",     "ls",   "pwd",  "cat",
    "grep",    "find",      "head",   "tail",     "wc",   "sed",  "awk",
    "which",   "command",   "echo",   "stat",     "du",   "df",   "ps",
    "uname",   "sw_vers",   "whoami",
};

const SAFE_TOOLS = [_][]const u8{ "read_file", "read_image", "list_directory", "system_info", "network_speed", "web_fetch", "http_request" };
const RISKY_TOOLS = [_][]const u8{ "run_command", "write_file", "take_screenshot", "edit_file" };
const GIT_READ_OPS = [_][]const u8{ "status", "diff", "log", "show", "branch", "rev-parse" };

fn isSafeTool(name: []const u8) bool {
    for (SAFE_TOOLS) |t| if (std.mem.eql(u8, name, t)) return true;
    return false;
}

fn isRiskyTool(name: []const u8) bool {
    for (RISKY_TOOLS) |t| if (std.mem.eql(u8, name, t)) return true;
    return false;
}

/// Returns an error message string if the tool/command is blocked by policy, else null.
fn evaluatePolicy(
    allocator: std.mem.Allocator,
    tool_name: []const u8,
    args: std.json.Value,
    mode: PermissionMode,
) !?[]const u8 {
    if (mode == .full) return null;
    if (isSafeTool(tool_name)) return null;

    if (std.mem.eql(u8, tool_name, "run_command")) {
        const cmd = getStringArg(args, "command") orelse return try allocator.dupe(u8, "Command is empty.");
        if (platform.hasDangerousPattern(cmd)) {
            return try allocator.dupe(u8, "Command matches a dangerous pattern.");
        }
        const allowlist = if (mode == .limited) LIMITED_COMMANDS[0..] else SANDBOX_COMMANDS[0..];
        const segs = try platform.splitCommandSegments(allocator, cmd);
        defer allocator.free(segs);
        for (segs) |seg| {
            const exe = try platform.extractExecutable(allocator, seg);
            defer allocator.free(exe);
            var allowed = false;
            for (allowlist) |a| if (std.mem.eql(u8, exe, a)) { allowed = true; break; };
            if (!allowed) {
                return try std.fmt.allocPrint(allocator, "Command '{s}' is not permitted in this mode.", .{exe});
            }
        }
        return null;
    }

    if (std.mem.eql(u8, tool_name, "write_file") or
        std.mem.eql(u8, tool_name, "take_screenshot") or
        std.mem.eql(u8, tool_name, "edit_file"))
    {
        return try std.fmt.allocPrint(allocator, "Tool '{s}' is disabled in {s} mode.", .{ tool_name, @tagName(mode) });
    }

    if (std.mem.eql(u8, tool_name, "git_operations")) {
        const op = getStringArg(args, "operation") orelse return try allocator.dupe(u8, "Missing 'operation'.");
        for (GIT_READ_OPS) |ro| if (std.mem.eql(u8, op, ro)) return null;
        return try std.fmt.allocPrint(allocator, "git operation '{s}' is not permitted in {s} mode.", .{ op, @tagName(mode) });
    }

    return try std.fmt.allocPrint(allocator, "Tool '{s}' is not permitted in {s} mode.", .{ tool_name, @tagName(mode) });
}

// ── Tool dispatch ────────────────────────────────────────────────────────────

fn handleToolCall(
    allocator: std.mem.Allocator,
    tool_name: []const u8,
    args: std.json.Value,
    session_id: []const u8,
    state: *AppState,
) ![]const u8 {
    if (state.log_enabled) {
        const ts = timestamp();
        std.debug.print("[{s}] tool: {s}\n", .{ ts, tool_name });
    }

    const status_cmd = std.fmt.allocPrint(allocator, "{{\"type\":\"status_update\",\"status\":\"Running: {s}\"}}", .{tool_name}) catch null;
    if (status_cmd) |cmd| {
        state.sendToBridge(cmd);
        allocator.free(cmd);
    }

    const result = handleToolCallInner(allocator, tool_name, args, session_id, state);

    const idle_cmd = std.fmt.allocPrint(allocator, "{{\"type\":\"status_update\",\"status\":\"Idle\"}}", .{}) catch null;
    if (idle_cmd) |cmd| {
        state.sendToBridge(cmd);
        allocator.free(cmd);
    }

    return result;
}

fn handleToolCallInner(
    allocator: std.mem.Allocator,
    tool_name: []const u8,
    args: std.json.Value,
    session_id: []const u8,
    state: *AppState,
) ![]const u8 {
    // Access policy check
    if (try evaluatePolicy(allocator, tool_name, args, state.permission_mode)) |reason| {
        defer allocator.free(reason);
        return makeErrorResponse(allocator, try std.fmt.allocPrint(allocator, "Blocked by access mode policy: {s}", .{reason}));
    }

    // Approval check for risky tools in full mode
    const needs_approval = switch (state.permission_mode) {
        .full => blk: {
            if (std.mem.eql(u8, tool_name, "write_file")) break :blk true;
            if (std.mem.eql(u8, tool_name, "edit_file")) break :blk true;
            if (std.mem.eql(u8, tool_name, "run_command")) {
                const cmd = getStringArg(args, "command") orelse break :blk false;
                break :blk platform.isDestructiveCommand(cmd);
            }
            if (std.mem.eql(u8, tool_name, "take_screenshot")) break :blk true;
            if (std.mem.eql(u8, tool_name, "git_operations")) {
                const op = getStringArg(args, "operation") orelse break :blk false;
                for (GIT_READ_OPS) |ro| if (std.mem.eql(u8, op, ro)) break :blk false;
                break :blk true; // write ops need approval
            }
            break :blk false;
        },
        else => isRiskyTool(tool_name),
    };

    if (needs_approval) {
        const auto = state.isAutoApprove(session_id);
        const cmd_text = getStringArg(args, "command") orelse "";
        const pattern_ok = if (cmd_text.len > 0) state.perm_svc.isAllowedByPattern(session_id, cmd_text) else false;

        if (!auto and !pattern_ok) {
            // Check if approval token provided
            const has_token = getBoolArg(args, "approve") == true and getStringArg(args, "approval_token") != null;
            if (has_token) {
                const token = getStringArg(args, "approval_token").?;
                // Build clean args JSON for validation
                const clean_args = try buildCleanArgsJson(allocator, args);
                defer allocator.free(clean_args);

                if (state.perm_svc.validateToken(session_id, token, tool_name, clean_args)) {
                    // Approved — configure session
                    if (getBoolArg(args, "remember_all_risky") == true) {
                        state.setAutoApprove(session_id);
                    } else if (getBoolArg(args, "remember_in_session") == true and cmd_text.len > 0) {
                        state.perm_svc.allowPattern(session_id, cmd_text) catch {};
                    } else if (state.permission_mode == .full) {
                        state.setAutoApprove(session_id);
                    }
                } else {
                    return makeErrorResponse(allocator, "Approval token invalid or expired.");
                }
            } else {
                // Request approval
                const clean_args = try buildCleanArgsJson(allocator, args);
                defer allocator.free(clean_args);
                const approval = try state.perm_svc.requestApproval(session_id, tool_name, clean_args);

                const summary = if (std.mem.eql(u8, tool_name, "run_command"))
                    try std.fmt.allocPrint(allocator, "Run command: {s}", .{cmd_text})
                else if (std.mem.eql(u8, tool_name, "write_file"))
                    try std.fmt.allocPrint(allocator, "Write file: {s}", .{getStringArg(args, "path") orelse "?"})
                else
                    try allocator.dupe(u8, "Take screenshot");
                defer allocator.free(summary);

                const summary_escaped = try jsonEscapeStr(allocator, summary);
                defer allocator.free(summary_escaped);
                const req_id_str = approval.approval_request_id;
                const token_str = approval.token_hex;

                return try std.fmt.allocPrint(allocator,
                    \\{{"content":[{{"type":"text","text":"AWAITING_APPROVAL: Ask the user in chat to approve this action. Re-call the same tool with approve=true and approval_token from structuredContent."}}],"structuredContent":{{"status":"AWAITING_APPROVAL","approvalRequestId":"{s}","approvalToken":"{s}","toolName":"{s}","summary":{s}}},"isError":true}}
                , .{ &req_id_str, &token_str, tool_name, summary_escaped });
            }
        }
    }

    // Execute tool
    return executeTool(allocator, tool_name, args, session_id, state);
}

fn executeTool(
    allocator: std.mem.Allocator,
    tool_name: []const u8,
    args: std.json.Value,
    session_id: []const u8,
    state: *AppState,
) ![]const u8 {
    if (std.mem.eql(u8, tool_name, "run_command")) return toolRunCommand(allocator, args, session_id, state);
    if (std.mem.eql(u8, tool_name, "read_file")) return toolReadFile(allocator, args, state);
    if (std.mem.eql(u8, tool_name, "write_file")) return toolWriteFile(allocator, args, state);
    if (std.mem.eql(u8, tool_name, "list_directory")) return toolListDirectory(allocator, args, state);
    if (std.mem.eql(u8, tool_name, "system_info")) return toolSystemInfo(allocator, state);
    if (std.mem.eql(u8, tool_name, "read_image")) return toolReadImage(allocator, args, state);
    if (std.mem.eql(u8, tool_name, "take_screenshot")) return toolTakeScreenshot(allocator, state);
    if (std.mem.eql(u8, tool_name, "network_speed")) return toolNetworkSpeed(allocator, args, state);
    if (std.mem.eql(u8, tool_name, "run_agent")) return toolRunAgent(allocator, args, state);
    if (std.mem.eql(u8, tool_name, "edit_file")) return toolEditFile(allocator, args, state);
    if (std.mem.eql(u8, tool_name, "web_fetch")) return toolWebFetch(allocator, args, state);
    if (std.mem.eql(u8, tool_name, "http_request")) return toolHttpRequest(allocator, args, state);
    if (std.mem.eql(u8, tool_name, "git_operations")) return toolGitOperations(allocator, args, state);
    return makeErrorResponse(allocator, try std.fmt.allocPrint(allocator, "Unknown tool: {s}", .{tool_name}));
}

// ── Tool implementations ────────────────────────────────────────────────────

const CommandResult = struct {
    stdout: []u8,
    stderr: []u8,
    exit_code: i32,
    timed_out: bool,
    duration_ms: i64,
};

const TimeoutCtx = struct {
    child: *std.process.Child,
    done: std.atomic.Value(bool),
    allocator: std.mem.Allocator,
    refs: std.atomic.Value(u32), // shared ref count; last to decrement frees ctx
};

fn releaseTimeoutCtx(ctx: *TimeoutCtx) void {
    if (ctx.refs.fetchSub(1, .acq_rel) == 1) {
        ctx.allocator.destroy(ctx);
    }
}

fn timeoutThread(ctx: *TimeoutCtx, timeout_ms: u64) void {
    defer releaseTimeoutCtx(ctx);
    std.Thread.sleep(timeout_ms * std.time.ns_per_ms);
    if (!ctx.done.load(.acquire)) {
        _ = ctx.child.kill() catch std.process.Child.Term{ .Signal = 15 };
    }
}

fn runCommandInternal(
    allocator: std.mem.Allocator,
    timeout_allocator: std.mem.Allocator,
    command: []const u8,
    cwd: ?[]const u8,
    mode: PermissionMode,
    home: []const u8,
) !CommandResult {
    // Sandbox wrapping (macOS/Linux)
    const sandbox_result = if (mode == .sandbox) try platform.wrapSandbox(allocator, command, home) else null;
    const final_cmd = if (sandbox_result) |r| r.cmd else command;
    const sandbox_note: ?[]const u8 = if (sandbox_result) |r| r.note else null;
    defer if (sandbox_result) |r| if (r.applied) allocator.free(r.cmd);

    const shell_args = try platform.shellArgs(allocator, final_cmd);
    defer allocator.free(shell_args);

    var child = std.process.Child.init(shell_args, allocator);
    child.stdout_behavior = .Pipe;
    child.stderr_behavior = .Pipe;
    child.stdin_behavior = .Ignore;
    if (cwd) |c| child.cwd = c else child.cwd = home;

    const start_ms = std.time.milliTimestamp();
    try child.spawn();

    // Allocate from the long-lived timeout_allocator so ctx survives the arena.
    // refs starts at 2: one for main, one for the watchdog thread.
    // The last participant to call releaseTimeoutCtx frees the allocation.
    const timeout_ctx = try timeout_allocator.create(TimeoutCtx);
    timeout_ctx.* = .{
        .child = &child,
        .done = std.atomic.Value(bool).init(false),
        .allocator = timeout_allocator,
        .refs = std.atomic.Value(u32).init(2),
    };
    const watchdog = std.Thread.spawn(.{}, timeoutThread, .{ timeout_ctx, 30_000 }) catch blk: {
        // Spawn failed — release thread's ref now (main still holds its ref).
        releaseTimeoutCtx(timeout_ctx);
        break :blk null;
    };

    const stdout = child.stdout.?.readToEndAlloc(allocator, 1024 * 1024) catch try allocator.dupe(u8, "");
    const stderr_raw = child.stderr.?.readToEndAlloc(allocator, 10 * 1024) catch try allocator.dupe(u8, "");

    // Signal the thread that the command finished before it reads done.
    // Safe: main still holds its own ref, so ctx is alive here.
    const timed_out = timeout_ctx.done.load(.acquire) == false;
    timeout_ctx.done.store(true, .release);

    const wait_result = child.wait() catch std.process.Child.Term{ .Exited = 1 };
    const exit_code: i32 = switch (wait_result) {
        .Exited => |c| @intCast(c),
        else => 1,
    };

    if (watchdog) |t| t.detach();
    releaseTimeoutCtx(timeout_ctx); // release main's ref; thread may free ctx

    const stderr = if (sandbox_note) |note|
        try std.fmt.allocPrint(allocator, "{s}\n{s}", .{ stderr_raw, note })
    else
        stderr_raw;
    if (sandbox_note != null) allocator.free(stderr_raw);

    return .{
        .stdout = stdout,
        .stderr = stderr,
        .exit_code = exit_code,
        .timed_out = timed_out and exit_code != 0,
        .duration_ms = std.time.milliTimestamp() - start_ms,
    };
}

fn toolRunCommand(
    allocator: std.mem.Allocator,
    args: std.json.Value,
    session_id: []const u8,
    state: *AppState,
) ![]const u8 {
    const command = getStringArg(args, "command") orelse
        return makeErrorResponse(allocator, "Missing 'command' argument.");
    const cwd = getStringArg(args, "cwd");

    // Loop guard (per session + command)
    const fp = try std.fmt.allocPrint(allocator, "{s}|{s}|{s}", .{ session_id, command, cwd orelse "" });
    defer allocator.free(fp);
    if (!state.loop_guard.tryAcquire(fp)) {
        return makeErrorResponse(allocator, try std.fmt.allocPrint(
            allocator,
            "Command suppressed (already running or recently failed): {s}",
            .{command},
        ));
    }

    const result = runCommandInternal(allocator, state.allocator, command, cwd, state.permission_mode, state.home_dir) catch |err| {
        state.loop_guard.release(fp, false);
        return makeErrorResponse(allocator, try std.fmt.allocPrint(allocator, "Command execution error: {}", .{err}));
    };
    defer allocator.free(result.stdout);
    defer allocator.free(result.stderr);

    state.loop_guard.release(fp, result.exit_code == 0);

    const stdout_escaped = try jsonEscapeStr(allocator, result.stdout[0..@min(result.stdout.len, 50_000)]);
    defer allocator.free(stdout_escaped);
    const stderr_escaped = try jsonEscapeStr(allocator, result.stderr[0..@min(result.stderr.len, 10_000)]);
    defer allocator.free(stderr_escaped);

    const text = try std.fmt.allocPrint(allocator,
        "{{\"stdout\":{s},\"stderr\":{s},\"exitCode\":{d},\"durationMs\":{d},\"timedOut\":{s}}}",
        .{ stdout_escaped, stderr_escaped, result.exit_code, result.duration_ms, if (result.timed_out) "true" else "false" },
    );
    defer allocator.free(text);

    if (state.log_enabled) {
        const ts = timestamp();
        std.debug.print("[{s}]   $ {s}\n", .{ ts, command });
        std.debug.print("[{s}]   exit={d} {d}ms\n", .{ ts, result.exit_code, result.duration_ms });
    }

    const text_escaped = try jsonEscapeStr(allocator, text);
    defer allocator.free(text_escaped);
    const is_error = result.exit_code != 0;
    return std.fmt.allocPrint(allocator,
        "{{\"content\":[{{\"type\":\"text\",\"text\":{s}}}],\"isError\":{s}}}",
        .{ text_escaped, if (is_error) "true" else "false" },
    );
}

fn toolReadFile(allocator: std.mem.Allocator, args: std.json.Value, state: *AppState) ![]const u8 {
    const raw_path = getStringArg(args, "path") orelse
        return makeErrorResponse(allocator, "Missing 'path' argument.");
    const expanded = try expandHome(allocator, raw_path, state.home_dir);
    defer allocator.free(expanded);
    const abs = try std.fs.realpathAlloc(allocator, expanded);
    defer allocator.free(abs);

    const file = std.fs.openFileAbsolute(abs, .{}) catch |err|
        return makeErrorResponse(allocator, try std.fmt.allocPrint(allocator, "Error: {}", .{err}));
    defer file.close();

    const content = file.readToEndAlloc(allocator, 100 * 1024) catch |err|
        return makeErrorResponse(allocator, try std.fmt.allocPrint(allocator, "Error: {}", .{err}));
    defer allocator.free(content);

    const escaped = try jsonEscapeStr(allocator, content);
    defer allocator.free(escaped);
    return std.fmt.allocPrint(allocator, "{{\"content\":[{{\"type\":\"text\",\"text\":{s}}}]}}", .{escaped});
}

fn toolWriteFile(allocator: std.mem.Allocator, args: std.json.Value, state: *AppState) ![]const u8 {
    const raw_path = getStringArg(args, "path") orelse
        return makeErrorResponse(allocator, "Missing 'path' argument.");
    const file_content = getStringArg(args, "content") orelse "";

    const expanded = try expandHome(allocator, raw_path, state.home_dir);
    defer allocator.free(expanded);
    const abs = blk: {
        const parent = std.fs.path.dirname(expanded) orelse ".";
        const pabs = std.fs.realpathAlloc(allocator, parent) catch {
            break :blk try allocator.dupe(u8, expanded);
        };
        defer allocator.free(pabs);
        break :blk try std.fs.path.join(allocator, &.{ pabs, std.fs.path.basename(expanded) });
    };
    defer allocator.free(abs);

    const f = std.fs.createFileAbsolute(abs, .{}) catch |err|
        return makeErrorResponse(allocator, try std.fmt.allocPrint(allocator, "Error: {}", .{err}));
    defer f.close();
    f.writeAll(file_content) catch |err|
        return makeErrorResponse(allocator, try std.fmt.allocPrint(allocator, "Error: {}", .{err}));

    const msg = try std.fmt.allocPrint(allocator, "Written to {s}", .{abs});
    defer allocator.free(msg);
    const escaped = try jsonEscapeStr(allocator, msg);
    defer allocator.free(escaped);
    return std.fmt.allocPrint(allocator, "{{\"content\":[{{\"type\":\"text\",\"text\":{s}}}]}}", .{escaped});
}

fn toolListDirectory(allocator: std.mem.Allocator, args: std.json.Value, state: *AppState) ![]const u8 {
    const raw_path = getStringArg(args, "path") orelse "~";
    const expanded = try expandHome(allocator, raw_path, state.home_dir);
    defer allocator.free(expanded);
    const abs = std.fs.realpathAlloc(allocator, expanded) catch expanded;
    defer allocator.free(abs);

    var dir = std.fs.openDirAbsolute(abs, .{ .iterate = true }) catch |err|
        return makeErrorResponse(allocator, try std.fmt.allocPrint(allocator, "Error: {}", .{err}));
    defer dir.close();

    var lines = std.ArrayList(u8).empty;
    var it = dir.iterate();
    var first = true;
    while (try it.next()) |entry| {
        if (!first) try lines.appendSlice(allocator, "\n");
        first = false;
        const prefix: u8 = if (entry.kind == .directory) 'd' else '-';
        try lines.writer(allocator).print("{c} {s}", .{ prefix, entry.name });
    }

    const escaped = try jsonEscapeStr(allocator, lines.items);
    defer allocator.free(escaped);
    return std.fmt.allocPrint(allocator, "{{\"content\":[{{\"type\":\"text\",\"text\":{s}}}]}}", .{escaped});
}

fn toolSystemInfo(allocator: std.mem.Allocator, state: *AppState) ![]const u8 {
    // Use uname / systeminfo for cross-platform info
    const cmd = switch (builtin.os.tag) {
        .windows =>
        \\powershell -NoProfile -Command "Get-ComputerInfo | Select-Object -Property CsName,OsArchitecture,OsVersion,CsTotalPhysicalMemory | ConvertTo-Json"
        ,
        .macos =>
        \\echo "{\"hostname\":\"$(hostname)\",\"platform\":\"darwin\",\"arch\":\"$(uname -m)\",\"uptime\":\"$(uptime | awk '{print $3}' | tr -d ',')\",\"totalMemory\":\"$(sysctl -n hw.memsize | awk '{printf \"%.0fGB\", $1/1024/1024/1024}')\",\"freeMemory\":\"?\"}"
        ,
        else =>
        \\echo "{\"hostname\":\"$(hostname)\",\"platform\":\"linux\",\"arch\":\"$(uname -m)\",\"uptime\":\"$(uptime -p 2>/dev/null || uptime)\",\"totalMemory\":\"$(free -h 2>/dev/null | awk '/^Mem:/{print $2}' || echo '?')\",\"freeMemory\":\"$(free -h 2>/dev/null | awk '/^Mem:/{print $4}' || echo '?')\"}"
        ,
    };

    const result = runCommandInternal(allocator, state.allocator, cmd, null, .full, state.home_dir) catch |err|
        return makeErrorResponse(allocator, try std.fmt.allocPrint(allocator, "system_info error: {}", .{err}));
    defer allocator.free(result.stdout);
    defer allocator.free(result.stderr);

    const text = if (result.stdout.len > 0) result.stdout else result.stderr;
    const escaped = try jsonEscapeStr(allocator, std.mem.trim(u8, text, " \n\r\t"));
    defer allocator.free(escaped);
    return std.fmt.allocPrint(allocator, "{{\"content\":[{{\"type\":\"text\",\"text\":{s}}}]}}", .{escaped});
}

fn toolReadImage(allocator: std.mem.Allocator, args: std.json.Value, state: *AppState) ![]const u8 {
    const raw_path = getStringArg(args, "path") orelse
        return makeErrorResponse(allocator, "Missing 'path' argument.");
    const expanded = try expandHome(allocator, raw_path, state.home_dir);
    defer allocator.free(expanded);
    const abs = std.fs.realpathAlloc(allocator, expanded) catch expanded;
    defer allocator.free(abs);

    const file = std.fs.openFileAbsolute(abs, .{}) catch |err|
        return makeErrorResponse(allocator, try std.fmt.allocPrint(allocator, "Error: {}", .{err}));
    defer file.close();

    const data = file.readToEndAlloc(allocator, 50 * 1024 * 1024) catch |err|
        return makeErrorResponse(allocator, try std.fmt.allocPrint(allocator, "Error: {}", .{err}));
    defer allocator.free(data);

    const encoded_len = std.base64.standard.Encoder.calcSize(data.len);
    const b64 = try allocator.alloc(u8, encoded_len);
    defer allocator.free(b64);
    _ = std.base64.standard.Encoder.encode(b64, data);

    const ext = std.fs.path.extension(abs);
    const mime = mimeForExt(ext);

    if (std.mem.startsWith(u8, mime, "image/")) {
        const mime_escaped = try jsonEscapeStr(allocator, mime);
        defer allocator.free(mime_escaped);
        const b64_escaped = try jsonEscapeStr(allocator, b64);
        defer allocator.free(b64_escaped);
        const desc = try std.fmt.allocPrint(allocator, "Image: {s} ({s}, {d} bytes)", .{ abs, mime, data.len });
        defer allocator.free(desc);
        const desc_escaped = try jsonEscapeStr(allocator, desc);
        defer allocator.free(desc_escaped);
        return std.fmt.allocPrint(allocator,
            "{{\"content\":[{{\"type\":\"image\",\"data\":{s},\"mimeType\":{s}}},{{\"type\":\"text\",\"text\":{s}}}]}}",
            .{ b64_escaped, mime_escaped, desc_escaped },
        );
    }

    const desc = try std.fmt.allocPrint(allocator, "File: {s} ({s}, {d} bytes)\nBase64: {s}...", .{
        abs, mime, data.len, b64[0..@min(b64.len, 200)],
    });
    defer allocator.free(desc);
    const desc_escaped = try jsonEscapeStr(allocator, desc);
    defer allocator.free(desc_escaped);
    return std.fmt.allocPrint(allocator, "{{\"content\":[{{\"type\":\"text\",\"text\":{s}}}]}}", .{desc_escaped});
}

fn toolTakeScreenshot(allocator: std.mem.Allocator, state: *AppState) ![]const u8 {
    const b64 = screenshot.captureBase64(allocator) catch |err| {
        return makeErrorResponse(allocator, try std.fmt.allocPrint(
            allocator,
            "Screenshot failed: {}. Make sure screen recording permission is granted.",
            .{err},
        ));
    };
    defer allocator.free(b64);

    // Send to bridge via webhook
    const msg = try std.fmt.allocPrint(
        allocator,
        "Here is a screenshot of my screen right now. Reply me with the image.\n\n```\ndata:image/png;base64,{s}\n```",
        .{b64},
    );
    defer allocator.free(msg);

    const msg_escaped = try jsonEscapeStr(allocator, msg);
    defer allocator.free(msg_escaped);
    const bridge_cmd = try std.fmt.allocPrint(allocator, "{{\"type\":\"send_webhook\",\"message\":{s}}}", .{msg_escaped});
    defer allocator.free(bridge_cmd);

    state.sendToBridge(bridge_cmd);

    return makeTextResponse(allocator, "Screenshot captured and sent to Poke.", false);
}

fn toolNetworkSpeed(allocator: std.mem.Allocator, args: std.json.Value, state: *AppState) ![]const u8 {
    const test_sel = getStringArg(args, "tests") orelse "both";
    const run_dl = std.mem.eql(u8, test_sel, "download") or std.mem.eql(u8, test_sel, "both");
    const run_ul = std.mem.eql(u8, test_sel, "upload") or std.mem.eql(u8, test_sel, "both");

    var parts = std.ArrayList(u8).empty;
    defer parts.deinit(allocator);

    if (run_dl) try parts.appendSlice(allocator, "DL=$(curl -s -o /dev/null -w '%{time_total}' 'https://speed.cloudflare.com/__down?bytes=26214400')");
    if (run_ul) {
        if (run_dl) try parts.appendSlice(allocator, " && ");
        try parts.appendSlice(allocator, "TMP=$(mktemp /tmp/poke-speed.XXXXXX) && dd if=/dev/zero of=\"$TMP\" bs=1m count=10 2>/dev/null && UL=$(curl -s -o /dev/null -w '%{time_total}' -X POST --data-binary @\"$TMP\" 'https://speed.cloudflare.com/__up') && rm -f \"$TMP\"");
    }
    try parts.appendSlice(allocator, " && printf 'DL=%s\\nUL=%s\\n' \"${DL:-}\" \"${UL:-}\"");

    const result = runCommandInternal(allocator, state.allocator, parts.items, null, .full, state.home_dir) catch |err|
        return makeErrorResponse(allocator, try std.fmt.allocPrint(allocator, "Speed test error: {}", .{err}));
    defer allocator.free(result.stdout);
    defer allocator.free(result.stderr);

    var lines = std.ArrayList(u8).empty;
    defer lines.deinit(allocator);
    try lines.appendSlice(allocator, "Network Speed Test");

    if (run_dl) {
        const dl_secs = parseSpeedField(result.stdout, "DL=");
        if (dl_secs > 0) {
            const mbps = (26_214_400.0 * 8.0) / dl_secs / 1_000_000.0;
            try lines.writer(allocator).print("\n- Download: {d:.2} Mbps ({d:.2}s for 25 MiB)", .{ mbps, dl_secs });
        } else {
            try lines.appendSlice(allocator, "\n- Download: unavailable");
        }
    }
    if (run_ul) {
        const ul_secs = parseSpeedField(result.stdout, "UL=");
        if (ul_secs > 0) {
            const mbps = (10_485_760.0 * 8.0) / ul_secs / 1_000_000.0;
            try lines.writer(allocator).print("\n- Upload: {d:.2} Mbps ({d:.2}s for 10 MiB)", .{ mbps, ul_secs });
        } else {
            try lines.appendSlice(allocator, "\n- Upload: unavailable");
        }
    }

    const escaped = try jsonEscapeStr(allocator, lines.items);
    defer allocator.free(escaped);
    return std.fmt.allocPrint(allocator, "{{\"content\":[{{\"type\":\"text\",\"text\":{s}}}]}}", .{escaped});
}

fn parseSpeedField(output: []const u8, field: []const u8) f64 {
    const start = std.mem.indexOf(u8, output, field) orelse return 0;
    const rest = output[start + field.len ..];
    const end = std.mem.indexOfScalar(u8, rest, '\n') orelse rest.len;
    const val = std.mem.trim(u8, rest[0..end], " \r\t");
    return std.fmt.parseFloat(f64, val) catch 0;
}

fn toolRunAgent(allocator: std.mem.Allocator, args: std.json.Value, state: *AppState) ![]const u8 {
    const agent_name = getStringArg(args, "name") orelse
        return makeErrorResponse(allocator, "Missing 'name' argument.");
    agents.runAgentByName(allocator, agent_name) catch |err| {
        return makeErrorResponse(allocator, try std.fmt.allocPrint(allocator, "Agent '{s}' failed: {}", .{ agent_name, err }));
    };
    _ = state;
    return makeTextResponse(allocator, try std.fmt.allocPrint(allocator, "Agent '{s}' completed.", .{agent_name}), false);
}

fn toolEditFile(allocator: std.mem.Allocator, args: std.json.Value, state: *AppState) ![]const u8 {
    const raw_path = getStringArg(args, "path") orelse
        return makeErrorResponse(allocator, "Missing 'path' argument.");
    const old_str = getStringArg(args, "old_string") orelse
        return makeErrorResponse(allocator, "Missing 'old_string' argument.");
    const new_str = getStringArg(args, "new_string") orelse
        return makeErrorResponse(allocator, "Missing 'new_string' argument.");

    const path = try expandHome(allocator, raw_path, state.home_dir);
    defer allocator.free(path);

    const content = std.fs.cwd().readFileAlloc(allocator, path, 10 * 1024 * 1024) catch |err| {
        return makeErrorResponse(allocator, try std.fmt.allocPrint(allocator, "Cannot read '{s}': {}", .{ path, err }));
    };
    defer allocator.free(content);

    // Count occurrences
    var count: usize = 0;
    var search_pos: usize = 0;
    while (std.mem.indexOfPos(u8, content, search_pos, old_str)) |pos| {
        count += 1;
        search_pos = pos + old_str.len;
        if (count > 1) break;
    }

    if (count == 0) {
        return makeErrorResponse(allocator, "old_string not found in file.");
    }
    if (count > 1) {
        return makeErrorResponse(allocator, "old_string appears more than once — provide more context to make it unique.");
    }

    const idx = std.mem.indexOf(u8, content, old_str).?;
    const new_content = try std.mem.concat(allocator, u8, &.{ content[0..idx], new_str, content[idx + old_str.len ..] });
    defer allocator.free(new_content);

    const file = std.fs.cwd().createFile(path, .{ .truncate = true }) catch |err| {
        return makeErrorResponse(allocator, try std.fmt.allocPrint(allocator, "Cannot write '{s}': {}", .{ path, err }));
    };
    defer file.close();
    file.writeAll(new_content) catch |err| {
        return makeErrorResponse(allocator, try std.fmt.allocPrint(allocator, "Write error: {}", .{err}));
    };

    return makeTextResponse(allocator, try std.fmt.allocPrint(allocator, "Replaced 1 occurrence in '{s}'.", .{path}), false);
}

fn toolWebFetch(allocator: std.mem.Allocator, args: std.json.Value, state: *AppState) ![]const u8 {
    const url = getStringArg(args, "url") orelse
        return makeErrorResponse(allocator, "Missing 'url' argument.");
    const max_chars: usize = blk: {
        const obj = switch (args) {
            .object => |o| o,
            else => break :blk 20_000,
        };
        if (obj.get("max_chars")) |v| {
            break :blk switch (v) {
                .integer => |n| @intCast(@max(1, n)),
                else => 20_000,
            };
        }
        break :blk 20_000;
    };

    const cmd = try std.fmt.allocPrint(allocator, "curl -fsSL --max-time 15 {s}", .{url});
    defer allocator.free(cmd);

    const res = runCommandInternal(allocator, state.allocator, cmd, null, state.permission_mode, state.home_dir) catch |err| {
        return makeErrorResponse(allocator, try std.fmt.allocPrint(allocator, "fetch failed: {}", .{err}));
    };
    defer allocator.free(res.stdout);
    defer allocator.free(res.stderr);

    if (res.exit_code != 0) {
        return makeErrorResponse(allocator, try std.fmt.allocPrint(allocator, "curl exited {d}: {s}", .{ res.exit_code, res.stderr }));
    }

    const body = if (res.stdout.len > max_chars) res.stdout[0..max_chars] else res.stdout;
    return makeTextResponse(allocator, try allocator.dupe(u8, body), false);
}

fn toolHttpRequest(allocator: std.mem.Allocator, args: std.json.Value, state: *AppState) ![]const u8 {
    const method = getStringArg(args, "method") orelse "GET";
    const url = getStringArg(args, "url") orelse
        return makeErrorResponse(allocator, "Missing 'url' argument.");
    const body_arg = getStringArg(args, "body");

    // Build curl command
    var parts = std.ArrayList(u8).empty;
    defer parts.deinit(allocator);
    try parts.appendSlice(allocator, "curl -fsSL -i --max-time 30 -X ");
    try parts.appendSlice(allocator, method);
    try parts.append(allocator, ' ');

    // Add headers
    const obj = switch (args) {
        .object => |o| o,
        else => std.json.ObjectMap.init(allocator),
    };
    if (obj.get("headers")) |hv| {
        if (hv == .object) {
            var hit = hv.object.iterator();
            while (hit.next()) |hentry| {
                try parts.appendSlice(allocator, "-H '");
                try parts.appendSlice(allocator, hentry.key_ptr.*);
                try parts.appendSlice(allocator, ": ");
                const hval = switch (hentry.value_ptr.*) {
                    .string => |s| s,
                    else => continue,
                };
                try parts.appendSlice(allocator, hval);
                try parts.appendSlice(allocator, "' ");
            }
        }
    }

    if (body_arg) |b| {
        try parts.appendSlice(allocator, "--data-raw '");
        // Escape single quotes in body
        for (b) |c| {
            if (c == '\'') {
                try parts.appendSlice(allocator, "'\\''");
            } else {
                try parts.append(allocator, c);
            }
        }
        try parts.appendSlice(allocator, "' ");
    }

    try parts.append(allocator, '\'');
    try parts.appendSlice(allocator, url);
    try parts.append(allocator, '\'');

    const cmd = try parts.toOwnedSlice(allocator);
    defer allocator.free(cmd);

    const res = runCommandInternal(allocator, state.allocator, cmd, null, state.permission_mode, state.home_dir) catch |err| {
        return makeErrorResponse(allocator, try std.fmt.allocPrint(allocator, "http_request failed: {}", .{err}));
    };
    defer allocator.free(res.stdout);
    defer allocator.free(res.stderr);

    const out = if (res.stdout.len > 0) res.stdout else res.stderr;
    return makeTextResponse(allocator, try std.fmt.allocPrint(allocator, "exit={d}\n{s}", .{ res.exit_code, out }), res.exit_code != 0);
}

fn toolGitOperations(allocator: std.mem.Allocator, args: std.json.Value, state: *AppState) ![]const u8 {
    _ = state;
    const op = getStringArg(args, "operation") orelse
        return makeErrorResponse(allocator, "Missing 'operation' argument.");
    const cwd_arg = getStringArg(args, "cwd");

    // Build argv: git <op> [args...]
    var argv = std.ArrayList([]const u8).empty;
    defer argv.deinit(allocator);
    try argv.append(allocator, "git");
    try argv.append(allocator, op);

    const obj = switch (args) {
        .object => |o| o,
        else => std.json.ObjectMap.init(allocator),
    };
    if (obj.get("args")) |av| {
        if (av == .array) {
            for (av.array.items) |item| {
                if (item == .string) try argv.append(allocator, item.string);
            }
        }
    }

    var child = std.process.Child.init(argv.items, allocator);
    child.stdout_behavior = .Pipe;
    child.stderr_behavior = .Pipe;
    child.stdin_behavior = .Ignore;
    if (cwd_arg) |cwd| child.cwd = cwd;
    child.spawn() catch |err| {
        return makeErrorResponse(allocator, try std.fmt.allocPrint(allocator, "Failed to spawn git: {}", .{err}));
    };

    const stdout = child.stdout.?.readToEndAlloc(allocator, 1024 * 1024) catch "";
    const stderr = child.stderr.?.readToEndAlloc(allocator, 64 * 1024) catch "";
    defer allocator.free(stdout);
    defer allocator.free(stderr);
    const term = child.wait() catch std.process.Child.Term{ .Exited = 1 };
    const exit_code: i32 = switch (term) {
        .Exited => |c| @intCast(c),
        else => -1,
    };

    const combined = if (stderr.len > 0)
        try std.fmt.allocPrint(allocator, "{s}\nstderr: {s}", .{ stdout, stderr })
    else
        try allocator.dupe(u8, stdout);
    defer allocator.free(combined);

    return makeTextResponse(allocator, try std.fmt.allocPrint(allocator, "exit={d}\n{s}", .{ exit_code, combined }), exit_code != 0);
}

// ── Utilities ─────────────────────────────────────────────────────────────────

fn getStringArg(args: std.json.Value, key: []const u8) ?[]const u8 {
    const obj = switch (args) {
        .object => |o| o,
        else => return null,
    };
    const val = obj.get(key) orelse return null;
    return switch (val) {
        .string => |s| s,
        else => null,
    };
}

fn getBoolArg(args: std.json.Value, key: []const u8) ?bool {
    const obj = switch (args) {
        .object => |o| o,
        else => return null,
    };
    const val = obj.get(key) orelse return null;
    return switch (val) {
        .bool => |b| b,
        else => null,
    };
}

fn makeErrorResponse(allocator: std.mem.Allocator, msg: []const u8) ![]const u8 {
    const escaped = try jsonEscapeStr(allocator, msg);
    defer allocator.free(escaped);
    return std.fmt.allocPrint(allocator, "{{\"content\":[{{\"type\":\"text\",\"text\":{s}}}],\"isError\":true}}", .{escaped});
}

fn makeTextResponse(allocator: std.mem.Allocator, msg: []const u8, is_error: bool) ![]const u8 {
    const escaped = try jsonEscapeStr(allocator, msg);
    defer allocator.free(escaped);
    return std.fmt.allocPrint(allocator,
        "{{\"content\":[{{\"type\":\"text\",\"text\":{s}}}],\"isError\":{s}}}",
        .{ escaped, if (is_error) "true" else "false" },
    );
}

/// JSON-encode a string (with surrounding quotes and proper escaping).
fn jsonEscapeStr(allocator: std.mem.Allocator, s: []const u8) ![]u8 {
    return std.json.Stringify.valueAlloc(allocator, s, .{});
}

/// Build a JSON object of args, excluding approval-control fields.
fn buildCleanArgsJson(allocator: std.mem.Allocator, args: std.json.Value) ![]u8 {
    const skip = [_][]const u8{ "approval_token", "approve", "remember_in_session", "remember_all_risky" };
    var obj = std.json.ObjectMap.init(allocator);
    defer obj.deinit();
    const src = switch (args) {
        .object => |o| o,
        else => {
            return std.json.Stringify.valueAlloc(allocator, args, .{});
        },
    };
    var it = src.iterator();
    while (it.next()) |entry| {
        var skip_it = false;
        for (skip) |s| if (std.mem.eql(u8, entry.key_ptr.*, s)) { skip_it = true; break; };
        if (!skip_it) try obj.put(entry.key_ptr.*, entry.value_ptr.*);
    }
    return std.json.Stringify.valueAlloc(allocator, std.json.Value{ .object = obj }, .{});
}

fn expandHome(allocator: std.mem.Allocator, path: []const u8, home: []const u8) ![]u8 {
    if (std.mem.startsWith(u8, path, "~/") or std.mem.eql(u8, path, "~")) {
        return std.fs.path.join(allocator, &.{ home, path[1..] });
    }
    return allocator.dupe(u8, path);
}

fn mimeForExt(ext: []const u8) []const u8 {
    const map = [_]struct { ext: []const u8, mime: []const u8 }{
        .{ .ext = ".png", .mime = "image/png" },
        .{ .ext = ".jpg", .mime = "image/jpeg" },
        .{ .ext = ".jpeg", .mime = "image/jpeg" },
        .{ .ext = ".gif", .mime = "image/gif" },
        .{ .ext = ".webp", .mime = "image/webp" },
        .{ .ext = ".svg", .mime = "image/svg+xml" },
        .{ .ext = ".pdf", .mime = "application/pdf" },
        .{ .ext = ".ico", .mime = "image/x-icon" },
        .{ .ext = ".bmp", .mime = "image/bmp" },
    };
    for (map) |entry| if (std.ascii.eqlIgnoreCase(ext, entry.ext)) return entry.mime;
    return "application/octet-stream";
}

fn timestamp() [8]u8 {
    const now = std.time.timestamp();
    const secs = @mod(now, 86400);
    const h = @divTrunc(secs, 3600);
    const m = @divTrunc(@mod(secs, 3600), 60);
    const s = @mod(secs, 60);
    var buf: [8]u8 = undefined;
    _ = std.fmt.bufPrint(&buf, "{d:0>2}:{d:0>2}:{d:0>2}", .{ h, m, s }) catch {};
    return buf;
}
