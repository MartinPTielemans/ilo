# The desktop's notification server, org.freedesktop.Notifications, as the notifications and mcp
# runs' stand-in for one on Linux: on the run's own session bus, where Chromium's notification
# bridge (notification_platform_bridge_linux.cc) posts a page's notifications, and the mcp run's
# app its needs' (app/src/needs_you/desktop.rs). It
# prints "ready" once it has the name, and "notify <id> <summary>" for each notification, then
# "body <id> <body>" and "hints <id> <names>", and a
# line "click <id>" on stdin sends ActionInvoked for it with the default action, as a desktop's
# server does when the user clicks one. It answers with the capabilities Chromium needs to use it.
import sys

from gi.repository import Gio, GLib

NAME = "org.freedesktop.Notifications"
PATH = "/org/freedesktop/Notifications"
INTERFACE = """<node><interface name="org.freedesktop.Notifications">
<method name="GetCapabilities"><arg type="as" direction="out"/></method>
<method name="Notify">
  <arg type="s" direction="in"/><arg type="u" direction="in"/><arg type="s" direction="in"/>
  <arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="as" direction="in"/>
  <arg type="a{sv}" direction="in"/><arg type="i" direction="in"/><arg type="u" direction="out"/>
</method>
<method name="CloseNotification"><arg type="u" direction="in"/></method>
<method name="GetServerInformation">
  <arg type="s" direction="out"/><arg type="s" direction="out"/><arg type="s" direction="out"/>
  <arg type="s" direction="out"/>
</method>
<signal name="NotificationClosed"><arg type="u"/><arg type="u"/></signal>
<signal name="ActionInvoked"><arg type="u"/><arg type="s"/></signal>
</interface></node>"""

bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
ids = iter(range(1, 1 << 31))


def say(line):
    print(line, flush=True)


def call(connection, sender, path, interface, method, arguments, invocation):
    if method == "GetCapabilities":
        invocation.return_value(GLib.Variant("(as)", (["actions", "body"],)))
    elif method == "GetServerInformation":
        invocation.return_value(GLib.Variant("(ssss)", ("check", "ilo", "1.0", "1.2")))
    elif method == "Notify":
        replaces, summary = arguments[1], arguments[3]
        notification = replaces or next(ids)
        say(f"notify {notification} {summary}")
        say(f"body {notification} {arguments[4]}")
        say(f"hints {notification} {' '.join(sorted(arguments[6].keys()))}")
        invocation.return_value(GLib.Variant("(u)", (notification,)))
    elif method == "CloseNotification":
        invocation.return_value(None)


bus.register_object(PATH, Gio.DBusNodeInfo.new_for_xml(INTERFACE).interfaces[0], call)


def command(source, condition):
    line = sys.stdin.readline()
    if not line:
        loop.quit()
        return False
    words = line.split()
    if len(words) == 2 and words[0] == "click":
        bus.emit_signal(None, PATH, NAME, "ActionInvoked",
                        GLib.Variant("(us)", (int(words[1]), "default")))
        say(f"clicked {words[1]}")
    return True


Gio.bus_own_name_on_connection(bus, NAME, Gio.BusNameOwnerFlags.NONE,
                               lambda *_: say("ready"), lambda *_: say("lost"))
GLib.io_add_watch(sys.stdin, GLib.IO_IN | GLib.IO_HUP, command)
loop = GLib.MainLoop()
loop.run()
