# Owned native UI integration; fixed read-only CPU helper uses explicit Windows UAC.
#requires -Version 5.1
param([string]$CaptureDirectory,[ValidateRange(1,20)][int]$ResumeCycles=3)
$ErrorActionPreference='Stop'
# UI labels below are German; pin the app language so the checks stay stable.
$env:OMEN_HUB_LANG = 'de'
$taskRepo=Split-Path -Parent $PSScriptRoot
$taskExe=Join-Path $taskRepo 'dist\Omen Hub++.exe'
New-Item -ItemType Directory -Force -Path (Join-Path $taskRepo '.local') | Out-Null
if(-not('OmenCpuSensorSmoke'-as[type])){Add-Type @'
using System;
using System.Text;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class OmenCpuSensorSmoke {
 public delegate bool EnumWindow(IntPtr hwnd,IntPtr arg);
 [DllImport("user32.dll")] static extern bool EnumWindows(EnumWindow cb,IntPtr arg);
 [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr hwnd,EnumWindow cb,IntPtr arg);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd,out uint owner);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetClassName(IntPtr hwnd,StringBuilder value,int length);
 [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr hwnd,int id);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hwnd,uint message,UIntPtr value,IntPtr other);
 [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr hwnd,int x,int y,int width,int height,bool repaint);
 [DllImport("user32.dll",EntryPoint="SendMessageTimeoutW")] static extern IntPtr Query(IntPtr hwnd,uint msg,UIntPtr p,IntPtr other,uint flags,uint timeout,out UIntPtr result);
 public static uint ReaderPid(IntPtr hwnd){UIntPtr result;if(Query(hwnd,0x803D,UIntPtr.Zero,IntPtr.Zero,2,1000,out result)==IntPtr.Zero)throw new Exception("Owned diagnostic did not respond");return (uint)result.ToUInt64();}
 public static bool Checked(IntPtr control){UIntPtr result;if(Query(control,0x00F0,UIntPtr.Zero,IntPtr.Zero,2,1000,out result)==IntPtr.Zero)throw new Exception("Owned setting did not respond");return result.ToUInt64()==1;}
 [DllImport("user32.dll",EntryPoint="SendMessageTimeoutW",CharSet=CharSet.Unicode)] static extern IntPtr Read(IntPtr hwnd,uint msg,UIntPtr p,StringBuilder value,uint flags,uint timeout,out UIntPtr result);
 public static string Text(IntPtr hwnd){var text=new StringBuilder(16384);UIntPtr result;Read(hwnd,13,(UIntPtr)text.Capacity,text,2,1000,out result);return text.ToString();}
 public static string AllText(IntPtr hwnd){var values=new List<string>();EnumChildWindows(hwnd,(h,a)=>{values.Add(Text(h));return true;},IntPtr.Zero);return string.Join("\n",values);}
 public static IntPtr Find(uint process){IntPtr found=IntPtr.Zero;EnumWindows((h,a)=>{uint owner;GetWindowThreadProcessId(h,out owner);var name=new StringBuilder(128);GetClassName(h,name,name.Capacity);if(owner==process&&name.ToString()=="OmenHubPlusPlusWindow"){found=h;return false;}return true;},IntPtr.Zero);return found;}
}
'@}
function Wait-CpuText {param([IntPtr]$Window,[string]$Pattern,[int]$Seconds=15)
 $taskEnd=(Get-Date).AddSeconds($Seconds)
 do{$taskText=[OmenCpuSensorSmoke]::AllText($Window);if($taskText-match$Pattern){return $taskText};Start-Sleep -Milliseconds 80}while((Get-Date)-lt$taskEnd)
 throw ('Missing CPU native state: '+$Pattern+' / '+$taskText)
}
function Send-CpuCommand {param([IntPtr]$Window,[uint64]$Command)
 [void][OmenCpuSensorSmoke]::PostMessage($Window,0x0111,[UIntPtr]::new($Command),[IntPtr]::Zero)
}
$taskProcess=$null;$taskReader=$null
try{
 # Public NVML offset GETs remain available across driver updates; do not
 # bypass the exact driver binding of the separate private V/F adapter.
 $taskBefore=& $taskExe gpu-capabilities|Out-String|ConvertFrom-Json
 if($LASTEXITCODE-ne0){throw 'GPU baseline GET failed.'}
 if($null-eq$taskBefore.gpu.core.Ok-or$null-eq$taskBefore.gpu.memory.Ok){throw 'Public GPU baseline offsets unavailable.'}
 $taskProcess=Start-Process -FilePath $taskExe -PassThru -WindowStyle Hidden
 $taskEnd=(Get-Date).AddSeconds(12);$taskWindow=[IntPtr]::Zero
 do{$taskProcess.Refresh();if($taskProcess.HasExited){throw 'Dashboard exited early.'};$taskWindow=[OmenCpuSensorSmoke]::Find([uint32]$taskProcess.Id);if($taskWindow-ne[IntPtr]::Zero){break};Start-Sleep -Milliseconds 80}while((Get-Date)-lt$taskEnd)
 if($taskWindow-eq[IntPtr]::Zero){throw 'No owned dashboard.'}
 Wait-CpuText $taskWindow 'Board: 8E35'|Out-Null
 if(-not[OmenCpuSensorSmoke]::Checked([OmenCpuSensorSmoke]::GetDlgItem($taskWindow,191))){throw 'Close-to-tray is disabled; refusing to change the user preference for this sensor smoke.'}
 foreach($taskControl in @(108,190,191)){if([OmenCpuSensorSmoke]::GetDlgItem($taskWindow,$taskControl)-eq[IntPtr]::Zero){throw 'Current dashboard Apply/settings controls are missing.'}}
 # The dashboard now requests read-only CPU approval on first opening. 106/107
 # select fan/graphics modes: never send them as part of a sensor smoke.
 $taskLive=Wait-CpuText $taskWindow '(?s)CPU-Paketleistung\s+\d+[\.,]\d+ W.*Effektiv max\.: \d+ MHz.*32 logische CPUs · Mittel über' 45
 if($taskLive-notmatch'CPU-Temperatur \(Tctl\)\s+\d+[\.,]\d+ °C'-or$taskLive-notmatch'Windows-Takt: \d+ MHz'){throw 'CPU temperature/clock semantics are missing.'}
 $taskReaderPid=[OmenCpuSensorSmoke]::ReaderPid($taskWindow)
 if($taskReaderPid-eq0){throw 'Approved CPU reader PID is missing.'}
 $taskReader=Get-Process -Id $taskReaderPid
 $taskLive|Set-Content -LiteralPath (Join-Path $taskRepo '.local\cpu-ui-live-text.txt') -Encoding UTF8
 # Closing to tray pauses the existing helper without another UAC prompt.
 for($taskCycle=0;$taskCycle-lt$ResumeCycles;$taskCycle++){
  [void][OmenCpuSensorSmoke]::PostMessage($taskWindow,0x0010,[UIntPtr]::Zero,[IntPtr]::Zero)
  Start-Sleep -Milliseconds 500
  $taskProcess.Refresh();if($taskProcess.HasExited){throw 'Close exited instead of pausing in tray.'}
  Send-CpuCommand $taskWindow 201
  Wait-CpuText $taskWindow '(?s)CPU-Paketleistung\s+Wird gemessen.*Effektivtakt wird gemessen'|Out-Null
  Wait-CpuText $taskWindow '(?s)CPU-Paketleistung\s+\d+[\.,]\d+ W.*Effektiv max\.: \d+ MHz.*32 logische CPUs'|Out-Null
  if([OmenCpuSensorSmoke]::ReaderPid($taskWindow)-ne$taskReaderPid){throw 'Tray resume replaced the approved helper.'}
 }
 # Dispatch our own power-event handler; this does not suspend the computer.
 [void][OmenCpuSensorSmoke]::PostMessage($taskWindow,0x0218,[UIntPtr]::new([uint64]4),[IntPtr]::Zero)
 Start-Sleep -Milliseconds 300
 [void][OmenCpuSensorSmoke]::PostMessage($taskWindow,0x0218,[UIntPtr]::new([uint64]18),[IntPtr]::Zero)
 Wait-CpuText $taskWindow 'CPU-Paketleistung\s+Wird gemessen'|Out-Null
 Wait-CpuText $taskWindow 'Effektivtakt wird gemessen'|Out-Null
 Wait-CpuText $taskWindow '(?s)CPU-Paketleistung\s+\d+[\.,]\d+ W.*Effektiv max\.: \d+ MHz.*32 logische CPUs'|Out-Null
 if([OmenCpuSensorSmoke]::ReaderPid($taskWindow)-ne$taskReaderPid){throw 'Simulated resume replaced the approved helper.'}
 # Refresh also drops both measurement baselines; it does not touch a profile.
 Send-CpuCommand $taskWindow 101
 Wait-CpuText $taskWindow 'Effektivtakt wird gemessen'|Out-Null
 Wait-CpuText $taskWindow '(?s)CPU-Paketleistung\s+\d+[\.,]\d+ W.*Effektiv max\.: \d+ MHz.*32 logische CPUs'|Out-Null
 if($CaptureDirectory){
  New-Item -ItemType Directory -Force -Path $CaptureDirectory|Out-Null
  Add-Type -AssemblyName System.Drawing
  if(-not('OmenCpuCapture'-as[type])){Add-Type -ReferencedAssemblies System.Drawing @'
using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
public static class OmenCpuCapture {
 [StructLayout(LayoutKind.Sequential)] public struct Rect {public int left,top,right,bottom;}
 [DllImport("user32.dll")] static extern bool GetClientRect(IntPtr window,out Rect rect);
 [DllImport("user32.dll")] static extern bool PrintWindow(IntPtr window,IntPtr dc,uint flags);
 public static void Save(IntPtr window,string path){Rect r;if(!GetClientRect(window,out r))throw new Exception("No owned client");using(var bitmap=new Bitmap(r.right,r.bottom)){using(var graphics=Graphics.FromImage(bitmap)){IntPtr dc=graphics.GetHdc();try{if(!PrintWindow(window,dc,1))throw new Exception("Owned client render failed");}finally{graphics.ReleaseHdc(dc);}}bitmap.Save(path,ImageFormat.Png);}}
}
'@}
  foreach($taskSize in @(@(1140,820),@(960,790))){
   [void][OmenCpuSensorSmoke]::MoveWindow($taskWindow,80,80,$taskSize[0],$taskSize[1],$true)
   Start-Sleep -Milliseconds 150
   [OmenCpuCapture]::Save($taskWindow,(Join-Path $CaptureDirectory ('cpu-clocks-'+$taskSize[0]+'.png')))
  }
 }
 $taskAfter=& $taskExe gpu-capabilities|Out-String|ConvertFrom-Json
 if($LASTEXITCODE-ne0){throw 'GPU final public GET failed.'}
 $taskControlsBefore=@($taskBefore.gpu.identity,$taskBefore.gpu.core,$taskBefore.gpu.memory)|ConvertTo-Json -Depth 8 -Compress
 $taskControlsAfter=@($taskAfter.gpu.identity,$taskAfter.gpu.core,$taskAfter.gpu.memory)|ConvertTo-Json -Depth 8 -Compress
 if($taskControlsBefore-cne$taskControlsAfter){throw 'GPU controls changed.'}
 Send-CpuCommand $taskWindow 202
 if(-not$taskProcess.WaitForExit(3000)-or$taskProcess.ExitCode-ne0){throw 'Dashboard failed to exit cleanly.'}
 if(-not$taskReader.WaitForExit(3000)){throw 'Approved reader failed to exit with its parent.'}
 [pscustomobject]@{measured_at=(Get-Date).ToString('o');binary_sha256=(Get-FileHash -LiteralPath $taskExe).Hash;automatic_read_only_uac=$true;real_energy_mean_and_tctl=$true;effective_clock_32_processors=$true;windows_clock_separate=$true;tray_resume_cycles=$ResumeCycles;tray_resume_new_baseline=$true;simulated_suspend_resume_new_baseline=$true;refresh_new_baseline=$true;same_approved_helper=$true;actual_computer_suspend=$false;public_gpu_offsets_unchanged=$true;private_gpu_curve_checked=$false;hardware_tuning_writes=$false;clean_exit=$true}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $taskRepo '.local\cpu-ui-smoke.json') -Encoding UTF8
 'Native mean CPU watts/Tctl, effective clocks, separate Windows MHz, tray/simulated suspend/refresh baselines and clean exit passed.'
}finally{
 if($taskProcess){$taskProcess.Refresh();if(-not$taskProcess.HasExited){Stop-Process -Id $taskProcess.Id};$taskProcess.Dispose()}
 if($taskReader){$taskReader.Dispose()}
}
