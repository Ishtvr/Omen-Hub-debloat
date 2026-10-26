# Installs the release executable into the protected program directory used by
# install-startup.ps1 and creates the current user's Start menu and desktop
# shortcuts with the embedded icon. Like the logon task, this is per user: run
# it as the Windows user who will use Omen Hub++.
# No profile is selected, no logon task is registered and no hardware is touched.
# Windows PowerShell 5.1. Run without arguments to install; the script requests
# Windows-UAC itself when needed.
#requires -Version 5.1
param(
    [ValidateSet('Install','Inspect','Remove')][string]$Mode = 'Install',
    [string]$SourceBinary = '',
    [switch]$NoDesktopShortcut,
    # Internal: keeps the self-elevated window readable when it fails.
    [switch]$PauseOnError
)
$ErrorActionPreference = 'Stop'
trap {
    if ($PauseOnError) {
        Write-Host $_ -ForegroundColor Red
        [void](Read-Host 'Press Enter to close')
    }
    break
}
$appName = 'Omen Hub++'
$appRoot = [IO.Path]::GetFullPath((Join-Path $env:ProgramFiles 'OmenHubPlusPlus'))
$appBinary = Join-Path $appRoot 'Omen Hub++.exe'
$appStartupReceipt = Join-Path $appRoot 'installation.json'
$appShortcuts = @(
    (Join-Path ([Environment]::GetFolderPath('Programs')) "$appName.lnk"),
    (Join-Path ([Environment]::GetFolderPath('Desktop')) "$appName.lnk")
)
$appIdentity = [Security.Principal.WindowsIdentity]::GetCurrent()
$appTaskName = 'OmenHubPP-Profile-' + $appIdentity.User.Value

function Get-OmenShortcut {
    param([string]$Path)
    if (-not (Test-Path -LiteralPath $Path)) { return $null }
    $link = (New-Object -ComObject 'WScript.Shell').CreateShortcut($Path)
    [pscustomobject]@{ Path = $Path; Target = $link.TargetPath; Icon = $link.IconLocation }
}

if ($Mode -eq 'Inspect') {
    [pscustomobject]@{
        Installed = (Test-Path -LiteralPath $appBinary -PathType Leaf)
        Executable = $appBinary
        SHA256 = if (Test-Path -LiteralPath $appBinary) { (Get-FileHash -LiteralPath $appBinary -Algorithm SHA256).Hash } else { $null }
        Shortcuts = @($appShortcuts | ForEach-Object { Get-OmenShortcut $_ } | Where-Object { $null -ne $_ })
        LogonTask = ($null -ne (Get-ScheduledTask -TaskName $appTaskName -ErrorAction SilentlyContinue))
    } | ConvertTo-Json -Depth 3
    return
}

$appSource = if (-not [string]::IsNullOrWhiteSpace($SourceBinary)) {
    [IO.Path]::GetFullPath($SourceBinary)
} elseif (Test-Path -LiteralPath (Join-Path $PSScriptRoot 'Omen Hub++.exe') -PathType Leaf) {
    # Release package: the executable sits next to this script.
    Join-Path $PSScriptRoot 'Omen Hub++.exe'
} else {
    Join-Path (Split-Path -Parent $PSScriptRoot) 'dist\Omen Hub++.exe'
}

