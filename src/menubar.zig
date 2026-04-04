const std = @import("std");
const builtin = @import("builtin");
const objc = if (builtin.os.tag == .macos) @import("objc") else struct {};
const win = if (builtin.os.tag == .windows) std.os.windows else struct {};

const app = @import("app.zig");

pub const Menubar = struct {
    // macOS state
    var statusItem: if (builtin.os.tag == .macos) objc.Object else void = undefined;
    var menu: if (builtin.os.tag == .macos) objc.Object else void = undefined;

    pub fn run() !void {
        if (builtin.os.tag == .macos) {
            try runMacos();
        } else if (builtin.os.tag == .windows) {
            try runWindows();
        } else if (builtin.os.tag == .linux) {
            try runLinux();
        }
    }

    // ...

    fn runLinux() !void {
        const startup = @import("startup.zig");
        const allocator = std.heap.page_allocator;
        const exe_path = try std.fs.selfExePathAlloc(allocator);
        defer allocator.free(exe_path);
        const exe_dir = std.fs.path.dirname(exe_path) orelse ".";

        const helper_path = try std.fs.path.join(allocator, &.{ exe_dir, "menubar_linux.py" });
        defer allocator.free(helper_path);

        // Pass current startup state so Python can show the correct checkmark.
        const argv: []const []const u8 = if (startup.isEnabled(allocator))
            &.{ "python3", helper_path, "--startup-enabled" }
        else
            &.{ "python3", helper_path };

        var child = std.process.Child.init(argv, allocator);
        child.stdout_behavior = .Pipe;
        try child.spawn();

        var reader = child.stdout.?.reader();
        var buf: [1024]u8 = undefined;
        while (try reader.readUntilDelimiterOrEof(&buf, '\n')) |line| {
            if (std.mem.eql(u8, line, "QUIT_REQUESTED")) {
                app.initiateShutdown();
                break;
            } else if (std.mem.eql(u8, line, "STARTUP_ENABLE")) {
                startup.enable(allocator) catch {};
            } else if (std.mem.eql(u8, line, "STARTUP_DISABLE")) {
                startup.disable(allocator) catch {};
            }
        }
        _ = try child.wait();
    }

    fn runMacos() !void {
        const startup = @import("startup.zig");

        const NSApplication = objc.getClass("NSApplication").?;
        const NSStatusBar = objc.getClass("NSStatusBar").?;
        const NSMenu = objc.getClass("NSMenu").?;
        const NSMenuItem = objc.getClass("NSMenuItem").?;
        const NSString = objc.getClass("NSString").?;
        const NSObject = objc.getClass("NSObject").?;

        const sharedApp = NSApplication.msgSend(objc.Object, "sharedApplication", .{});
        _ = sharedApp.msgSend(void, "setActivationPolicy:", .{ @as(isize, 1) }); // NSApplicationActivationPolicyAccessory

        const statusBar = NSStatusBar.msgSend(objc.Object, "systemStatusBar", .{});
        statusItem = statusBar.msgSend(objc.Object, "statusItemWithLength:", .{ @as(f64, -1.0) }); // NSVariableStatusItemLength

        const button = statusItem.msgSend(objc.Object, "button", .{});
        const title = NSString.msgSend(objc.Object, "stringWithUTF8String:", .{ @as([*c]const u8, "● Poke") });
        button.msgSend(void, "setTitle:", .{ title });

        menu = NSMenu.msgSend(objc.Object, "alloc", .{}).msgSend(objc.Object, "init", .{});

        // ── Delegate for "Launch at Login" toggle ─────────────────────────────
        const delegate_class = objc.allocateClassPair(NSObject, "PokeAroundMenuDelegate").?;
        _ = delegate_class.addMethod("toggleStartup:", struct {
            fn imp(self: objc.c.id, sel: objc.c.SEL, sender: objc.c.id) callconv(.c) void {
                _ = self;
                _ = sel;
                var arena = std.heap.ArenaAllocator.init(std.heap.page_allocator);
                defer arena.deinit();
                const alloc = arena.allocator();
                const item = objc.Object{ .value = sender };
                // NSControlStateValueOn = 1, NSControlStateValueOff = 0
                const state = item.msgSend(isize, "state", .{});
                if (state == 0) {
                    startup.enable(alloc) catch {};
                    item.msgSend(void, "setState:", .{@as(isize, 1)});
                } else {
                    startup.disable(alloc) catch {};
                    item.msgSend(void, "setState:", .{@as(isize, 0)});
                }
            }
        }.imp);
        objc.registerClassPair(delegate_class);
        const delegate = delegate_class.msgSend(objc.Object, "alloc", .{})
            .msgSend(objc.Object, "init", .{});

        // "Launch at Login" menu item
        const loginTitle = NSString.msgSend(objc.Object, "stringWithUTF8String:", .{@as([*c]const u8, "Launch at Login")});
        const emptyKey = NSString.msgSend(objc.Object, "stringWithUTF8String:", .{@as([*c]const u8, "")});
        const loginItem = menu.msgSend(objc.Object, "addItemWithTitle:action:keyEquivalent:", .{
            loginTitle,
            objc.sel("toggleStartup:"),
            emptyKey,
        });
        loginItem.msgSend(void, "setTarget:", .{delegate});
        const login_state: isize = if (startup.isEnabled(std.heap.page_allocator)) 1 else 0;
        loginItem.msgSend(void, "setState:", .{login_state});

        // Separator
        const sep = NSMenuItem.msgSend(objc.Object, "separatorItem", .{});
        menu.msgSend(void, "addItem:", .{sep});

        // "Quit" item
        const quitTitle = NSString.msgSend(objc.Object, "stringWithUTF8String:", .{ @as([*c]const u8, "Quit") });
        const quitKey = NSString.msgSend(objc.Object, "stringWithUTF8String:", .{ @as([*c]const u8, "q") });
        _ = menu.msgSend(objc.Object, "addItemWithTitle:action:keyEquivalent:", .{
            quitTitle,
            objc.sel("terminate:"),
            quitKey,
        });

        statusItem.msgSend(void, "setMenu:", .{ menu });

        const DaemonFailureAlert = struct {
            fn show(err: anyerror) void {
                const NSStringLocal = objc.getClass("NSString").?;
                const NSAlertLocal = objc.getClass("NSAlert").?;
                const alert_local = NSAlertLocal.msgSend(objc.Object, "alloc", .{}).msgSend(objc.Object, "init", .{});
                const title_local = NSStringLocal.msgSend(objc.Object, "stringWithUTF8String:", .{@as([*c]const u8, @ptrCast("Poke Around failed to start"))});
                alert_local.msgSend(void, "setMessageText:", .{title_local});

                var detail_buf: [512]u8 = undefined;
                const detail_z = std.fmt.bufPrintZ(
                    &detail_buf,
                    "The background service could not start ({s}). Try poke-around --foreground in Terminal for details.",
                    .{@errorName(err)},
                ) catch {
                    const fallback = NSStringLocal.msgSend(objc.Object, "stringWithUTF8String:", .{@as([*c]const u8, @ptrCast(
                        "The background service could not start. Try poke-around --foreground in Terminal for details.",
                    ))});
                    alert_local.msgSend(void, "setInformativeText:", .{fallback});
                    _ = alert_local.msgSend(isize, "runModal", .{});
                    return;
                };
                const detail_ns = NSStringLocal.msgSend(objc.Object, "stringWithUTF8String:", .{@as([*c]const u8, @ptrCast(detail_z.ptr))});
                alert_local.msgSend(void, "setInformativeText:", .{detail_ns});
                _ = alert_local.msgSend(isize, "runModal", .{});
            }
        };

        const watchdog_class = objc.allocateClassPair(NSObject, "PokeAroundDaemonWatchdog").?;
        _ = watchdog_class.addMethod("tick:", struct {
            fn imp(_: objc.c.id, _: objc.c.SEL, timer: objc.c.id) callconv(.c) void {
                const state = app.macos_menubar_daemon_state.load(.acquire);
                if (state == 1) {
                    const timer_obj = objc.Object{ .value = timer };
                    timer_obj.msgSend(void, "invalidate", .{});
                    return;
                }
                if (state == 2) {
                    const timer_obj = objc.Object{ .value = timer };
                    timer_obj.msgSend(void, "invalidate", .{});
                    app.macos_menubar_daemon_err_mutex.lock();
                    const err = app.macos_menubar_daemon_startup_err;
                    app.macos_menubar_daemon_err_mutex.unlock();
                    DaemonFailureAlert.show(err orelse error.Unexpected);
                    const NSApplicationClass = objc.getClass("NSApplication").?;
                    const shared = NSApplicationClass.msgSend(objc.Object, "sharedApplication", .{});
                    shared.msgSend(void, "terminate:", .{@as(?*anyopaque, null)});
                }
            }
        }.imp);
        objc.registerClassPair(watchdog_class);
        const daemon_watchdog = watchdog_class.msgSend(objc.Object, "alloc", .{})
            .msgSend(objc.Object, "init", .{});

        const NSTimer = objc.getClass("NSTimer").?;
        _ = NSTimer.msgSend(objc.Object, "scheduledTimerWithTimeInterval:target:selector:userInfo:repeats:", .{
            @as(f64, 0.25),
            daemon_watchdog,
            objc.sel("tick:"),
            @as(?*anyopaque, null),
            true,
        });

        _ = sharedApp.msgSend(void, "finishLaunching", .{});
        sharedApp.msgSend(void, "run", .{});
    }

    fn runWindows() !void {
        // Basic Windows tray implementation using Shell_NotifyIconW
        const kernel32 = win.kernel32;

        const NIM_ADD = 0x00000000;
        const NIM_DELETE = 0x00000002;
        const NIF_ICON = 0x00000002;
        const NIF_MESSAGE = 0x00000001;
        const NIF_TIP = 0x00000004;
        const WM_USER = 0x0400;
        const MY_WM_TRAYICON = WM_USER + 1;

        const NOTIFYICONDATAW = extern struct {
            cbSize: u32 = @sizeOf(@This()),
            hWnd: win.HWND,
            uID: u32,
            uFlags: u32,
            uCallbackMessage: u32,
            hIcon: win.HICON,
            szTip: [128]u16,
            dwState: u32 = 0,
            dwStateMask: u32 = 0,
            szInfo: [256]u16 = [_]u16{0} ** 256,
            uTimeout: u32 = 0,
            szInfoTitle: [64]u16 = [_]u16{0} ** 64,
            dwInfoFlags: u32 = 0,
            guidItem: win.GUID = std.mem.zeroes(win.GUID),
            hBalloonIcon: win.HICON = null,
        };

        const shell32 = struct {
            pub extern "shell32" fn Shell_NotifyIconW(dwMessage: u32, lpData: *const NOTIFYICONDATAW) callconv(.stdcall) win.BOOL;
        };

        // We need a dummy window to receive messages
        const className = win.L("PokeAroundTrayClass");
        const wndClass = win.user32.WNDCLASSEXW{
            .style = 0,
            .lpfnWndProc = trayWndProc,
            .cbClsExtra = 0,
            .cbWndExtra = 0,
            .hInstance = kernel32.GetModuleHandleW(null).?,
            .hIcon = null,
            .hCursor = null,
            .hbrBackground = null,
            .lpszMenuName = null,
            .lpszClassName = className,
            .hIconSm = null,
        };
        _ = win.user32.RegisterClassExW(&wndClass);

        const hwnd = win.user32.CreateWindowExW(
            0,
            className,
            win.L("Poke Around"),
            0,
            0, 0, 0, 0,
            null,
            null,
            wndClass.hInstance,
            null,
        ) orelse return error.WindowCreationFailed;

        var nid = NOTIFYICONDATAW{
            .hWnd = hwnd,
            .uID = 1,
            .uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP,
            .uCallbackMessage = MY_WM_TRAYICON,
            .hIcon = win.user32.LoadIconW(null, @ptrFromInt(32512)), // IDI_APPLICATION
            .szTip = undefined,
        };
        const tip = win.L("Poke Around is running");
        @memcpy(nid.szTip[0..tip.len], tip);
        nid.szTip[tip.len] = 0;

        _ = shell32.Shell_NotifyIconW(NIM_ADD, &nid);
        defer _ = shell32.Shell_NotifyIconW(NIM_DELETE, &nid);

        var msg: win.user32.MSG = undefined;
        while (win.user32.GetMessageW(&msg, null, 0, 0) != 0) {
            _ = win.user32.TranslateMessage(&msg);
            _ = win.user32.DispatchMessageW(&msg);
        }
    }

    fn trayWndProc(hwnd: win.HWND, msg: u32, wParam: win.WPARAM, lParam: win.LPARAM) callconv(.stdcall) win.LRESULT {
        const WM_USER = 0x0400;
        const MY_WM_TRAYICON = WM_USER + 1;
        const WM_RBUTTONUP = 0x0205;
        const WM_DESTROY = 0x0002;
        const MF_CHECKED: u32 = 0x00000008;
        const MF_SEPARATOR: u32 = 0x00000800;

        switch (msg) {
            MY_WM_TRAYICON => {
                if (lParam == WM_RBUTTONUP) {
                    const startup = @import("startup.zig");
                    const alloc = std.heap.page_allocator;

                    const hmenu = win.user32.CreatePopupMenu() orelse return 0;
                    const startup_flags: u32 = if (startup.isEnabled(alloc)) MF_CHECKED else 0;
                    _ = win.user32.AppendMenuW(hmenu, startup_flags, 2, win.L("Launch at Login"));
                    _ = win.user32.AppendMenuW(hmenu, MF_SEPARATOR, 0, win.L(""));
                    _ = win.user32.AppendMenuW(hmenu, 0, 1, win.L("Quit"));

                    var pt: win.POINT = undefined;
                    _ = win.user32.GetCursorPos(&pt);

                    _ = win.user32.SetForegroundWindow(hwnd);
                    const id = win.user32.TrackPopupMenu(hmenu, 0x0100, pt.x, pt.y, 0, hwnd, null);
                    switch (id) {
                        1 => {
                            app.initiateShutdown();
                            win.user32.PostQuitMessage(0);
                        },
                        2 => {
                            if (startup.isEnabled(alloc)) {
                                startup.disable(alloc) catch {};
                            } else {
                                startup.enable(alloc) catch {};
                            }
                        },
                        else => {},
                    }
                    _ = win.user32.DestroyMenu(hmenu);
                }
            },
            WM_DESTROY => {
                win.user32.PostQuitMessage(0);
                return 0;
            },
            else => return win.user32.DefWindowProcW(hwnd, msg, wParam, lParam),
        }
        return 0;
    }
};
