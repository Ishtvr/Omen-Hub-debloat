# Owned native UI integration; fixed read-only CPU helper uses explicit Windows UAC.
#requires -Version 5.1
param()
$ErrorActionPreference='Stop'
# UI labels below are German; pin the app language so the checks stay stable.
$env:OMEN_HUB_LANG = 'de'
$taskRepo=Split-Path -Parent $PSScriptRoot
$taskExe=Join-Path $taskRepo 'dist\Omen Hub++.exe'
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
$taskProcess=$null
try{
 $taskBefore=& $taskExe gpu-curve-read|Out-String|ConvertFrom-Json
 if($LASTEXITCODE-ne0){throw 'GPU baseline GET failed.'}
 $taskProcess=Start-Process -FilePath $taskExe -PassThru -WindowStyle Hidden
 $taskEnd=(Get-Date).AddSeconds(12);$taskWindow=[IntPtr]::Zero
 do{$taskProcess.Refresh();if($taskProcess.HasExited){throw 'Dashboard exited early.'};$taskWindow=[OmenCpuSensorSmoke]::Find([uint32]$taskProcess.Id);if($taskWindow-ne[IntPtr]::Zero){break};Start-Sleep -Milliseconds 80}while((Get-Date)-lt$taskEnd)
 if($taskWindow-eq[IntPtr]::Zero){throw 'No owned dashboard.'}
 Wait-CpuText $taskWindow 'Board: 8E35'|Out-Null
 $taskInitial=[OmenCpuSensorSmoke]::AllText($taskWindow)
 if($taskInitial-notmatch'CPU-Paketleistung\s+Nicht verfügbar'-or$taskInitial-notmatch'CPU-Temperatur \(Tctl\)\s+Nicht verfügbar'){throw 'CPU hardware readings appeared before explicit approval.'}
 foreach($taskNotification in @(5,6,7)){
  [void][OmenCpuSensorSmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]::new([uint64](106+($taskNotification-shl16))),[OmenCpuSensorSmoke]::GetDlgItem($taskWindow,106))
 }
 Start-Sleep -Milliseconds 150
 if([OmenCpuSensorSmoke]::Text([OmenCpuSensorSmoke]::GetDlgItem($taskWindow,106))-cne'CPU-Sensoren …'){throw 'A non-click notification started CPU authorization.'}
 Send-CpuCommand $taskWindow 106
 $taskLive=Wait-CpuText $taskWindow '(?s)CPU-Paketleistung\s+\d+[\.,]\d+ W.*Mittelwert' 45
 if($taskLive-notmatch'CPU-Temperatur \(Tctl\)\s+\d+[\.,]\d+ °C'-or$taskLive-notmatch'Quelle: CPU-Energiezähler und Tctl über PawnIO'){throw 'CPU provenance/temperature semantics are missing.'}
 $taskLive|Set-Content -LiteralPath (Join-Path $taskRepo '.local\cpu-ui-live-text.txt') -Encoding UTF8
 # Closing to tray pauses the existing helper without another UAC prompt.
 [void][OmenCpuSensorSmoke]::PostMessage($taskWindow,0x0010,[UIntPtr]::Zero,[IntPtr]::Zero)
 Start-Sleep -Milliseconds 500
 $taskProcess.Refresh();if($taskProcess.HasExited){throw 'Close exited instead of pausing in tray.'}
 Send-CpuCommand $taskWindow 201
 Wait-CpuText $taskWindow 'CPU-Paketleistung\s+Wird gemessen'|Out-Null
 Wait-CpuText $taskWindow '(?s)CPU-Paketleistung\s+\d+[\.,]\d+ W.*Mittelwert'|Out-Null
 # Dispatch our own power-event handler; this does not suspend the computer.
 [void][OmenCpuSensorSmoke]::PostMessage($taskWindow,0x0218,[UIntPtr]::new([uint64]4),[IntPtr]::Zero)
 Start-Sleep -Milliseconds 300
 [void][OmenCpuSensorSmoke]::PostMessage($taskWindow,0x0218,[UIntPtr]::new([uint64]18),[IntPtr]::Zero)
 Wait-CpuText $taskWindow 'CPU-Paketleistung\s+Wird gemessen'|Out-Null
 Wait-CpuText $taskWindow '(?s)CPU-Paketleistung\s+\d+[\.,]\d+ W.*Mittelwert'|Out-Null
 Send-CpuCommand $taskWindow 106
 Wait-CpuText $taskWindow 'CPU-Paketleistung\s+Nicht verfügbar'|Out-Null
 $taskAfter=& $taskExe gpu-curve-read|Out-String|ConvertFrom-Json
 $taskControlsBefore=@($taskBefore.nodes|ForEach-Object{@($_.index,$_.voltage_uv,$_.delta_khz)})|ConvertTo-Json -Depth 5 -Compress
 $taskControlsAfter=@($taskAfter.nodes|ForEach-Object{@($_.index,$_.voltage_uv,$_.delta_khz)})|ConvertTo-Json -Depth 5 -Compress
 if($taskControlsBefore-cne$taskControlsAfter){throw 'GPU controls changed.'}
 Send-CpuCommand $taskWindow 202
 if(-not$taskProcess.WaitForExit(3000)-or$taskProcess.ExitCode-ne0){throw 'Dashboard failed to exit cleanly.'}
 [pscustomobject]@{measured_at=(Get-Date).ToString('o');binary_sha256=(Get-FileHash -LiteralPath $taskExe).Hash;explicit_uac_cpu_enable=$true;real_energy_mean_and_tctl=$true;provenance=$true;tray_resume_new_baseline=$true;simulated_suspend_resume_new_baseline=$true;actual_computer_suspend=$false;disable_clears_values=$true;gpu_controls_unchanged=$true;hardware_tuning_writes=$false;clean_exit=$true}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $taskRepo '.local\cpu-ui-smoke.json') -Encoding UTF8
 'Native CPU sensors, explicit approval, mean package watts/Tctl, pause/resume, fresh baselines, disable and clean exit passed.'
}finally{
 if($taskProcess){$taskProcess.Refresh();if(-not$taskProcess.HasExited){Stop-Process -Id $taskProcess.Id};$taskProcess.Dispose()}
}
