# UI Automation for proof.sh: the windows WinSparkle and the installer show, as a user sees them.
#   ui.ps1 dump WINDOW            every top-level window WINDOW matches, with what's in it
#   ui.ps1 click WINDOW BUTTON    presses the first enabled button whose name matches BUTTON, in a
#                                 window WINDOW matches; exits 1 if there's none
# WINDOW is a process id, or else a regular expression for the window's title.
param([string]$Command, [string]$Window, [string]$Button)
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$Element = [System.Windows.Automation.AutomationElement]
$Scope = [System.Windows.Automation.TreeScope]
$All = [System.Windows.Automation.Condition]::TrueCondition
$Buttons = New-Object System.Windows.Automation.PropertyCondition(
  $Element::ControlTypeProperty, [System.Windows.Automation.ControlType]::Button)

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
      foreach ($e in $w.FindAll($Scope::Descendants, $Buttons)) {
        if ($e.Current.IsEnabled -and $e.Current.Name -match $Button) {
          "pressed '$($e.Current.Name)' in '$($w.Current.Name)'"
          $e.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
          exit 0
        }
      }
    }
    exit 1
  }
  default { Write-Error "ui.ps1 dump|click WINDOW [BUTTON]"; exit 2 }
}
