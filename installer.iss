; ModlandSearch Windows installer - build with Inno Setup 6:
;
;   cargo build --release
;   "%LocalAppData%\Programs\Inno Setup 6\ISCC.exe" installer.iss
;
; Output: dist\ModlandSearchSetup-1.1.0.exe
;
; The app reads its settings and preview cache from %LOCALAPPDATA%\ModlandSearch,
; so the installer only ships the executables and never touches that user data.

#define MyAppName "ModlandSearch"
#define MyAppVersion "1.1.0"
#define MyPublisher "NeofastFTL"
#define MyExeGui "ModlandZoeker.exe"
#define MyExeCli "modland-cli.exe"

[Setup]
AppId={{6C1F2B84-3A7D-4E52-9B0C-2D8A5F1E7C93}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher={#MyPublisher}
DefaultDirName={localappdata}\Programs\{#MyAppName}
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
OutputDir=dist
OutputBaseFilename=ModlandSearchSetup-1.1.0
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
UninstallDisplayName={#MyAppName} {#MyAppVersion}
UninstallDisplayIcon={app}\{#MyExeGui}
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop shortcut"; GroupDescription: "Additional shortcuts:"

[Files]
Source: "target\release\{#MyExeGui}"; DestDir: "{app}"; Flags: ignoreversion
Source: "target\release\{#MyExeCli}"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "third_party\libopenmpt\bin\amd64\libopenmpt.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "third_party\libopenmpt\bin\amd64\openmpt-mpg123.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "third_party\libopenmpt\bin\amd64\openmpt-ogg.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "third_party\libopenmpt\bin\amd64\openmpt-vorbis.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "third_party\libopenmpt\bin\amd64\openmpt-zlib.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "third_party\libopenmpt\LICENSE.txt"; DestDir: "{app}\Licenses"; DestName: "libopenmpt.txt"; Flags: ignoreversion skipifsourcedoesntexist
Source: "target\release\*.dll"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "README.md"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyExeGui}"
Name: "{group}\{#MyAppName} (command line)"; Filename: "{app}\{#MyExeCli}"; Check: FileExists(ExpandConstant('{app}\{#MyExeCli}'))
Name: "{group}\Uninstall {#MyAppName}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyExeGui}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyExeGui}"; Description: "Launch {#MyAppName}"; Flags: nowait postinstall skipifsilent
