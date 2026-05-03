/// Permission service — HMAC-SHA256 approval tokens + session whitelist.
/// Direct port of src/permission-service.js.
const std = @import("std");

const TOKEN_TTL_NS: i128 = 5 * 60 * std.time.ns_per_s;

const HmacSha256 = std.crypto.auth.hmac.sha2.HmacSha256;

// ── Approval record ────────────────────────────────────────────────────────

const Approval = struct {
    approval_request_id: [36]u8, // UUID as hex string
    session_id: []u8,
    tool_name: []u8,
    args_hash: [64]u8, // SHA-256 as hex
    expires_at_ns: i128,
    consumed: bool,
};

// ── PermissionService ──────────────────────────────────────────────────────

pub const PermissionService = struct {
    allocator: std.mem.Allocator,
    secret: []u8,
    pending: std.StringHashMap(Approval),
    whitelist: std.StringHashMap(std.ArrayList([]u8)),
    mutex: std.Thread.Mutex,

    pub fn init(allocator: std.mem.Allocator, secret: []const u8) !PermissionService {
        return .{
            .allocator = allocator,
            .secret = try allocator.dupe(u8, secret),
            .pending = std.StringHashMap(Approval).init(allocator),
            .whitelist = std.StringHashMap(std.ArrayList([]u8)).init(allocator),
            .mutex = .{},
        };
    }

    pub fn deinit(self: *PermissionService) void {
        self.allocator.free(self.secret);
        var pit = self.pending.iterator();
        while (pit.next()) |entry| {
            self.allocator.free(entry.key_ptr.*);
            self.allocator.free(entry.value_ptr.session_id);
            self.allocator.free(entry.value_ptr.tool_name);
        }
        self.pending.deinit();

        var wit = self.whitelist.iterator();
        while (wit.next()) |entry| {
            self.allocator.free(entry.key_ptr.*);
            for (entry.value_ptr.items) |pat| self.allocator.free(pat);
            entry.value_ptr.deinit(self.allocator);
        }
        self.whitelist.deinit();
    }

    pub const ApprovalResult = struct {
        approval_request_id: [36]u8,
        token_hex: [64]u8, // HMAC-SHA256 as hex
        expires_at_ms: i64,
    };

    /// Request approval for a tool call. Thread-safe.
    pub fn requestApproval(
        self: *PermissionService,
        session_id: []const u8,
        tool_name: []const u8,
        args_json: []const u8,
    ) !ApprovalResult {
        self.mutex.lock();
        defer self.mutex.unlock();

        const now_ns = std.time.nanoTimestamp();
        const expires_ns = now_ns + TOKEN_TTL_NS;
        const expires_ms = @divTrunc(expires_ns, std.time.ns_per_ms);

        // Generate UUID-ish request ID
        var rand_bytes: [16]u8 = undefined;
        std.crypto.random.bytes(&rand_bytes);
        var req_id: [36]u8 = undefined;
        uuidFromBytes(rand_bytes, &req_id);

        // Hash args
        const args_hash = sha256Hex(args_json);

        // Build HMAC payload
        const args_hash_str = args_hash[0..];
        const payload = try std.fmt.allocPrint(
            self.allocator,
            "{s}:{s}:{s}:{s}:{d}",
            .{ &req_id, session_id, tool_name, args_hash_str, @as(i64, @intCast(expires_ms)) },
        );
        defer self.allocator.free(payload);

        // Compute HMAC
        var mac: [HmacSha256.mac_length]u8 = undefined;
        HmacSha256.create(&mac, payload, self.secret);
        const token_hex = hexEncode(mac[0..]);

        // Store approval
        const approval = Approval{
            .approval_request_id = req_id,
            .session_id = try self.allocator.dupe(u8, session_id),
            .tool_name = try self.allocator.dupe(u8, tool_name),
            .args_hash = args_hash,
            .expires_at_ns = expires_ns,
            .consumed = false,
        };

        const token_key = try self.allocator.dupe(u8, &token_hex);
        try self.pending.put(token_key, approval);

        return .{
            .approval_request_id = req_id,
            .token_hex = token_hex,
            .expires_at_ms = @intCast(expires_ms),
        };
    }

    /// Validate and consume an approval token. Thread-safe. Returns true if valid.
    pub fn validateToken(
        self: *PermissionService,
        session_id: []const u8,
        token_hex: []const u8,
        tool_name: []const u8,
        args_json: []const u8,
    ) bool {
        self.mutex.lock();
        defer self.mutex.unlock();

        const entry = self.pending.getPtr(token_hex) orelse return false;
        if (entry.consumed) return false;

        const now_ns = std.time.nanoTimestamp();
        if (now_ns > entry.expires_at_ns) {
            _ = self.pending.remove(token_hex);
            return false;
        }

        if (!std.mem.eql(u8, entry.session_id, session_id)) return false;
        if (!std.mem.eql(u8, entry.tool_name, tool_name)) return false;

        const expected_hash = sha256Hex(args_json);
        if (!std.mem.eql(u8, &expected_hash, &entry.args_hash)) return false;

        entry.consumed = true;
        return true;
    }

    /// Allow a glob pattern for a session. Thread-safe.
    pub fn allowPattern(
        self: *PermissionService,
        session_id: []const u8,
        pattern: []const u8,
    ) !void {
        self.mutex.lock();
        defer self.mutex.unlock();

        if (!self.whitelist.contains(session_id)) {
            const key = try self.allocator.dupe(u8, session_id);
            try self.whitelist.put(key, std.ArrayList([]u8).empty);
        }
        const list = self.whitelist.getPtr(session_id).?;
        const pat_copy = try self.allocator.dupe(u8, pattern);
        try list.append(self.allocator, pat_copy);
    }

    /// Check if a command matches any allowed pattern for the session. Thread-safe.
    pub fn isAllowedByPattern(
        self: *PermissionService,
        session_id: []const u8,
        command: []const u8,
    ) bool {
        self.mutex.lock();
        defer self.mutex.unlock();

        const list = self.whitelist.get(session_id) orelse return false;
        for (list.items) |pattern| {
            if (globMatch(pattern, command)) return true;
        }
        return false;
    }

    /// Clear all state for a session. Thread-safe.
    pub fn clearSession(self: *PermissionService, session_id: []const u8) void {
        self.mutex.lock();
        defer self.mutex.unlock();

        if (self.whitelist.fetchRemove(session_id)) |entry| {
            self.allocator.free(entry.key);
            var list = entry.value;
            for (list.items) |pat| self.allocator.free(pat);
            list.deinit(self.allocator);
        }

        // Remove pending approvals for this session
        var to_remove = std.ArrayList([]const u8).empty;
        defer to_remove.deinit(self.allocator);

        var it = self.pending.iterator();
        while (it.next()) |kv| {
            if (std.mem.eql(u8, kv.value_ptr.session_id, session_id)) {
                to_remove.append(self.allocator, kv.key_ptr.*) catch {};
            }
        }
        for (to_remove.items) |key| {
            if (self.pending.fetchRemove(key)) |removed| {
                self.allocator.free(removed.key);
                self.allocator.free(removed.value.session_id);
                self.allocator.free(removed.value.tool_name);
            }
        }
    }
};

