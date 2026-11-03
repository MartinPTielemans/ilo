# What the runs use from the Mac that Git Bash lacks or has only for its own processes. check.sh
# sources it on Windows.

# The ids of the Windows processes whose executable's path and command line, their backslashes as
# slashes, match $1, as pgrep -f finds them. Git Bash's ps shows a native process's executable,
# never its arguments. The path is there for the app's own process, whose command line Git Bash
# writes: after a failed run its helpers were killed, and it lived on and started new ones.
windows_procs() {
  PATTERN="$1" powershell.exe -NoProfile -NonInteractive -Command '
    Get-CimInstance Win32_Process | Where-Object {
      ("$($_.ExecutablePath) $($_.CommandLine)" -replace "\\", "/") -match $env:PATTERN
    } | ForEach-Object { $_.ProcessId }' | tr -d '\r'
}
# pgrep -f and pkill [-9 | -KILL] -f on those, ending them by id, never by image name, which would
# reach any other Chrome too.
pgrep() {
  [ "$1" = -f ] || { echo "pgrep: only -f" >&2; return 2; }
  ids=$(windows_procs "$2")
  [ -n "$ids" ] && echo "$ids"
}
pkill() {
  force=
  case "$1" in -9 | -KILL) force=//F; shift ;; esac
  [ "$1" = -f ] || { echo "pkill: only -f" >&2; return 2; }
  ids=$(windows_procs "$2")
  [ -n "$ids" ] || return 1
  for id in $ids; do
    taskkill $force //PID "$id" > /dev/null 2>&1 || true
  done
}
# Git Bash rewrites what looks like a path in a native program's environment, and took the </script>
# in a run's data: URL for one, so the page the app opened never ran its script (#872). The URLs a
# run opens are never paths.
export MSYS2_ENV_CONV_EXCL=BROWSER_URL
# It rewrites a switch's value that starts with a slash too, and took adblock's filter /check-ad.js
# for a path under its own folder, so the ad script loaded (#872). The filters are never paths.
export MSYS2_ARG_CONV_EXCL=--ad-block-filters=
# Runs the app. A native program can't read Git Bash's fifo, so cat passes the driver's commands on
# through a pipe, and exec keeps $! the app's, whose Windows id is in /proc/$!/winpid. chrome_elf
# starts Crashpad before the app adds its profile, in the folder chrome.exe's own --user-data-dir
# names, or else the prod build's, so the profile the app will use, a run's own too, goes there
# as well: the crash run's reports are then in its Crashpad folder, where the core works with them.
windows_browser() {
  exec "$app/chrome.exe" "--user-data-dir=$(cygpath -w "${BROWSER_PROFILE_DIR:-$profile_dir}")" \
    "$@" < <(cat)
}
# Runs the bridge $1 for the server $2 on the fifos $3, its stdin, and $4, its stdout, which it
# can't open itself, as windows_browser runs the app, so $! is the bridge's.
windows_bridge() {
  exec "$1" "$2" < <(cat "$3") > >(cat > "$4")
}
# Writes the image $1 as the BMP $2, as sips does on the Mac.
to_bmp() {
  IN="$(cygpath -w "$1")" OUT="$(cygpath -w "$2")" powershell.exe -NoProfile -NonInteractive \
    -Command 'Add-Type -AssemblyName System.Drawing
      $image = [System.Drawing.Image]::FromFile($env:IN)
      $image.Save($env:OUT, [System.Drawing.Imaging.ImageFormat]::Bmp)
      $image.Dispose()'
}
# Turns Windows' "Show animations in Windows" on with 1 and off with 0, in the profile, as
# Settings' switch does: the system's, so only a check VM's runs turn it.
windows_animations() {
  ON="$1" powershell.exe -NoProfile -NonInteractive -Command '
    Add-Type -Namespace Check -Name User32 -MemberDefinition @"
[DllImport("user32.dll")]
public static extern bool SystemParametersInfoW(
  uint action, uint param, System.IntPtr value, uint ini);
"@
    # SPI_SETCLIENTAREAANIMATION, its value the pointer itself, and SPIF_UPDATEINIFILE with
    # SPIF_SENDCHANGE, as Windows broadcasts WM_SETTINGCHANGE only once the profile has the value.
    if (-not [Check.User32]::SystemParametersInfoW(0x1043, 0, [System.IntPtr][int]$env:ON, 3)) {
      exit 1
    }'
}
