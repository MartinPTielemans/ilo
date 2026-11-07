"""Proof for #1455 only, never merged: sets the primary display's scale to PERCENT, as Settings'
Scale does, without signing out, so the session's system DPI stays the one it signed in at and
differs from the display's. Prints the scales Windows offers, then the system's and the display's
DPI as a per-monitor-aware process reads them.
Usage: display-scale.py PERCENT"""

import ctypes
import ctypes.wintypes as w
import sys

user32 = ctypes.windll.user32
shcore = ctypes.windll.shcore

QDC_ONLY_ACTIVE_PATHS = 2
GET_DPI_SCALE = -3
SET_DPI_SCALE = -4
STEPS = [100, 125, 150, 175, 200, 225, 250, 300, 350, 400, 450, 500]


class LUID(ctypes.Structure):
    _fields_ = [("LowPart", w.DWORD), ("HighPart", w.LONG)]


class PathInfo(ctypes.Structure):
    _fields_ = [("sourceAdapterId", LUID), ("sourceId", ctypes.c_uint32),
                ("sourceModeInfoIdx", ctypes.c_uint32), ("sourceStatusFlags", ctypes.c_uint32),
                ("targetAdapterId", LUID), ("targetId", ctypes.c_uint32),
                ("targetModeInfoIdx", ctypes.c_uint32), ("outputTechnology", ctypes.c_uint32),
                ("rotation", ctypes.c_uint32), ("scaling", ctypes.c_uint32),
                ("refreshNumerator", ctypes.c_uint32), ("refreshDenominator", ctypes.c_uint32),
                ("scanLineOrdering", ctypes.c_uint32), ("targetAvailable", w.BOOL),
                ("targetStatusFlags", ctypes.c_uint32), ("flags", ctypes.c_uint32)]


class ModeInfo(ctypes.Structure):
    _fields_ = [("raw", ctypes.c_byte * 64)]


class Header(ctypes.Structure):
    _fields_ = [("type", ctypes.c_int32), ("size", ctypes.c_uint32), ("adapterId", LUID),
                ("id", ctypes.c_uint32)]


class ScaleGet(ctypes.Structure):
    _fields_ = [("header", Header), ("minScaleRel", ctypes.c_int32),
                ("curScaleRel", ctypes.c_int32), ("maxScaleRel", ctypes.c_int32)]


class ScaleSet(ctypes.Structure):
    _fields_ = [("header", Header), ("scaleRel", ctypes.c_int32)]


def primary_source():
    paths, modes = ctypes.c_uint32(), ctypes.c_uint32()
    if user32.GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, ctypes.byref(paths),
                                          ctypes.byref(modes)):
        sys.exit("GetDisplayConfigBufferSizes failed")
    path_array = (PathInfo * paths.value)()
    mode_array = (ModeInfo * modes.value)()
    if user32.QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS, ctypes.byref(paths), path_array,
                                 ctypes.byref(modes), mode_array, None):
        sys.exit("QueryDisplayConfig failed")
    print(f"display paths {paths.value}")
    return path_array[0]


def main():
    wanted = int(sys.argv[1])
    path = primary_source()
    got = ScaleGet()
    got.header = Header(GET_DPI_SCALE, ctypes.sizeof(ScaleGet), path.sourceAdapterId, path.sourceId)
    if ctypes.windll.user32.DisplayConfigGetDeviceInfo(ctypes.byref(got)):
        sys.exit("DisplayConfigGetDeviceInfo failed")
    recommended = abs(got.minScaleRel)
    offered = STEPS[: recommended + got.maxScaleRel + 1]
    print(f"scales offered {offered}, recommended {STEPS[recommended]}, "
          f"current {STEPS[recommended + got.curScaleRel]}")
    if wanted not in offered:
        sys.exit(f"{wanted}% isn't offered")
    put = ScaleSet()
    put.header = Header(SET_DPI_SCALE, ctypes.sizeof(ScaleSet), path.sourceAdapterId, path.sourceId)
    put.scaleRel = STEPS.index(wanted) - recommended
    if ctypes.windll.user32.DisplayConfigSetDeviceInfo(ctypes.byref(put)):
        sys.exit("DisplayConfigSetDeviceInfo failed")
    # Per-monitor aware, as chrome.exe is, so the display's DPI is its own, not the system's.
    user32.SetProcessDpiAwarenessContext(ctypes.c_void_p(-4))
    monitor = user32.MonitorFromPoint(w.POINT(0, 0), 1)
    x, y = ctypes.c_uint(), ctypes.c_uint()
    shcore.GetDpiForMonitor(monitor, 0, ctypes.byref(x), ctypes.byref(y))
    print(f"system dpi {user32.GetDpiForSystem()}, display dpi {x.value}")


main()
