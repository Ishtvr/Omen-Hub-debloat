# Offline checkpoints only. This script never sleeps/restarts the PC, applies a
# profile, changes startup approval, starts/stops a service or installs a driver.
#requires -Version 5.1
param(
 [ValidateSet('Prepare','AfterStandby','AfterReboot')][string]$Phase='Prepare',
 [string]$RunDirectory='',
 [string]$Executable='C:\Program Files\OmenHubPlusPlus\Omen Hub++.exe',
 [switch]$FunctionsOnly
)
$ErrorActionPreference='Stop'

function Test-OmenLifecycleEvidence {
 param($Before,$After,[ValidateSet('AfterStandby','AfterReboot')][string]$Kind,$Events)
 $taskBootChanged=$Before.boot_unix_seconds-ne$After.boot_unix_seconds
 $taskErrors=[Collections.Generic.List[string]]::new()
 if(-not$Before.live_profile_verified-or-not$Before.startup_ready-or-not$Before.dashboard_ready-or-not$Before.cpu_diagnostic_ok-or-not$Before.gpu_diagnostic_ok){$taskErrors.Add('The prepared baseline was incomplete.')}
 if($Before.app_pid-le0-or$Before.reader_pid-le0-or$After.app_pid-le0-or$After.reader_pid-le0){$taskErrors.Add('Missing attributed app/reader identity.')}
 if($After.recorded_at_unix_seconds-le$Before.recorded_at_unix_seconds-or$After.recorded_at_unix_seconds-lt$After.boot_unix_seconds){$taskErrors.Add('Checkpoint times are not chronological.')}
 if($Before.binary_sha256-cne$After.binary_sha256){$taskErrors.Add('App build changed between checkpoints.')}
 if($Before.profile_digest-cne$After.profile_digest){$taskErrors.Add('Selected profile revision changed.')}
 if(-not$After.live_profile_verified){$taskErrors.Add('Live profile targets did not verify.')}
 if(-not$After.startup_ready){$taskErrors.Add('Protected startup approval did not verify.')}
 if(-not$After.dashboard_ready){$taskErrors.Add('Dashboard sensors/tray are not ready.')}
 if(-not$After.cpu_diagnostic_ok-or-not$After.gpu_diagnostic_ok){$taskErrors.Add('A fixed CPU/GPU read diagnostic failed.')}
 if($Kind-eq'AfterStandby'){
  if($taskBootChanged){$taskErrors.Add('This was a reboot, not a same-boot resume.')}
  $taskOrdered=@($Events|Where-Object {$_.unix_seconds-ge$Before.recorded_at_unix_seconds-and$_.unix_seconds-le$After.recorded_at_unix_seconds}|Sort-Object record_id)
  $taskPair=$false;$taskEntered=$false
  foreach($taskEvent in $taskOrdered){
   if($taskEvent.id-eq506){$taskEntered=$true}
   elseif($taskEvent.id-eq507-and$taskEntered){$taskPair=$true}
  }
  if(-not$taskPair){$taskErrors.Add('No ordered Windows S0 entry/exit pair after preparation.')}
  if($Before.app_pid-ne$After.app_pid-or$Before.reader_pid-ne$After.reader_pid){$taskErrors.Add('App/approved reader did not survive standby.')}
  if($After.power_generation-le$Before.power_generation){$taskErrors.Add('The dashboard observed no new power transition.')}
 }elseif($Kind-eq'AfterReboot'){
  if(-not$taskBootChanged-or$After.boot_unix_seconds-le$Before.boot_unix_seconds){$taskErrors.Add('No later Windows boot identity was recorded.')}
  if($After.startup_result_time-lt$After.boot_unix_seconds-or-not$After.startup_result_ok){$taskErrors.Add('No successful profile startup result from the new boot.')}
  if($After.startup_task_result-ne0-or$After.startup_task_time-lt$After.boot_unix_seconds){$taskErrors.Add('No successful scheduled startup execution from the new boot.')}
 }
 [pscustomobject]@{passed=($taskErrors.Count-eq0);errors=$taskErrors.ToArray();kind=$Kind;actual_boot_changed=$taskBootChanged;hardware_writes_by_checker=$false;deep_idle_residency_measured=$false}
}

