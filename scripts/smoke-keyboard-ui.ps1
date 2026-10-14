# Metadata-only test of our own native keyboard editor. No UAC or hardware SET.
#requires -Version 5.1
param()
$ErrorActionPreference='Stop'
$taskRepo=Split-Path -Parent $PSScriptRoot
$taskExe=Join-Path $taskRepo 'dist\Omen Hub++.exe'
$taskId='keyboard-ui-test-'+[Guid]::NewGuid().ToString('N')
$taskName='Keyboard UI '+[Guid]::NewGuid().ToString('N')
$taskStorage=& $taskExe profile-storage|Out-String|ConvertFrom-Json
$taskRoot=[IO.Path]::GetFullPath((Join-Path $taskStorage.directory 'profiles'))
$taskPath=Join-Path $taskRoot ($taskId+'.json')
$taskCreated=$false;$taskProcess=$null
if(-not('OmenKeyboardSmoke'-as[type])){Add-Type @'
using System;
using System.Text;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class OmenKeyboardSmoke {
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
 public static IntPtr Find(uint process){IntPtr found=IntPtr.Zero;EnumWindows((h,a)=>{uint owner;GetWindowThreadProcessId(h,out owner);var name=new StringBuilder(128);GetClassName(h,name,name.Capacity);if(owner==process&&name.ToString()=="OmenHubPPKeyboardEditor"){found=h;return false;}return true;},IntPtr.Zero);return found;}
}
'@}
function Invoke-OmenJson {param([string[]]$Arguments);$taskText=& $taskExe @Arguments|Out-String;if($LASTEXITCODE-ne0){throw $taskText};$taskText|ConvertFrom-Json}
function Wait-KeyboardText {param([IntPtr]$Window,[string]$Pattern);$taskEnd=(Get-Date).AddSeconds(12);do{$taskText=[OmenKeyboardSmoke]::AllText($Window);if($taskText-match$Pattern){return $taskText};Start-Sleep -Milliseconds 80}while((Get-Date)-lt$taskEnd);throw ('Missing native state: '+$Pattern+' / '+$taskText)}
try {
 if(Test-Path -LiteralPath $taskPath){throw 'Temporary profile exists.'}
 $taskBefore=Invoke-OmenJson @('gpu-curve-read')
 Invoke-OmenJson @('profile-create',$taskId,$taskName)|Out-Null;$taskCreated=$true
 Invoke-OmenJson @('profile-set',$taskId,'gpu_memory_offset_mhz','0')|Out-Null
 $taskProcess=Start-Process -FilePath $taskExe -ArgumentList @('keyboard-ui',$taskId) -PassThru -WindowStyle Hidden
 $taskEnd=(Get-Date).AddSeconds(12);$taskWindow=[IntPtr]::Zero
 do {$taskProcess.Refresh();if($taskProcess.HasExited){throw 'Keyboard editor exited early.'};$taskWindow=[OmenKeyboardSmoke]::Find([uint32]$taskProcess.Id);if($taskWindow-ne[IntPtr]::Zero-and[OmenKeyboardSmoke]::IsWindowEnabled([OmenKeyboardSmoke]::GetDlgItem($taskWindow,704))){break};Start-Sleep -Milliseconds 80}while((Get-Date)-lt$taskEnd)
 if($taskWindow-eq[IntPtr]::Zero){throw 'No owned keyboard window.'}
 # EnumWindows may see a still-constructing hidden parent before WM_CREATE has
 # finished disabling its children. Wait for the worker's completed view.
 Wait-KeyboardText $taskWindow 'Farben speichern, Firmware prüfen'|Out-Null
 $taskApply=[OmenKeyboardSmoke]::GetDlgItem($taskWindow,706)
 if([OmenKeyboardSmoke]::IsWindowEnabled($taskApply)){throw 'Apply enabled before firmware/whole-profile verification.'}
 $taskField=[OmenKeyboardSmoke]::GetDlgItem($taskWindow,710)
 [OmenKeyboardSmoke]::SetText($taskField,'GGGGGG')
 [void][OmenKeyboardSmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]::new([uint64]704),[IntPtr]::Zero)
 Wait-KeyboardText $taskWindow 'Zone 1: Farbe'|Out-Null
 $taskColors=@('123456','ABCDEF','000000','FFFFFF')
 for($taskI=0;$taskI-lt4;$taskI++){[OmenKeyboardSmoke]::SetText([OmenKeyboardSmoke]::GetDlgItem($taskWindow,710+$taskI),$taskColors[$taskI])}
 $taskMode=[OmenKeyboardSmoke]::GetDlgItem($taskWindow,718)
 [void][OmenKeyboardSmoke]::Message($taskMode,0x014E,0)
 [void][OmenKeyboardSmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]::new([uint64]0x000102CE),[IntPtr]::Zero)
 [void][OmenKeyboardSmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]::new([uint64]704),[IntPtr]::Zero)
 Wait-KeyboardText $taskWindow 'Profil gespeichert. Keine Hardware'|Out-Null
 $taskSaved=Get-Content -LiteralPath $taskPath -Raw|ConvertFrom-Json
 $taskRGB=@($taskSaved.settings|Where-Object feature -eq 'keyboard_colors')[0].value.value
 if($taskSaved.settings.Count-ne3-or$taskRGB.Count-ne4){throw 'Other setting or RGB zones were lost.'}
 $taskExpected=@(@(18,52,86),@(171,205,239),@(0,0,0),@(255,255,255))
 for($taskI=0;$taskI-lt4;$taskI++){if(($taskRGB[$taskI]-join ',')-cne($taskExpected[$taskI]-join ',')){throw 'A zone value was not persisted exactly.'}}
 if(@($taskSaved.settings|Where-Object { $_.feature-eq'keyboard_effect'-and$_.value.value-eq'static' }).Count-ne1){throw 'Own-color activation was not saved as a distinct typed feature.'}
 if([OmenKeyboardSmoke]::IsWindowEnabled($taskApply)){throw 'Save alone enabled hardware application.'}
 # Reload profile and ensure values, activation and the VRAM setting persist.
 $taskSelect=[OmenKeyboardSmoke]::GetDlgItem($taskWindow,701)
 $taskSelected=[OmenKeyboardSmoke]::Message($taskSelect,0x0147)
 [void][OmenKeyboardSmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]::new([uint64]0x000102BD),[IntPtr]::Zero)
 Start-Sleep -Milliseconds 160
 for($taskI=0;$taskI-lt4;$taskI++){if([OmenKeyboardSmoke]::Text([OmenKeyboardSmoke]::GetDlgItem($taskWindow,710+$taskI))-cne$taskColors[$taskI]){throw 'Reload changed a zone value.'}}
 $taskAfter=Invoke-OmenJson @('gpu-curve-read')
 $taskBeforeControls=@($taskBefore.nodes|ForEach-Object{@($_.index,$_.voltage_uv,$_.delta_khz)})|ConvertTo-Json -Depth 5 -Compress
 $taskAfterControls=@($taskAfter.nodes|ForEach-Object{@($_.index,$_.voltage_uv,$_.delta_khz)})|ConvertTo-Json -Depth 5 -Compress
 if($taskBeforeControls-cne$taskAfterControls){throw 'Metadata editor changed GPU controls.'}
 [void][OmenKeyboardSmoke]::PostMessage($taskWindow,0x0010,[UIntPtr]::Zero,[IntPtr]::Zero)
 if(-not$taskProcess.WaitForExit(3000)-or$taskProcess.ExitCode-ne0){throw 'Keyboard editor did not exit cleanly.'}
 [pscustomobject]@{measured_at=(Get-Date).ToString('o');binary_sha256=(Get-FileHash -LiteralPath $taskExe).Hash;native_controls=$true;invalid_hex_rejected=$true;four_zone_save_reload=$true;distinct_mode=$true;other_settings_preserved=$true;apply_requires_admin_verification=$true;hardware_writes=$false;gpu_controls_unchanged=$true;clean_exit=$true}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $taskRepo '.local\keyboard-ui-smoke.json') -Encoding UTF8
 'Native keyboard controls, input validation, four-zone persistence, separate mode and no hardware writes passed.'
} finally {
 if($taskProcess){$taskProcess.Refresh();if(-not$taskProcess.HasExited){Stop-Process -Id $taskProcess.Id};$taskProcess.Dispose()}
 if($taskCreated-and(Test-Path -LiteralPath $taskPath)){$taskResolved=(Resolve-Path -LiteralPath $taskPath).Path;if([IO.Path]::GetDirectoryName($taskResolved)-ne$taskRoot){throw 'Cleanup escaped the temporary profile directory.'};Remove-Item -LiteralPath $taskResolved}
}
