const std = @import("std");
const builtin = @import("builtin");

/// Returns the poke-around config directory (caller must free).
pub fn getConfigDir(allocator: std.mem.Allocator) ![]u8 {
    if (builtin.os.tag == .windows) {
        if (std.process.getEnvVarOwned(allocator, "APPDATA")) |appdata| {
            defer allocator.free(appdata);
            return std.fs.path.join(allocator, &.{ appdata, "poke-around" });
        } else |_| {}
        const home = try getHomeDir(allocator);
        defer allocator.free(home);
        return std.fs.path.join(allocator, &.{ home, "AppData", "Roaming", "poke-around" });
    }
    if (std.process.getEnvVarOwned(allocator, "XDG_CONFIG_HOME")) |xdg| {
        defer allocator.free(xdg);
        return std.fs.path.join(allocator, &.{ xdg, "poke-around" });
    } else |_| {}
    const home = try getHomeDir(allocator);
    defer allocator.free(home);
    return std.fs.path.join(allocator, &.{ home, ".config", "poke-around" });
}

/// Returns the user home directory (caller must free).
pub fn getHomeDir(allocator: std.mem.Allocator) ![]u8 {
    if (builtin.os.tag == .windows) {
        if (std.process.getEnvVarOwned(allocator, "USERPROFILE")) |p| return p else |_| {}
        if (std.process.getEnvVarOwned(allocator, "HOMEDRIVE")) |d| {
            defer allocator.free(d);
            const path_env = std.process.getEnvVarOwned(allocator, "HOMEPATH") catch
                try allocator.dupe(u8, "\\Users\\Default");
            defer allocator.free(path_env);
            return std.mem.concat(allocator, u8, &.{ d, path_env });
        } else |_| {}
        return allocator.dupe(u8, "C:\\Users\\Default");
    }
    if (std.process.getEnvVarOwned(allocator, "HOME")) |h| return h else |_| {}
    return allocator.dupe(u8, "/tmp");
}

/// Returns agents directory path (caller must free).
pub fn getAgentsDir(allocator: std.mem.Allocator) ![]u8 {
    const cfg = try getConfigDir(allocator);
    defer allocator.free(cfg);
    return std.fs.path.join(allocator, &.{ cfg, "agents" });
}

/// Ensures the config directory exists; returns its path (caller must free).
pub fn ensureConfigDir(allocator: std.mem.Allocator) ![]u8 {
    const dir = try getConfigDir(allocator);
    std.fs.makeDirAbsolute(dir) catch |err| switch (err) {
        error.PathAlreadyExists => {},
        else => return err,
    };
    return dir;
}

/// Ensures agents directory exists; returns its path (caller must free).
pub fn ensureAgentsDir(allocator: std.mem.Allocator) ![]u8 {
    const cfg = try ensureConfigDir(allocator);
    defer allocator.free(cfg);
    const agents = try std.fs.path.join(allocator, &.{ cfg, "agents" });
    std.fs.makeDirAbsolute(agents) catch |err| switch (err) {
        error.PathAlreadyExists => {},
        else => return err,
    };
    return agents;
}

/// Reads state.json as a raw JSON string (caller must free). Returns "{}" if missing.
pub fn readStateJson(allocator: std.mem.Allocator) ![]u8 {
    const cfg = try getConfigDir(allocator);
    defer allocator.free(cfg);
    const path = try std.fs.path.join(allocator, &.{ cfg, "state.json" });
    defer allocator.free(path);

    const file = std.fs.openFileAbsolute(path, .{}) catch |err| switch (err) {
        error.FileNotFound => return allocator.dupe(u8, "{}"),
        else => return err,
    };
    defer file.close();
    return file.readToEndAlloc(allocator, 64 * 1024);
}

/// Writes raw JSON string to state.json.
pub fn writeStateJson(allocator: std.mem.Allocator, json: []const u8) !void {
    const cfg = try ensureConfigDir(allocator);
    defer allocator.free(cfg);
    const path = try std.fs.path.join(allocator, &.{ cfg, "state.json" });
    defer allocator.free(path);
    const file = try std.fs.createFileAbsolute(path, .{});
    defer file.close();
    try file.writeAll(json);
}

