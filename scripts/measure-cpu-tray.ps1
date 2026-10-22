# Measure both our dashboard and its approved, paused read-only CPU helper.
#requires -Version 5.1
param([ValidateRange(5,55)][int]$DurationSeconds=30,[ValidateRange(0,5)][int]$ResumeCycles=2)
$ErrorActionPreference='Stop'
$env:OMEN_HUB_LANG='de'
$taskRepo=Split-Path -Parent $PSScriptRoot
$taskExe=Join-Path $taskRepo 'dist\Omen Hub++.exe'
if(-not('OmenCpuTrayMeasure'-as[type])){Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class OmenCpuTrayMeasure {
 public delegate bool EnumWindow(IntPtr hwnd,IntPtr arg);
 [DllImport("user32.dll")] static extern bool EnumWindows(EnumWindow cb,IntPtr arg);
 [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr hwnd,EnumWindow cb,IntPtr arg);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd,out uint owner);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetClassName(IntPtr hwnd,StringBuilder value,int length);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hwnd,uint message,UIntPtr value,IntPtr other);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
 [DllImport("user32.dll",EntryPoint="SendMessageTimeoutW",CharSet=CharSet.Unicode)] static extern IntPtr Read(IntPtr hwnd,uint msg,UIntPtr p,StringBuilder value,uint flags,uint timeout,out UIntPtr result);
 [DllImport("user32.dll",EntryPoint="SendMessageTimeoutW")] static extern IntPtr Query(IntPtr hwnd,uint msg,UIntPtr p,IntPtr other,uint flags,uint timeout,out UIntPtr result);
 public static uint ReaderPid(IntPtr hwnd){UIntPtr result;if(Query(hwnd,0x803D,UIntPtr.Zero,IntPtr.Zero,2,1000,out result)==IntPtr.Zero)throw new Exception("Owned diagnostic did not respond");return (uint)result.ToUInt64();}
 public static string AllText(IntPtr hwnd){var all=new StringBuilder();EnumChildWindows(hwnd,(h,a)=>{var text=new StringBuilder(16384);UIntPtr result;Read(h,13,(UIntPtr)text.Capacity,text,2,1000,out result);all.AppendLine(text.ToString());return true;},IntPtr.Zero);return all.ToString();}
 public static IntPtr Find(uint process){IntPtr found=IntPtr.Zero;EnumWindows((h,a)=>{uint owner;GetWindowThreadProcessId(h,out owner);var name=new StringBuilder(128);GetClassName(h,name,name.Capacity);if(owner==process&&name.ToString()=="OmenHubPlusPlusWindow"){found=h;return false;}return true;},IntPtr.Zero);return found;}
}
'@}
$taskParent=$null;$taskReader=$null;$taskResidents=@()
try{
 $taskResidents=@(Get-Process -Name 'Omen Hub++ Fan' -ErrorAction SilentlyContinue)
 $taskOtherApps=@(Get-Process -Name 'Omen Hub++','omen-hub-pp' -ErrorAction SilentlyContinue)
 if($taskOtherApps.Count-ne0){throw 'Another app or diagnostic is running; close it before the complete tray measurement.'}
 $taskFanTasks=@(Get-ScheduledTask -TaskName ('OmenHubPP-Fan-'+[Security.Principal.WindowsIdentity]::GetCurrent().User.Value) -ErrorAction SilentlyContinue)
 if($taskFanTasks.Count-ne0-and$taskResidents.Count-eq0){throw 'The installed fan helper is stopped; do not report a complete tray budget without it.'}
 $taskParent=Start-Process -FilePath $taskExe -WindowStyle Hidden -PassThru
 $taskDeadline=(Get-Date).AddSeconds(12);$taskWindow=[IntPtr]::Zero
 do{$taskWindow=[OmenCpuTrayMeasure]::Find([uint32]$taskParent.Id);if($taskWindow-ne[IntPtr]::Zero-and[OmenCpuTrayMeasure]::AllText($taskWindow)-match'Board: 8E35'){break};Start-Sleep -Milliseconds 80}while((Get-Date)-lt$taskDeadline)
 if($taskWindow-eq[IntPtr]::Zero){throw 'No owned dashboard.'}
 [void][OmenCpuTrayMeasure]::PostMessage($taskWindow,0x0111,[UIntPtr]::new([uint64]201),[IntPtr]::Zero)
 $taskShowDeadline=(Get-Date).AddSeconds(3)
 do{if([OmenCpuTrayMeasure]::IsWindowVisible($taskWindow)){break};Start-Sleep -Milliseconds 50}while((Get-Date)-lt$taskShowDeadline)
 if(-not[OmenCpuTrayMeasure]::IsWindowVisible($taskWindow)){throw 'Owned open-window phase is not visible.'}
 # CPU approval is automatic on opening. 106/107 select fan/graphics modes and must
 # never be sent by a read-only memory measurement.
 $taskDeadline=(Get-Date).AddSeconds(45)
 do{$taskText=[OmenCpuTrayMeasure]::AllText($taskWindow);if($taskText-match'(?s)CPU-Paketleistung\s+\d+[\.,]\d+ W.*Effektiv max\.: \d+ MHz.*32 logische CPUs'){break};Start-Sleep -Milliseconds 100}while((Get-Date)-lt$taskDeadline)
 if($taskText-notmatch'Effektiv max\.: \d+ MHz.*'-or$taskText-notmatch'32 logische CPUs'){throw ('CPU activation failed: '+$taskText)}
 $taskReaderPid=[OmenCpuTrayMeasure]::ReaderPid($taskWindow)
 if($taskReaderPid-eq0){throw 'Cannot identify approved helper; refuse parent-only measurement.'}
 $taskReader=Get-Process -Id $taskReaderPid
 if($taskReader.ProcessName-cne'Omen Hub++'){throw 'Diagnostic PID differs from our helper.'}
 for($taskCycle=0;$taskCycle-lt$ResumeCycles;$taskCycle++){
  [void][OmenCpuTrayMeasure]::PostMessage($taskWindow,0x0010,[UIntPtr]::Zero,[IntPtr]::Zero)
  Start-Sleep -Milliseconds 500
  [void][OmenCpuTrayMeasure]::PostMessage($taskWindow,0x0111,[UIntPtr]::new([uint64]201),[IntPtr]::Zero)
  $taskDeadline=(Get-Date).AddSeconds(12)
  do{$taskText=[OmenCpuTrayMeasure]::AllText($taskWindow);if($taskText-match'(?s)CPU-Paketleistung\s+\d+[\.,]\d+ W.*Effektiv max\.: \d+ MHz.*32 logische CPUs'){break};Start-Sleep -Milliseconds 100}while((Get-Date)-lt$taskDeadline)
  if($taskText-notmatch'(?s)CPU-Paketleistung\s+\d+[\.,]\d+ W.*Effektiv max\.: \d+ MHz.*32 logische CPUs'){throw ('CPU resume did not produce a fresh interval: '+$taskText)}
  if([OmenCpuTrayMeasure]::ReaderPid($taskWindow)-ne$taskReaderPid){throw 'Resume changed approved helper identity.'}
 }
 $taskParent.Refresh();$taskReader.Refresh()
 $taskOpenCombinedMB=[math]::Round(($taskParent.WorkingSet64+$taskReader.WorkingSet64)/1000000,2)
 foreach($taskResident in $taskResidents){$taskResident.Refresh()}
 $taskResidentOpenBytes=($taskResidents|Measure-Object -Property WorkingSet64 -Sum).Sum
 $taskOpenTotalMB=[math]::Round(($taskParent.WorkingSet64+$taskReader.WorkingSet64+$taskResidentOpenBytes)/1000000,2)
 [void][OmenCpuTrayMeasure]::PostMessage($taskWindow,0x0010,[UIntPtr]::Zero,[IntPtr]::Zero)
 Start-Sleep -Seconds 5
 if([OmenCpuTrayMeasure]::IsWindowVisible($taskWindow)){throw 'Owned dashboard is visible; refuse a tray label.'}
 $taskParent.Refresh();$taskReader.Refresh()
 if($taskParent.WorkingSet64-le0-or$taskReader.WorkingSet64-le0){throw 'Cannot query both working sets; refuse incomplete measurement.'}
 if($taskParent.HasExited-or$taskReader.HasExited){throw 'Expected both owned processes to be idle and alive.'}
 $taskParentStartCpu=$taskParent.TotalProcessorTime.TotalSeconds
 $taskReaderStartCpu=$taskReader.TotalProcessorTime.TotalSeconds
 $taskResidentStartCpu=@{};foreach($taskResident in $taskResidents){$taskResident.Refresh();$taskResidentStartCpu[$taskResident.Id]=$taskResident.TotalProcessorTime.TotalSeconds}
 Start-Sleep -Seconds ($DurationSeconds-5)
 $taskParent.Refresh();$taskReader.Refresh()
 $taskResidentsAfter=@(Get-Process -Name 'Omen Hub++ Fan' -ErrorAction SilentlyContinue)
 if((@($taskResidents.Id|Sort-Object)-join',')-cne(@($taskResidentsAfter.Id|Sort-Object)-join',')){throw 'The resident helper set changed during the sample.'}
 $taskResidentRecords=@(foreach($taskResident in $taskResidents){$taskResident.Refresh();if($taskResident.HasExited){throw 'A resident helper ended during measurement.'};[pscustomobject]@{Id=$taskResident.Id;Name=$taskResident.ProcessName;WorkingSetMB=[math]::Round($taskResident.WorkingSet64/1000000,2);WorkingSetBytes=$taskResident.WorkingSet64;CpuDelta=$taskResident.TotalProcessorTime.TotalSeconds-$taskResidentStartCpu[$taskResident.Id]}})
 $taskResidentBytes=($taskResidentRecords|Measure-Object -Property WorkingSetBytes -Sum).Sum
 $taskParentGpuLibraries=@($taskParent.Modules|Where-Object {$_.ModuleName-match'^(nvapi64(?:_impl)?|nvml)\.dll$'}|ForEach-Object {$_.ModuleName})
 if($taskParentGpuLibraries.Count-ne0){throw 'Dashboard retained NVIDIA libraries after isolated telemetry; inspect module lifetime.'}
 $taskRemainingReads=@(Get-CimInstance Win32_Process -Filter ('ParentProcessId = '+$taskParent.Id) -OperationTimeoutSec 5|Where-Object {$_.Name-eq'Omen Hub++.exe'})
 if($taskRemainingReads.Count-ne0){throw 'An owned read child remained during the tray measurement.'}
 $taskRecord=[pscustomobject]@{
  MeasuredAt=(Get-Date).ToString('o');DurationSeconds=$DurationSeconds
  ExecutableBytes=(Get-Item -LiteralPath $taskExe).Length
  ExecutableSHA256=(Get-FileHash -LiteralPath $taskExe).Hash
  OpenAppAndCpuReaderWorkingSetMB=$taskOpenCombinedMB
  OpenTotalIncludingResidentHelpersMB=$taskOpenTotalMB
  OpenSampleExcludesTransientReadChildren=$true
  WindowHidden=-not[OmenCpuTrayMeasure]::IsWindowVisible($taskWindow)
  ParentWorkingSetMiB=[math]::Round($taskParent.WorkingSet64/1MB,2)
  ReaderWorkingSetMiB=[math]::Round($taskReader.WorkingSet64/1MB,2)
  CombinedWorkingSetMiB=[math]::Round(($taskParent.WorkingSet64+$taskReader.WorkingSet64)/1MB,2)
  CombinedWorkingSetMB=[math]::Round(($taskParent.WorkingSet64+$taskReader.WorkingSet64)/1000000,2)
  TotalTrayWorkingSetMB=[math]::Round(($taskParent.WorkingSet64+$taskReader.WorkingSet64+$taskResidentBytes)/1000000,2)
  TotalTrayWithin50MBBudget=(($taskParent.WorkingSet64+$taskReader.WorkingSet64+$taskResidentBytes)-le50000000)
  IncludesAllResidentHelpers=$true
  ResidentHelpers=$taskResidentRecords
  ParentPrivateMiB=[math]::Round($taskParent.PrivateMemorySize64/1MB,2)
  ReaderPrivateMiB=[math]::Round($taskReader.PrivateMemorySize64/1MB,2)
  ParentCpuDelta=$taskParent.TotalProcessorTime.TotalSeconds-$taskParentStartCpu
  ReaderCpuDelta=$taskReader.TotalProcessorTime.TotalSeconds-$taskReaderStartCpu
  ParentNvidiaLibraries=$taskParentGpuLibraries
  RemainingReadChildren=$taskRemainingReads.Count
  PausedReader=$true;ResumeCycles=$ResumeCycles;WorkingSetTrimmed=$false;SharedPagesCountedTwice=$true
 }
 $taskRecord|ConvertTo-Json -Depth 5|Set-Content -LiteralPath (Join-Path $taskRepo '.local\cpu-tray-measurement.json') -Encoding UTF8
 $taskRecord|ConvertTo-Json -Depth 5
 [void][OmenCpuTrayMeasure]::PostMessage($taskWindow,0x0111,[UIntPtr]::new([uint64]202),[IntPtr]::Zero)
 if(-not$taskParent.WaitForExit(3000)-or-not$taskReader.WaitForExit(3000)){throw 'Owned app/helper did not exit cleanly.'}
 if(-not$taskRecord.TotalTrayWithin50MBBudget){throw 'Total tray working set including all resident helpers exceeds the 50 MB budget; the complete measurement was retained.'}
}finally{
 if($taskParent){$taskParent.Refresh();if(-not$taskParent.HasExited){Stop-Process -Id $taskParent.Id};$taskParent.Dispose()}
 if($taskReader){$taskReader.Dispose()}
 foreach($taskResident in $taskResidents){$taskResident.Dispose()}
}
