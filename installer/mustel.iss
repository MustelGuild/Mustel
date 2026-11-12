; Instalador do Windows do Mustel (Inno Setup 6/7).
;
; Os defines abaixo são passados pelo installer/publish-installer.ps1; os valores padrão servem
; para compilar/testar à mão. O .iss em si roda só no Windows (ISCC.exe); o teste local no Linux
; usa installer/wine-test.

#ifndef MyAppVersion
  #define MyAppVersion "1.1.0"
#endif
#ifndef MyAppVersion4
  #define MyAppVersion4 MyAppVersion + ".0"
#endif
#ifndef MyAppStage
  #define MyAppStage "..\packaging\mustel-1.1.0-x86_64-pc-windows-msvc"
#endif
#ifndef MyArch
  #define MyArch "x86_64-pc-windows-msvc"
#endif
#ifndef MyOutputDir
  #define MyOutputDir "..\packaging"
#endif

[Setup]
; O AppId é a identidade de upgrade/desinstalação; nunca mude este GUID.
AppId={{A73ECE3A-F2C1-45B5-A5F0-A3EE4D7BB91B}
AppName=Mustel
AppVersion={#MyAppVersion}
AppVerName=Mustel {#MyAppVersion}
AppPublisher=MustelGuild
AppPublisherURL=https://github.com/MustelGuild/Mustel
AppSupportURL=https://github.com/MustelGuild/Mustel/issues
AppUpdatesURL=https://github.com/MustelGuild/Mustel/releases/latest
AppCopyright=AGPL-3.0-only
AppComments=PostgreSQL query automation, database backups and server settings.
; Por usuário: %LOCALAPPDATA%\Programs\Mustel, sem UAC. O updater troca os .exe no lugar.
DefaultDirName={autopf}\Mustel
DisableDirPage=yes
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
LicenseFile={#MyAppStage}\LICENSE
OutputDir={#MyOutputDir}
OutputBaseFilename=mustel-{#MyAppVersion}-{#MyArch}-setup
SetupIconFile=mustel.ico
UninstallDisplayIcon={app}\mustel.ico
UninstallDisplayName=Mustel
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
; O teste local em Wine é win32; /DWineTest=1 omite só a checagem de arquitetura.
; O release Windows x64 não define WineTest.
#ifndef WineTest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
#endif
MinVersion=10.0
; Fecha mustel.exe/mustel-gui.exe antes de substituir; a tarefa Run reabre a GUI.
CloseApplications=yes
RestartApplications=no
ChangesEnvironment=yes
UsePreviousAppDir=yes
UsePreviousTasks=yes
UsePreviousLanguage=no
ShowLanguageDialog=auto
VersionInfoVersion={#MyAppVersion4}
VersionInfoProductVersion={#MyAppVersion}
VersionInfoCompany=MustelGuild
VersionInfoDescription=Mustel installer
VersionInfoProductName=Mustel
VersionInfoCopyright=AGPL-3.0-only

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "brazilianportuguese"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"

[CustomMessages]
english.AddToPath=Add Mustel to the user PATH
brazilianportuguese.AddToPath=Adicionar o Mustel ao PATH do usuário
english.CommandLine=Command line:
brazilianportuguese.CommandLine=Linha de comando:

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "addtopath"; Description: "{cm:AddToPath}"; GroupDescription: "{cm:CommandLine}"; Flags: checkedonce

[Files]
Source: "{#MyAppStage}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "mustel.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{userprograms}\Mustel\Mustel"; Filename: "{app}\mustel-gui.exe"; WorkingDir: "{app}"; IconFilename: "{app}\mustel.ico"; Comment: "Mustel"
Name: "{userprograms}\Mustel\{cm:UninstallProgram,Mustel}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\Mustel"; Filename: "{app}\mustel-gui.exe"; WorkingDir: "{app}"; IconFilename: "{app}\mustel.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\mustel-gui.exe"; Description: "{cm:LaunchProgram,Mustel}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
; Restos do updater no diretório de instalação; nada de dados do usuário.
Type: files; Name: "{app}\*.old"
Type: files; Name: "{app}\.mustel-update-*"
Type: files; Name: "{app}\.mustel-helper-*"

[Code]
const
  EnvironmentKey = 'Environment';

procedure AddToPath();
var
  Path, Dir: string;
begin
  if not RegQueryStringValue(HKCU, EnvironmentKey, 'Path', Path) then
    Path := '';
  Dir := ExpandConstant('{app}');
  if Pos(';' + Lowercase(Dir) + ';', ';' + Lowercase(Path) + ';') = 0 then
  begin
    if (Path <> '') and (Path[Length(Path)] <> ';') then
      Path := Path + ';';
    Path := Path + Dir;
    // REG_EXPAND_SZ preserva entradas com %VAR% que o usuário já tinha no PATH.
    RegWriteExpandStringValue(HKCU, EnvironmentKey, 'Path', Path);
  end;
end;

procedure RemoveFromPath();
var
  Path, Wrapped, Needle: string;
  P: Integer;
begin
  if not RegQueryStringValue(HKCU, EnvironmentKey, 'Path', Path) then
    exit;
  Wrapped := ';' + Path + ';';
  Needle := ';' + Lowercase(ExpandConstant('{app}')) + ';';
  P := Pos(Needle, Lowercase(Wrapped));
  if P > 0 then
  begin
    Delete(Wrapped, P, Length(Needle));
    if (Length(Wrapped) > 0) and (Wrapped[1] = ';') then
      Delete(Wrapped, 1, 1);
    if (Length(Wrapped) > 0) and (Wrapped[Length(Wrapped)] = ';') then
      Delete(Wrapped, Length(Wrapped), 1);
    RegWriteExpandStringValue(HKCU, EnvironmentKey, 'Path', Wrapped);
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if (CurStep = ssPostInstall) and WizardIsTaskSelected('addtopath') then
    AddToPath();
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
    RemoveFromPath();
end;