if($FunctionsOnly){return}
if(-not([Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){
 throw 'Use an elevated Windows PowerShell for the fixed read-only hardware checks.'
}
$taskRepo=Split-Path -Parent $PSScriptRoot
$taskExe=(Resolve-Path -LiteralPath $Executable).Path
if((Get-Item -LiteralPath $taskExe).VersionInfo.ProductName-cne'Omen Hub++'){throw 'Unexpected executable product.'}
if($Phase-eq'Prepare'){
 if(-not$RunDirectory){$RunDirectory=Join-Path $taskRepo ('.local\research\power-check-'+(Get-Date -Format 'yyyyMMdd-HHmmss')+'-'+[Guid]::NewGuid().ToString('N').Substring(0,8))}
 if(Test-Path -LiteralPath $RunDirectory){throw 'Prepare requires a new directory; previous checkpoints are preserved.'}
 New-Item -ItemType Directory -Path $RunDirectory|Out-Null
}elseif(-not$RunDirectory-or-not(Test-Path -LiteralPath (Join-Path $RunDirectory 'before.json'))){throw 'Select the prepared run directory containing before.json.'}
$taskRun=(Resolve-Path -LiteralPath $RunDirectory).Path
$taskCapture=Join-Path $taskRun ($Phase+'-'+(Get-Date -Format 'yyyyMMdd-HHmmssfff'))
New-Item -ItemType Directory -Path $taskCapture|Out-Null

function Invoke-OmenLifecycleRead {
 param([string]$Name,[string]$Arguments)
 $taskInfo=[Diagnostics.ProcessStartInfo]::new()
 $taskInfo.FileName=$taskExe;$taskInfo.Arguments=$Arguments;$taskInfo.UseShellExecute=$false;$taskInfo.CreateNoWindow=$true
 $taskInfo.RedirectStandardOutput=$true;$taskInfo.RedirectStandardError=$true
 $taskProcess=[Diagnostics.Process]::new();$taskProcess.StartInfo=$taskInfo
 try{
  [void]$taskProcess.Start();$taskStdout=$taskProcess.StandardOutput.ReadToEndAsync();$taskStderr=$taskProcess.StandardError.ReadToEndAsync()
  if(-not$taskProcess.WaitForExit(20000)){$taskProcess.Kill();throw ('Owned read exceeded its 20-second deadline: '+$Name)}
  $taskText=$taskStdout.GetAwaiter().GetResult();$taskError=$taskStderr.GetAwaiter().GetResult()
  if($taskText.Length-gt1048576-or$taskError.Length-gt65536){throw 'Read diagnostic exceeded its output bound.'}
  [IO.File]::WriteAllText((Join-Path $taskCapture ($Name+'.json')),$taskText,[Text.UTF8Encoding]::new($false))
  [IO.File]::WriteAllText((Join-Path $taskCapture ($Name+'.stderr.txt')),$taskError,[Text.UTF8Encoding]::new($false))
  [pscustomobject]@{ok=($taskProcess.ExitCode-eq0);data=$(if($taskText.TrimStart().StartsWith('{')){$taskText|ConvertFrom-Json}else{$null});error=$taskError}
 }finally{$taskProcess.Dispose()}
}

if(-not('OmenLifecycleWindow'-as[type])){Add-Type @'
using System;using System.Text;using System.Runtime.InteropServices;
public static class OmenLifecycleWindow {
 [StructLayout(LayoutKind.Sequential)] public struct IconId {public uint Size;public IntPtr Window;public uint Number;public Guid Guid;}
 [StructLayout(LayoutKind.Sequential)] public struct Rect {public int Left,Top,Right,Bottom;}
 public delegate bool EnumWindow(IntPtr w,IntPtr p);
 [DllImport("user32.dll")] static extern bool EnumWindows(EnumWindow cb,IntPtr p);
 [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr w,EnumWindow cb,IntPtr p);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr w,out uint id);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetClassName(IntPtr w,StringBuilder b,int n);
 [DllImport("user32.dll",EntryPoint="SendMessageTimeoutW")] static extern IntPtr Query(IntPtr w,uint m,UIntPtr p,IntPtr l,uint f,uint ms,out UIntPtr r);
 [DllImport("user32.dll",EntryPoint="SendMessageTimeoutW",CharSet=CharSet.Unicode)] static extern IntPtr Read(IntPtr w,uint m,UIntPtr p,StringBuilder b,uint f,uint ms,out UIntPtr r);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr w,uint m,UIntPtr p,IntPtr l);
 [DllImport("shell32.dll")] static extern int Shell_NotifyIconGetRect(ref IconId id,out Rect rect);
 public static IntPtr Find(uint id){IntPtr found=IntPtr.Zero;EnumWindows((w,p)=>{uint owner;GetWindowThreadProcessId(w,out owner);if(owner==id){var b=new StringBuilder(128);GetClassName(w,b,b.Capacity);if(b.ToString()=="OmenHubPlusPlusWindow"){found=w;return false;}}return true;},IntPtr.Zero);return found;}
 public static ulong Number(IntPtr w,uint m){UIntPtr r;if(Query(w,m,UIntPtr.Zero,IntPtr.Zero,2,1000,out r)==IntPtr.Zero)throw new Exception("Own dashboard query failed");return r.ToUInt64();}
 public static bool Icon(IntPtr w){var id=new IconId{Size=(uint)Marshal.SizeOf(typeof(IconId)),Window=w,Number=1};Rect rect;return Shell_NotifyIconGetRect(ref id,out rect)==0&&rect.Right>rect.Left&&rect.Bottom>rect.Top;}
 public static string Text(IntPtr w){var all=new StringBuilder();EnumChildWindows(w,(c,p)=>{var b=new StringBuilder(16384);UIntPtr r;Read(c,13,(UIntPtr)b.Capacity,b,2,1000,out r);all.AppendLine(b.ToString());return true;},IntPtr.Zero);return all.ToString();}
}
'@}

$taskStartup=Invoke-OmenLifecycleRead 'startup-check' 'profile-startup-check'
if(-not$taskStartup.ok-or-not$taskStartup.data.ready){throw 'Startup readiness check failed; no profile or approval was changed.'}
$taskProfileId=[string]$taskStartup.data.selection.profile_id
if($taskProfileId-notmatch'^[a-z0-9][a-z0-9_-]{0,63}$'){throw 'Unexpected profile identifier.'}
$taskLive=Invoke-OmenLifecycleRead 'profile-live' ('profile-verify-current '+$taskProfileId)
$taskCpu=Invoke-OmenLifecycleRead 'cpu-read' 'cpu-hardware-read'
$taskGpu=Invoke-OmenLifecycleRead 'gpu-read' 'gpu-capabilities'
$taskBoot=[DateTimeOffset](Get-CimInstance Win32_OperatingSystem).LastBootUpTime
$taskStore=Join-Path ([Environment]::GetFolderPath('UserProfile')) '.omen-hub-pp'
$taskStartupResult=Get-Content -LiteralPath (Join-Path $taskStore 'last-startup-result.json') -Raw|ConvertFrom-Json
$taskSid=[Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$taskTask=Get-ScheduledTaskInfo -TaskName ('OmenHubPP-Profile-'+$taskSid)
$taskApp=$null;$taskWindow=[IntPtr]::Zero
foreach($taskCandidate in @(Get-Process -Name 'Omen Hub++','omen-hub-pp' -ErrorAction SilentlyContinue)){
 $taskHandle=[OmenLifecycleWindow]::Find([uint32]$taskCandidate.Id)
 if($taskHandle-ne[IntPtr]::Zero){if($taskApp){throw 'Multiple dashboards; cannot attribute lifecycle evidence.'};$taskApp=$taskCandidate;$taskWindow=$taskHandle}
}
if(-not$taskApp){throw 'Open the installed dashboard before preparing/checking this transition.'}
if($taskApp.Path-cne$taskExe){throw 'The dashboard executable differs from the selected release.'}
[void][OmenLifecycleWindow]::PostMessage($taskWindow,0x0111,[UIntPtr]::new([uint64]201),[IntPtr]::Zero)
$taskDeadline=(Get-Date).AddSeconds(45)
do{
 $taskText=[OmenLifecycleWindow]::Text($taskWindow)
 $taskReady=$taskText-match'(?s)CPU-Paketleistung\s+\d+[\.,]\d+ W.*CPU-Temperatur \(Tctl\)\s+\d+[\.,]\d+ °C.*Effektiv max\.: \d+ MHz.*32 logische CPUs'
 if($taskReady){break};Start-Sleep -Milliseconds 100
}while((Get-Date)-lt$taskDeadline)
[IO.File]::WriteAllText((Join-Path $taskCapture 'dashboard.txt'),$taskText,[Text.UTF8Encoding]::new($false))
$taskPower=[OmenLifecycleWindow]::Number($taskWindow,0x803F)
$taskSnapshot=[pscustomobject]@{
 schema=1;recorded_at_unix_seconds=[DateTimeOffset]::Now.ToUnixTimeSeconds();boot_unix_seconds=$taskBoot.ToUnixTimeSeconds()
 executable=$taskExe;binary_sha256=(Get-FileHash -LiteralPath $taskExe).Hash
 profile_id=$taskProfileId;profile_digest=$taskStartup.data.selection.profile_digest
 startup_ready=[bool]$taskStartup.data.ready;live_profile_verified=($taskLive.ok-and$taskLive.data.verified)
 app_pid=$taskApp.Id;reader_pid=[OmenLifecycleWindow]::Number($taskWindow,0x803D);power_generation=($taskPower-shr2)
 dashboard_ready=($taskReady-and($taskPower-band3)-eq0-and[OmenLifecycleWindow]::Icon($taskWindow));cpu_diagnostic_ok=$taskCpu.ok;gpu_diagnostic_ok=$taskGpu.ok
 startup_result_time=$taskStartupResult.recorded_at_unix_seconds
 startup_result_ok=($null-ne$taskStartupResult.result.Ok-and$taskStartupResult.selection_at_start.profile_digest-ceq$taskStartup.data.selection.profile_digest)
 startup_task_result=$taskTask.LastTaskResult;startup_task_time=([DateTimeOffset]$taskTask.LastRunTime).ToUnixTimeSeconds()
}
$taskSnapshot|ConvertTo-Json -Depth 6|Set-Content -LiteralPath (Join-Path $taskCapture 'snapshot.json') -Encoding UTF8
if($Phase-eq'Prepare'){
 if(-not$taskSnapshot.live_profile_verified-or-not$taskSnapshot.dashboard_ready-or-not$taskSnapshot.cpu_diagnostic_ok-or-not$taskSnapshot.gpu_diagnostic_ok-or$taskSnapshot.reader_pid-le0){throw ('Baseline is not ready; diagnostics retained: '+$taskCapture)}
 $taskSnapshot|ConvertTo-Json -Depth 6|Set-Content -LiteralPath (Join-Path $taskRun 'before.json') -Encoding UTF8
 [pscustomobject]@{prepared=$true;run_directory=$taskRun;automatic_sleep_or_restart=$false;hardware_writes=$false}|ConvertTo-Json
}else{
 $taskBefore=Get-Content -LiteralPath (Join-Path $taskRun 'before.json') -Raw|ConvertFrom-Json
 $taskEvents=@(Get-WinEvent -FilterHashtable @{LogName='System';ProviderName='Microsoft-Windows-Kernel-Power';Id=@(506,507);StartTime=[DateTimeOffset]::FromUnixTimeSeconds($taskBefore.recorded_at_unix_seconds).LocalDateTime} -ErrorAction SilentlyContinue|ForEach-Object{[pscustomobject]@{id=$_.Id;record_id=$_.RecordId;unix_seconds=([DateTimeOffset]$_.TimeCreated).ToUnixTimeSeconds()}})
 $taskEvents|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $taskCapture 'windows-events.json') -Encoding UTF8
 $taskVerdict=Test-OmenLifecycleEvidence $taskBefore $taskSnapshot $Phase $taskEvents
 $taskVerdict|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $taskCapture 'verdict.json') -Encoding UTF8
 $taskVerdict|ConvertTo-Json
 if(-not$taskVerdict.passed){throw ('Lifecycle checkpoint did not pass; see '+$taskCapture)}
}
