const std = @import("std");
const builtin = @import("builtin");
const config = @import("config.zig");

pub const PokeClient = struct {
    allocator: std.mem.Allocator,
    token: []const u8,
    base_url: []const u8,

    pub fn init(allocator: std.mem.Allocator) !PokeClient {
        const token = try readPokeToken(allocator);
        const base_url = std.process.getEnvVarOwned(allocator, "POKE_API") catch
            try allocator.dupe(u8, "https://poke.com/api/v1");
        
        return PokeClient{
            .allocator = allocator,
            .token = token,
            .base_url = base_url,
        };
    }

    pub fn deinit(self: *PokeClient) void {
        self.allocator.free(self.token);
        self.allocator.free(self.base_url);
    }

    pub fn deleteConnection(self: *PokeClient, connection_id: []const u8) !void {
        const url = try std.fmt.allocPrint(self.allocator, "{s}/mcp/connections/{s}", .{ self.base_url, connection_id });
        defer self.allocator.free(url);

        const auth_hdr = try std.fmt.allocPrint(self.allocator, "Authorization: Bearer {s}", .{self.token});
        defer self.allocator.free(auth_hdr);

        const argv = [_][]const u8{ "curl", "-fsSL", "-X", "DELETE", "-H", auth_hdr, url };
        var child = std.process.Child.init(&argv, self.allocator);
        child.stdout_behavior = .Ignore;
        child.stderr_behavior = .Ignore;
        child.stdin_behavior = .Ignore;
        
        try child.spawn();
        _ = try child.wait();
    }

    pub fn sendWebhook(self: *PokeClient, webhook_url: []const u8, webhook_token: []const u8, message: []const u8) !void {
        const auth_hdr = try std.fmt.allocPrint(self.allocator, "Authorization: Bearer {s}", .{webhook_token});
        defer self.allocator.free(auth_hdr);

        const payload = try std.json.stringifyAlloc(self.allocator, .{ .message = message }, .{});
        defer self.allocator.free(payload);

        const argv = [_][]const u8{ "curl", "-fsSL", "-X", "POST", "-H", auth_hdr, "-H", "Content-Type: application/json", "-d", payload, webhook_url };
        var child = std.process.Child.init(&argv, self.allocator);
        child.stdout_behavior = .Ignore;
        child.stderr_behavior = .Ignore;
        
        try child.spawn();
        _ = try child.wait();
    }
};

fn readPokeToken(allocator: std.mem.Allocator) ![]u8 {
    const home = try config.getHomeDir(allocator);
    defer allocator.free(home);
    const cred_path = try std.fs.path.join(allocator, &.{ home, ".config", "poke", "credentials.json" });
    defer allocator.free(cred_path);

    const file = std.fs.openFileAbsolute(cred_path, .{}) catch return error.NoCredentials;
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
