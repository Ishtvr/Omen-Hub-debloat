# Installs the fixed, native profile entry point for the current user's logon.
# No profile is selected or applied by this installer. Windows PowerShell 5.1.
#requires -Version 5.1
param([ValidateSet('Install','Inspect','Remove')][string]$Mode = 'Install',[string]$SourceBinary='')
$ErrorActionPreference = 'Stop'
$taskIdentity = [Security.Principal.WindowsIdentity]::GetCurrent()
$taskSid = $taskIdentity.User.Value
$taskName = 'OmenHubPP-Profile-' + $taskSid
$taskInstallRoot = [IO.Path]::GetFullPath((Join-Path $env:ProgramFiles 'OmenHubPlusPlus'))
$taskBinary = Join-Path $taskInstallRoot 'Omen Hub++.exe'
$taskReceipt = Join-Path $taskInstallRoot 'installation.json'
$taskApproval = Join-Path $taskInstallRoot ('startup-authorized-' + $taskSid + '.json')
$taskExisting = Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue

function Assert-OmenTaskAction {
    param($Task)
    if ($Task.Actions.Count -ne 1 -or $Task.Actions[0].Execute -ne $taskBinary -or $Task.Actions[0].Arguments -ne 'profile-startup-apply') {
        throw 'The existing task has a different action. It was left unchanged.'
    }
}
if ($Mode -eq 'Inspect') {
    if ($null -ne $taskExisting) { Assert-OmenTaskAction $taskExisting }
    [pscustomobject]@{
        Installed = ($null -ne $taskExisting)
        TaskName = $taskName
        Executable = $taskBinary
        Arguments = 'profile-startup-apply'
        State = if ($null -ne $taskExisting) { [string]$taskExisting.State } else { 'Not installed' }
        LogonType = if ($null -ne $taskExisting) { [string]$taskExisting.Principal.LogonType } else { $null }
        RunLevel = if ($null -ne $taskExisting) { [string]$taskExisting.Principal.RunLevel } else { $null }
        BinaryHash = if (Test-Path -LiteralPath $taskBinary) { (Get-FileHash -LiteralPath $taskBinary -Algorithm SHA256).Hash } else { $null }
    } | ConvertTo-Json
    return
}
if (-not ([Security.Principal.WindowsPrincipal]::new($taskIdentity)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Run this fixed installer in an elevated Windows PowerShell. It needs permission to protect the executable and register its logon task.'
}
if ($null -ne $taskExisting) { Assert-OmenTaskAction $taskExisting }
if ((Test-Path -LiteralPath $taskInstallRoot) -and ((Get-Item -LiteralPath $taskInstallRoot).Attributes -band [IO.FileAttributes]::ReparsePoint)) {
    throw 'The installation directory is a reparse point; installation was blocked.'
}
foreach ($taskPath in @($taskBinary,$taskReceipt,$taskApproval)) {
    if ([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($taskPath)) -ne $taskInstallRoot) { throw 'Invalid installation target.' }
    if ((Test-Path -LiteralPath $taskPath) -and ((Get-Item -LiteralPath $taskPath).Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Installation file is a reparse point.' }
}
if ($Mode -eq 'Remove') {
    if ($null -ne $taskExisting) { Unregister-ScheduledTask -TaskName $taskName -Confirm:$false }
    # Remove only these known installation files, never user profiles or directories.
    foreach ($taskPath in @($taskApproval,$taskReceipt,$taskBinary)) {
        if (Test-Path -LiteralPath $taskPath) { Remove-Item -LiteralPath $taskPath }
    }
    [pscustomobject]@{ Removed = $true; TaskName = $taskName; ProfilesRetained = $true } | ConvertTo-Json
    return
}
$taskSource = if([string]::IsNullOrWhiteSpace($SourceBinary)){Join-Path (Split-Path -Parent $PSScriptRoot) 'dist\Omen Hub++.exe'}else{[IO.Path]::GetFullPath($SourceBinary)}
if (-not (Test-Path -LiteralPath $taskSource -PathType Leaf)) { throw 'Build the release executable before installing startup.' }
$taskSourceHash = (Get-FileHash -LiteralPath $taskSource -Algorithm SHA256).Hash
New-Item -ItemType Directory -Force -Path $taskInstallRoot | Out-Null
$taskAcl = [Security.AccessControl.DirectorySecurity]::new()
$taskAcl.SetAccessRuleProtection($true,$false)
$taskAdmins = [Security.Principal.SecurityIdentifier]::new('S-1-5-32-544')
$taskSystem = [Security.Principal.SecurityIdentifier]::new('S-1-5-18')
$taskUser = [Security.Principal.SecurityIdentifier]::new($taskSid)
$taskAcl.SetOwner($taskAdmins)
$taskInheritance = [Security.AccessControl.InheritanceFlags]'ContainerInherit,ObjectInherit'
$taskPropagation = [Security.AccessControl.PropagationFlags]::None
foreach ($taskPrincipal in @($taskAdmins,$taskSystem)) {
    $taskAcl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($taskPrincipal,[Security.AccessControl.FileSystemRights]::FullControl,$taskInheritance,$taskPropagation,[Security.AccessControl.AccessControlType]::Allow))
}
$taskAcl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($taskUser,[Security.AccessControl.FileSystemRights]::ReadAndExecute,$taskInheritance,$taskPropagation,[Security.AccessControl.AccessControlType]::Allow))
Set-Acl -LiteralPath $taskInstallRoot -AclObject $taskAcl
if(-not [IO.Path]::GetFullPath($taskSource).Equals($taskBinary,[StringComparison]::OrdinalIgnoreCase)){
    Copy-Item -LiteralPath $taskSource -Destination $taskBinary -Force
}
if ((Get-FileHash -LiteralPath $taskBinary -Algorithm SHA256).Hash -ne $taskSourceHash) { throw 'Installed executable hash differs from the release.' }
# Protect the file even when replacing a pre-existing file with explicit permissions.
$taskFileAcl = [Security.AccessControl.FileSecurity]::new()
$taskFileAcl.SetAccessRuleProtection($false,$false)
$taskFileAcl.SetOwner($taskAdmins)
Set-Acl -LiteralPath $taskBinary -AclObject $taskFileAcl

$taskAction = New-ScheduledTaskAction -Execute $taskBinary -Argument 'profile-startup-apply' -WorkingDirectory $taskInstallRoot
$taskTrigger = New-ScheduledTaskTrigger -AtLogOn -User $taskSid
$taskTrigger.Delay = 'PT15S'
$taskPrincipal = New-ScheduledTaskPrincipal -UserId $taskSid -LogonType Interactive -RunLevel Highest
$taskSettings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Minutes 2) -MultipleInstances IgnoreNew
$taskRegistered = $false
try {
    Register-ScheduledTask -TaskName $taskName -Action $taskAction -Trigger $taskTrigger -Principal $taskPrincipal -Settings $taskSettings -Description 'Omen Hub++: apply only the previously verified, selected profile after user logon.' -Force | Out-Null
    $taskRegistered = $true
    $taskScheduler = New-Object -ComObject 'Schedule.Service'
    $taskScheduler.Connect()
    $taskProtected = $taskScheduler.GetFolder('\').GetTask($taskName)
    # The user may read/run the fixed task. Only administrators/SYSTEM may change it.
    $taskProtected.SetSecurityDescriptor("O:BAG:BAD:P(A;;FA;;;SY)(A;;FA;;;BA)(A;;GRGX;;;$taskSid)",0x10)
    $taskVerified = Get-ScheduledTask -TaskName $taskName
    Assert-OmenTaskAction $taskVerified
    if ($taskVerified.Principal.RunLevel -ne 'Highest' -or $taskVerified.Principal.LogonType -ne 'Interactive') { throw 'Task principal readback differs.' }
    [ordered]@{ schema = 1; executable = $taskBinary; sha256 = $taskSourceHash; task_name = $taskName; user_sid = $taskSid; installed_at = (Get-Date).ToString('o') } | ConvertTo-Json | Set-Content -LiteralPath $taskReceipt -Encoding UTF8
    [pscustomobject]@{ Installed = $true; TaskName = $taskName; Executable = $taskBinary; SHA256 = $taskSourceHash; ProfileSelected = $false; HardwareApplied = $false } | ConvertTo-Json
} catch {
    if ($taskRegistered) { Unregister-ScheduledTask -TaskName $taskName -Confirm:$false -ErrorAction SilentlyContinue }
    throw
}
