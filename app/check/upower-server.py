# UPower, org.freedesktop.UPower, as the battery-os run's stand-in for it on Linux: on a bus of the
# run's own, which the app takes for the system bus (DBUS_SYSTEM_BUS_ADDRESS), with the display
# device core/platform/linux/upower.cc reads. It prints "ready" once it has the name, and "read"
# each time the app reads the device's properties. A line "set <percent> <state>" on stdin sets the
# battery, UPower's State number, and "rate <watts>" its EnergyRate, which the app has no need to
# read again for; each says PropertiesChanged with what changed, as UPower does, then "set" or
# "rate" again.
import os
import sys

from gi.repository import Gio, GLib

NAME = "org.freedesktop.UPower"
PATH = "/org/freedesktop/UPower/devices/DisplayDevice"
DEVICE = "org.freedesktop.UPower.Device"
INTERFACE = """<node><interface name="org.freedesktop.UPower.Device">
<property name="Type" type="u" access="read"/>
<property name="IsPresent" type="b" access="read"/>
<property name="Percentage" type="d" access="read"/>
<property name="State" type="u" access="read"/>
<property name="EnergyRate" type="d" access="read"/>
</interface></node>"""

bus = Gio.bus_get_sync(Gio.BusType.SYSTEM, None)
# A battery at 50%, discharging.
device = {
    "Type": GLib.Variant("u", 2),
    "IsPresent": GLib.Variant("b", True),
    "Percentage": GLib.Variant("d", 50.0),
    "State": GLib.Variant("u", 2),
    "EnergyRate": GLib.Variant("d", 8.0),
}


def say(line):
    print(line, flush=True)


def call(connection, sender, path, interface, method, arguments, invocation):
    invocation.return_dbus_error("org.freedesktop.DBus.Error.UnknownMethod", method)


def get(connection, sender, path, interface, name):
    # GetAll asks for each property in turn.
    if name == "Percentage":
        say("read")
    return device[name]


def changed(values):
    device.update(values)
    bus.emit_signal(None, PATH, "org.freedesktop.DBus.Properties", "PropertiesChanged",
                    GLib.Variant("(sa{sv}as)", (DEVICE, values, [])))


bus.register_object(PATH, Gio.DBusNodeInfo.new_for_xml(INTERFACE).interfaces[0], call, get, None)


pending = b""


# Reads the fd itself, as a buffered readline would keep a second line that came with the first
# and wait for more.
def command(source, condition):
    global pending
    data = os.read(sys.stdin.fileno(), 4096)
    if not data:
        loop.quit()
        return False
    pending += data
    *lines, pending = pending.split(b"\n")
    for line in lines:
        words = line.decode().split()
        if len(words) == 3 and words[0] == "set":
            changed({"Percentage": GLib.Variant("d", float(words[1])),
                     "State": GLib.Variant("u", int(words[2]))})
            say(" ".join(words))
        elif len(words) == 2 and words[0] == "rate":
            changed({"EnergyRate": GLib.Variant("d", float(words[1]))})
            say(" ".join(words))
    return True


Gio.bus_own_name_on_connection(bus, NAME, Gio.BusNameOwnerFlags.NONE,
                               lambda *_: say("ready"), lambda *_: say("lost"))
GLib.io_add_watch(sys.stdin, GLib.IO_IN | GLib.IO_HUP, command)
loop = GLib.MainLoop()
loop.run()
