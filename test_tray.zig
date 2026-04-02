const std = @import("std");
const objc = @import("objc");

pub fn main() !void {
    const NSApplication = objc.getClass("NSApplication").?;
    const NSStatusBar = objc.getClass("NSStatusBar").?;
    const NSMenu = objc.getClass("NSMenu").?;
    const NSString = objc.getClass("NSString").?;

    const sharedApp = NSApplication.msgSend(objc.Object, "sharedApplication", .{});
    _ = sharedApp.msgSend(void, "setActivationPolicy:", .{ @as(isize, 1) }); // NSApplicationActivationPolicyAccessory

    const statusBar = NSStatusBar.msgSend(objc.Object, "systemStatusBar", .{});
    const statusItem = statusBar.msgSend(objc.Object, "statusItemWithLength:", .{ @as(f64, -1.0) }); // NSVariableStatusItemLength

    const button = statusItem.msgSend(objc.Object, "button", .{});
    const title = NSString.msgSend(objc.Object, "stringWithUTF8String:", .{ @as([*c]const u8, "● PokeTest") });
    button.msgSend(void, "setTitle:", .{ title });

    const menu = NSMenu.msgSend(objc.Object, "alloc", .{}).msgSend(objc.Object, "init", .{});
    const quitTitle = NSString.msgSend(objc.Object, "stringWithUTF8String:", .{ @as([*c]const u8, "Quit") });
    const quitKey = NSString.msgSend(objc.Object, "stringWithUTF8String:", .{ @as([*c]const u8, "q") });
    _ = menu.msgSend(objc.Object, "addItemWithTitle:action:keyEquivalent:", .{
        quitTitle,
        objc.sel("terminate:"),
        quitKey,
    });
    statusItem.msgSend(void, "setMenu:", .{ menu });

    // Important: keep reference
    _ = statusItem;
    _ = menu;

    std.debug.print("Running app...\n", .{});
    _ = sharedApp.msgSend(void, "finishLaunching", .{});
    sharedApp.msgSend(void, "run", .{});
}
