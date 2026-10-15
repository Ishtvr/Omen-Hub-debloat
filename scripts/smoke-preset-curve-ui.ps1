# Reproduce the actual preset -> chart path. Metadata only; no Apply/UAC/SET.
#requires -Version 5.1
param([string]$Executable='')
$ErrorActionPreference='Stop'
$taskRepo=Split-Path -Parent $PSScriptRoot
$taskExe=if($Executable){(Resolve-Path -LiteralPath $Executable).Path}else{Join-Path $taskRepo 'dist\Omen Hub++.exe'}
$taskName='Curve draft '+[Guid]::NewGuid().ToString('N')
function Read-CurveJson {param([string[]]$Arguments);$taskRaw=& $taskExe @Arguments|Out-String;if($LASTEXITCODE-ne0){throw $taskRaw};$taskRaw|ConvertFrom-Json}
$taskStore=Read-CurveJson @('profile-storage')
$taskProfiles=[IO.Path]::GetFullPath((Join-Path $taskStore.directory 'profiles'))
$taskFiles=@('control-journal.json','startup.json','keyboard-control\control-journal.json')|ForEach-Object{Join-Path $taskStore.directory $_}
$taskMetadataBefore=@($taskFiles|ForEach-Object{if(Test-Path -LiteralPath $_){(Get-FileHash -LiteralPath $_).Hash}else{'absent'}})
$taskBefore=Read-CurveJson @('gpu-curve-read')
function Programmed-Image {param($Curve);@($Curve.nodes|ForEach-Object{@($_.index,$_.voltage_uv,$_.delta_khz)})|ConvertTo-Json -Depth 5 -Compress}
$taskProcess=$null;$taskFile=$null
if(-not('OmenPresetCurveSmoke'-as[type])){Add-Type @'
using System;
using System.Text;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class OmenPresetCurveSmoke {
 public delegate bool EnumWindow(IntPtr hwnd,IntPtr other);
 [DllImport("user32.dll")] static extern bool EnumWindows(EnumWindow cb,IntPtr arg);
 [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr hwnd,EnumWindow cb,IntPtr arg);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd,out uint owner);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetClassName(IntPtr hwnd,StringBuilder text,int count);
 [DllImport("user32.dll")] static extern int GetDlgCtrlID(IntPtr hwnd);
 [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr hwnd,int id);
 [DllImport("user32.dll")] public static extern bool IsWindowEnabled(IntPtr hwnd);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hwnd,uint message,UIntPtr value,IntPtr other);
 [DllImport("user32.dll",EntryPoint="SendMessageTimeoutW")] static extern IntPtr Send(IntPtr hwnd,uint message,UIntPtr value,IntPtr other,uint flags,uint timeout,out UIntPtr result);
 [DllImport("user32.dll",EntryPoint="SendMessageTimeoutW",CharSet=CharSet.Unicode)] static extern IntPtr Read(IntPtr hwnd,uint message,UIntPtr value,StringBuilder text,uint flags,uint timeout,out UIntPtr result);
 [DllImport("user32.dll",EntryPoint="SendMessageTimeoutW",CharSet=CharSet.Unicode)] static extern IntPtr Write(IntPtr hwnd,uint message,UIntPtr value,string text,uint flags,uint timeout,out UIntPtr result);
 public static ulong Message(IntPtr hwnd,uint msg,ulong value=0,long other=0){UIntPtr result;if(Send(hwnd,msg,(UIntPtr)value,(IntPtr)other,2,1000,out result)==IntPtr.Zero)throw new Exception("Owned control did not answer");return result.ToUInt64();}
 public static string Text(IntPtr hwnd){var text=new StringBuilder(8192);UIntPtr result;Read(hwnd,13,(UIntPtr)text.Capacity,text,2,1000,out result);return text.ToString();}
 public static void SetText(IntPtr hwnd,string text){UIntPtr result;if(Write(hwnd,12,UIntPtr.Zero,text,2,1000,out result)==IntPtr.Zero)throw new Exception("Owned input did not answer");}
 public static string AllText(IntPtr hwnd){var values=new List<string>();EnumChildWindows(hwnd,(h,a)=>{values.Add(Text(h));return true;},IntPtr.Zero);return string.Join("\n",values);}
 public static IntPtr Find(uint process,string expected){IntPtr found=IntPtr.Zero;EnumWindows((h,a)=>{uint owner;GetWindowThreadProcessId(h,out owner);var name=new StringBuilder(128);GetClassName(h,name,name.Capacity);if(owner==process&&name.ToString()==expected){found=h;return false;}return true;},IntPtr.Zero);return found;}
 public static IntPtr Control(IntPtr hwnd,int id){var found=GetDlgItem(hwnd,id);if(found!=IntPtr.Zero)return found;EnumChildWindows(hwnd,(h,a)=>{if(GetDlgCtrlID(h)==id){found=h;return false;}return true;},IntPtr.Zero);return found;}
}
'@
}
function Wait-Curve {param([scriptblock]$Ready,[string]$What)
 $taskUntil=(Get-Date).AddSeconds(12)
 do{if(& $Ready){return};Start-Sleep -Milliseconds 60}while((Get-Date)-lt$taskUntil)
 throw ('Missing native state: '+$What)
}
function Click-Curve {param([IntPtr]$Window,[int]$Id)
 [void][OmenPresetCurveSmoke]::PostMessage($Window,0x0111,[UIntPtr]::new([uint64]$Id),[OmenPresetCurveSmoke]::Control($Window,$Id))
}
function Open-CurvePreset {param([string]$Id='')
 $script:taskProcess=Start-Process -FilePath $taskExe -ArgumentList (@('presets-ui')+@($Id|Where-Object{$_})) -WindowStyle Hidden -PassThru
 Wait-Curve { [OmenPresetCurveSmoke]::Find([uint32]$script:taskProcess.Id,'OmenHubPPPresetEditor')-ne[IntPtr]::Zero } 'preset window'
 $script:taskWindow=[OmenPresetCurveSmoke]::Find([uint32]$script:taskProcess.Id,'OmenHubPPPresetEditor')
 Wait-Curve { [OmenPresetCurveSmoke]::Text([OmenPresetCurveSmoke]::Control($script:taskWindow,711))-match'Preset benennen|Preset geladen|entfernte Optionen' } 'loaded preset'
 Click-Curve $script:taskWindow 1200
}
function Open-CurveDraft {
 if(-not[OmenPresetCurveSmoke]::IsWindowEnabled([OmenPresetCurveSmoke]::Control($script:taskWindow,2129))){throw 'Chart action is disabled for an unsaved/dirty draft.'}
 Click-Curve $script:taskWindow 2129
 Wait-Curve { [OmenPresetCurveSmoke]::Find([uint32]$script:taskProcess.Id,'OmenHubPPCurveEditor')-ne[IntPtr]::Zero } 'owned chart'
 $script:taskChart=[OmenPresetCurveSmoke]::Find([uint32]$script:taskProcess.Id,'OmenHubPPCurveEditor')
 Wait-Curve { [OmenPresetCurveSmoke]::Text([OmenPresetCurveSmoke]::Control($script:taskChart,520))-match'Kurvenentwurf' } 'draft chart loaded'
 if([OmenPresetCurveSmoke]::IsWindowEnabled($script:taskWindow)){throw 'Parent is editable while its modal draft is open.'}
 if([OmenPresetCurveSmoke]::Message([OmenPresetCurveSmoke]::Control($script:taskChart,509),0x0146)-ne127){throw 'Chart lost the full GPU grid.'}
}
function Select-FirstCurvePoint {
 $taskPoints=[OmenPresetCurveSmoke]::Control($script:taskChart,509)
 [void][OmenPresetCurveSmoke]::Message($taskPoints,0x014E,0)
 [void][OmenPresetCurveSmoke]::Message($script:taskChart,0x0111,0x000101FD,$taskPoints.ToInt64())
 return [int][OmenPresetCurveSmoke]::Text([OmenPresetCurveSmoke]::Control($script:taskChart,511))
}
function Close-CurveDraft {param([bool]$Accept)
 Click-Curve $script:taskChart $(if($Accept){503}else{517})
 Wait-Curve { [OmenPresetCurveSmoke]::Find([uint32]$script:taskProcess.Id,'OmenHubPPCurveEditor')-eq[IntPtr]::Zero-and[OmenPresetCurveSmoke]::IsWindowEnabled($script:taskWindow) } 'return to retained preset'
}
function Close-CurvePreset {
 [void][OmenPresetCurveSmoke]::PostMessage($script:taskWindow,0x0010,[UIntPtr]::Zero,[IntPtr]::Zero)
 if(-not$script:taskProcess.WaitForExit(4000)-or$script:taskProcess.ExitCode-ne0){throw 'Preset did not close cleanly.'}
 $script:taskProcess.Dispose();$script:taskProcess=$null
}
try{
 $taskCountBefore=@(Get-ChildItem -LiteralPath $taskProfiles -Filter '*.json').Count
 Open-CurvePreset
 $taskText=[OmenPresetCurveSmoke]::AllText($taskWindow)
 if($taskText-match'(?m)^(GPU-Spannungs|VRAM-Spannungs|Bildratenlimit|GPU-Leistungslimit|GPU-Temperaturlimit|GPU-Taktlimit|VRAM-Taktlimit|Eigener GPU-Leistungsrahmen)'){throw 'Retired GPU control remains visible.'}
 # New unnamed preset opens; cancel creates no hidden profile or setting.
 Open-CurveDraft
 $taskCancelledFirst=Select-FirstCurvePoint
 [OmenPresetCurveSmoke]::SetText([OmenPresetCurveSmoke]::Control($taskChart,511),[string]($taskCancelledFirst-1))
 [void][OmenPresetCurveSmoke]::Message($taskChart,0x0111,512)
 Close-CurveDraft $false
 if([OmenPresetCurveSmoke]::Message([OmenPresetCurveSmoke]::Control($taskWindow,2128),0x00F0)-ne0){throw 'Cancel included an untouched V/F setting.'}
 [OmenPresetCurveSmoke]::SetText([OmenPresetCurveSmoke]::Control($taskWindow,703),$taskName)
 [void][OmenPresetCurveSmoke]::Message([OmenPresetCurveSmoke]::Control($taskWindow,2064),0x00F1,1);Click-Curve $taskWindow 2064
 [OmenPresetCurveSmoke]::SetText([OmenPresetCurveSmoke]::Control($taskWindow,2065),'-24')
 [void][OmenPresetCurveSmoke]::Message([OmenPresetCurveSmoke]::Control($taskWindow,2128),0x00F1,1);Click-Curve $taskWindow 2128
 Open-CurveDraft
 $taskInitial=Select-FirstCurvePoint
 if($taskInitial-ne$taskCancelledFirst){throw 'Cancel retained changes from the rejected chart draft.'}
 if($taskInitial-lt2){throw 'Reference first frequency is unsuitable for a metadata decrement.'}
 Click-Curve $taskChart 527
 [OmenPresetCurveSmoke]::SetText([OmenPresetCurveSmoke]::Control($taskChart,511),[string]($taskInitial-1))
 [void][OmenPresetCurveSmoke]::Message($taskChart,0x0111,512)
 Close-CurveDraft $true
 if([OmenPresetCurveSmoke]::Text([OmenPresetCurveSmoke]::Control($taskWindow,703))-cne$taskName-or[OmenPresetCurveSmoke]::Text([OmenPresetCurveSmoke]::Control($taskWindow,2065))-cne'-24'){throw 'Chart return replaced other dirty fields.'}
 if(@(Get-ChildItem -LiteralPath $taskProfiles -Filter '*.json').Count-ne$taskCountBefore){throw 'Chart open/accept persisted a profile without Save.'}
 Open-CurveDraft
 if((Select-FirstCurvePoint)-ne($taskInitial-1)){throw 'Reopen lost the accepted curve draft.'}
 Close-CurveDraft $false
 Click-Curve $taskWindow 704
 Wait-Curve { [OmenPresetCurveSmoke]::Text([OmenPresetCurveSmoke]::Control($taskWindow,711))-match'Preset gespeichert' } 'explicit parent Save'
 $taskItem=@(Get-ChildItem -LiteralPath $taskProfiles -Filter '*.json'|Where-Object{(Get-Content -LiteralPath $_.FullName -Raw|ConvertFrom-Json).name-ceq$taskName})
 if($taskItem.Count-ne1){throw 'Saved curve preset not found exactly once.'}
 $taskFile=$taskItem[0].FullName;$taskProfile=Get-Content -LiteralPath $taskFile -Raw|ConvertFrom-Json;$taskId=$taskProfile.id
 $taskCurve=@($taskProfile.settings|Where-Object feature -eq 'gpu_vf_curve')[0].value.value
 if($taskCurve.Count-ne127-or$taskCurve[0].frequency_mhz-ne($taskInitial-1)-or@($taskProfile.settings|Where-Object feature -eq 'gpu_memory_offset_mhz')[0].value.value-ne-24){throw 'Parent Save lost the exact curve/VRAM draft.'}
 $taskCurveImage=$taskCurve|ConvertTo-Json -Depth 6 -Compress
 # Existing saved preset also opens while another field is dirty; cancelling
 # neither reloads that field nor writes its name to disk.
 [OmenPresetCurveSmoke]::SetText([OmenPresetCurveSmoke]::Control($taskWindow,703),($taskName+' dirty'))
 Open-CurveDraft;Close-CurveDraft $false
 if([OmenPresetCurveSmoke]::Text([OmenPresetCurveSmoke]::Control($taskWindow,703))-cne($taskName+' dirty')-or(Get-Content -LiteralPath $taskFile -Raw|ConvertFrom-Json).name-cne$taskName){throw 'Saved dirty preset lost its draft or was written by opening the chart.'}
 [OmenPresetCurveSmoke]::SetText([OmenPresetCurveSmoke]::Control($taskWindow,703),$taskName)
 Click-Curve $taskWindow 704
 Wait-Curve { [OmenPresetCurveSmoke]::Text([OmenPresetCurveSmoke]::Control($taskWindow,711))-match'Preset gespeichert' } 'resave retained preset'
 Close-CurvePreset
 # Old metadata loads; only explicit Save retires removed controls, preserving
 # the full curve and another setting which the lighting UI manages separately.
 $taskRetired=@('gpu_voltage_offset_mv','gpu_memory_voltage_offset_mv','gpu_voltage_limit_mv','gpu_frame_rate_limit','gpu_power_limit_mw','gpu_temperature_limit_c','gpu_core_clock_limit_mhz','gpu_memory_clock_limit_mhz','gpu_custom_tgp','omen_performance_mode')
 foreach($taskFeature in $taskRetired){
  $taskValue=switch($taskFeature){'gpu_voltage_limit_mv'{'900'}'gpu_frame_rate_limit'{'60'}'gpu_power_limit_mw'{'80000'}'gpu_temperature_limit_c'{'87'}'gpu_core_clock_limit_mhz'{'2000'}'gpu_memory_clock_limit_mhz'{'7000'}'gpu_unleashed'{'true'}'gpu_dynamic_boost'{'true'}'gpu_custom_tgp'{'true'}'omen_performance_mode'{'balanced'}default{'0'}}
  Read-CurveJson @('profile-set',$taskId,$taskFeature,$taskValue)|Out-Null
 }
 $taskApu=[ordered]@{cpu_skin_temperature_limit_c=45;cpu_fast_ppt_limit_mw=55000;cpu_slow_ppt_limit_mw=45000;cpu_stapm_limit_mw=45000;cpu_stapm_time_seconds=28;cpu_slow_ppt_time_seconds=28;soc_tdc_ma=15000;soc_edc_ma=25000;gfx_tdc_ma=15000;gfx_edc_ma=25000;gfx_curve_optimizer=0;amd_igpu_clock_mhz=800;igpu_soft_min_mhz=400;igpu_soft_max_mhz=1200;cpu_soft_min_mhz=1000;cpu_soft_max_mhz=4000;data_soft_min_mhz=800;data_soft_max_mhz=1600;fabric_soft_min_mhz=800;fabric_soft_max_mhz=1600;vcn_soft_min_mhz=400;vcn_soft_max_mhz=1000;soc_soft_min_mhz=400;soc_soft_max_mhz=1000;intel_gpu_power_balance=0;intel_gpu_voltage_offset_mv=0}
 foreach($taskEntry in $taskApu.GetEnumerator()){Read-CurveJson @('profile-set',$taskId,[string]$taskEntry.Key,[string]$taskEntry.Value)|Out-Null}
 $taskProfile=Get-Content -LiteralPath $taskFile -Raw|ConvertFrom-Json
 $taskProfile.settings+=@([pscustomobject]@{feature='keyboard_colors';value=[pscustomobject]@{kind='colors';value=@(@(1,2,3),@(4,5,6),@(7,8,9),@(10,11,12))}})
 foreach($taskBoostKey in @('gpu_unleashed','gpu_dynamic_boost')){$taskProfile.settings+=@([pscustomobject]@{feature=$taskBoostKey;value=[pscustomobject]@{kind='boolean';value=$true}})}
 [IO.File]::WriteAllText($taskFile,($taskProfile|ConvertTo-Json -Depth 12),[Text.UTF8Encoding]::new($false))
 $taskLegacyHash=(Get-FileHash -LiteralPath $taskFile).Hash
 Open-CurvePreset $taskId
 if((Get-FileHash -LiteralPath $taskFile).Hash-cne$taskLegacyHash){throw 'Opening an old profile rewrote it.'}
 Click-Curve $taskWindow 704
 Wait-Curve { [OmenPresetCurveSmoke]::Text([OmenPresetCurveSmoke]::Control($taskWindow,711))-match'Preset gespeichert' } 'legacy cleanup on Save'
 $taskProfile=Get-Content -LiteralPath $taskFile -Raw|ConvertFrom-Json
 if(@($taskProfile.settings|Where-Object {$_.feature-in$taskRetired}).Count-ne0){throw 'Retired controls remain in saved preset.'}
 if(@($taskProfile.settings|Where-Object {$_.feature-in@($taskApu.Keys)}).Count-ne0){throw 'Removed APU/iGPU instructions remain after explicit Save.'}
 foreach($taskBoostKey in @('gpu_unleashed','gpu_dynamic_boost')){$taskBoostSetting=@($taskProfile.settings|Where-Object feature -eq $taskBoostKey);if($taskBoostSetting.Count-ne1-or$taskBoostSetting[0].value.value-ne$true){throw 'Save removed a required Dynamic Boost instruction.'}}
 if((@($taskProfile.settings|Where-Object feature -eq 'gpu_vf_curve')[0].value.value|ConvertTo-Json -Depth 6 -Compress)-cne$taskCurveImage-or@($taskProfile.settings|Where-Object feature -eq 'keyboard_colors').Count-ne1){throw 'Legacy cleanup changed full curve or hidden settings.'}
 Close-CurvePreset
 $taskAfter=Read-CurveJson @('gpu-curve-read')
 $taskMetadataAfter=@($taskFiles|ForEach-Object{if(Test-Path -LiteralPath $_){(Get-FileHash -LiteralPath $_).Hash}else{'absent'}})
 if((Programmed-Image $taskBefore)-cne(Programmed-Image $taskAfter)-or($taskMetadataBefore-join',')-cne($taskMetadataAfter-join',')){throw 'Draft editor changed hardware/journal/startup.'}
 [pscustomobject]@{measured_at=(Get-Date).ToString('o');binary_sha256=(Get-FileHash -LiteralPath $taskExe).Hash;new_unnamed_chart=$true;dirty_checked_curve_opens=$true;cancel_preserves_parent=$true;accept_reopen_full_curve=$true;parent_save_only=$true;other_dirty_fields_retained=$true;legacy_cleanup_on_save=$true;apu_26_old_instructions_removed_on_save=$true;both_dynamic_boost_keys_retained=$true;hardware_journals_startup_unchanged=$true;clean_exit=$true}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $taskRepo '.local\preset-curve-ui-smoke.json') -Encoding UTF8
 'Actual preset chart: unnamed/dirty open, cancel/accept/reopen, explicit Save, retired GPU cleanup and unchanged hardware passed.'
}finally{
 if($taskProcess){$taskProcess.Refresh();if(-not$taskProcess.HasExited){Stop-Process -Id $taskProcess.Id};$taskProcess.Dispose()}
 if($taskFile-and(Test-Path -LiteralPath $taskFile)){if([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($taskFile))-cne$taskProfiles){throw 'Curve test cleanup escaped profiles.'};Remove-Item -LiteralPath $taskFile}
}
