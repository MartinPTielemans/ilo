#!/usr/bin/env python3
"""A HID device for the devices run (check/devices.sh), through Linux's uhid, which CI's runner
makes as root: `sudo modprobe uhid` and `sudo python3 check/uhid.py`. It's a vendor-defined one,
so nothing on the machine takes it as a keyboard or mouse, named "ilo check HID" and with a serial
number, for which Chrome keeps a site's grant. It prints its hidraw node once the kernel has made
it, lets anyone open it, and stays until killed, answering nothing the kernel asks."""

import glob
import os
import struct
import sys
import time

UHID_CREATE2 = 11
BUS_USB = 3
NAME = b"ilo check HID"

# Usage Page (Vendor 0xFF00), Usage 1, Collection (Application), 8 bytes in and out.
DESCRIPTOR = bytes.fromhex(
    "0600ff 0901 a101 1500 26ff00 7508 9508 0901 8102 0901 9102 c0".replace(" ", "")
)


def main():
    fd = os.open("/dev/uhid", os.O_RDWR | os.O_CLOEXEC)
    create = struct.pack(
        "<I128s64s64sHHIIII4096s",
        UHID_CREATE2,
        NAME,
        b"ilo-check",
        b"ilo-check-1",
        len(DESCRIPTOR),
        BUS_USB,
        0x1209,  # pid.codes' vendor id
        0x0001,  # its test product id
        1,
        0,
        DESCRIPTOR,
    )
    os.write(fd, create)
    for _ in range(100):
        for uevent in glob.glob("/sys/class/hidraw/*/device/uevent"):
            with open(uevent) as f:
                if "HID_NAME=" + NAME.decode() + "\n" in f.read():
                    node = "/dev/" + uevent.split("/")[4]
                    if os.path.exists(node):
                        os.chmod(node, 0o666)
                        print(node, flush=True)
                        break
        else:
            time.sleep(0.1)
            continue
        break
    else:
        sys.exit("uhid: the kernel made no hidraw node for the device")
    while True:
        # START, OPEN and CLOSE, which need no answer.
        os.read(fd, 4380)


main()
