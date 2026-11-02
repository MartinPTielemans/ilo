# UI Automation for proof.sh: the windows WinSparkle and the installer show, as a user sees them.
#   ui.ps1 dump WINDOW            every top-level window WINDOW matches, with what's in it
#   ui.ps1 click WINDOW BUTTON    presses the first enabled control whose name matches BUTTON, in a
#                                 window WINDOW matches; exits 1 if there's none. WinSparkle's
#                                 buttons are wxWidgets panes with no Invoke, so those get a click
#                                 at their centre, as the mouse would.
# WINDOW is a process id, or else a regular expression for the window's title.
param([string]$Command, [string]$Window, [string]$Button)
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$Element = [System.Windows.Automation.AutomationElement]
$Scope = [System.Windows.Automation.TreeScope]
$All = [System.Windows.Automation.Condition]::TrueCondition
Add-Type -Namespace Proof -Name Mouse -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
[DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint x, uint y, uint data, System.UIntPtr extra);
'@

function Matching {
  foreach ($w in $Element::RootElement.FindAll($Scope::Children, $All)) {
    if ($Window -match '^\d+$') {
      if ($w.Current.ProcessId -eq [int]$Window) { $w }
    } elseif ($w.Current.Name -match $Window) { $w }
  }
}

switch ($Command) {
  'dump' {
    foreach ($w in Matching) {
      "window '$($w.Current.Name)' process $($w.Current.ProcessId) class $($w.Current.ClassName)"
      foreach ($e in $w.FindAll($Scope::Descendants, $All)) {
        $type = $e.Current.ControlType.ProgrammaticName -replace '^ControlType\.', ''
        "  $type '$($e.Current.Name)'$(if (-not $e.Current.IsEnabled) { ' disabled' })"
      }
    }
  }
  'click' {
    foreach ($w in Matching) {
      foreach ($e in $w.FindAll($Scope::Descendants, $All)) {
        if ($e.Current.IsEnabled -and -not $e.Current.IsOffscreen -and $e.Current.Name -match $Button) {
          $invoke = $null
          if ($e.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern, [ref]$invoke)) {
            "pressed '$($e.Current.Name)' in '$($w.Current.Name)'"
            $invoke.Invoke()
          } else {
            $r = $e.Current.BoundingRectangle
            if ($r.IsEmpty) { continue }
            "clicked '$($e.Current.Name)' in '$($w.Current.Name)' at $([int]($r.X + $r.Width / 2)),$([int]($r.Y + $r.Height / 2))"
            [Proof.Mouse]::SetCursorPos([int]($r.X + $r.Width / 2), [int]($r.Y + $r.Height / 2)) | Out-Null
            [Proof.Mouse]::mouse_event(2, 0, 0, 0, [System.UIntPtr]::Zero)
            [Proof.Mouse]::mouse_event(4, 0, 0, 0, [System.UIntPtr]::Zero)
          }
          exit 0
        }
      }
    }
    exit 1
  }
  default { Write-Error "ui.ps1 dump|click WINDOW [BUTTON]"; exit 2 }
}
