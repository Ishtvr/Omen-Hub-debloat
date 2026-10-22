# Installs the fixed fan service for the current user's logon (approved by the
# user on 16 October 2026): a protected copy of this release runs 'fan-service'
# elevated and follows the saved shared fan curve. Removing it stops software
# fan commands; a firmware-ownership handover requires separate evidence.
#requires -Version 5.1
param([ValidateSet('Install','Inspect','Remove')][string]$Mode = 'Install',[string]$SourceBinary='')
$ErrorActionPreference = 'Stop'
$fanIdentity = [Security.Principal.WindowsIdentity]::GetCurrent()
$fanSid = $fanIdentity.User.Value
$fanTask = 'OmenHubPP-Fan-' + $fanSid
$fanRoot = [IO.Path]::GetFullPath((Join-Path $env:ProgramFiles 'OmenHubPlusPlus'))
$fanBinary = Join-Path $fanRoot 'Omen Hub++ Fan.exe'
$fanReceipt = Join-Path $fanRoot ('fan-service-' + $fanSid + '.json')
$fanExisting = Get-ScheduledTask -TaskName $fanTask -ErrorAction SilentlyContinue

function Assert-FanTaskAction {
    param($Task)
    if ($Task.Actions.Count -ne 1 -or $Task.Actions[0].Execute -ne $fanBinary -or $Task.Actions[0].Arguments -ne 'fan-service') {
        throw 'The existing fan task has a different action. It was left unchanged.'
    }
}
if ($Mode -eq 'Inspect') {
    if ($null -ne $fanExisting) { Assert-FanTaskAction $fanExisting }
    [pscustomobject]@{
        Installed = ($null -ne $fanExisting)
        TaskName = $fanTask
        State = if ($null -ne $fanExisting) { [string]$fanExisting.State } else { 'Not installed' }
        BinaryHash = if (Test-Path -LiteralPath $fanBinary) { (Get-FileHash -LiteralPath $fanBinary -Algorithm SHA256).Hash } else { $null }
    } | ConvertTo-Json
    return
}
if (-not ([Security.Principal.WindowsPrincipal]::new($fanIdentity)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Run this fixed installer elevated. It protects the executable and registers the fan task.'
}
if ($null -ne $fanExisting) {
    Assert-FanTaskAction $fanExisting
    $fanSession = [Diagnostics.Process]::GetCurrentProcess().SessionId
    if (Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $fanBinary -and $_.SessionId -ne $fanSession }) {
        throw 'A fan helper in another Windows session uses this executable; it was left running.'
    }
    # Let the controller restore foreground affinities before ending its task.
    # A new source can signal an old installed controller without executing
    # untrusted per-user files as administrator.
    $fanStopTool = if ([string]::IsNullOrWhiteSpace($SourceBinary)) { $fanBinary } else { [IO.Path]::GetFullPath($SourceBinary) }
    if (Test-Path -LiteralPath $fanStopTool -PathType Leaf) {
        $fanPreference = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
        & $fanStopTool fan-service-stop 2>$null | Out-Null
        $ErrorActionPreference = $fanPreference
    }
    $fanStopDeadline = (Get-Date).AddSeconds(5)
    do {
        $fanRunning = @(Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $fanBinary -and $_.SessionId -eq $fanSession })
        if ($fanRunning.Count -eq 0) { break }
        Start-Sleep -Milliseconds 100
    } while ((Get-Date) -lt $fanStopDeadline)
    if ($fanRunning.Count -ne 0) {
        # Legacy builds had no stop event. A forced upgrade is allowed only
        # after the saved CCD selection is released and no pins are reported.
        $fanUserRoot = Join-Path ([Environment]::GetFolderPath('UserProfile')) '.omen-hub-pp'
        $fanStatusPath = Join-Path $fanUserRoot 'fan-service-status.json'
        $fanOldStatus = if (Test-Path -LiteralPath $fanStatusPath) { Get-Content -LiteralPath $fanStatusPath -Raw | ConvertFrom-Json } else { $null }
        if ((Test-Path -LiteralPath (Join-Path $fanUserRoot 'ccd-affinity.json')) -or ($null -ne $fanOldStatus -and $fanOldStatus.ccd_pinned -gt 0)) {
            throw 'The fan controller did not stop gracefully. Select all CCDs and retry; pinned programs were left unchanged.'
        }
    }
    Stop-ScheduledTask -TaskName $fanTask -ErrorAction SilentlyContinue
    $fanRunning | ForEach-Object { if (-not $_.HasExited) { Stop-Process -InputObject $_ -Force } }
}
if ((Test-Path -LiteralPath $fanRoot) -and ((Get-Item -LiteralPath $fanRoot).Attributes -band [IO.FileAttributes]::ReparsePoint)) {
    throw 'The installation directory is a reparse point; installation was blocked.'
}
foreach ($fanPath in @($fanBinary,$fanReceipt)) {
    if ([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($fanPath)) -ne $fanRoot) { throw 'Invalid installation target.' }
    if ((Test-Path -LiteralPath $fanPath) -and ((Get-Item -LiteralPath $fanPath).Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Installation file is a reparse point.' }
}
if ($Mode -eq 'Remove') {
    if ($null -ne $fanExisting) { Unregister-ScheduledTask -TaskName $fanTask -Confirm:$false }
    foreach ($fanPath in @($fanReceipt,$fanBinary)) {
        if (Test-Path -LiteralPath $fanPath) { Remove-Item -LiteralPath $fanPath }
    }
    [pscustomobject]@{ Removed = $true; TaskName = $fanTask; SoftwareFanCommandsStopped = $true; BiosHandoverVerified = $false } | ConvertTo-Json
    return
}
$fanSource = if([string]::IsNullOrWhiteSpace($SourceBinary)){Join-Path (Split-Path -Parent $PSScriptRoot) 'dist\Omen Hub++.exe'}else{[IO.Path]::GetFullPath($SourceBinary)}
if (-not (Test-Path -LiteralPath $fanSource -PathType Leaf)) { throw 'Build the release executable before installing the fan service.' }
$fanSourceHash = (Get-FileHash -LiteralPath $fanSource -Algorithm SHA256).Hash
New-Item -ItemType Directory -Force -Path $fanRoot | Out-Null
# Same protection as the profile startup installer: administrators and SYSTEM
# own the directory, the user may only read and execute.
$fanAcl = [Security.AccessControl.DirectorySecurity]::new()
$fanAcl.SetAccessRuleProtection($true,$false)
$fanAdmins = [Security.Principal.SecurityIdentifier]::new('S-1-5-32-544')
$fanSystem = [Security.Principal.SecurityIdentifier]::new('S-1-5-18')
$fanUser = [Security.Principal.SecurityIdentifier]::new($fanSid)
$fanAcl.SetOwner($fanAdmins)
$fanInheritance = [Security.AccessControl.InheritanceFlags]'ContainerInherit,ObjectInherit'
$fanPropagation = [Security.AccessControl.PropagationFlags]::None
foreach ($fanPrincipal in @($fanAdmins,$fanSystem)) {
    $fanAcl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($fanPrincipal,[Security.AccessControl.FileSystemRights]::FullControl,$fanInheritance,$fanPropagation,[Security.AccessControl.AccessControlType]::Allow))
}
$fanAcl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($fanUser,[Security.AccessControl.FileSystemRights]::ReadAndExecute,$fanInheritance,$fanPropagation,[Security.AccessControl.AccessControlType]::Allow))
Set-Acl -LiteralPath $fanRoot -AclObject $fanAcl
Copy-Item -LiteralPath $fanSource -Destination $fanBinary -Force
if ((Get-FileHash -LiteralPath $fanBinary -Algorithm SHA256).Hash -ne $fanSourceHash) { throw 'Installed fan executable hash differs from the release.' }
$fanFileAcl = [Security.AccessControl.FileSecurity]::new()
$fanFileAcl.SetAccessRuleProtection($false,$false)
$fanFileAcl.SetOwner($fanAdmins)
Set-Acl -LiteralPath $fanBinary -AclObject $fanFileAcl

