; Inno Setup script for RAH. Built by scripts\package.ps1, which passes
; /DAppVersion and /DStageDir (the directory holding the files to install).

#ifndef AppVersion
  #error AppVersion is not defined; build with scripts\package.ps1
#endif
#ifndef StageDir
  #error StageDir is not defined; build with scripts\package.ps1
#endif

[Setup]
AppId={{6F0E5B1C-3B8E-4C1F-9A55-2D7A4B9C1E01}
AppName=RAH
AppVersion={#AppVersion}
AppPublisher=RAH
; Per-user install: no administrator rights, nothing outside the user profile.
PrivilegesRequired=lowest
DefaultDirName={autopf}\RAH
DefaultGroupName=RAH
DisableProgramGroupPage=yes
OutputDir={#StageDir}\..
OutputBaseFilename=rah-{#AppVersion}-setup
Compression=lzma2
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
CloseApplications=yes
UninstallDisplayName=RAH
UninstallDisplayIcon={app}\rah.ico
SetupIconFile={#StageDir}\rah.ico
WizardStyle=modern

[Tasks]
Name: "addtopath"; Description: "Add RAH to the PATH (run `rah` and `rah-desktop` from any terminal)"; Flags: checkedonce

[Files]
Source: "{#StageDir}\rah.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#StageDir}\rah-desktop.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#StageDir}\rah-web.ps1"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#StageDir}\rah.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#StageDir}\README.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\RAH Web"; Filename: "powershell.exe"; Parameters: "-NoProfile -ExecutionPolicy Bypass -File ""{app}\rah-web.ps1"""; WorkingDir: "{%USERPROFILE}"; IconFilename: "{app}\rah.ico"; Comment: "RAH Web UI in the default browser"
Name: "{group}\RAH Desktop"; Filename: "{app}\rah-desktop.exe"; WorkingDir: "{%USERPROFILE}"; IconFilename: "{app}\rah.ico"; Comment: "RAH Web UI in a native window"
Name: "{group}\Uninstall RAH"; Filename: "{uninstallexe}"

; The data directory (~\.rah: sessions, settings, rah.toml) is kept on
; uninstall unless the user says otherwise (see [Code]).

[Code]
const
  EnvKey = 'Environment';

function WebView2Installed(): Boolean;
var
  Version: String;
  Guid: String;
begin
  Guid := '{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}';
  Result :=
    RegQueryStringValue(HKLM32, 'SOFTWARE\Microsoft\EdgeUpdate\Clients\' + Guid, 'pv', Version) or
    RegQueryStringValue(HKLM64, 'SOFTWARE\Microsoft\EdgeUpdate\Clients\' + Guid, 'pv', Version) or
    RegQueryStringValue(HKCU, 'Software\Microsoft\EdgeUpdate\Clients\' + Guid, 'pv', Version);
  Result := Result and (Version <> '') and (Version <> '0.0.0.0');
end;

function PathContains(const Dir: String): Boolean;
var
  Current: String;
begin
  Result := RegQueryStringValue(HKCU, EnvKey, 'Path', Current) and
    (Pos(';' + Lowercase(Dir) + ';', ';' + Lowercase(Current) + ';') > 0);
end;

procedure AddToPath(const Dir: String);
var
  Current: String;
begin
  if PathContains(Dir) then Exit;
  if not RegQueryStringValue(HKCU, EnvKey, 'Path', Current) then Current := '';
  if (Current <> '') and (Current[Length(Current)] <> ';') then Current := Current + ';';
  RegWriteExpandStringValue(HKCU, EnvKey, 'Path', Current + Dir);
end;

procedure RemoveFromPath(const Dir: String);
var
  Current: String;
  Wrapped: String;
  At: Integer;
begin
  if not RegQueryStringValue(HKCU, EnvKey, 'Path', Current) then Exit;
  Wrapped := ';' + Current + ';';
  At := Pos(';' + Lowercase(Dir) + ';', Lowercase(Wrapped));
  if At = 0 then Exit;
  Delete(Wrapped, At, Length(Dir) + 1);
  // Strip the sentinel semicolons added above.
  Wrapped := Copy(Wrapped, 2, Length(Wrapped) - 2);
  RegWriteExpandStringValue(HKCU, EnvKey, 'Path', Wrapped);
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if (CurStep = ssPostInstall) and WizardIsTaskSelected('addtopath') then
    AddToPath(ExpandConstant('{app}'));
end;

// The data directory rah uses: RAH_HOME, else .rah in the user profile.
function DataDir(): String;
begin
  Result := GetEnv('RAH_HOME');
  if Result = '' then Result := ExpandConstant('{%USERPROFILE}\.rah');
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
  begin
    RemoveFromPath(ExpandConstant('{app}'));
    // The data is the user's: sessions, settings and rah.toml. Keep it unless
    // asked, and always keep it for a silent uninstall.
    if (not UninstallSilent()) and DirExists(DataDir()) then
      if MsgBox('Also delete your RAH data (sessions, settings and rah.toml)?' + #13#10 + DataDir(),
         mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES then
        DelTree(DataDir(), True, True, True);
  end;
end;

function InitializeSetup(): Boolean;
begin
  Result := True;
  // Not fatal: rah (the Web UI in a browser) works without it.
  if not WebView2Installed() then
    MsgBox('The WebView2 runtime was not found. "RAH Desktop" needs it; "RAH Web" works without it.' + #13#10 +
      'Install it from https://developer.microsoft.com/microsoft-edge/webview2/', mbInformation, MB_OK);
end;
