# Control for #2474, on its handover branch only: a plain WinForms window, no Chromium or GPUI,
# composes with Microsoft's Japanese IME as check/keys.sh does in ilo, turns it off, and leaves it
# for the US keyboard, which froze ilo's UI thread. Logs each step to $args[0].
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
      $ja = [K]::Layout(0x0411)
      Say ("activating Japanese " + $ja.ToString("x"))
      [K]::ActivateKeyboardLayout($ja, 0) | Out-Null
      Say ("the thread's " + [K]::GetKeyboardLayout(0).ToString("x"))
    }
    2 { [K]::Key(0xF2); Say "hiragana sent" }
    3 { [K]::Key(0x4E); [K]::Key(0x49); [K]::Key(0x48); [K]::Key(0x4F); Say "niho typed" }
    4 { [K]::Key(0x0D); Say ("return sent, the field has " + $box.Text) }
    5 { [K]::Key(0x19); Say "kanji sent" }
    6 {
      $us = [K]::Layout(0x0409)
      Say ("activating US " + $us.ToString("x"))
      [K]::ActivateKeyboardLayout($us, 0) | Out-Null
      Say ("left the Japanese IME: the thread's " + [K]::GetKeyboardLayout(0).ToString("x"))
    }
    7 {
      $ko = [K]::Layout(0x0412)
      Say ("activating Korean " + $ko.ToString("x"))
      [K]::ActivateKeyboardLayout($ko, 0) | Out-Null
      Say ("the thread's " + [K]::GetKeyboardLayout(0).ToString("x"))
    }
    8 { $form.Close() }
  }
  $script:step++
})
$form.Add_Shown({ $timer.Start() })
[Windows.Forms.Application]::Run($form)
Say "done"
