#define MyAppName "MicYou"
#ifndef MyAppVersion
  #define MyAppVersion "2.2.1"
#endif
#define MyAppPublisher "LanRhyme"
#define MyAppURL "https://github.com/MicYou-Dev/MicYou"
#define MyAppExeName "micyou.exe"
#define MyCliExeName "micyou-cli.exe"
#define MyTuiExeName "micyou-tui.exe"

[Setup]
AppId={{C8E6D8A6-3A1B-4E38-B76B-C9DB2A0058C0}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\{#MyAppName}
DisableDirPage=no
DisableProgramGroupPage=yes
; Always install for all users into Program Files: the firewall rules and the
; Steam Streaming audio driver both need elevation, so a per-user install that
; skips them is no longer offered. Do not inherit a previous per-user path.
UsePreviousAppDir=no
PrivilegesRequired=admin
OutputBaseFilename={#MyAppName}_{#MyAppVersion}_x64-setup
OutputDir=target\release\bundle\inno
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
SetupIconFile=icons\icon.ico
UninstallDisplayIcon={app}\{#MyAppExeName}

[Languages]
Name: "chinesesimplified"; MessagesFile: "compiler:Default.isl,SimpChinese.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "..\target\release\micyou.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "binaries\micyou-cli-x86_64-pc-windows-msvc.exe"; DestDir: "{app}"; DestName: "{#MyCliExeName}"; Flags: ignoreversion
Source: "binaries\micyou-tui-x86_64-pc-windows-msvc.exe"; DestDir: "{app}"; DestName: "{#MyTuiExeName}"; Flags: ignoreversion
Source: "resources\*"; DestDir: "{app}\resources"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
; Each shortcut gets its own AppUserModelID. Sharing one AUMID across GUI/CLI/TUI
; made Windows collapse the three shortcuts into a single Start-menu search
; result / taskbar identity ("overlapping" shortcuts).
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; AppUserModelID: "com.lanrhyme.micyou"
Name: "{autoprograms}\{#MyAppName} TUI"; Filename: "{app}\{#MyTuiExeName}"; IconFilename: "{app}\{#MyAppExeName}"; AppUserModelID: "com.lanrhyme.micyou.tui"
Name: "{autoprograms}\{#MyAppName} CLI"; Filename: "{app}\{#MyCliExeName}"; IconFilename: "{app}\{#MyAppExeName}"; AppUserModelID: "com.lanrhyme.micyou.cli"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon; AppUserModelID: "com.lanrhyme.micyou"

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent

[Code]
// Windows Firewall handling, modeled after the qBittorrent installer
// (nsisFirewallW::AddAuthorizedApplication on install /
//  RemoveAuthorizedApplication on uninstall):
//   - install: add inbound allow rules for the GUI, CLI and TUI executables;
//     all three frontends can host the same server that Android devices
//     connect to. The installer always runs elevated, so this is mandatory.
//   - uninstall: remove those rules unconditionally (a delete for a
//     non-existent rule is a harmless no-op).
// Inno Setup has no built-in firewall support, so netsh advfirewall is used.
// Non-zero exit codes are ignored on purpose.

procedure ExecNetsh(const Arguments: String);
var
  ResultCode: Integer;
begin
  Exec(ExpandConstant('{sys}\netsh.exe'), Arguments, '', SW_HIDE,
    ewWaitUntilTerminated, ResultCode);
end;

procedure RemoveFirewallRules;
begin
  ExecNetsh('advfirewall firewall delete rule name="{#MyAppName}"');
  ExecNetsh('advfirewall firewall delete rule name="{#MyAppName} CLI"');
  ExecNetsh('advfirewall firewall delete rule name="{#MyAppName} TUI"');
end;

procedure AddFirewallRules;
var
  AppDir: String;
begin
  AppDir := ExpandConstant('{app}');
  // Delete first so re-installs/upgrades never leave duplicate rules behind.
  RemoveFirewallRules;
  ExecNetsh('advfirewall firewall add rule name="{#MyAppName}" dir=in action=allow enable=yes profile=any program="' + AppDir + '\{#MyAppExeName}"');
  ExecNetsh('advfirewall firewall add rule name="{#MyAppName} CLI" dir=in action=allow enable=yes profile=any program="' + AppDir + '\{#MyCliExeName}"');
  ExecNetsh('advfirewall firewall add rule name="{#MyAppName} TUI" dir=in action=allow enable=yes profile=any program="' + AppDir + '\{#MyTuiExeName}"');
end;

// Install Valve's Steam Streaming audio driver from the files bundled under
// resources\steam-audio\x64. pnputil returns 3010 when a reboot is required,
// which still means the package was staged successfully. A missing or failed
// driver package must not abort the application install.
// The installer is a 32-bit process: {sys} maps to SysWOW64, whose pnputil
// cannot install the 64-bit Steam driver. Use Sysnative for the 64-bit tools
// when it exists, and fall back for 32-bit Windows.
function NativeSystemFile(const RelativePath: String): String;
begin
  Result := ExpandConstant('{win}\Sysnative\' + RelativePath);
  if not FileExists(Result) then
    Result := ExpandConstant('{sys}\' + RelativePath);
end;

procedure MarkSteamAudioPending;
var
  PendingDir: String;
begin
  PendingDir := ExpandConstant('{userappdata}\micyou');
  ForceDirectories(PendingDir);
  SaveStringToFile(PendingDir + '\steam_audio_pending',
    'restart required to configure the Steam Streaming Microphone', False);
end;

procedure RemovePendingSteamAudioMarker;
begin
  DeleteFile(ExpandConstant('{userappdata}\micyou\steam_audio_pending'));
end;

// Per-user installs from before the all-users-only switch keep their uninstall
// entry in HKCU, so Inno cannot see the old installation from HKLM and would
// leave a second copy behind. Remove it first (silently, keeping user data).
procedure RemovePreviousPerUserInstall;
var
  UninstallString: String;
  ResultCode: Integer;
begin
  if RegQueryStringValue(HKCU,
      'Software\Microsoft\Windows\CurrentVersion\Uninstall\{C8E6D8A6-3A1B-4E38-B76B-C9DB2A0058C0}_is1',
      'UninstallString', UninstallString) then
  begin
    Log('Removing previous per-user installation: ' + UninstallString);
    Exec(RemoveQuotes(UninstallString), '/VERYSILENT /NORESTART', '', SW_HIDE,
      ewWaitUntilTerminated, ResultCode);
  end;
end;

procedure InstallSteamAudioDriver;
var
  ResultCode: Integer;
  DriverDir, MicInf, SpeakerInf: String;
  RebootRequired: Boolean;
begin
  DriverDir := ExpandConstant('{app}\resources\steam-audio\x64');
  MicInf := DriverDir + '\SteamStreamingMicrophone.inf';
  SpeakerInf := DriverDir + '\SteamStreamingSpeakers.inf';
  if not (FileExists(MicInf) and FileExists(SpeakerInf)) then
  begin
    Log('Steam Streaming audio driver files are missing; skipping driver install');
    exit;
  end;

  RebootRequired := False;
  if (not Exec(NativeSystemFile('pnputil.exe'),
      '/add-driver "' + SpeakerInf + '" /install', '', SW_HIDE,
      ewWaitUntilTerminated, ResultCode)) or
     ((ResultCode <> 0) and (ResultCode <> 3010)) then
    Log('Steam Streaming Speakers install failed with code ' + IntToStr(ResultCode))
  else if ResultCode = 3010 then
    RebootRequired := True;

  if (not Exec(NativeSystemFile('pnputil.exe'),
      '/add-driver "' + MicInf + '" /install', '', SW_HIDE,
      ewWaitUntilTerminated, ResultCode)) or
     ((ResultCode <> 0) and (ResultCode <> 3010)) then
    Log('Steam Streaming Microphone install failed with code ' + IntToStr(ResultCode))
  else if ResultCode = 3010 then
    RebootRequired := True;

  // Valve's driver defaults to an 8-channel render format and +30 dB capture
  // gain. Rendering 8 channels makes the loopback decimate the signal (a 1 kHz
  // tone arrives as 125 Hz) and the capture boost clips every recording app.
  // Set mono 48 kHz and unity capture gain, then cycle the audio endpoint
  // builder; see resources/steam-audio/configure.ps1.
  if RebootRequired then
  begin
    // The endpoints do not exist until the reboot; persist the pending state
    // so the next MicYou setup run (which sees endpoint_ready() == false)
    // executes configure.ps1 after the reboot.
    MarkSteamAudioPending;
    Log('Steam Streaming audio driver staged; endpoint configuration deferred until reboot');
  end
  else if FileExists(ExpandConstant('{app}\resources\steam-audio\configure.ps1')) then
  begin
    if (not Exec(NativeSystemFile('WindowsPowerShell\v1.0\powershell.exe'),
        '-NoProfile -ExecutionPolicy Bypass -File "' +
        ExpandConstant('{app}\resources\steam-audio\configure.ps1') + '"',
        '', SW_HIDE, ewWaitUntilTerminated, ResultCode)) or (ResultCode <> 0) then
      Log('Steam Streaming Microphone format/volume configuration failed with code ' +
        IntToStr(ResultCode))
    else
      RemovePendingSteamAudioMarker;
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssInstall then
    RemovePreviousPerUserInstall;
  if CurStep = ssPostInstall then
  begin
    // Both are mandatory: the installer requires administrator privileges for
    // exactly these system-level steps.
    InstallSteamAudioDriver;
    AddFirewallRules;
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
    RemoveFirewallRules;
  // The Steam Streaming audio driver is intentionally left in place: Steam and
  // other applications may use it, and removing a shared driver on uninstall
  // can break those applications.
end;
