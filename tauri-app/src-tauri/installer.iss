#define MyAppName "MicYou"
#ifndef MyAppVersion
  #define MyAppVersion "2.1.1"
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
UsePreviousAppDir=yes
PrivilegesRequired=admin
PrivilegesRequiredOverridesAllowed=dialog
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

[Messages]
; The firewall task is only offered in admin ("all users") install mode, so the
; install-mode selection dialog nudges users toward that option by mentioning
; the automatic Windows Firewall configuration.
chinesesimplified.PrivilegesRequiredOverrideText1=%1 可以为所有用户安装（需要管理员权限，将自动配置 Windows 防火墙），或仅为当前用户安装。
chinesesimplified.PrivilegesRequiredOverrideAllUsers=为所有用户安装（自动配置防火墙）(&A)
chinesesimplified.PrivilegesRequiredOverrideAllUsersRecommended=为所有用户安装（推荐，自动配置防火墙）(&A)
english.PrivilegesRequiredOverrideText1=%1 can be installed for all users (requires administrative privileges; Windows Firewall rules will be configured automatically), or for you only.
english.PrivilegesRequiredOverrideAllUsers=Install for &all users (configures the Windows Firewall)
english.PrivilegesRequiredOverrideAllUsersRecommended=Install for &all users (recommended; configures the Windows Firewall)
chinesesimplified.PrivilegesRequiredOverrideCurrentUser=仅为当前用户安装（需手动配置防火墙）(&U)
chinesesimplified.PrivilegesRequiredOverrideCurrentUserRecommended=仅为当前用户安装（需手动配置防火墙）(&U)
english.PrivilegesRequiredOverrideCurrentUser=Install for &you only (requires manual firewall configuration)
english.PrivilegesRequiredOverrideCurrentUserRecommended=Install for &you only (requires manual firewall configuration)

[CustomMessages]
chinesesimplified.NetworkGroupDescription=网络设置
chinesesimplified.FirewallTaskDescription=放行 Windows 防火墙（推荐，允许 MicYou 通过防火墙接收来自 Android 设备的音频连接）
english.NetworkGroupDescription=Network settings
english.FirewallTaskDescription=Add Windows Firewall rules (recommended; allow MicYou to receive audio connections from Android devices through the firewall)

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
; Checked by default, like the firewall component in the qBittorrent installer.
; Only offered in admin install mode: managing firewall rules requires
; elevation, and elevating separately at the end of a per-user install proved
; unreliable in practice. Users choosing "current user only" are informed via
; the [Messages] overrides above that the all-users mode configures the
; firewall automatically.
Name: "firewall"; Description: "{cm:FirewallTaskDescription}"; GroupDescription: "{cm:NetworkGroupDescription}"; Check: IsAdminInstallMode

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
//   - install (task "firewall", checked by default, admin install mode only):
//     add inbound allow rules for the GUI, CLI and TUI executables; all three
//     frontends can host the same server that Android devices connect to.
//   - uninstall: remove those rules unconditionally (a delete for a
//     non-existent rule is a harmless no-op).
// Per-user (non-admin) installs do not touch the firewall; the install-mode
// dialog text ([Messages] overrides) points users to the all-users option
// which configures it automatically.
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

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if (CurStep = ssPostInstall) and IsAdminInstallMode and
     WizardIsTaskSelected('firewall') then
    AddFirewallRules;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if (CurUninstallStep = usPostUninstall) and IsAdminInstallMode then
    RemoveFirewallRules;
end;
