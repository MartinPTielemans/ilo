# diag(#454): types "nihongo", Space and Enter into Notepad through the Japanese IME with SendInput,
# then reads what Notepad has.
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class Probe {
  [StructLayout(LayoutKind.Sequential)] struct KEYBDINPUT { public ushort vk, scan; public uint flags, time; public IntPtr extra; }
  [StructLayout(LayoutKind.Explicit, Size = 40)] struct INPUT { [FieldOffset(0)] public uint type; [FieldOffset(8)] public KEYBDINPUT ki; }
  [DllImport("user32.dll", SetLastError = true)] static extern uint SendInput(uint count, INPUT[] inputs, int size);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr FindWindowEx(IntPtr parent, IntPtr after, string cls, string title);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern IntPtr SendMessage(IntPtr hwnd, uint msg, IntPtr w, StringBuilder l);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr LoadKeyboardLayout(string id, uint flags);
  [DllImport("user32.dll")] public static extern IntPtr PostMessage(IntPtr hwnd, uint msg, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern int GetKeyboardLayoutList(int n, IntPtr[] list);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, IntPtr pid);
  [DllImport("user32.dll")] public static extern IntPtr GetKeyboardLayout(uint thread);
  public static uint Key(ushort vk, bool up) {
    var i = new INPUT[1];
    i[0].type = 1; i[0].ki.vk = vk; i[0].ki.flags = up ? 2u : 0u;
    return SendInput(1, i, Marshal.SizeOf(typeof(INPUT)));
  }
  public static void Press(ushort vk) { Key(vk, false); Key(vk, true); System.Threading.Thread.Sleep(60); }
  public static string Text(IntPtr edit) {
    var s = new StringBuilder(1024);
    SendMessage(edit, 0x000D, (IntPtr)1024, s);
    return s.ToString();
  }
}
"@
$layouts = New-Object IntPtr[] 16
$n = [Probe]::GetKeyboardLayoutList(16, $layouts)
"layouts: " + (($layouts[0..($n-1)] | ForEach-Object { "{0:x}" -f $_.ToInt64() }) -join " ")
$ja = [Probe]::LoadKeyboardLayout("00000411", 1)
"LoadKeyboardLayout 0411: {0:x}" -f $ja.ToInt64()
$notepad = Start-Process notepad -PassThru
Start-Sleep 2
$window = $notepad.MainWindowHandle
$edit = [Probe]::FindWindowEx($window, [IntPtr]::Zero, "Edit", $null)
"notepad $window edit $edit"
[Probe]::SetForegroundWindow($window) | Out-Null
Start-Sleep 1
"foreground is notepad: " + ([Probe]::GetForegroundWindow() -eq $window)
# WM_INPUTLANGCHANGEREQUEST to the Japanese layout, as the language bar would.
[Probe]::PostMessage($window, 0x0050, [IntPtr]::Zero, $ja) | Out-Null
Start-Sleep 1
$thread = [Probe]::GetWindowThreadProcessId($window, [IntPtr]::Zero)
"notepad's layout: {0:x}" -f [Probe]::GetKeyboardLayout($thread).ToInt64()
function Try-Type($label, $toggle) {
  if ($toggle) { & $toggle; Start-Sleep -Milliseconds 500 }
  foreach ($c in "NIHONGO".ToCharArray()) { [Probe]::Press([uint16][char]$c) }
  $composing = [Probe]::Text($edit)
  [Probe]::Press(0x20)
  Start-Sleep -Milliseconds 500
  $converted = [Probe]::Text($edit)
  [Probe]::Press(0x0D)
  Start-Sleep -Milliseconds 500
  $committed = [Probe]::Text($edit)
  "$label`: composing [$composing] converted [$converted] committed [$committed] codes " +
    (($committed.ToCharArray() | ForEach-Object { "{0:x4}" -f [int]$_ }) -join " ")
}
Try-Type "as is" $null
# VK_KANJI, which turns the IME on with a US keyboard (Alt+`).
Try-Type "after VK_KANJI" { [Probe]::Press(0x19) }
# Alt+` itself.
Try-Type "after Alt+backquote" { [Probe]::Key(0x12, $false); [Probe]::Press(0xC0); [Probe]::Key(0x12, $true) }
# VK_DBE_HIRAGANA.
Try-Type "after VK_DBE_HIRAGANA" { [Probe]::Press(0xF2) }