// ── helpers ────────────────────────────────────────────────────────────────

/// SHA-256 of a string, returned as 64-char lowercase hex.
fn sha256Hex(data: []const u8) [64]u8 {
    var hash: [std.crypto.hash.sha2.Sha256.digest_length]u8 = undefined;
    std.crypto.hash.sha2.Sha256.hash(data, &hash, .{});
    return hexEncode(hash[0..]);
}

/// Encode bytes as lowercase hex. Input must be <= 32 bytes (output is 2x).
fn hexEncode(bytes: []const u8) [64]u8 {
    const charset = "0123456789abcdef";
    var out: [64]u8 = undefined;
    for (bytes, 0..) |b, i| {
        out[i * 2] = charset[b >> 4];
        out[i * 2 + 1] = charset[b & 0xf];
    }
    // zero-fill remaining
    for (bytes.len * 2..64) |i| out[i] = '0';
    return out;
}

/// Format 16 raw bytes as a UUID string (xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx).
fn uuidFromBytes(bytes: [16]u8, out: *[36]u8) void {
    const charset = "0123456789abcdef";
    const groups = [_]usize{ 4, 2, 2, 2, 6 };
    var b: usize = 0;
    var pos: usize = 0;
    for (groups, 0..) |g, gi| {
        if (gi > 0) {
            out[pos] = '-';
            pos += 1;
        }
        for (0..g) |_| {
            out[pos] = charset[bytes[b] >> 4];
            out[pos + 1] = charset[bytes[b] & 0xf];
            pos += 2;
            b += 1;
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

test "PermissionService init and deinit" {
    var svc = try PermissionService.init(std.testing.allocator, "test-secret");
    svc.deinit();
}

test "requestApproval returns UUID-shaped request ID" {
    var svc = try PermissionService.init(std.testing.allocator, "test-secret");
    defer svc.deinit();
    const result = try svc.requestApproval("sess-1", "run_shell", "{\"cmd\":\"ls\"}");
    const id = &result.approval_request_id;
    try std.testing.expectEqual(@as(usize, 36), id.len);
    try std.testing.expectEqual(@as(u8, '-'), id[8]);
    try std.testing.expectEqual(@as(u8, '-'), id[13]);
    try std.testing.expectEqual(@as(u8, '-'), id[18]);
    try std.testing.expectEqual(@as(u8, '-'), id[23]);
}

test "validateToken accepts a valid token" {
    var svc = try PermissionService.init(std.testing.allocator, "test-secret");
    defer svc.deinit();
    const args = "{\"cmd\":\"ls -la\"}";
    const result = try svc.requestApproval("sess-1", "run_shell", args);
    const ok = svc.validateToken("sess-1", &result.token_hex, "run_shell", args);
    try std.testing.expect(ok);
}

test "validateToken rejects wrong session" {
    var svc = try PermissionService.init(std.testing.allocator, "test-secret");
    defer svc.deinit();
    const args = "{\"cmd\":\"ls\"}";
    const result = try svc.requestApproval("sess-1", "run_shell", args);
    const ok = svc.validateToken("sess-WRONG", &result.token_hex, "run_shell", args);
    try std.testing.expect(!ok);
}

test "validateToken rejects wrong tool" {
    var svc = try PermissionService.init(std.testing.allocator, "test-secret");
    defer svc.deinit();
    const args = "{\"cmd\":\"ls\"}";
    const result = try svc.requestApproval("sess-1", "run_shell", args);
    const ok = svc.validateToken("sess-1", &result.token_hex, "write_file", args);
    try std.testing.expect(!ok);
}

test "validateToken rejects wrong args" {
    var svc = try PermissionService.init(std.testing.allocator, "test-secret");
    defer svc.deinit();
    const result = try svc.requestApproval("sess-1", "run_shell", "{\"cmd\":\"ls\"}");
    const ok = svc.validateToken("sess-1", &result.token_hex, "run_shell", "{\"cmd\":\"rm -rf /\"}");
    try std.testing.expect(!ok);
}

test "validateToken rejects replayed (already-consumed) token" {
    var svc = try PermissionService.init(std.testing.allocator, "test-secret");
    defer svc.deinit();
    const args = "{\"cmd\":\"ls\"}";
    const result = try svc.requestApproval("sess-1", "run_shell", args);
    _ = svc.validateToken("sess-1", &result.token_hex, "run_shell", args);
    const second = svc.validateToken("sess-1", &result.token_hex, "run_shell", args);
    try std.testing.expect(!second);
}

test "validateToken rejects unknown token" {
    var svc = try PermissionService.init(std.testing.allocator, "test-secret");
    defer svc.deinit();
    const fake_token = "0" ** 64;
    const ok = svc.validateToken("sess-1", fake_token, "run_shell", "{}");
    try std.testing.expect(!ok);
}

test "isAllowedByPattern exact match" {
    var svc = try PermissionService.init(std.testing.allocator, "s");
    defer svc.deinit();
    try svc.allowPattern("sess-1", "ls");
    try std.testing.expect(svc.isAllowedByPattern("sess-1", "ls"));
    try std.testing.expect(!svc.isAllowedByPattern("sess-1", "ls -la"));
}

test "isAllowedByPattern wildcard suffix" {
    var svc = try PermissionService.init(std.testing.allocator, "s");
    defer svc.deinit();
    try svc.allowPattern("sess-1", "ls*");
    try std.testing.expect(svc.isAllowedByPattern("sess-1", "ls"));
    try std.testing.expect(svc.isAllowedByPattern("sess-1", "ls -la"));
    try std.testing.expect(!svc.isAllowedByPattern("sess-1", "rm -rf /"));
}

test "isAllowedByPattern wildcard prefix and suffix" {
    var svc = try PermissionService.init(std.testing.allocator, "s");
    defer svc.deinit();
    try svc.allowPattern("sess-1", "*cat*");
    try std.testing.expect(svc.isAllowedByPattern("sess-1", "cat foo.txt"));
    try std.testing.expect(svc.isAllowedByPattern("sess-1", "scat"));
    try std.testing.expect(svc.isAllowedByPattern("sess-1", "concatenate args"));
    try std.testing.expect(!svc.isAllowedByPattern("sess-1", "ls foo.txt"));
    try std.testing.expect(!svc.isAllowedByPattern("sess-1", "bat"));
}

test "isAllowedByPattern returns false with no patterns" {
    var svc = try PermissionService.init(std.testing.allocator, "s");
    defer svc.deinit();
    try std.testing.expect(!svc.isAllowedByPattern("sess-1", "ls"));
}

test "isAllowedByPattern is per-session" {
    var svc = try PermissionService.init(std.testing.allocator, "s");
    defer svc.deinit();
    try svc.allowPattern("sess-A", "ls");
    try std.testing.expect(svc.isAllowedByPattern("sess-A", "ls"));
    try std.testing.expect(!svc.isAllowedByPattern("sess-B", "ls"));
}

test "clearSession removes whitelist" {
    var svc = try PermissionService.init(std.testing.allocator, "s");
    defer svc.deinit();
    try svc.allowPattern("sess-1", "ls*");
    try std.testing.expect(svc.isAllowedByPattern("sess-1", "ls -la"));
    svc.clearSession("sess-1");
    try std.testing.expect(!svc.isAllowedByPattern("sess-1", "ls -la"));
}

test "clearSession removes pending approvals" {
    var svc = try PermissionService.init(std.testing.allocator, "s");
    defer svc.deinit();
    const args = "{}";
    const result = try svc.requestApproval("sess-1", "run_shell", args);
    svc.clearSession("sess-1");
    // token should no longer be valid
    const ok = svc.validateToken("sess-1", &result.token_hex, "run_shell", args);
    try std.testing.expect(!ok);
}

test "clearSession on unknown session is a no-op" {
    var svc = try PermissionService.init(std.testing.allocator, "s");
    defer svc.deinit();
    svc.clearSession("nobody"); // must not crash
}

test "multiple patterns per session — any match allows" {
    var svc = try PermissionService.init(std.testing.allocator, "s");
    defer svc.deinit();
    try svc.allowPattern("sess-1", "ls*");
    try svc.allowPattern("sess-1", "cat *");
    try std.testing.expect(svc.isAllowedByPattern("sess-1", "ls -la"));
    try std.testing.expect(svc.isAllowedByPattern("sess-1", "cat README.md"));
    try std.testing.expect(!svc.isAllowedByPattern("sess-1", "rm -rf /"));
}

/// Simple glob match: pattern can contain '*' wildcards.
fn globMatch(pattern: []const u8, text: []const u8) bool {
    var p: usize = 0;
    var t: usize = 0;
    var star_p: usize = std.math.maxInt(usize);
    var star_t: usize = 0;

    while (t < text.len) {
        if (p < pattern.len and (pattern[p] == text[t] or pattern[p] == '?')) {
            p += 1;
            t += 1;
        } else if (p < pattern.len and pattern[p] == '*') {
            star_p = p;
            star_t = t;
            p += 1;
        } else if (star_p != std.math.maxInt(usize)) {
            p = star_p + 1;
            star_t += 1;
            t = star_t;
        } else {
            return false;
        }
    }
    while (p < pattern.len and pattern[p] == '*') p += 1;
    return p == pattern.len;
}
