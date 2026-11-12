# Control for #2474, on its handover branch only: a plain WinForms window, no Chromium or GPUI,
# composes with Microsoft's Korean IME, turns it off, and leaves it for the
# Japanese IME: leaving the Japanese IME froze a plain window's thread too (run 43139097668). Logs each step to $args[0].
$log = $args[0]
function Say($message) { Add-Content -Path $log -Value $message }
Add-Type -AssemblyName System.Windows.Forms
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class K {
  [DllImport("user32.dll")] public static extern IntPtr ActivateKeyboardLayout(IntPtr hkl, uint flags);
  [DllImport("user32.dll")] public static extern IntPtr GetKeyboardLayout(uint thread);
  [DllImport("user32.dll")] public static extern int GetKeyboardLayoutList(int count, [Out] IntPtr[] list);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr window);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [StructLayout(LayoutKind.Sequential)]
  public struct KEYBDINPUT { public ushort wVk; public ushort wScan; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }
  [StructLayout(LayoutKind.Explicit, Size = 40)]
  public struct INPUT { [FieldOffset(0)] public uint type; [FieldOffset(8)] public KEYBDINPUT ki; }
  [DllImport("user32.dll")] public static extern uint SendInput(uint count, INPUT[] inputs, int size);
  public static void Key(ushort vk) {
    var inputs = new INPUT[2];
    inputs[0].type = 1; inputs[0].ki.wVk = vk;
    inputs[1].type = 1; inputs[1].ki.wVk = vk; inputs[1].ki.dwFlags = 2;
    SendInput(2, inputs, Marshal.SizeOf(typeof(INPUT)));
  }
  public static IntPtr Layout(int language) {
    var list = new IntPtr[32];
    int count = GetKeyboardLayoutList(32, list);
    for (int i = 0; i < count; i++) if (((long)list[i] & 0xffff) == language) return list[i];
    return IntPtr.Zero;
  }
}
'@
$form = New-Object Windows.Forms.Form
$form.Text = "IME control"
$box = New-Object Windows.Forms.TextBox
$box.Dock = "Fill"
$form.Controls.Add($box)
$script:step = 0
$timer = New-Object Windows.Forms.Timer
$timer.Interval = 500
$timer.Add_Tick({
  switch ($script:step) {
    0 {
      [K]::SetForegroundWindow($form.Handle) | Out-Null
      $box.Focus() | Out-Null
      Say ("in front: " + ([K]::GetForegroundWindow() -eq $form.Handle))
    }
    1 {
      $ko = [K]::Layout(0x0412)
      Say ("activating Korean " + $ko.ToString("x"))
      [K]::ActivateKeyboardLayout($ko, 0) | Out-Null
      Say ("the thread's " + [K]::GetKeyboardLayout(0).ToString("x"))
    }
    2 { [K]::Key(0x15); Say "hangul sent" }
    3 { [K]::Key(0x47); [K]::Key(0x4B); [K]::Key(0x53); [K]::Key(0x52); Say "gksr typed" }
    4 { [K]::Key(0x0D); Say ("return sent, the field has " + $box.Text) }
    5 { [K]::Key(0x15); Say "hangul sent again" }
    6 {
      $ja = [K]::Layout(0x0411)
      Say ("activating Japanese " + $ja.ToString("x"))
      [K]::ActivateKeyboardLayout($ja, 0) | Out-Null
      Say ("left the Korean IME: the thread's " + [K]::GetKeyboardLayout(0).ToString("x"))
    }
    7 { Say ("the field has " + $box.Text) }
    8 { $form.Close() }
  }
  $script:step++
})
$form.Add_Shown({ $timer.Start() })
[Windows.Forms.Application]::Run($form)
Say "done"
