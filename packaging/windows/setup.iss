; Urban Sprawl Windows setup (Inno Setup 6). Built by .github/workflows/installers.yml:
;   iscc /DDisplayVersion=1.0.0-preview.1 /DNumericVersion=1.0.0 /DAppImage=<dir with urban-sprawl.exe> /DOutDir=<dir> setup.iss
; One AppId for every version, so any version installs over any other in place. Saves live outside
; the install folder and this script never touches them.
; The Lakeside launcher detects the install by the uninstall key of this AppId (..._is1) and by
; Software\Lakeside Games\Urban Sprawl (InstallPath, Version). Silent install:
;   UrbanSprawl-<version>-setup.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /DIR="<path>"

#ifndef DisplayVersion
  #error Pass /DDisplayVersion=<version shown to players>
#endif
#ifndef NumericVersion
  #define NumericVersion "0.0.0"
#endif
#ifndef AppImage
  #error Pass /DAppImage=<folder holding urban-sprawl.exe>
#endif
#ifndef OutDir
  #define OutDir "."
#endif

#define AppName "Urban Sprawl"
#define ExeName "urban-sprawl.exe"

[Setup]
AppId={{B3D7A1E4-5C28-4F0B-9A6E-7D41C2E8F935}
AppName={#AppName}
AppVersion={#DisplayVersion}
AppVerName={#AppName} {#DisplayVersion}
AppPublisher=Lakeside Games
AppMutex=UrbanSprawlRunning
VersionInfoVersion={#NumericVersion}.0
VersionInfoProductName={#AppName}
VersionInfoDescription={#AppName} setup
DefaultDirName={autopf}\{#AppName}
UsePreviousAppDir=yes
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
; Per user by default (no administrator prompt); the player can choose all users.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog commandline
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayIcon={app}\{#ExeName}
WizardStyle=modern
CloseApplications=yes
CloseApplicationsFilter=*.exe,*.dll
RestartApplications=no
Compression=lzma2/ultra64
SolidCompression=yes
OutputDir={#OutDir}
OutputBaseFilename=UrbanSprawl-{#DisplayVersion}-setup

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Messages]
SetupAppRunningError=Urban Sprawl is open.%n%nClose the game, then click OK to continue. Your saves are not touched by setup.
UninstallAppRunningError=Urban Sprawl is open.%n%nClose the game, then click OK to continue the uninstall.

[Tasks]
Name: "startmenu"; Description: "Add Urban Sprawl to the &Start menu"
Name: "desktopicon"; Description: "Add a shortcut to the &desktop"

[InstallDelete]
Type: filesandordirs; Name: "{app}\assets"

[Files]
Source: "{#AppImage}\*"; DestDir: "{app}"; Flags: recursesubdirs createallsubdirs ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#ExeName}"; Tasks: startmenu
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#ExeName}"; Tasks: desktopicon

[Registry]
Root: HKA; Subkey: "Software\Lakeside Games\Urban Sprawl"; ValueType: string; ValueName: "InstallPath"; ValueData: "{app}"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Lakeside Games\Urban Sprawl"; ValueType: string; ValueName: "Version"; ValueData: "{#DisplayVersion}"
Root: HKA; Subkey: "Software\Lakeside Games\Urban Sprawl"; ValueType: string; ValueName: "Executable"; ValueData: "{app}\{#ExeName}"

[Run]
Filename: "{app}\{#ExeName}"; Description: "Play Urban Sprawl now"; Flags: nowait postinstall skipifsilent
