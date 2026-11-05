; Omen Hub++ setup (Inno Setup 6.3 or newer). Built by scripts/build-release.ps1:
;   ISCC.exe /DAppVersion=<version> /DStageDir=<package staging> /O<output> OmenHubPlusPlus.iss
; The protected installation, logon tasks and fan service are created by the
; same reviewed scripts as a manual install; this setup only stages them,
; runs them elevated and reports their errors. PawnIO is never bundled: when
; chosen, the official signed 2.2.0 setup is downloaded and SHA-256 checked,
; and its own window asks for confirmation.

#ifndef AppVersion
  #error Pass /DAppVersion=<version>
#endif
#ifndef StageDir
  #error Pass /DStageDir=<package staging directory>
#endif

#define AppName "Omen Hub++"
#define AppExe "Omen Hub++.exe"
#define PawnIOVersion "2.2.0"
#define PawnIOUrl "https://github.com/namazso/PawnIO.Setup/releases/download/2.2.0/PawnIO_setup.exe"
#define PawnIOSha256 "1f519a22e47187f70a1379a48ca604981c4fcf694f4e65b734aaa74a9fba3032"

[Setup]
AppId={{6F3B6D2A-0C47-4E0B-9C1D-5A7E2B9F4C31}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=Omen Hub++ contributors
AppPublisherURL=https://github.com/Ishtvr/Omen-Hub-debloat
AppSupportURL=https://github.com/Ishtvr/Omen-Hub-debloat/issues
VersionInfoVersion=0.9.0.0
VersionInfoProductName={#AppName}
VersionInfoProductTextVersion={#AppVersion}
DefaultDirName={autopf}\OmenHubPlusPlus
DisableDirPage=yes
DisableProgramGroupPage=yes
DisableReadyMemo=no
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.19041
WizardStyle=modern
WizardSizePercent=110
SetupIconFile=..\crates\app\assets\omen-hub-pp.ico
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppName}
LicenseFile=..\LICENSE
OutputBaseFilename=OmenHubPlusPlus-{#AppVersion}-Setup
Compression=lzma2/max
SolidCompression=yes
CloseApplications=no
RestartApplications=no
ShowLanguageDialog=auto
UsePreviousTasks=yes

[Languages]
Name: "en"; MessagesFile: "compiler:Default.isl"
Name: "de"; MessagesFile: "compiler:Languages\German.isl"

[CustomMessages]
en.TaskDesktop=Create a &desktop shortcut
de.TaskDesktop=&Desktop-Verknüpfung erstellen
en.TaskTray=&Start with Windows (in the tray)
de.TaskTray=Mit &Windows starten (im Tray)
en.TaskFan=Set up the &fan service (fan curve, Max fan and CCD selection)
de.TaskFan=&Lüfterdienst einrichten (Lüfterkurve, Max-Lüfter und CCD-Auswahl)
en.TaskPawnIO=Install the &PawnIO {#PawnIOVersion} driver (CPU power, temperature and tuning); downloaded from the official release
de.TaskPawnIO=&PawnIO-{#PawnIOVersion}-Treiber installieren (CPU-Leistung, -Temperatur und -Tuning); wird vom offiziellen Release geladen
en.GroupOptional=Optional components:
de.GroupOptional=Optionale Komponenten:
en.StepApp=Installing Omen Hub++ into the protected program folder...
de.StepApp=Omen Hub++ wird in den geschützten Programmordner installiert...
en.StepStartup=Registering the sign-in task for presets...
de.StepStartup=Anmeldeaufgabe für Presets wird eingerichtet...
en.StepFan=Setting up the fan service...
de.StepFan=Lüfterdienst wird eingerichtet...
en.StepTray=Setting up the start with Windows...
de.StepTray=Start mit Windows wird eingerichtet...
en.StepPawnIO=The PawnIO setup is open. Please confirm the installation there.
de.StepPawnIO=Das PawnIO-Setup ist geöffnet. Bitte die Installation dort bestätigen.
en.StepFailed=Step failed (exit code %1): %2%n%nDetails: %3
de.StepFailed=Schritt fehlgeschlagen (Exitcode %1): %2%n%nDetails: %3
en.PawnIOFailed=The PawnIO setup ended with exit code %1. Omen Hub++ is installed; CPU values stay unavailable until PawnIO {#PawnIOVersion} is installed.
de.PawnIOFailed=Das PawnIO-Setup endete mit Exitcode %1. Omen Hub++ ist installiert; CPU-Werte bleiben nicht verfügbar, bis PawnIO {#PawnIOVersion} installiert ist.
en.PawnIOReboot=PawnIO asks for a restart before CPU values are available.
de.PawnIOReboot=PawnIO benötigt einen Neustart, bevor CPU-Werte verfügbar sind.
en.DeleteData=Also delete your presets, fan curves and settings (%1)?%n%nChoose No to keep them for a later installation.
de.DeleteData=Auch Presets, Lüfterkurven und Einstellungen löschen (%1)?%n%nNein behält sie für eine spätere Installation.
en.PawnIOKept=PawnIO stays installed; it can be removed in Windows Settings > Apps.
de.PawnIOKept=PawnIO bleibt installiert; es lässt sich unter Windows-Einstellungen > Apps entfernen.

[Tasks]
Name: "desktopicon"; Description: "{cm:TaskDesktop}"; GroupDescription: "{cm:GroupOptional}"
Name: "tray"; Description: "{cm:TaskTray}"; GroupDescription: "{cm:GroupOptional}"
Name: "fanservice"; Description: "{cm:TaskFan}"; GroupDescription: "{cm:GroupOptional}"
Name: "pawnio"; Description: "{cm:TaskPawnIO}"; GroupDescription: "{cm:GroupOptional}"; Check: NeedsPawnIO

[Files]
; Staged payload. The scripts copy the executable into the protected layout.
Source: "{#StageDir}\{#AppExe}"; DestDir: "{app}\setup"; Flags: ignoreversion
Source: "{#StageDir}\install-app.ps1"; DestDir: "{app}\setup"; Flags: ignoreversion
Source: "{#StageDir}\install-startup.ps1"; DestDir: "{app}\setup"; Flags: ignoreversion
Source: "{#StageDir}\install-fan-service.ps1"; DestDir: "{app}\setup"; Flags: ignoreversion
Source: "{#StageDir}\LICENSE"; DestDir: "{app}\docs"; Flags: ignoreversion
Source: "{#StageDir}\THIRD_PARTY.md"; DestDir: "{app}\docs"; Flags: ignoreversion
Source: "{#StageDir}\RECOVERY.md"; DestDir: "{app}\docs"; Flags: ignoreversion
Source: "{#StageDir}\HARDWARE_MATRIX.md"; DestDir: "{app}\docs"; Flags: ignoreversion
Source: "{#StageDir}\RELEASE_NOTES.md"; DestDir: "{app}\docs"; Flags: ignoreversion skipifsourcedoesntexist
Source: "{#StageDir}\third-party\*"; DestDir: "{app}\docs\third-party"; Flags: ignoreversion recursesubdirs createallsubdirs skipifsourcedoesntexist

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: postinstall nowait skipifsilent

[Code]
var
  DownloadPage: TDownloadWizardPage;
  PawnIOResult: Integer;

function PawnIOVersionInstalled: String;
begin
  if not RegQueryStringValue(HKLM64, 'SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\PawnIO',
    'DisplayVersion', Result) then
    Result := '';
end;

{ Offered (and preselected) only when PawnIO {#PawnIOVersion} is missing. }
function NeedsPawnIO: Boolean;
begin
  Result := Copy(PawnIOVersionInstalled, 1, Length('{#PawnIOVersion}')) <> '{#PawnIOVersion}';
end;

function OnDownloadProgress(const Url, FileName: String; const Progress, ProgressMax: Int64): Boolean;
begin
  Result := True;
end;

procedure InitializeWizard;
begin
  DownloadPage := CreateDownloadPage(SetupMessage(msgWizardPreparing),
    SetupMessage(msgPreparingDesc), @OnDownloadProgress);
end;

function NextButtonClick(CurPageID: Integer): Boolean;
begin
  Result := True;
  if (CurPageID = wpReady) and WizardIsTaskSelected('pawnio') then
  begin
    DownloadPage.Clear;
    DownloadPage.Add('{#PawnIOUrl}', 'PawnIO_setup.exe', '{#PawnIOSha256}');
    DownloadPage.Show;
    try
      try
        DownloadPage.Download;
      except
        if not DownloadPage.AbortedByUser then
          SuppressibleMsgBox(AddPeriod(GetExceptionMessage), mbCriticalError, MB_OK, IDOK);
        Result := False;
      end;
    finally
      DownloadPage.Hide;
    end;
  end;
end;

function LogPath: String;
begin
  Result := ExpandConstant('{app}\setup\setup-log.txt');
end;

function PowerShell: String;
begin
  Result := ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe');
end;

{ Runs one step hidden, appending its output to the setup log. }
function RunLogged(const Exe, Arguments: String; var Code: Integer): Boolean;
begin
  Result := Exec(ExpandConstant('{cmd}'),
    '/s /c ""' + Exe + '" ' + Arguments + ' >> "' + LogPath + '" 2>&1"',
    ExpandConstant('{app}\setup'), SW_HIDE, ewWaitUntilTerminated, Code);
end;

procedure Step(const Status, Exe, Arguments: String);
var
  Code: Integer;
begin
  WizardForm.StatusLabel.Caption := Status;
  if not RunLogged(Exe, Arguments, Code) or (Code <> 0) then
    RaiseException(FmtMessage(CustomMessage('StepFailed'), [IntToStr(Code), Status, LogPath]));
end;

function Script(const Name, Arguments: String): String;
begin
  Result := '-NoProfile -ExecutionPolicy Bypass -File "' + ExpandConstant('{app}\setup\') + Name + '" ' + Arguments;
end;

procedure StopRunningApp;
var
  Code: Integer;
begin
  { The dashboard and its helpers share one image name. The fan service is
    stopped gracefully by install-fan-service.ps1 itself. }
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /T /IM "{#AppExe}"', '', SW_HIDE,
    ewWaitUntilTerminated, Code);
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  StopRunningApp;
  Result := '';
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  Source, Desktop: String;
  Code: Integer;
begin
  if CurStep <> ssPostInstall then
    exit;
  if WizardIsTaskSelected('pawnio') then
  begin
    WizardForm.StatusLabel.Caption := CustomMessage('StepPawnIO');
    if not Exec(ExpandConstant('{tmp}\PawnIO_setup.exe'), '', '', SW_SHOWNORMAL,
      ewWaitUntilTerminated, PawnIOResult) then
      PawnIOResult := -1;
    if PawnIOResult = 3010 then
      SuppressibleMsgBox(CustomMessage('PawnIOReboot'), mbInformation, MB_OK, IDOK)
    else if PawnIOResult <> 0 then
      SuppressibleMsgBox(FmtMessage(CustomMessage('PawnIOFailed'), [IntToStr(PawnIOResult)]),
        mbError, MB_OK, IDOK);
  end;
  Source := '-SourceBinary "' + ExpandConstant('{app}\setup\{#AppExe}') + '"';
  Desktop := '';
  if not WizardIsTaskSelected('desktopicon') then
    Desktop := ' -NoDesktopShortcut';
  Step(CustomMessage('StepApp'), PowerShell, Script('install-app.ps1', Source + Desktop));
  Step(CustomMessage('StepStartup'), PowerShell, Script('install-startup.ps1', Source));
  if WizardIsTaskSelected('fanservice') then
    Step(CustomMessage('StepFan'), PowerShell, Script('install-fan-service.ps1', Source));
  WizardForm.StatusLabel.Caption := CustomMessage('StepTray');
  if WizardIsTaskSelected('tray') then
    Step(CustomMessage('StepTray'), ExpandConstant('{app}\{#AppExe}'), 'tray-task-set tray')
  else
    { An existing "Start with Windows" entry (set in the app) moves to the
      installed executable; none is created. }
    RunLogged(ExpandConstant('{app}\{#AppExe}'), 'tray-task-set keep', Code);
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Code: Integer;
  Data: String;
begin
  if CurUninstallStep = usUninstall then
  begin
    StopRunningApp;
    { Return HPOmenCap only if Omen Hub++ disabled it, then remove the tasks
      and the protected files through the same scripts (fan service first:
      it stops gracefully and releases CCD affinities). }
    RunLogged(ExpandConstant('{app}\{#AppExe}'), 'hp-service-restore', Code);
    RunLogged(ExpandConstant('{app}\{#AppExe}'), 'tray-task-set off', Code);
    RunLogged(PowerShell, Script('install-fan-service.ps1', '-Mode Remove'), Code);
    RunLogged(PowerShell, Script('install-startup.ps1', '-Mode Remove'), Code);
    RunLogged(PowerShell, Script('install-app.ps1', '-Mode Remove'), Code);
    DeleteFile(LogPath);
  end;
  if CurUninstallStep = usPostUninstall then
  begin
    Data := ExpandConstant('{%USERPROFILE}\.omen-hub-pp');
    if DirExists(Data) and not UninstallSilent and
      (MsgBox(FmtMessage(CustomMessage('DeleteData'), [Data]), mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES) then
      DelTree(Data, True, True, True);
    if (PawnIOVersionInstalled <> '') and not UninstallSilent then
      MsgBox(CustomMessage('PawnIOKept'), mbInformation, MB_OK);
  end;
end;
