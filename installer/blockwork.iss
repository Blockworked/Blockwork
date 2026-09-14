#define MyAppName "Blockwork"
#define MyAppExeName "blockwork.exe"
#ifndef MyAppVersion
  #define MyAppVersion "0.0.0-dev"
#endif
#ifndef TargetTriple
  #define TargetTriple "x86_64-pc-windows-msvc"
#endif
#ifndef InstallerArch
  #define InstallerArch "x64compatible"
#endif
#ifndef OutputBaseFilename
  #define OutputBaseFilename "blockwork-windows-x86_64-setup"
#endif
#define MyAppPublisher "Blockworked"
#define MyAppURL "https://github.com/Blockworked/Blockwork"
#define CEFDir "..\target\" + TargetTriple + "\release"

[Setup]
AppId={{0F7D9CCE-3F15-4AE6-B10B-209D104AC2CC}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=commandline dialog
DisableProgramGroupPage=yes
; Lets a silent re-install (used for in-app updates, see src/updater.rs) close the running
; app via Restart Manager if it's still holding blockwork.exe open, and relaunch it afterwards.
CloseApplications=yes
RestartApplications=yes
OutputDir=..\dist
OutputBaseFilename={#OutputBaseFilename}
SetupIconFile=..\res\icons\blockwork.ico
UninstallDisplayIcon={app}\{#MyAppExeName}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed={#InstallerArch}
ArchitecturesInstallIn64BitMode={#InstallerArch}

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop shortcut"; GroupDescription: "Additional shortcuts:"; Flags: unchecked

[Files]
; Main binary
Source: "{#CEFDir}\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
; CEF runtime
Source: "{#CEFDir}\libcef.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#CEFDir}\chrome_elf.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#CEFDir}\libEGL.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#CEFDir}\libGLESv2.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#CEFDir}\vk_swiftshader.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#CEFDir}\vulkan-1.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#CEFDir}\vk_swiftshader_icd.json"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#CEFDir}\icudtl.dat"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#CEFDir}\v8_context_snapshot.bin"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#CEFDir}\chrome_100_percent.pak"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#CEFDir}\chrome_200_percent.pak"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#CEFDir}\resources.pak"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#CEFDir}\locales\*"; DestDir: "{app}\locales"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "Launch {#MyAppName}"; Flags: nowait postinstall skipifsilent
