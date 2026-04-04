#!/usr/bin/env python3
import os
import sys
import signal

try:
    import gi
    gi.require_version('Gtk', '3.0')
    try:
        gi.require_version('AppIndicator3', '0.1')
    except:
        gi.require_version('AyatanaAppIndicator3', '0.1')
    from gi.repository import Gtk as gtk
    try:
        from gi.repository import AppIndicator3 as appindicator
    except:
        from gi.repository import AyatanaAppIndicator3 as appindicator
except ImportError:
    print("Missing dependencies for Linux tray. Install python3-gi and libayatana-appindicator3-0.1")
    sys.exit(1)

APPINDICATOR_ID = 'poke-around'

startup_enabled = '--startup-enabled' in sys.argv

def main():
    indicator = appindicator.Indicator.new(APPINDICATOR_ID, 'system-run', appindicator.IndicatorCategory.SYSTEM_SERVICES)
    indicator.set_status(appindicator.IndicatorStatus.ACTIVE)
    indicator.set_menu(build_menu())

    signal.signal(signal.SIGINT, signal.SIG_DFL)
    gtk.main()

def build_menu():
    menu = gtk.Menu()

    item_status = gtk.MenuItem(label='poke-around is running')
    item_status.set_sensitive(False)
    menu.append(item_status)

    menu.append(gtk.SeparatorMenuItem())

    item_startup = gtk.CheckMenuItem(label='Launch at Login')
    item_startup.set_active(startup_enabled)
    item_startup.connect('toggled', toggle_startup)
    menu.append(item_startup)

    menu.append(gtk.SeparatorMenuItem())

    item_quit = gtk.MenuItem(label='Quit')
    item_quit.connect('activate', quit_app)
    menu.append(item_quit)

    menu.show_all()
    return menu

def toggle_startup(item):
    if item.get_active():
        print("STARTUP_ENABLE")
    else:
        print("STARTUP_DISABLE")
    sys.stdout.flush()

def quit_app(_):
    print("QUIT_REQUESTED")
    sys.stdout.flush()
    gtk.main_quit()

if __name__ == "__main__":
    main()