if (-not ([Security.Principal.WindowsPrincipal]::new($appIdentity)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    # Elevate this same script with the same explicit arguments and wait.
    $appArguments = @('-NoProfile','-ExecutionPolicy','Bypass','-File',('"' + $PSCommandPath + '"'),'-Mode',$Mode,'-PauseOnError')
    if ($Mode -eq 'Install') { $appArguments += @('-SourceBinary',('"' + $appSource + '"')) }
    if ($NoDesktopShortcut) { $appArguments += '-NoDesktopShortcut' }
    $appProcess = Start-Process -FilePath (Join-Path $PSHOME 'powershell.exe') -ArgumentList $appArguments -Verb RunAs -Wait -PassThru
    if ($appProcess.ExitCode -ne 0) { throw "Elevated $Mode failed with exit code $($appProcess.ExitCode)." }
    & $PSCommandPath -Mode Inspect
    return
}

# A pending coupled recovery must keep both protected binaries and tasks intact.
# Serialize against the native qualification owner before any installer action.
$recoveryGate = [Threading.Mutex]::new($false, 'Global\OmenHubPP.HpControllerOwner')
$recoveryGateOwned = $false
try {
    try { $recoveryGateOwned = $recoveryGate.WaitOne(0) }
    catch [Threading.AbandonedMutexException] { $recoveryGateOwned = $true }
    if (-not $recoveryGateOwned) { throw 'An Omen Hub++ controller session is active; installation/removal was blocked.' }
    foreach ($recoveryFile in @('controller-ownership.json','controller-cooling.json')) {
        $recoveryPath = Join-Path $appRoot $recoveryFile
        if (Test-Path -LiteralPath $recoveryPath) {
            $recoveryItem = Get-Item -LiteralPath $recoveryPath
            if ($recoveryItem.Length -gt 16384 -or ($recoveryItem.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Controller recovery record is oversized or redirected; installation/removal was blocked.' }
            $recoveryRecord = Get-Content -LiteralPath $recoveryPath -Raw | ConvertFrom-Json
            if ($recoveryRecord.schema -ne 1 -or $recoveryRecord.phase -cnotin @('restored','superseded') -or $recoveryRecord.owner.session -cnotmatch '^[a-fA-F0-9]{64}$') {
                throw 'Complete controller/cooling recovery before updating or removing either helper.'
            }
        }
    }
if ((Test-Path -LiteralPath $appRoot) -and ((Get-Item -LiteralPath $appRoot).Attributes -band [IO.FileAttributes]::ReparsePoint)) {
    throw 'The installation directory is a reparse point; nothing was changed.'
}
if ((Test-Path -LiteralPath $appBinary) -and ((Get-Item -LiteralPath $appBinary).Attributes -band [IO.FileAttributes]::ReparsePoint)) {
    throw 'The installed executable is a reparse point; nothing was changed.'
}
$appRunning = @(Get-Process -ErrorAction SilentlyContinue | Where-Object {
    try { $_.Path -and [IO.Path]::GetFullPath($_.Path).Equals($appBinary, [StringComparison]::OrdinalIgnoreCase) } catch { $false }
})
if ($appRunning.Count -gt 0) {
    throw 'Omen Hub++ is running from the installation directory. Exit it from the tray menu first.'
}

if ($Mode -eq 'Remove') {
    foreach ($appShortcut in $appShortcuts) {
        $appLink = Get-OmenShortcut $appShortcut
        # Remove only our own shortcuts, never a same-named link to another target.
        if ($null -ne $appLink -and $appLink.Target -eq $appBinary) { Remove-Item -LiteralPath $appShortcut }
    }
    $appTask = $null -ne (Get-ScheduledTask -TaskName $appTaskName -ErrorAction SilentlyContinue)
    if ($appTask -or (Test-Path -LiteralPath $appStartupReceipt)) {
        # The logon task executes this binary. install-startup.ps1 -Mode Remove owns it.
        [pscustomobject]@{ ShortcutsRemoved = $true; ExecutableRemoved = $false; Reason = 'Logon task installed; run scripts/install-startup.ps1 -Mode Remove first.'; ProfilesRetained = $true } | ConvertTo-Json
        return
    }
    if (Test-Path -LiteralPath $appBinary) { Remove-Item -LiteralPath $appBinary }
    [pscustomobject]@{ ShortcutsRemoved = $true; ExecutableRemoved = $true; ProfilesRetained = $true } | ConvertTo-Json
    return
}

if (-not (Test-Path -LiteralPath $appSource -PathType Leaf)) { throw 'Build the release executable first: scripts/build-release.ps1.' }
$appSourceHash = (Get-FileHash -LiteralPath $appSource -Algorithm SHA256).Hash
$appPreviousHash = if (Test-Path -LiteralPath $appBinary) { (Get-FileHash -LiteralPath $appBinary -Algorithm SHA256).Hash } else { $null }

# Same protected layout as install-startup.ps1: administrators/SYSTEM own the
# directory, the current user may only read and execute.
New-Item -ItemType Directory -Force -Path $appRoot | Out-Null
$appAcl = [Security.AccessControl.DirectorySecurity]::new()
$appAcl.SetAccessRuleProtection($true, $false)
$appAdmins = [Security.Principal.SecurityIdentifier]::new('S-1-5-32-544')
$appSystem = [Security.Principal.SecurityIdentifier]::new('S-1-5-18')
$appAcl.SetOwner($appAdmins)
$appInheritance = [Security.AccessControl.InheritanceFlags]'ContainerInherit,ObjectInherit'
$appPropagation = [Security.AccessControl.PropagationFlags]::None
foreach ($appPrincipal in @($appAdmins, $appSystem)) {
    $appAcl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($appPrincipal, [Security.AccessControl.FileSystemRights]::FullControl, $appInheritance, $appPropagation, [Security.AccessControl.AccessControlType]::Allow))
}
$appAcl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($appIdentity.User, [Security.AccessControl.FileSystemRights]::ReadAndExecute, $appInheritance, $appPropagation, [Security.AccessControl.AccessControlType]::Allow))
Set-Acl -LiteralPath $appRoot -AclObject $appAcl
if (-not $appSource.Equals($appBinary, [StringComparison]::OrdinalIgnoreCase)) {
    Copy-Item -LiteralPath $appSource -Destination $appBinary -Force
}
if ((Get-FileHash -LiteralPath $appBinary -Algorithm SHA256).Hash -ne $appSourceHash) { throw 'Installed executable hash differs from the release.' }
$appFileAcl = [Security.AccessControl.FileSecurity]::new()
$appFileAcl.SetAccessRuleProtection($false, $false)
$appFileAcl.SetOwner($appAdmins)
Set-Acl -LiteralPath $appBinary -AclObject $appFileAcl

$appShell = New-Object -ComObject 'WScript.Shell'
$appCreated = @()
foreach ($appShortcut in $(if ($NoDesktopShortcut) { @($appShortcuts[0]) } else { $appShortcuts })) {
    $appLink = $appShell.CreateShortcut($appShortcut)
    $appLink.TargetPath = $appBinary
    $appLink.WorkingDirectory = $appRoot
    $appLink.IconLocation = "$appBinary,0"
    $appLink.Description = "GPU- und CPU-Werte, Leistungsprofile, L$([char]0x00FC)fter und Beleuchtung"
    $appLink.Save()
    $appCreated += $appShortcut
}
[pscustomobject]@{
    Installed = $true
    Executable = $appBinary
    SHA256 = $appSourceHash
    Shortcuts = $appCreated
    # A changed binary no longer matches an earlier startup approval; the app
    # then labels it invalid until it is applied and approved again.
    StartupApprovalInvalidated = ($null -ne $appPreviousHash -and $appPreviousHash -ne $appSourceHash -and (Test-Path -LiteralPath $appStartupReceipt))
    ProfileSelected = $false
    HardwareApplied = $false
} | ConvertTo-Json

} finally {
    if ($recoveryGateOwned) { $recoveryGate.ReleaseMutex() }
    $recoveryGate.Dispose()
}