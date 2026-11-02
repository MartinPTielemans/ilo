; ilo's installer for winget and direct downloads, unsigned for 0.1.0: per user and without admin,
; into %LOCALAPPDATA%\Programs\ilo, with its uninstaller in the user's Apps list. ilo's updater
; (app/platform/windows/updates.cc) runs a newer one over it, silently, once ilo has quit:
; /waitpid=<ilo's process> waits for it to go, and /relaunch=1 starts ilo again after. package.sh
; compiles it with ISCC, giving each value below.
#ifndef AppVersion
  #error ISCC /DAppName=ilo /DAppPublisher=ilo /DAppVersion=0.1.0 /DSource=<staged app> /DOutput=<dir> ilo.iss
#endif
; -app-only for Browser.dll packed without an engine, which doesn't run.
#ifndef Suffix
  #define Suffix ""
#endif
; The CLSID Windows starts to hand ilo a click on one of its notifications, the engine's
; toast_activator_clsid (engine/patches/ours/chromium-windows-app-names.patch).
#define ToastActivator "BF0AC29E-6375-476D-946E-0EBEB1F03DD4"

[Setup]
; Never changes: Windows and winget know the installed app by it (ProductCode app.ilo.browser_is1).
AppId=app.ilo.browser
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher={#AppPublisher}
VersionInfoVersion={#AppVersion}
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
DefaultDirName={autopf}\{#AppName}
DisableDirPage=yes
DisableProgramGroupPage=yes
DisableReadyPage=yes
DisableWelcomePage=yes
; ilo running is closed for the update, as files in use can't be replaced.
CloseApplications=force
SetupIconFile=Assets\ilo.ico
UninstallDisplayIcon={app}\ilo.ico
UninstallDisplayName={#AppName}
WizardStyle=modern
Compression=lzma2/max
SolidCompression=yes
OutputDir={#Output}
OutputBaseFilename={#AppName}-{#AppVersion}-x64-setup{#Suffix}

[Files]
Source: "{#Source}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[InstallDelete]
; WinSparkle, the updater before ilo's own, and its licence.
Type: files; Name: "{app}\WinSparkle.dll"
Type: files; Name: "{app}\WinSparkle-COPYING.txt"

[UninstallDelete]
; What the updater downloaded.
Type: filesandordirs; Name: "{localappdata}\ilo\Updates"

[Icons]
; With the AppUserModelID Chromium gives ilo's notifications and the core its windows, so Windows
; shows the notifications, groups the windows with the shortcut pinned, and starts the CLSID's
; server for a click on a notification.
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\chrome.exe"; IconFilename: "{app}\ilo.ico"; AppUserModelID: "{code:BrowserModelId}"; AppUserModelToastActivatorCLSID: "{#ToastActivator}"

[Registry]
; So ilo can be the default browser. Per user (PrivilegesRequired=lowest), named IloHTML to match
; the ProgId host.cc's host_is_default_browser reads from the user's http/https UserChoice. Windows
; itself lets only the user set that choice, in Settings, which host_make_default_browser opens.
; --single-argument, as Chromium's installer has it, so a link Windows hands over can't carry a
; switch. Application names ilo where Settings lists what opens a type, which was the engine's
; "Chromium" without it.
Root: HKCU; Subkey: "Software\Classes\IloHTML"; ValueType: string; ValueData: "ilo HTML Document"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\IloHTML\DefaultIcon"; ValueType: string; ValueData: "{app}\ilo.ico,0"
Root: HKCU; Subkey: "Software\Classes\IloHTML\shell\open\command"; ValueType: string; ValueData: """{app}\chrome.exe"" --single-argument %1"
Root: HKCU; Subkey: "Software\Classes\IloHTML\Application"; ValueType: string; ValueName: "ApplicationName"; ValueData: "{#AppName}"
Root: HKCU; Subkey: "Software\Classes\IloHTML\Application"; ValueType: string; ValueName: "ApplicationIcon"; ValueData: "{app}\ilo.ico,0"
Root: HKCU; Subkey: "Software\Classes\IloHTML\Application"; ValueType: string; ValueName: "AppUserModelId"; ValueData: "{code:BrowserModelId}"
; And PDFs, under the engine's PDF ProgId (chromium-windows-app-names.patch). Only offered: .pdf's
; own key isn't touched, so ilo opens PDFs only once the user picks it for them.
Root: HKCU; Subkey: "Software\Classes\IloPDF"; ValueType: string; ValueData: "ilo PDF Document"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\IloPDF\DefaultIcon"; ValueType: string; ValueData: "{app}\ilo.ico,0"
Root: HKCU; Subkey: "Software\Classes\IloPDF\shell\open\command"; ValueType: string; ValueData: """{app}\chrome.exe"" --single-argument %1"
Root: HKCU; Subkey: "Software\Classes\IloPDF\Application"; ValueType: string; ValueName: "ApplicationName"; ValueData: "{#AppName}"
Root: HKCU; Subkey: "Software\Classes\IloPDF\Application"; ValueType: string; ValueName: "ApplicationIcon"; ValueData: "{app}\ilo.ico,0"
Root: HKCU; Subkey: "Software\Classes\IloPDF\Application"; ValueType: string; ValueName: "AppUserModelId"; ValueData: "{code:BrowserModelId}"
; ilo as a browser, under StartMenuInternet as Chromium's installer registers one: Settings' "Make
; ilo your default browser" takes only an app registered there. 0.1.0's capabilities under
; Software\ilo go, as that key is the engine's own.
Root: HKCU; Subkey: "Software\ilo\Capabilities"; Flags: deletekey
; WinSparkle's settings, which nothing reads now, and the updater's last check, which goes with ilo.
Root: HKCU; Subkey: "Software\ilo\WinSparkle"; Flags: deletekey
Root: HKCU; Subkey: "Software\ilo\Updates"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\{#AppName}"; ValueType: string; ValueData: "{#AppName}"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\{#AppName}\DefaultIcon"; ValueType: string; ValueData: "{app}\ilo.ico,0"
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\{#AppName}\shell\open\command"; ValueType: string; ValueData: """{app}\chrome.exe"""
; The capabilities Windows lists in Settings' Default apps, so http, https, HTML files and PDFs can
; be ilo's.
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\{#AppName}\Capabilities"; ValueType: string; ValueName: "ApplicationName"; ValueData: "{#AppName}"
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\{#AppName}\Capabilities"; ValueType: string; ValueName: "ApplicationDescription"; ValueData: "{#AppName}, an Arc-style browser"
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\{#AppName}\Capabilities"; ValueType: string; ValueName: "ApplicationIcon"; ValueData: "{app}\ilo.ico,0"
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\{#AppName}\Capabilities\StartMenu"; ValueType: string; ValueName: "StartMenuInternet"; ValueData: "{#AppName}"
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\{#AppName}\Capabilities\URLAssociations"; ValueType: string; ValueName: "http"; ValueData: "IloHTML"
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\{#AppName}\Capabilities\URLAssociations"; ValueType: string; ValueName: "https"; ValueData: "IloHTML"
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\{#AppName}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".htm"; ValueData: "IloHTML"
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\{#AppName}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".html"; ValueData: "IloHTML"
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\{#AppName}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".pdf"; ValueData: "IloPDF"
; Register those capabilities with Windows, so it shows ilo as a choice.
Root: HKCU; Subkey: "Software\RegisteredApplications"; ValueType: string; ValueName: "{#AppName}"; ValueData: "Software\Clients\StartMenuInternet\{#AppName}\Capabilities"; Flags: uninsdeletevalue
; The engine's notification helper, which Windows starts for a click on a notification and which
; hands it to ilo, as Chromium's installer registers it.
Root: HKCU; Subkey: "Software\Classes\CLSID\{{{#ToastActivator}}"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\CLSID\{{{#ToastActivator}}\LocalServer32"; ValueType: string; ValueData: """{app}\notification_helper.exe"""
Root: HKCU; Subkey: "Software\Classes\CLSID\{{{#ToastActivator}}\LocalServer32"; ValueType: string; ValueName: "ServerExecutable"; ValueData: "{app}\notification_helper.exe"

[Run]
Filename: "{app}\chrome.exe"; Description: "Open {#AppName}"; Flags: nowait postinstall skipifsilent
; Restart to Update's silent install, which the line above skips.
Filename: "{app}\chrome.exe"; Flags: nowait; Check: Relaunching

[Code]
var
  ModelId: String;

function OpenProcess(Access: Cardinal; Inherit: Integer; ProcessId: Cardinal): THandle;
  external 'OpenProcess@kernel32.dll stdcall';
function WaitForSingleObject(Handle: THandle; Milliseconds: Cardinal): Cardinal;
  external 'WaitForSingleObject@kernel32.dll stdcall';
function CloseHandle(Handle: THandle): Integer;
  external 'CloseHandle@kernel32.dll stdcall';

// An update ilo started as it quit waits for ilo's process to end, up to a minute, so ilo goes
// as it would on any quit and keeps its windows and tabs for the next launch. What's left of it
// after, such as a page's process, CloseApplications closes.
function InitializeSetup: Boolean;
var
  Process: THandle;
begin
  Process := OpenProcess($00100000 { SYNCHRONIZE }, 0,
    StrToIntDef(ExpandConstant('{param:waitpid|0}'), 0));
  if Process <> 0 then begin
    WaitForSingleObject(Process, 60000);
    CloseHandle(Process);
  end;
  Result := True;
end;

// Whether ilo asked to be started again once it's updated: Restart to Update.
function Relaunching: Boolean;
begin
  Result := ExpandConstant('{param:relaunch|0}') = '1';
end;

// ilo's AppUserModelID as Chromium makes it for a per-user install (ShellUtil::GetBrowserModelId):
// the base app ID, then a dot and the base32 of the MD5 of the user's SID. Without the SID the
// shortcut gets none, as before, and Windows shows no notification of ilo's.
function BrowserModelId(Param: String): String;
var
  Output: TExecOutput;
  ResultCode, I, Start, Buffer, Bits: Integer;
  Line, Sid, Hash: String;
begin
  if ModelId = '' then begin
    Sid := '';
    // whoami /user /fo csv /nh prints "<domain>\<user>","<SID>".
    if ExecAndCaptureOutput(ExpandConstant('{sys}\whoami.exe'), '/user /fo csv /nh', '',
        SW_SHOWNORMAL, ewWaitUntilTerminated, ResultCode, Output) and (ResultCode = 0) then
      for I := 0 to GetArrayLength(Output.StdOut) - 1 do begin
        Line := Output.StdOut[I];
        Start := Pos('"S-1-', Line);
        if Start > 0 then begin
          Sid := Copy(Line, Start + 1, Length(Line));
          Sid := Copy(Sid, 1, Pos('"', Sid) - 1);
        end;
      end;
    if Sid = '' then begin
      Log('No user SID from whoami, so the shortcut has no AppUserModelID.');
      Result := '';
      exit;
    end;
    Hash := LowerCase(GetMD5OfString(Sid));
    ModelId := '{#AppName}.';
    Buffer := 0;
    Bits := 0;
    for I := 0 to 15 do begin
      Buffer := ((Buffer shl 8) or ((Pos(Copy(Hash, I * 2 + 1, 1), '0123456789abcdef') - 1) shl 4)
        or (Pos(Copy(Hash, I * 2 + 2, 1), '0123456789abcdef') - 1)) and $FFFF;
      Bits := Bits + 8;
      while Bits >= 5 do begin
        ModelId := ModelId + Copy('ABCDEFGHIJKLMNOPQRSTUVWXYZ234567', ((Buffer shr (Bits - 5)) and 31) + 1, 1);
        Bits := Bits - 5;
      end;
    end;
    if Bits > 0 then
      ModelId := ModelId + Copy('ABCDEFGHIJKLMNOPQRSTUVWXYZ234567', ((Buffer shl (5 - Bits)) and 31) + 1, 1);
  end;
  Result := ModelId;
end;
