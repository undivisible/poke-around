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
        // Look for the python helper alongside the executable or in src
        const allocator = std.heap.page_allocator;
        const exe_path = try std.fs.selfExePathAlloc(allocator);
        defer allocator.free(exe_path);
        const exe_dir = std.fs.path.dirname(exe_path) orelse ".";
        
        const helper_path = try std.fs.path.join(allocator, &.{ exe_dir, "menubar_linux.py" });
        defer allocator.free(helper_path);

        var child = std.process.Child.init(&.{ "python3", helper_path }, allocator);
        child.stdout_behavior = .Pipe;
        try child.spawn();

        var reader = child.stdout.?.reader();
        var buf: [1024]u8 = undefined;
        while (try reader.readUntilDelimiterOrEof(&buf, '\n')) |line| {
            if (std.mem.eql(u8, line, "QUIT_REQUESTED")) {
                app.initiateShutdown();
                break;
            }
        }
        _ = try child.wait();
    }

    fn runMacos() !void {
        const NSApplication = objc.getClass("NSApplication").?;
        const NSStatusBar = objc.getClass("NSStatusBar").?;
        const NSMenu = objc.getClass("NSMenu").?;
        const NSString = objc.getClass("NSString").?;

        const sharedApp = NSApplication.msgSend(objc.Object, "sharedApplication", .{});
        _ = sharedApp.msgSend(void, "setActivationPolicy:", .{ @as(isize, 1) }); // NSApplicationActivationPolicyAccessory

        const statusBar = NSStatusBar.msgSend(objc.Object, "systemStatusBar", .{});
        statusItem = statusBar.msgSend(objc.Object, "statusItemWithLength:", .{ @as(f64, -1.0) }); // NSVariableStatusItemLength

        const button = statusItem.msgSend(objc.Object, "button", .{});
        const title = NSString.msgSend(objc.Object, "stringWithUTF8String:", .{ @as([*c]const u8, "● Poke") });
        button.msgSend(void, "setTitle:", .{ title });

        menu = NSMenu.msgSend(objc.Object, "alloc", .{}).msgSend(objc.Object, "init", .{});
        
        const quitTitle = NSString.msgSend(objc.Object, "stringWithUTF8String:", .{ @as([*c]const u8, "Quit") });
        const quitKey = NSString.msgSend(objc.Object, "stringWithUTF8String:", .{ @as([*c]const u8, "q") });
        
        _ = menu.msgSend(objc.Object, "addItemWithTitle:action:keyEquivalent:", .{
            quitTitle,
            objc.sel("terminate:"),
            quitKey,
        });

        statusItem.msgSend(void, "setMenu:", .{ menu });
        
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

        switch (msg) {
            MY_WM_TRAYICON => {
                if (lParam == WM_RBUTTONUP) {
                    // Show a simple context menu
                    const hmenu = win.user32.CreatePopupMenu() orelse return 0;
                    _ = win.user32.AppendMenuW(hmenu, 0, 1, win.L("Quit"));
                    
                    var pt: win.POINT = undefined;
                    _ = win.user32.GetCursorPos(&pt);
                    
                    _ = win.user32.SetForegroundWindow(hwnd);
                    const id = win.user32.TrackPopupMenu(hmenu, 0x0100, pt.x, pt.y, 0, hwnd, null);
                    if (id == 1) {
                        app.initiateShutdown();
                        win.user32.PostQuitMessage(0);
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
