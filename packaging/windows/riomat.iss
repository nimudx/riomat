; Instalador de Riomat para Windows (Inno Setup 6). Compilar:  iscc /DAppVersion=0.1.0 packaging\windows\riomat.iss
; Espera encontrar target\release\riomat.exe ya compilado. Salida: dist\Riomat-Setup-<versión>.exe
#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif

[Setup]
AppId={{6B7A1D52-4E0C-4C0B-9A57-2E6C4D5B1A10}
AppName=Riomat
AppVersion={#AppVersion}
AppPublisher=Riomat
DefaultDirName={autopf}\Riomat
DefaultGroupName=Riomat
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
OutputDir=..\..\dist
OutputBaseFilename=Riomat-Setup-{#AppVersion}
SetupIconFile=..\..\assets\icon.ico
UninstallDisplayIcon={app}\riomat.exe
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

[Languages]
Name: "spanish"; MessagesFile: "compiler:Languages\Spanish.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Files]
Source: "..\..\target\release\riomat.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\assets\icon.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Riomat"; Filename: "{app}\riomat.exe"; IconFilename: "{app}\icon.ico"
Name: "{autodesktop}\Riomat"; Filename: "{app}\riomat.exe"; IconFilename: "{app}\icon.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\riomat.exe"; Description: "{cm:LaunchProgram,Riomat}"; Flags: nowait postinstall skipifsilent