$fanAction = New-ScheduledTaskAction -Execute $fanBinary -Argument 'fan-service' -WorkingDirectory $fanRoot
$fanTrigger = New-ScheduledTaskTrigger -AtLogOn -User $fanSid
$fanPrincipal = New-ScheduledTaskPrincipal -UserId $fanSid -LogonType Interactive -RunLevel Highest
# No time limit; restart after a failure so the BIOS is never left waiting long.
$fanSettings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit ([TimeSpan]::Zero) -MultipleInstances IgnoreNew -RestartCount 999 -RestartInterval (New-TimeSpan -Minutes 1)
$fanRegistered = $false
try {
    Register-ScheduledTask -TaskName $fanTask -Action $fanAction -Trigger $fanTrigger -Principal $fanPrincipal -Settings $fanSettings -Description 'Omen Hub++: saved fan curve and foreground CCD selection; Auto pauses software fan commands.' -Force | Out-Null
    $fanRegistered = $true
    $fanScheduler = New-Object -ComObject 'Schedule.Service'
    $fanScheduler.Connect()
    $fanScheduler.GetFolder('\').GetTask($fanTask).SetSecurityDescriptor("O:BAG:BAD:P(A;;FA;;;SY)(A;;FA;;;BA)(A;;GRGX;;;$fanSid)",0x10)
    $fanVerified = Get-ScheduledTask -TaskName $fanTask
    Assert-FanTaskAction $fanVerified
    if ($fanVerified.Principal.RunLevel -ne 'Highest' -or $fanVerified.Principal.LogonType -ne 'Interactive') { throw 'Fan task principal readback differs.' }
    [ordered]@{ schema = 1; executable = $fanBinary; sha256 = $fanSourceHash; task_name = $fanTask; user_sid = $fanSid; installed_at = (Get-Date).ToString('o') } | ConvertTo-Json | Set-Content -LiteralPath $fanReceipt -Encoding UTF8
    Start-ScheduledTask -TaskName $fanTask
    [pscustomobject]@{ Installed = $true; TaskName = $fanTask; SHA256 = $fanSourceHash; Started = $true } | ConvertTo-Json
} catch {
    if ($fanRegistered) { Unregister-ScheduledTask -TaskName $fanTask -Confirm:$false -ErrorAction SilentlyContinue }
    throw
}
