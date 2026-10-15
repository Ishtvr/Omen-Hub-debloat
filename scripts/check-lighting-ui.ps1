# Actual, bounded lighting UI qualification. Requires Windows administrator access.
# Toggles only qualified keyboard lighting, changes one zone, and restores its
# complete original image. Main tuning journal/startup and GPU controls are checked.
#requires -Version 5.1
param([switch]$SwitchOnly,[ValidateRange(1,10)][int]$HoldSeconds=4)
$ErrorActionPreference='Stop'
$taskRepo=Split-Path -Parent $PSScriptRoot
$taskExe=Join-Path $taskRepo 'dist\Omen Hub++.exe'
$taskRoot=Join-Path $taskRepo '.local\research\keyboard-direct'
$taskRecovery=Join-Path $taskRepo '.tools\keyboard-direct-proof-target\release\omen-keyboard-direct-recovery.exe'
$taskIdentity=[Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())
if(-not$taskIdentity.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Run the lighting UI qualification in an administrator PowerShell.'}
if(-not(Test-Path -LiteralPath $taskRecovery)){throw 'Bounded qualification recovery helper has not been built.'}
New-Item -ItemType Directory -Force -Path $taskRoot|Out-Null
if(-not('OmenLightingUiProof'-as[type])){
Add-Type @'
using System;
using System.Text;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class OmenLightingUiProof {
 public delegate bool EnumWindow(IntPtr hwnd,IntPtr arg);
 [DllImport("user32.dll")] static extern bool EnumWindows(EnumWindow cb,IntPtr arg);
 [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr hwnd,EnumWindow cb,IntPtr arg);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd,out uint owner);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetClassName(IntPtr hwnd,StringBuilder value,int length);
 [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr hwnd,int id);
 [DllImport("user32.dll")] public static extern bool IsWindowEnabled(IntPtr hwnd);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hwnd,uint message,UIntPtr value,IntPtr other);
 [DllImport("user32.dll",EntryPoint="SendMessageTimeoutW",CharSet=CharSet.Unicode)] static extern IntPtr Read(IntPtr hwnd,uint msg,UIntPtr p,StringBuilder value,uint flags,uint timeout,out UIntPtr result);
 [DllImport("user32.dll",EntryPoint="SendMessageTimeoutW",CharSet=CharSet.Unicode)] static extern IntPtr Write(IntPtr hwnd,uint msg,UIntPtr p,string value,uint flags,uint timeout,out UIntPtr result);
 [DllImport("user32.dll",EntryPoint="SendMessageTimeoutW")] static extern IntPtr Send(IntPtr hwnd,uint msg,UIntPtr p,IntPtr other,uint flags,uint timeout,out UIntPtr result);
 public static void SetText(IntPtr hwnd,string value){UIntPtr result;if(Write(hwnd,12,UIntPtr.Zero,value,2,1000,out result)==IntPtr.Zero)throw new Exception("Owned field did not answer");}
 public static string Text(IntPtr hwnd){var text=new StringBuilder(8192);UIntPtr result;Read(hwnd,13,(UIntPtr)text.Capacity,text,2,1000,out result);return text.ToString();}
 public static ulong Message(IntPtr hwnd,uint msg,ulong p=0,long other=0){UIntPtr result;if(Send(hwnd,msg,(UIntPtr)p,(IntPtr)other,2,1000,out result)==IntPtr.Zero)throw new Exception("Owned control did not answer");return result.ToUInt64();}
 public static string AllText(IntPtr hwnd){var values=new List<string>();EnumChildWindows(hwnd,(h,a)=>{values.Add(Text(h));return true;},IntPtr.Zero);return string.Join("\n",values);}
 public static IntPtr ColorDialog(uint process){IntPtr found=IntPtr.Zero;EnumWindows((h,a)=>{uint owner;GetWindowThreadProcessId(h,out owner);var name=new StringBuilder(128);GetClassName(h,name,name.Capacity);if(owner==process&&name.ToString()=="#32770"){found=h;return false;}return true;},IntPtr.Zero);return found;}
 public static IntPtr Find(uint process){IntPtr found=IntPtr.Zero;EnumWindows((h,a)=>{uint owner;GetWindowThreadProcessId(h,out owner);var name=new StringBuilder(128);GetClassName(h,name,name.Capacity);if(owner==process&&name.ToString()=="OmenHubPPKeyboardEditor"){found=h;return false;}return true;},IntPtr.Zero);return found;}
}
'@
}
function Read-LightJson {param([string[]]$Arguments);$taskRaw=& $taskExe @Arguments|Out-String;if($LASTEXITCODE-ne0){throw ('Read failed: '+($Arguments-join' '))};$taskRaw|ConvertFrom-Json}
function Wait-LightIdle {param([IntPtr]$Window)
 $taskUntil=(Get-Date).AddSeconds(15)
 do{if([OmenLightingUiProof]::IsWindowEnabled([OmenLightingUiProof]::GetDlgItem($Window,600))-and[OmenLightingUiProof]::Text([OmenLightingUiProof]::GetDlgItem($Window,600))-match'^(Ein|Aus)$'){return};Start-Sleep -Milliseconds 60}while((Get-Date)-lt$taskUntil)
 throw ('Lighting did not become ready: '+[OmenLightingUiProof]::AllText($Window))
}
function Click-Light {param([IntPtr]$Window,[int]$Id)
 [void][OmenLightingUiProof]::PostMessage($Window,0x0111,[UIntPtr]::new([uint64]$Id),[OmenLightingUiProof]::GetDlgItem($Window,$Id))
}
function Color-Dialog {param([IntPtr]$Window,[uint32]$Owner)
 Click-Light $Window 610
 $taskUntil=(Get-Date).AddSeconds(6)
 do{$taskDialog=[OmenLightingUiProof]::ColorDialog($Owner);if($taskDialog-ne[IntPtr]::Zero){return $taskDialog};Start-Sleep -Milliseconds 50}while((Get-Date)-lt$taskUntil)
 throw 'Zone did not open our native color dialog.'
}
function Set-Color {param([IntPtr]$Window,[uint32]$Owner,[int[]]$Color)
 $taskDialog=Color-Dialog $Window $Owner
 # COLOR_RED/GREEN/BLUE706..708 are verified in the local Windows SDK colordlg.h.
 for($taskI=0;$taskI-lt3;$taskI++){$taskEdit=[OmenLightingUiProof]::GetDlgItem($taskDialog,706+$taskI);if($taskEdit-eq[IntPtr]::Zero){throw 'System RGB editor missing.'};[OmenLightingUiProof]::SetText($taskEdit,[string]$Color[$taskI])}
 [void][OmenLightingUiProof]::PostMessage($taskDialog,0x0111,[UIntPtr]::new([uint64]1),[IntPtr]::Zero)
 Start-Sleep -Milliseconds 150
 Wait-LightIdle $Window
}
function Control-Image {param($Curve);@($Curve.nodes|ForEach-Object{@($_.index,$_.voltage_uv,$_.delta_khz)})|ConvertTo-Json -Depth 5 -Compress}
$taskBefore=Read-LightJson @('keyboard-read')
if(($taskBefore.capability.raw-join',')-cne'1,0,0,0'-or$taskBefore.legacy.brightness_raw_byte-notin@(100,228)){throw 'Unexpected original keyboard; no UI write attempted.'}
$taskBefore|ConvertTo-Json -Depth 12|Set-Content -LiteralPath (Join-Path $taskRoot 'gui-original.json') -Encoding UTF8
$taskOriginalOn=$taskBefore.legacy.brightness_raw_byte-eq228
$taskOriginalColor=@([int]$taskBefore.legacy.colors_rgb[0].r,[int]$taskBefore.legacy.colors_rgb[0].g,[int]$taskBefore.legacy.colors_rgb[0].b)
$taskGpuBefore=Read-LightJson @('gpu-curve-read')
$taskPowerBefore=Read-LightJson @('hp-read')
$taskStorage=Read-LightJson @('profile-storage')
$taskFiles=@('control-journal.json','startup.json')|ForEach-Object{Join-Path $taskStorage.directory $_}
$taskHashesBefore=@($taskFiles|ForEach-Object{if(Test-Path -LiteralPath $_){(Get-FileHash -LiteralPath $_).Hash}else{'absent'}})
$taskProcess=$null;$taskRestored=$false;$taskWindow=[IntPtr]::Zero
try{
 $taskProcess=Start-Process -FilePath $taskExe -ArgumentList 'keyboard-ui' -PassThru -WindowStyle Hidden
 $taskUntil=(Get-Date).AddSeconds(12)
 do{$taskWindow=[OmenLightingUiProof]::Find([uint32]$taskProcess.Id);if($taskWindow-ne[IntPtr]::Zero){break};Start-Sleep -Milliseconds 60}while((Get-Date)-lt$taskUntil)
 if($taskWindow-eq[IntPtr]::Zero){throw 'No owned lighting editor.'}
 Wait-LightIdle $taskWindow
 for($taskI=0;$taskI-lt4;$taskI++){if([OmenLightingUiProof]::GetDlgItem($taskWindow,610+$taskI)-eq[IntPtr]::Zero){throw 'Missing native zone target.'}}
 if([OmenLightingUiProof]::GetDlgItem($taskWindow,400)-ne[IntPtr]::Zero-or[OmenLightingUiProof]::AllText($taskWindow)-match'Firmware prüfen|Gesamtes Profil|Original wiederherstellen|Speichern'){throw 'Legacy text-heavy controls remain.'}
 # Non-click button notifications must never toggle firmware or open pickers.
 foreach($taskNotification in @(4,5,6,7)){
  [void][OmenLightingUiProof]::PostMessage($taskWindow,0x0111,[UIntPtr]::new([uint64](600+($taskNotification-shl16))),[OmenLightingUiProof]::GetDlgItem($taskWindow,600))
 }
 Start-Sleep -Milliseconds 150
 if((Read-LightJson @('keyboard-read')|ConvertTo-Json -Depth 12 -Compress)-cne($taskBefore|ConvertTo-Json -Depth 12 -Compress)){throw 'Non-click notification changed lighting.'}
 if(-not$taskOriginalOn){Click-Light $taskWindow 600;Start-Sleep -Milliseconds 150;Wait-LightIdle $taskWindow}
 $taskOn=Read-LightJson @('keyboard-read')
 if($taskOn.legacy.brightness_raw_byte-ne228){throw 'On did not read back E4.'}
 if($SwitchOnly){
  Start-Sleep -Seconds $HoldSeconds
  Click-Light $taskWindow 600;Start-Sleep -Milliseconds 150;Wait-LightIdle $taskWindow
  if((Read-LightJson @('keyboard-read')).legacy.brightness_raw_byte-ne100){throw 'Observed Off did not read back64.'}
  Start-Sleep -Seconds $HoldSeconds
  Click-Light $taskWindow 600;Start-Sleep -Milliseconds 150;Wait-LightIdle $taskWindow
  if((Read-LightJson @('keyboard-read')).legacy.brightness_raw_byte-ne228){throw 'Observed second On did not read back E4.'}
  Start-Sleep -Seconds $HoldSeconds
  if(-not$taskOriginalOn){Click-Light $taskWindow 600;Start-Sleep -Milliseconds 150;Wait-LightIdle $taskWindow}
 }else{
 $taskDialog=Color-Dialog $taskWindow ([uint32]$taskProcess.Id)
 for($taskI=0;$taskI-lt3;$taskI++){if([OmenLightingUiProof]::Text([OmenLightingUiProof]::GetDlgItem($taskDialog,706+$taskI))-ne[string]$taskOriginalColor[$taskI]){throw 'Picker did not load the selected zone RGB.'}}
 [void][OmenLightingUiProof]::PostMessage($taskDialog,0x0111,[UIntPtr]::new([uint64]2),[IntPtr]::Zero)
 Start-Sleep -Milliseconds 100
 if((Read-LightJson @('keyboard-read')|ConvertTo-Json -Depth 12 -Compress)-cne($taskOn|ConvertTo-Json -Depth 12 -Compress)){throw 'Picker cancellation changed firmware.'}
 Set-Color $taskWindow ([uint32]$taskProcess.Id) @(255,80,80)
 $taskChanged=Read-LightJson @('keyboard-read')
 if(($taskChanged.legacy.colors_rgb[0].r-ne255)-or($taskChanged.legacy.colors_rgb[0].g-ne80)-or($taskChanged.legacy.colors_rgb[0].b-ne80)){throw 'Confirmed zone color was not applied.'}
 for($taskI=1;$taskI-lt4;$taskI++){if(($taskChanged.legacy.colors_rgb[$taskI]|ConvertTo-Json -Compress)-cne($taskBefore.legacy.colors_rgb[$taskI]|ConvertTo-Json -Compress)){throw 'Another zone changed.'}}
 Start-Sleep -Milliseconds 1400
 if((Read-LightJson @('keyboard-read')|ConvertTo-Json -Depth 12 -Compress)-cne($taskChanged|ConvertTo-Json -Depth 12 -Compress)){throw 'Color did not remain during the held read.'}
 Set-Color $taskWindow ([uint32]$taskProcess.Id) $taskOriginalColor
 Click-Light $taskWindow 600;Start-Sleep -Milliseconds 150;Wait-LightIdle $taskWindow
 $taskOff=Read-LightJson @('keyboard-read')
 if($taskOff.legacy.brightness_raw_byte-ne100){throw 'Off did not read back64.'}
 for($taskI=0;$taskI-lt4;$taskI++){if([OmenLightingUiProof]::IsWindowEnabled([OmenLightingUiProof]::GetDlgItem($taskWindow,610+$taskI))){throw 'Off retained an editable zone target.'}}
 if($taskOriginalOn){Click-Light $taskWindow 600;Start-Sleep -Milliseconds 150;Wait-LightIdle $taskWindow}
 }
 $taskAfter=Read-LightJson @('keyboard-read')
 $taskRestored=(($taskBefore|ConvertTo-Json -Depth 12 -Compress)-ceq($taskAfter|ConvertTo-Json -Depth 12 -Compress))
 if(-not$taskRestored){throw 'Complete original lighting image did not restore.'}
 $taskGpuAfter=Read-LightJson @('gpu-curve-read');$taskPowerAfter=Read-LightJson @('hp-read')
 $taskHashesAfter=@($taskFiles|ForEach-Object{if(Test-Path -LiteralPath $_){(Get-FileHash -LiteralPath $_).Hash}else{'absent'}})
 $taskGpuSame=(Control-Image $taskGpuBefore)-ceq(Control-Image $taskGpuAfter)
 $taskMetadataSame=($taskHashesBefore-join',')-ceq($taskHashesAfter-join',')
 $taskPowerSame=($taskPowerBefore|ConvertTo-Json -Compress)-ceq($taskPowerAfter|ConvertTo-Json -Compress)
 if(-not$taskGpuSame-or-not$taskMetadataSame-or-not$taskPowerSame){throw 'Independent controls or performance metadata changed.'}
 [void][OmenLightingUiProof]::PostMessage($taskWindow,0x0010,[UIntPtr]::Zero,[IntPtr]::Zero)
 if(-not$taskProcess.WaitForExit(4000)-or$taskProcess.ExitCode-ne0){throw 'Lighting editor did not close cleanly.'}
 [pscustomobject]@{measured_at=(Get-Date).ToString('o');binary_sha256=(Get-FileHash -LiteralPath $taskExe).Hash;binary_bytes=(Get-Item -LiteralPath $taskExe).Length;initial_mode=$taskBefore.legacy.brightness_raw_byte;on_e4=$true;off_64=$true;switch_only=[bool]$SwitchOnly;hold_seconds=$HoldSeconds;native_picker_loaded_zone=(-not$SwitchOnly);picker_cancel_no_write=(-not$SwitchOnly);zone0_red_readback=(-not$SwitchOnly);other_zones_retained=$true;non_click_guard=$true;complete_original_restored=$taskRestored;gpu_controls_unchanged=$taskGpuSame;hp_gpu_status_unchanged=$taskPowerSame;main_journal_startup_unchanged=$taskMetadataSame;hardware_writes='qualified RGB and64/E4 only';power_voltage_writes=$false;service_control=$false;driver_installation=$false;clean_exit=$true}|ConvertTo-Json -Depth 4|Set-Content -LiteralPath (Join-Path $taskRoot $(if($SwitchOnly){'observed-toggle-proof.json'}else{'gui-proof.json'})) -Encoding UTF8
 if($SwitchOnly){'Observed keyboard on/off/on and complete restore passed.'}else{'Direct native lighting: picker/cancel, zone RGB, on/off, complete restore and independent invariants passed.'}
}finally{
 if($taskProcess){
  $taskProcess.Refresh()
  if(-not$taskProcess.HasExited-and$taskWindow-ne[IntPtr]::Zero){
   $taskDialog=[OmenLightingUiProof]::ColorDialog([uint32]$taskProcess.Id)
   if($taskDialog-ne[IntPtr]::Zero){[void][OmenLightingUiProof]::PostMessage($taskDialog,0x0111,[UIntPtr]::new([uint64]2),[IntPtr]::Zero);Start-Sleep -Milliseconds 100}
   [void][OmenLightingUiProof]::PostMessage($taskWindow,0x0010,[UIntPtr]::Zero,[IntPtr]::Zero)
   if(-not$taskProcess.WaitForExit(10000)){throw 'Lighting transaction is still running; do not terminate it.'}
  }
  $taskProcess.Dispose()
 }
 if(-not$taskRestored){& $taskRecovery;if($LASTEXITCODE-ne0){throw 'Original lighting recovery needs attention; inspect private original/journal.'}}
}