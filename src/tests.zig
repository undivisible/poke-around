//! Test root for `zig build test`.
//! Importing a module here causes the test runner to discover every `test`
//! block declared inside it — including nested imports within those modules.
comptime {
    _ = @import("config.zig");
    _ = @import("permission.zig");
}
