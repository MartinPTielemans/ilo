"""Sends a link to an installed MSIX as Windows sends the default browser's links: ShellExecuteEx on
the ProgId the package registered for the link's scheme, which activates the package, its own
chrome.exe with what its manifest passes (#1427). The ProgId is the one the package's application
lists in its Capabilities, as Default apps reads them, so the user's own default browser, which
the http and https associations name, isn't asked.
Usage: msix.py PACKAGE_FULL_NAME URL"""

import ctypes
import ctypes.wintypes as w
import sys
import winreg

sys.stdout.reconfigure(encoding="utf-8", newline="\n")
sys.stderr.reconfigure(encoding="utf-8", newline="\n")
package, url = sys.argv[1:]
scheme = url.split(":", 1)[0]
REPOSITORY = (r"Software\Classes\Local Settings\Software\Microsoft\Windows\CurrentVersion"
              r"\AppModel\Repository\Packages")


def subkeys(key):
    names = []
    while True:
        try:
            names.append(winreg.EnumKey(key, len(names)))
        except OSError:
            return names


def tree(key, path, depth):
    """The keys and values under `key`, for a failure to say what Windows registered."""
    lines = [path]
    i = 0
    while True:
        try:
            name, value, _ = winreg.EnumValue(key, i)
        except OSError:
            break
        lines.append(f"{path}: {name} = {value}")
        i += 1
    if depth:
        for name in subkeys(key):
            with winreg.OpenKey(key, name) as sub:
                lines += tree(sub, f"{path}\\{name}", depth - 1)
    return lines


def prog_id():
    with winreg.OpenKey(winreg.HKEY_CURRENT_USER, f"{REPOSITORY}\\{package}") as root:
        for application in subkeys(root):
            try:
                with winreg.OpenKey(root, rf"{application}\Capabilities\URLAssociations") as links:
                    return winreg.QueryValueEx(links, scheme)[0]
            except OSError:
                continue
        sys.exit(f"msix: {package} lists no ProgId for {scheme}:\n" + "\n".join(tree(root, package, 3)))


class SHELLEXECUTEINFOW(ctypes.Structure):
    _fields_ = [("cbSize", w.DWORD), ("fMask", w.ULONG), ("hwnd", w.HWND), ("lpVerb", w.LPCWSTR),
                ("lpFile", w.LPCWSTR), ("lpParameters", w.LPCWSTR), ("lpDirectory", w.LPCWSTR),
                ("nShow", ctypes.c_int), ("hInstApp", w.HINSTANCE), ("lpIDList", ctypes.c_void_p),
                ("lpClass", w.LPCWSTR), ("hkeyClass", w.HKEY), ("dwHotKey", w.DWORD),
                ("hIconOrMonitor", w.HANDLE), ("hProcess", w.HANDLE)]


SEE_MASK_CLASSNAME = 0x1
# Returns once the activation is handed over, as the run's process ends right after.
SEE_MASK_NOASYNC = 0x100
SEE_MASK_FLAG_NO_UI = 0x400
SW_SHOWNORMAL = 1

progid = prog_id()
# The activation's handler is a COM object, as for a click in another app.
ctypes.windll.ole32.CoInitializeEx(None, 0x2 | 0x4)
info = SHELLEXECUTEINFOW()
info.cbSize = ctypes.sizeof(info)
info.fMask = SEE_MASK_CLASSNAME | SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI
info.lpVerb = "open"
info.lpFile = url
info.lpClass = progid
info.nShow = SW_SHOWNORMAL
shell32 = ctypes.WinDLL("shell32", use_last_error=True)
shell32.ShellExecuteExW.argtypes = [ctypes.POINTER(SHELLEXECUTEINFOW)]
if not shell32.ShellExecuteExW(ctypes.byref(info)):
    sys.exit(f"msix: ShellExecuteEx on {progid} failed: {ctypes.WinError(ctypes.get_last_error())}")
print(f"msix: {url} sent to {progid}")