/// Reads a specific string field from state.json (caller must free).
/// Returns null if the field doesn't exist or isn't a string.
pub fn readStateField(allocator: std.mem.Allocator, field: []const u8) !?[]u8 {
    const raw = try readStateJson(allocator);
    defer allocator.free(raw);

    const parsed = std.json.parseFromSlice(std.json.Value, allocator, raw, .{}) catch return null;
    defer parsed.deinit();

    const obj = switch (parsed.value) {
        .object => |o| o,
        else => return null,
    };
    const val = obj.get(field) orelse return null;
    const s = switch (val) {
        .string => |s| s,
        else => return null,
    };
    return allocator.dupe(u8, s);
}

/// Sets a string field in state.json, preserving other fields.
pub fn setStateField(allocator: std.mem.Allocator, field: []const u8, value: []const u8) !void {
    const raw = try readStateJson(allocator);
    defer allocator.free(raw);

    var parsed = std.json.parseFromSlice(std.json.Value, allocator, raw, .{}) catch {
        const fresh = try std.fmt.allocPrint(allocator, "{{\"{s}\":\"{s}\"}}", .{ field, value });
        defer allocator.free(fresh);
        return writeStateJson(allocator, fresh);
    };
    defer parsed.deinit();

    var obj = switch (parsed.value) {
        .object => |o| o,
        else => std.json.ObjectMap.init(allocator),
    };
    try obj.put(field, std.json.Value{ .string = value });

    const json = try std.json.Stringify.valueAlloc(allocator, std.json.Value{ .object = obj }, .{});
    defer allocator.free(json);
    try writeStateJson(allocator, json);
}

/// Reads config.json; returns "{}" if missing (caller must free).
pub fn readConfigJson(allocator: std.mem.Allocator) ![]u8 {
    const cfg = try getConfigDir(allocator);
    defer allocator.free(cfg);
    const path = try std.fs.path.join(allocator, &.{ cfg, "config.json" });
    defer allocator.free(path);

    const file = std.fs.openFileAbsolute(path, .{}) catch |err| switch (err) {
        error.FileNotFound => return allocator.dupe(u8, "{}"),
        else => return err,
    };
    defer file.close();
    return file.readToEndAlloc(allocator, 64 * 1024);
}

/// Reads the saved permission mode from config.json ("full", "limited", or "sandbox").
pub fn readPermissionMode(allocator: std.mem.Allocator) ![]u8 {
    const raw = try readConfigJson(allocator);
    defer allocator.free(raw);

    const parsed = std.json.parseFromSlice(std.json.Value, allocator, raw, .{}) catch
        return allocator.dupe(u8, "full");
    defer parsed.deinit();

    const obj = switch (parsed.value) {
        .object => |o| o,
        else => return allocator.dupe(u8, "full"),
    };
    const val = obj.get("permissionMode") orelse return allocator.dupe(u8, "full");
    const s = switch (val) {
        .string => |s| s,
        else => return allocator.dupe(u8, "full"),
    };
    if (std.mem.eql(u8, s, "limited") or std.mem.eql(u8, s, "sandbox")) {
        return allocator.dupe(u8, s);
    }
    return allocator.dupe(u8, "full");
}

/// Saves the permission mode to config.json.
pub fn savePermissionMode(allocator: std.mem.Allocator, mode: []const u8) !void {
    const cfg = try ensureConfigDir(allocator);
    defer allocator.free(cfg);
    const path = try std.fs.path.join(allocator, &.{ cfg, "config.json" });
    defer allocator.free(path);

    var obj = std.json.ObjectMap.init(allocator);
    defer obj.deinit();
    try obj.put("permissionMode", std.json.Value{ .string = mode });

    const json = try std.json.Stringify.valueAlloc(allocator, std.json.Value{ .object = obj }, .{});
    defer allocator.free(json);

    const file = try std.fs.createFileAbsolute(path, .{});
    defer file.close();
    try file.writeAll(json);
}
