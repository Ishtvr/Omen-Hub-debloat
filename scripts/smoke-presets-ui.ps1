# Save-only preset integration. No Apply/Restore/Startup action is invoked.
#requires -Version 5.1
param()
$ErrorActionPreference='Stop'
$taskRepo=Split-Path -Parent $PSScriptRoot
$taskExe=Join-Path $taskRepo 'dist\Omen Hub++.exe'
$taskName='Preset UI '+[Guid]::NewGuid().ToString('N')
$taskStore=& $taskExe profile-storage|Out-String|ConvertFrom-Json
$taskProfiles=[IO.Path]::GetFullPath((Join-Path $taskStore.directory 'profiles'))
$taskFile=$null;$taskProcess=$null;$taskId=$null
$taskOtherId='preset-switch-test-'+[Guid]::NewGuid().ToString('N')
$taskOtherFile=Join-Path $taskProfiles ($taskOtherId+'.json')
$taskOtherCreated=$false
if(-not('OmenPresetSmoke'-as[type])){
    Add-Type @'
using System;
using System.Text;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class OmenPresetSmoke {
    public delegate bool EnumWindow(IntPtr hwnd, IntPtr arg);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumWindow callback, IntPtr arg);
    [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr parent, EnumWindow callback, IntPtr arg);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint owner);
    [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetClassName(IntPtr hwnd,StringBuilder value,int length);
    [DllImport("user32.dll",EntryPoint="SendMessageTimeoutW",CharSet=CharSet.Unicode)] static extern IntPtr Read(IntPtr hwnd,uint message,UIntPtr capacity,StringBuilder value,uint flags,uint timeout,out UIntPtr result);
    [DllImport("user32.dll",EntryPoint="SendMessageTimeoutW",CharSet=CharSet.Unicode)] static extern IntPtr Write(IntPtr hwnd,uint message,UIntPtr parameter,string text,uint flags,uint timeout,out UIntPtr result);
    public static void SetWindowText(IntPtr hwnd,string text) { UIntPtr result; if (hwnd == IntPtr.Zero || Write(hwnd,0x000C,UIntPtr.Zero,text,2,1000,out result) == IntPtr.Zero) throw new Exception("Cannot set owned test control text"); }
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hwnd,uint message,UIntPtr parameter,IntPtr other);
    [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr hwnd,int id);
    [DllImport("user32.dll")] public static extern bool IsWindowEnabled(IntPtr hwnd);
    [DllImport("user32.dll",EntryPoint="SendMessageTimeoutW")] static extern IntPtr Send(IntPtr hwnd,uint message,UIntPtr parameter,IntPtr other,uint flags,uint timeout,out UIntPtr result);
    public static ulong Message(IntPtr hwnd,uint message,ulong parameter=0,long other=0) { UIntPtr result; if (Send(hwnd,message,(UIntPtr)parameter,(IntPtr)other,2,1000,out result)==IntPtr.Zero) throw new Exception("Owned control did not answer"); return result.ToUInt64(); }
    [DllImport("user32.dll")] static extern int GetDlgCtrlID(IntPtr hwnd);
    public static IntPtr Control(IntPtr parent,int id) { var found=GetDlgItem(parent,id); if(found!=IntPtr.Zero)return found; EnumChildWindows(parent,(h,a)=>{if(GetDlgCtrlID(h)==id){found=h;return false;}return true;},IntPtr.Zero);return found; }
    public static string Caption(IntPtr hwnd) {var text=new StringBuilder(8192);UIntPtr result;Read(hwnd,13,(UIntPtr)text.Capacity,text,2,1000,out result);return text.ToString();}
    public static string Item(IntPtr hwnd,int index) {var text=new StringBuilder(512);UIntPtr result;Read(hwnd,0x0148,(UIntPtr)index,text,2,1000,out result);return text.ToString();}
    public static IntPtr Dialog(uint process) {IntPtr found=IntPtr.Zero;EnumWindows((h,a)=>{uint owner;GetWindowThreadProcessId(h,out owner);var name=new StringBuilder(128);GetClassName(h,name,name.Capacity);if(owner==process&&name.ToString()=="#32770"){found=h;return false;}return true;},IntPtr.Zero);return found;}
    public static IntPtr Find(uint process) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((hwnd,arg) => { uint owner; GetWindowThreadProcessId(hwnd,out owner); var name = new StringBuilder(128); GetClassName(hwnd,name,name.Capacity);
            if (owner == process && name.ToString() == "OmenHubPPPresetEditor") { found = hwnd; return false; } return true; },IntPtr.Zero);
        return found;
    }
    public static string Text(IntPtr parent) {
        var values = new List<string>();
        EnumChildWindows(parent,(hwnd,arg) => { var value = new StringBuilder(8192); UIntPtr result; Read(hwnd,0x000D,(UIntPtr)value.Capacity,value,2,500,out result); values.Add(value.ToString()); return true; },IntPtr.Zero);
        return string.Join("\n",values);
    }
}
'@
}
function Read-Json {param([string[]]$ArgsList);$taskRaw=& $taskExe @ArgsList|Out-String;if($LASTEXITCODE-ne0){throw $taskRaw};$taskRaw|ConvertFrom-Json}
function Wait-Ready {param([IntPtr]$Window,[string]$Pattern='Preset geladen.|Preset gespeichert.|Preset benennen|gewählte Einstellungen|entfernte GPU-Optionen')
 $taskUntil=(Get-Date).AddSeconds(15)
 do{ $taskText=[OmenPresetSmoke]::Caption([OmenPresetSmoke]::Control($Window,711));if($taskText-match$Pattern-and[OmenPresetSmoke]::IsWindowEnabled([OmenPresetSmoke]::Control($Window,704))){return $taskText};Start-Sleep -Milliseconds 80}while((Get-Date)-lt$taskUntil)
 throw ('Missing preset state: '+$Pattern+' / '+$taskText)
}
function Click {param([IntPtr]$Window,[int]$Id);[void][OmenPresetSmoke]::PostMessage($Window,0x0111,[UIntPtr]::new([uint64]$Id),[OmenPresetSmoke]::Control($Window,$Id))}
function Set-Included {param([IntPtr]$Window,[int]$Index,[bool]$Included)
 $taskId=2000+$Index*64;$taskControl=[OmenPresetSmoke]::Control($Window,$taskId)
 [void][OmenPresetSmoke]::Message($taskControl,0x00F1,[uint64]$Included)
 Click $Window $taskId
}
function Open-Preset {param([string]$Id='')
 $taskArgs=if($Id){@('presets-ui',$Id)}else{@('presets-ui')}
 $script:taskProcess=Start-Process -FilePath $taskExe -ArgumentList $taskArgs -PassThru -WindowStyle Hidden
 $taskUntil=(Get-Date).AddSeconds(15);$taskWindow=[IntPtr]::Zero
 do{$taskWindow=[OmenPresetSmoke]::Find([uint32]$script:taskProcess.Id);if($taskWindow-ne[IntPtr]::Zero){break};Start-Sleep -Milliseconds 80}while((Get-Date)-lt$taskUntil)
 if($taskWindow-eq[IntPtr]::Zero){throw 'No owned preset window.'}
 $null=Wait-Ready $taskWindow
 return $taskWindow
}
function Close-Preset {param([IntPtr]$Window)
 [void][OmenPresetSmoke]::PostMessage($Window,0x0010,[UIntPtr]::Zero,[IntPtr]::Zero)
 if(-not$script:taskProcess.WaitForExit(4000)-or$script:taskProcess.ExitCode-ne0){throw 'Preset window did not close cleanly.'}
 $script:taskProcess.Dispose();$script:taskProcess=$null
}
$taskBeforeCurve=Read-Json @('gpu-curve-read')
$taskControlImage=@($taskBeforeCurve.nodes|ForEach-Object{@($_.index,$_.voltage_uv,$_.delta_khz)})|ConvertTo-Json -Depth 5 -Compress
$taskFiles=@('control-journal.json','startup.json')|ForEach-Object{Join-Path $taskStore.directory $_}
$taskMetadataBefore=@($taskFiles|ForEach-Object{if(Test-Path -LiteralPath $_){(Get-FileHash -LiteralPath $_).Hash}else{'absent'}})
try{
 $taskWindow=Open-Preset
 $taskText=[OmenPresetSmoke]::Text($taskWindow)
 foreach($taskLabel in @('Custom Presets','GPU','CPU und System','Anwenden','Speichern','Löschen','Bei Windows-Start aktivieren')){if($taskText-notmatch[regex]::Escape($taskLabel)){throw ('Missing label '+$taskLabel)}}
 if($taskText-match'Radeon Graphics|Super Resolution'){throw 'Excluded option group is still shown.'}
 [OmenPresetSmoke]::SetWindowText([OmenPresetSmoke]::Control($taskWindow,703),$taskName)
 Click $taskWindow 1200
 Set-Included $taskWindow 1 $true
 [OmenPresetSmoke]::SetWindowText([OmenPresetSmoke]::Control($taskWindow,2065),'-24')
 if([OmenPresetSmoke]::IsWindowEnabled([OmenPresetSmoke]::Control($taskWindow,705))){throw 'Draft could apply without save.'}
 Click $taskWindow 704
 $null=Wait-Ready $taskWindow 'Preset gespeichert.'
 $taskItem=@(Get-ChildItem -LiteralPath $taskProfiles -Filter '*.json'|Where-Object{(Get-Content -LiteralPath $_.FullName -Raw|ConvertFrom-Json).name-eq$taskName})
 if($taskItem.Count-ne1){throw 'Created preset not found exactly once.'}
 $taskFile=$taskItem[0].FullName;$taskProfile=Get-Content -LiteralPath $taskFile -Raw|ConvertFrom-Json;$taskId=$taskProfile.id
 if(@($taskProfile.settings|Where-Object feature -eq 'gpu_memory_offset_mhz')[0].value.value-ne-24){throw 'GPU memory edit was not saved.'}
 Close-Preset $taskWindow
 # Import a complete custom curve and an unrelated lighting setting as metadata.
 $taskPoints=@($taskBeforeCurve.nodes|ForEach-Object{[ordered]@{voltage_mv=[uint32]($_.voltage_uv/1000);frequency_mhz=[uint32]($_.frequency_khz/1000)}})
 $taskProfile.settings=@($taskProfile.settings)+@([pscustomobject]@{feature='gpu_vf_curve';value=[pscustomobject]@{kind='vf_curve';value=$taskPoints}},[pscustomobject]@{feature='keyboard_colors';value=[pscustomobject]@{kind='colors';value=@(@(1,2,3),@(4,5,6),@(7,8,9),@(10,11,12))}})
 $taskCurveJson=$taskPoints|ConvertTo-Json -Depth 6 -Compress
 [IO.File]::WriteAllText($taskFile,($taskProfile|ConvertTo-Json -Depth 12),[Text.UTF8Encoding]::new($false))
 Read-Json @('profile-create',$taskOtherId,'Other preset for discard check')|Out-Null;$taskOtherCreated=$true
 $taskOtherHash=(Get-FileHash -LiteralPath $taskOtherFile).Hash
 $taskWindow=Open-Preset $taskId
 [OmenPresetSmoke]::SetWindowText([OmenPresetSmoke]::Control($taskWindow,703),($taskName+' edited'))
 # Reject switching dirty A to B: the native selection, draft and save target
 # must all remain A. Only our own process's discard dialog is answered.
 $taskSelect=[OmenPresetSmoke]::Control($taskWindow,701)
 $taskLoadedIndex=[OmenPresetSmoke]::Message($taskSelect,0x0147)
 $taskOtherIndex=-1
 for($taskI=1;$taskI-lt[OmenPresetSmoke]::Message($taskSelect,0x0146);$taskI++){if([OmenPresetSmoke]::Item($taskSelect,$taskI)-ceq'Other preset for discard check'){$taskOtherIndex=$taskI;break}}
 if($taskOtherIndex-lt1){throw 'Switch-test preset is missing.'}
 [void][OmenPresetSmoke]::Message($taskSelect,0x014E,[uint64]$taskOtherIndex)
 [void][OmenPresetSmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]::new([uint64]0x000102BD),$taskSelect)
 $taskUntil=(Get-Date).AddSeconds(5)
 do{$taskDialog=[OmenPresetSmoke]::Dialog([uint32]$taskProcess.Id);if($taskDialog-ne[IntPtr]::Zero){break};Start-Sleep -Milliseconds 50}while((Get-Date)-lt$taskUntil)
 if($taskDialog-eq[IntPtr]::Zero){throw 'No owned discard confirmation.'}
 [void][OmenPresetSmoke]::PostMessage($taskDialog,0x0111,[UIntPtr]::new([uint64]7),[OmenPresetSmoke]::Control($taskDialog,7))
 $taskUntil=(Get-Date).AddSeconds(5)
 do{if([OmenPresetSmoke]::Dialog([uint32]$taskProcess.Id)-eq[IntPtr]::Zero-and[OmenPresetSmoke]::Message($taskSelect,0x0147)-eq$taskLoadedIndex){break};Start-Sleep -Milliseconds 50}while((Get-Date)-lt$taskUntil)
 if([OmenPresetSmoke]::Message($taskSelect,0x0147)-ne$taskLoadedIndex-or[OmenPresetSmoke]::Caption([OmenPresetSmoke]::Control($taskWindow,703))-cne($taskName+' edited')){throw 'Declined switch lost the loaded preset identity or dirty name.'}
 # Concurrent changes to hidden fields must be retained by Save.
 $taskFresh=Get-Content -LiteralPath $taskFile -Raw|ConvertFrom-Json
 @($taskFresh.settings|Where-Object feature -eq 'keyboard_colors')[0].value.value[0][0]=22
 [IO.File]::WriteAllText($taskFile,($taskFresh|ConvertTo-Json -Depth 12),[Text.UTF8Encoding]::new($false))
 Click $taskWindow 1202
 Set-Included $taskWindow 9 $true
 [OmenPresetSmoke]::SetWindowText([OmenPresetSmoke]::Control($taskWindow,2577),'45,125')
 Click $taskWindow 704
 $null=Wait-Ready $taskWindow 'Preset gespeichert.'
 $taskProfile=Get-Content -LiteralPath $taskFile -Raw|ConvertFrom-Json
 if(@($taskProfile.settings|Where-Object feature -eq 'cpu_stapm_limit_mw')[0].value.value-ne45125){throw 'Watts were not saved in exact milliwatts.'}
 if((@($taskProfile.settings|Where-Object feature -eq 'gpu_vf_curve')[0].value.value|ConvertTo-Json -Depth 6 -Compress)-cne$taskCurveJson){throw 'Full custom curve was changed by unrelated editing.'}
 if(@($taskProfile.settings|Where-Object feature -eq 'keyboard_colors')[0].value.value[0][0]-ne22){throw 'Concurrent hidden setting was overwritten.'}
 if($taskProfile.id-cne$taskId-or(Get-FileHash -LiteralPath $taskOtherFile).Hash-cne$taskOtherHash){throw 'Save after declined switch changed the wrong preset.'}
 if([OmenPresetSmoke]::IsWindowEnabled([OmenPresetSmoke]::Control($taskWindow,705))-or[OmenPresetSmoke]::IsWindowEnabled([OmenPresetSmoke]::Control($taskWindow,707))){throw 'Unsupported full preset became eligible for apply/startup.'}
 Click $taskWindow 706
 $null=Wait-Ready $taskWindow 'Preset gelöscht.'
 if(Test-Path -LiteralPath $taskFile){throw 'Delete left the preset selectable.'}
 $taskArchiveRoot=[IO.Path]::GetFullPath((Join-Path $taskStore.directory 'deleted-profiles'))
 $taskArchived=@(Get-ChildItem -LiteralPath $taskArchiveRoot -Filter ($taskId+'-*.json'))
 if($taskArchived.Count-ne1){throw 'Delete did not preserve one exact archived preset.'}
 $taskArchivedProfile=Get-Content -LiteralPath $taskArchived[0].FullName -Raw|ConvertFrom-Json
 if($taskArchivedProfile.name-ne($taskName+' edited')){throw 'Archived preset differs.'}
 Close-Preset $taskWindow
 $taskAfterCurve=Read-Json @('gpu-curve-read')
 $taskAfterImage=@($taskAfterCurve.nodes|ForEach-Object{@($_.index,$_.voltage_uv,$_.delta_khz)})|ConvertTo-Json -Depth 5 -Compress
 $taskMetadataAfter=@($taskFiles|ForEach-Object{if(Test-Path -LiteralPath $_){(Get-FileHash -LiteralPath $_).Hash}else{'absent'}})
 if($taskControlImage-cne$taskAfterImage-or($taskMetadataBefore-join',')-cne($taskMetadataAfter-join',')){throw 'Metadata test changed programmed controls or main journal/startup.'}
 [pscustomobject]@{measured_at=(Get-Date).ToString('o');binary_sha256=(Get-FileHash -LiteralPath $taskExe).Hash;create_save_delete=$true;declined_switch_keeps_identity_draft_save_target=$true;cpu_gpu_separate=$true;exact_watt_units=$true;custom_curve_preserved=$true;concurrent_hidden_fields_preserved=$true;unsupported_apply_startup_blocked=$true;hardware_writes=$false;gpu_controls_and_metadata_unchanged=$true;clean_exit=$true}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $taskRepo '.local\presets-ui-smoke.json') -Encoding UTF8
 'Native custom presets: create, exact units, merged full curve/hidden fields, save/delete, blocked unsafe apply/startup and unchanged hardware passed.'
}finally{
 if($taskProcess){$taskProcess.Refresh();if(-not$taskProcess.HasExited){Stop-Process -Id $taskProcess.Id};$taskProcess.Dispose()}
 if($taskFile-and(Test-Path -LiteralPath $taskFile)){if([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($taskFile))-cne$taskProfiles){throw 'Preset cleanup escaped its directory.'};Remove-Item -LiteralPath $taskFile}
 if($taskOtherCreated-and(Test-Path -LiteralPath $taskOtherFile)){if([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($taskOtherFile))-cne$taskProfiles){throw 'Switch-test cleanup escaped profiles.'};Remove-Item -LiteralPath $taskOtherFile}
 if($taskId){$taskArchiveRoot=[IO.Path]::GetFullPath((Join-Path $taskStore.directory 'deleted-profiles'));if(Test-Path -LiteralPath $taskArchiveRoot){Get-ChildItem -LiteralPath $taskArchiveRoot -Filter ($taskId+'-*.json')|ForEach-Object{if([IO.Path]::GetDirectoryName($_.FullName)-cne$taskArchiveRoot){throw 'Archive cleanup escaped its directory.'};Remove-Item -LiteralPath $_.FullName}}}
}
