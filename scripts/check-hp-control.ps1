# Read-only controller/preflight evidence. No UAC, service commands, profile
# application, power changes, firmware writes or driver actions are requested.
#requires -Version 5.1
param(
 [string]$Executable='',
 [Parameter(Mandatory=$true)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$ExpectedSha256,
 [ValidatePattern('^$|^[a-z0-9][a-z0-9_-]{0,63}$')][string]$ProfileId='',
 [string]$RunDirectory=''
)
$ErrorActionPreference='Stop'
$taskRepo=Split-Path -Parent $PSScriptRoot
if(-not$Executable){$Executable=Join-Path $taskRepo 'dist\Omen Hub++.exe'}
$taskExe=(Resolve-Path -LiteralPath $Executable).Path
if((Get-FileHash -LiteralPath $taskExe).Hash-ine$ExpectedSha256){throw 'Candidate hash differs.'}
if((Get-Item -LiteralPath $taskExe).VersionInfo.ProductName-cne'Omen Hub++'){throw 'Unexpected executable product.'}
if(-not$RunDirectory){$RunDirectory=Join-Path $taskRepo ('.local\research\hp-control-check-'+[DateTimeOffset]::UtcNow.ToString('yyyyMMdd-HHmmss')+'-'+[Guid]::NewGuid().ToString('N').Substring(0,8))}
if(Test-Path -LiteralPath $RunDirectory){throw 'Use a new evidence directory.'}
New-Item -ItemType Directory -Path $RunDirectory|Out-Null
$taskRun=(Resolve-Path -LiteralPath $RunDirectory).Path
$taskStore=Join-Path ([Environment]::GetFolderPath('UserProfile')) '.omen-hub-pp'
$taskFiles=@('startup.json','control-journal.json','fan-mode.json','active-fan-curve.json')|ForEach-Object{Join-Path $taskStore $_}
if($ProfileId){$taskFiles+=Join-Path $taskStore ('profiles\'+$ProfileId+'.json')}
$taskInstalled=Join-Path ([Environment]::GetFolderPath('ProgramFiles')) 'OmenHubPlusPlus'
foreach($taskName in @('Omen Hub++.exe','Omen Hub++ Fan.exe','installation.json','controller-ownership.json')){$taskFiles+=Join-Path $taskInstalled $taskName}
function File-Snapshot {
 @($taskFiles|ForEach-Object{
  $taskPresent=Test-Path -LiteralPath $_
  [pscustomobject]@{path=$_;exists=$taskPresent;sha256=$(if($taskPresent){(Get-FileHash -LiteralPath $_).Hash}else{$null})}
 })
}
function Controller-Snapshot {
 $taskUxtu=Get-ScheduledTask -TaskName UXTU -ErrorAction SilentlyContinue
 [pscustomobject]@{
  service=(Get-CimInstance Win32_Service -Filter "Name='HPOmenCap'"|Select-Object Name,State,ProcessId,StartMode)
  drivers=@(Get-CimInstance Win32_SystemDriver|Where-Object{$_.Name-match'(?i)^(PawnIO|AMDRyzenMasterDriverV31|AmdPPM|AmdPpkg)$'}|Select-Object Name,State,StartMode|Sort-Object Name)
  processes=@(Get-CimInstance Win32_Process|Where-Object{$_.Name-in@('Omen Hub++.exe','Omen Hub++ Fan.exe','OmenCommandCenterBackground.exe','HP.Omen.OmenCommandCenter.exe')}|Select-Object Name,ProcessId|Sort-Object Name,ProcessId)
  uxtu_task_present=$null-ne$taskUxtu
  uxtu_task_enabled=$(if($taskUxtu){[bool]$taskUxtu.Settings.Enabled}else{$null})
 }
}
function Read-Command {
 param([ValidateSet('hp-control-status','profile-plan')][string]$Command,[string]$Id='')
 if(($Command-eq'profile-plan'-and$Id-notmatch'^[a-z0-9][a-z0-9_-]{0,63}$')-or($Command-eq'hp-control-status'-and$Id)){throw 'Unexpected read arguments.'}
 $taskInfo=[Diagnostics.ProcessStartInfo]::new()
 $taskInfo.FileName=$taskExe;$taskInfo.Arguments=$Command+$(if($Id){' '+$Id}else{''});$taskInfo.UseShellExecute=$false;$taskInfo.CreateNoWindow=$true
 $taskInfo.RedirectStandardOutput=$true;$taskInfo.RedirectStandardError=$true
 $taskInfo.StandardOutputEncoding=[Text.UTF8Encoding]::new($false);$taskInfo.StandardErrorEncoding=[Text.UTF8Encoding]::new($false)
 $taskChild=[Diagnostics.Process]::new();$taskChild.StartInfo=$taskInfo
 try{
  [void]$taskChild.Start();$taskOut=$taskChild.StandardOutput.ReadToEndAsync();$taskErr=$taskChild.StandardError.ReadToEndAsync()
  if(-not$taskChild.WaitForExit(15000)){$taskChild.Kill();throw ('Owned read timed out: '+$Command)}
  $taskText=$taskOut.GetAwaiter().GetResult();$taskError=$taskErr.GetAwaiter().GetResult()
  if($taskText.Length-gt131072-or$taskError.Length-gt8192){throw 'Read output exceeds its bound.'}
  [IO.File]::WriteAllText((Join-Path $taskRun ($Command+'.json')),$taskText,[Text.UTF8Encoding]::new($false))
  [IO.File]::WriteAllText((Join-Path $taskRun ($Command+'.stderr')),$taskError,[Text.UTF8Encoding]::new($false))
  if($taskChild.ExitCode-ne0){throw ($Command+' failed: '+$taskError)}
  $taskText|ConvertFrom-Json
 }finally{$taskChild.Dispose()}
}
$taskResult=[ordered]@{started_at=(Get-Date).ToString('o');executable=$taskExe;binary_sha256=$ExpectedSha256.ToUpperInvariant();passed=$false;error=$null;hardware_writes=$false;service_actions=$false;installed=$false}
$taskBeforeFiles=File-Snapshot;$taskBeforeControllers=Controller-Snapshot
$taskBeforeFiles|ConvertTo-Json -Depth 5|Set-Content -LiteralPath (Join-Path $taskRun 'files-before.json') -Encoding UTF8
$taskBeforeControllers|ConvertTo-Json -Depth 5|Set-Content -LiteralPath (Join-Path $taskRun 'controllers-before.json') -Encoding UTF8
try{
 $taskStatus=Read-Command 'hp-control-status'
 if($taskStatus.schema-ne1-or$taskStatus.service_name-cne'HPOmenCap'-or$taskStatus.hardware_applied-or$taskStatus.services_changed-or$taskStatus.automatic_takeover_enabled){throw 'Read-only status contract differs.'}
 if($taskStatus.state.PSObject.Properties.Name-contains'Err'){throw ('Service state is unknown: '+$taskStatus.state.Err)}
 $taskResult.observed_service_state=$taskStatus.state.Ok
 $taskResult.model_supported=[bool]$taskStatus.model_supported
 $taskResult.native_identity_audit_ok=$null-ne$taskStatus.audit.Ok
 if($ProfileId){
  $taskProfile=Get-Content -LiteralPath (Join-Path $taskStore ('profiles\'+$ProfileId+'.json')) -Raw|ConvertFrom-Json
  $taskLimits=@($taskProfile.settings|Where-Object{$_.feature-in@('cpu_ppt_limit_mw','cpu_slow_ppt_limit_mw')}).Count-gt0
  $taskPlan=Read-Command 'profile-plan' $ProfileId
  $taskResult.profile_id=$ProfileId;$taskResult.profile_applicable=[bool]$taskPlan.applicable
  if($taskLimits-and$taskStatus.state.Ok-in@('running','transitioning')){
   if($taskPlan.applicable-or-not(@($taskPlan.blockers|Where-Object{$_.reason-like'*hp_control.cpu_power_conflict*'}).Count)-or@($taskPlan.capabilities).Count-ne0){throw 'CPU-limit profile was not rejected at preparation before original capture.'}
   $taskResult.cpu_limit_conflict_rejected_before_capture=$true
  }
 }
 $taskAfterFiles=File-Snapshot;$taskAfterControllers=Controller-Snapshot
 $taskAfterFiles|ConvertTo-Json -Depth 5|Set-Content -LiteralPath (Join-Path $taskRun 'files-after.json') -Encoding UTF8
 $taskAfterControllers|ConvertTo-Json -Depth 5|Set-Content -LiteralPath (Join-Path $taskRun 'controllers-after.json') -Encoding UTF8
 $taskResult.files_unchanged=($taskBeforeFiles|ConvertTo-Json -Depth 5 -Compress)-ceq($taskAfterFiles|ConvertTo-Json -Depth 5 -Compress)
 $taskResult.controllers_unchanged=($taskBeforeControllers|ConvertTo-Json -Depth 5 -Compress)-ceq($taskAfterControllers|ConvertTo-Json -Depth 5 -Compress)
 if(-not$taskResult.files_unchanged-or-not$taskResult.controllers_unchanged){throw 'Controller or metadata changed during the read-only check.'}
 if((Get-FileHash -LiteralPath $taskExe).Hash-ine$ExpectedSha256){throw 'Candidate changed during the check.'}
 $taskResult.passed=$true
}catch{$taskResult.error=$_.Exception.Message}
finally{
 $taskResult.finished_at=(Get-Date).ToString('o')
 $taskResult|ConvertTo-Json -Depth 5|Set-Content -LiteralPath (Join-Path $taskRun 'result.json') -Encoding UTF8
 $taskResult|ConvertTo-Json -Depth 5
}
if(-not$taskResult.passed){exit 1}
