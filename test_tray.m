#import <Cocoa/Cocoa.h>

int main() {
    @autoreleasepool {
        NSApplication *app = [NSApplication sharedApplication];
        [app setActivationPolicy:NSApplicationActivationPolicyAccessory];

        NSStatusItem *statusItem = [[NSStatusBar systemStatusBar] statusItemWithLength:NSVariableStatusItemLength];
        statusItem.button.title = @"● Poke";

        NSMenu *menu = [[NSMenu alloc] init];
        [menu addItemWithTitle:@"Quit" action:@selector(terminate:) keyEquivalent:@"q"];
        statusItem.menu = menu;

        [app run];
    }
    return 0;
}
