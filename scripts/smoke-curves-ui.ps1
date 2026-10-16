# Own-process native editor test. All mutations are profile metadata, never SETs.
#requires -Version 5.1
param()
$ErrorActionPreference='Stop'
# UI labels below are German; pin the app language so the checks stay stable.
$env:OMEN_HUB_LANG = 'de'
$taskRepo=Split-Path -Parent $PSScriptRoot
$taskExe=Join-Path $taskRepo 'dist\Omen Hub++.exe'
$taskId='curve-ui-smoke-'+[Guid]::NewGuid().ToString('N')
$taskName='Curve Widget Test '+[Guid]::NewGuid().ToString('N')
$taskStorage=(& $taskExe profile-storage | Out-String | ConvertFrom-Json).directory
$taskProfileRoot=[IO.Path]::GetFullPath((Join-Path $taskStorage 'profiles'))
$taskProfilePath=Join-Path $taskProfileRoot ($taskId+'.json')
$taskCreated=$false
$taskProcess=$null
if(-not('OmenCurveSmoke' -as [type])){
Add-Type @'
using System;
using System.Text;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class OmenCurveSmoke {
    public delegate bool EnumWindow(IntPtr hwnd,IntPtr arg);
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left,Top,Right,Bottom; }
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumWindow cb,IntPtr arg);
    [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr parent,EnumWindow cb,IntPtr arg);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd,out uint owner);
    [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetClassName(IntPtr hwnd,StringBuilder text,int length);
    [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr hwnd,int id);
    [DllImport("user32.dll")] public static extern bool IsWindowEnabled(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr hwnd,out Rect rect);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hwnd,uint message,UIntPtr value,IntPtr other);
    [DllImport("user32.dll",EntryPoint="SendMessageTimeoutW")] static extern IntPtr Send(IntPtr hwnd,uint message,UIntPtr parameter,IntPtr other,uint flags,uint timeout,out UIntPtr result);
    [DllImport("user32.dll",EntryPoint="SendMessageTimeoutW",CharSet=CharSet.Unicode)] static extern IntPtr Read(IntPtr hwnd,uint message,UIntPtr parameter,StringBuilder text,uint flags,uint timeout,out UIntPtr result);
    [DllImport("user32.dll",EntryPoint="SendMessageTimeoutW",CharSet=CharSet.Unicode)] static extern IntPtr Write(IntPtr hwnd,uint message,UIntPtr parameter,string text,uint flags,uint timeout,out UIntPtr result);
    public static ulong Message(IntPtr hwnd,uint message,ulong value=0,long other=0){UIntPtr result;if(Send(hwnd,message,(UIntPtr)value,(IntPtr)other,2,1000,out result)==IntPtr.Zero)throw new Exception("Owned control did not answer");return result.ToUInt64();}
    public static void SetText(IntPtr hwnd,string value){UIntPtr result;if(Write(hwnd,0x000C,UIntPtr.Zero,value,2,1000,out result)==IntPtr.Zero)throw new Exception("Owned text field did not answer");}
    public static string Text(IntPtr hwnd){var value=new StringBuilder(8192);UIntPtr result;Read(hwnd,0x000D,(UIntPtr)value.Capacity,value,2,1000,out result);return value.ToString();}
    public static string[] Items(IntPtr hwnd){var values=new List<string>();int count=(int)Message(hwnd,0x0146);for(int i=0;i<count;i++){var text=new StringBuilder(256);UIntPtr result;if(Read(hwnd,0x0148,(UIntPtr)i,text,2,1000,out result)==IntPtr.Zero)throw new Exception("Owned point list did not answer");values.Add(text.ToString());}return values.ToArray();}
    public static string AllText(IntPtr parent){var values=new List<string>();EnumChildWindows(parent,(h,a)=>{values.Add(Text(h));return true;},IntPtr.Zero);return string.Join("\n",values);}
    public static IntPtr Find(uint process){IntPtr found=IntPtr.Zero;EnumWindows((h,a)=>{uint owner;GetWindowThreadProcessId(h,out owner);var name=new StringBuilder(128);GetClassName(h,name,name.Capacity);if(owner==process&&name.ToString()=="OmenHubPPCurveEditor"){found=h;return false;}return true;},IntPtr.Zero);return found;}
    public static void Mouse(IntPtr hwnd,uint message,int x,int y,ulong buttons){Message(hwnd,message,buttons,(long)((uint)(ushort)x|((uint)(ushort)y<<16)));}
}
'@
}
function Invoke-Omen {param([string[]]$Arguments);$taskText=& $taskExe @Arguments | Out-String;if($LASTEXITCODE -ne 0){throw $taskText};$taskText|ConvertFrom-Json}
function Wait-OmenText {param([IntPtr]$Window,[string]$Pattern);$taskEnd=(Get-Date).AddSeconds(12);do{$taskValue=[OmenCurveSmoke]::AllText($Window);if($taskValue -match $Pattern){return $taskValue};Start-Sleep -Milliseconds 80}while((Get-Date)-lt $taskEnd);throw "Missing editor state: $Pattern / $taskValue"}
try{
    if(Test-Path -LiteralPath $taskProfilePath){throw 'Test profile already exists.'}
    $taskBefore=Invoke-Omen @('gpu-capabilities')
    $taskCurveBefore=Invoke-Omen @('gpu-curve-read')
    Invoke-Omen @('profile-create',$taskId,$taskName)|Out-Null;$taskCreated=$true
    Invoke-Omen @('profile-set',$taskId,'gpu_memory_offset_mhz','0')|Out-Null
    $taskProcess=Start-Process -FilePath $taskExe -ArgumentList @('curve-ui',$taskId) -PassThru -WindowStyle Hidden
    $taskEnd=(Get-Date).AddSeconds(12);$taskWindow=[IntPtr]::Zero
    do{$taskProcess.Refresh();if($taskProcess.HasExited){throw 'Curve editor exited early.'};$taskWindow=[OmenCurveSmoke]::Find([uint32]$taskProcess.Id);if($taskWindow-ne[IntPtr]::Zero -and [OmenCurveSmoke]::IsWindowEnabled([OmenCurveSmoke]::GetDlgItem($taskWindow,503))){break};Start-Sleep -Milliseconds 80}while((Get-Date)-lt$taskEnd)
    if($taskWindow-eq[IntPtr]::Zero){throw 'No owned curve window found.'}
    Wait-OmenText $taskWindow 'GPU-Spannungsanker sind fest'|Out-Null
    # Reveal numeric/profile controls from the default chart-focused view.
    [void][OmenCurveSmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]::new([uint64]527),[IntPtr]::Zero)
    Start-Sleep -Milliseconds 100
    $taskCanvas=[OmenCurveSmoke]::GetDlgItem($taskWindow,519)
    $taskPoints=[OmenCurveSmoke]::GetDlgItem($taskWindow,509)
    $taskX=[OmenCurveSmoke]::GetDlgItem($taskWindow,510)
    $taskY=[OmenCurveSmoke]::GetDlgItem($taskWindow,511)
    if([OmenCurveSmoke]::Message($taskPoints,0x0146)-ne127){throw 'Complete GPU grid not present.'}
    # Numeric voltage-anchor tampering is rejected even outside the readonly UI.
    $taskAnchor=[int][OmenCurveSmoke]::Text($taskX)
    [OmenCurveSmoke]::SetText($taskX,([string]($taskAnchor+5)))
    [void][OmenCurveSmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]::new([uint64]512),[IntPtr]::Zero)
    Wait-OmenText $taskWindow 'GPU-Spannungsanker bleiben unverändert'|Out-Null
    [void][OmenCurveSmoke]::Message($taskCanvas,0x0100,0x28)
    Start-Sleep -Milliseconds 120
    if([int][OmenCurveSmoke]::Text($taskX)-ne$taskAnchor){throw 'GPU voltage anchor changed.'}
    # Exercise real mouse messages in the GPU canvas; horizontal drag stays fixed.
    $taskRect=New-Object OmenCurveSmoke+Rect
    [void][OmenCurveSmoke]::GetClientRect($taskCanvas,[ref]$taskRect)
    $taskGpuY=[int][OmenCurveSmoke]::Text($taskY)
    $taskFrequencies=@($taskCurveBefore.nodes|ForEach-Object{$_.frequency_khz/1000})
    $taskMin=[int]([math]::Floor([math]::Max(0,(($taskFrequencies|Measure-Object -Minimum).Minimum-200))/100)*100)
    $taskMax=[int]([math]::Floor((($taskFrequencies|Measure-Object -Maximum).Maximum+299)/100)*100)
    $taskViewXMin=[int]([math]::Floor($taskCurveBefore.nodes[0].voltage_uv/1000/25)*25)
    $taskViewXMax=[int]([math]::Ceiling($taskCurveBefore.nodes[-1].voltage_uv/1000/25)*25)
    $taskPx=[int](72+(($taskAnchor-$taskViewXMin)/($taskViewXMax-$taskViewXMin))*($taskRect.Right-100))
    $taskPy=[int]($taskRect.Bottom-54-(($taskGpuY-$taskMin)/($taskMax-$taskMin))*($taskRect.Bottom-92))
    [OmenCurveSmoke]::Mouse($taskCanvas,0x0201,$taskPx,$taskPy,1)
    [OmenCurveSmoke]::Mouse($taskCanvas,0x0200,($taskPx-25),($taskPy-2),1)
    [OmenCurveSmoke]::Mouse($taskCanvas,0x0202,($taskPx-25),($taskPy-2),0)
    Start-Sleep -Milliseconds 120
    if([int][OmenCurveSmoke]::Text($taskX)-ne$taskAnchor){throw 'GPU horizontal drag altered the voltage grid.'}
    # Start from the loaded draft; Shift-drag offsets all 127 nodes equally.
    [void][OmenCurveSmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]::new([uint64]516),[IntPtr]::Zero)
    Start-Sleep -Milliseconds 100
    $taskBaseline=[OmenCurveSmoke]::Items($taskPoints)
    $taskGroupY=[int][OmenCurveSmoke]::Text($taskY)
    $taskPy=[int]($taskRect.Bottom-54-(($taskGroupY-$taskMin)/($taskMax-$taskMin))*($taskRect.Bottom-92))
    [OmenCurveSmoke]::Mouse($taskCanvas,0x0201,$taskPx,$taskPy,5)
    [OmenCurveSmoke]::Mouse($taskCanvas,0x0200,$taskPx,($taskPy-6),5)
    [OmenCurveSmoke]::Mouse($taskCanvas,0x0202,$taskPx,($taskPy-6),4)
    Start-Sleep -Milliseconds 100
    $taskShifted=[OmenCurveSmoke]::Items($taskPoints)
    $taskDelta=$null
    for($taskI=0;$taskI-lt127;$taskI++){
        if($taskBaseline[$taskI]-notmatch '^(\d+) mV.*?(\d+) MHz$'){throw 'Cannot parse baseline point.'};$taskBaseX=[int]$Matches[1];$taskBaseY=[int]$Matches[2]
        if($taskShifted[$taskI]-notmatch '^(\d+) mV.*?(\d+) MHz$'){throw 'Cannot parse shifted point.'};$taskEditedX=[int]$Matches[1];$taskEditedY=[int]$Matches[2]
        if($taskBaseX-ne$taskEditedX){throw 'Shift drag moved a voltage anchor.'}
        $taskCurrentDelta=$taskEditedY-$taskBaseY
        if($null-eq$taskDelta){$taskDelta=$taskCurrentDelta}
        if($taskCurrentDelta-ne$taskDelta){throw 'Shift drag distorted the curve shape.'}
    }
    if($taskDelta-le0){throw 'Shift drag did not move the complete draft.'}
    # Native Ctrl+Z / Ctrl+Y, with modifiers local to this owned control.
    [void][OmenCurveSmoke]::Message($taskCanvas,0x0100,0x11)
    [void][OmenCurveSmoke]::Message($taskCanvas,0x0100,0x5a)
    [void][OmenCurveSmoke]::Message($taskCanvas,0x0101,0x11)
    Start-Sleep -Milliseconds 100
    if(([OmenCurveSmoke]::Items($taskPoints)-join '|')-cne($taskBaseline-join '|')){throw 'A drag was not restored by one undo.'}
    [void][OmenCurveSmoke]::Message($taskCanvas,0x0100,0x11)
    [void][OmenCurveSmoke]::Message($taskCanvas,0x0100,0x59)
    [void][OmenCurveSmoke]::Message($taskCanvas,0x0101,0x11)
    Start-Sleep -Milliseconds 100
    if(([OmenCurveSmoke]::Items($taskPoints)-join '|')-cne($taskShifted-join '|')){throw 'Redo did not restore the complete draft.'}
    # Shift in empty chart space selects a voltage band; Shift+Enter flattens it.
    $taskBandLeft=[int](72+0.7*($taskRect.Right-100));$taskBandRight=[int](72+0.9*($taskRect.Right-100))
    $taskBeforeBand=[OmenCurveSmoke]::Text($taskCanvas)
    [OmenCurveSmoke]::Mouse($taskCanvas,0x0201,$taskBandLeft,50,5)
    [OmenCurveSmoke]::Mouse($taskCanvas,0x0200,$taskBandRight,50,5)
    [void][OmenCurveSmoke]::Message($taskCanvas,0x0100,0x1b)
    Start-Sleep -Milliseconds 100
    if([OmenCurveSmoke]::Text($taskCanvas)-cne$taskBeforeBand){throw 'Esc did not cancel the active band selection.'}
    [OmenCurveSmoke]::Mouse($taskCanvas,0x0201,$taskBandLeft,50,5)
    [OmenCurveSmoke]::Mouse($taskCanvas,0x0200,$taskBandRight,50,5)
    [OmenCurveSmoke]::Mouse($taskCanvas,0x0202,$taskBandRight,50,4)
    Wait-OmenText $taskWindow 'Auswahl:'|Out-Null
    [void][OmenCurveSmoke]::Message($taskCanvas,0x0100,0x10)
    [void][OmenCurveSmoke]::Message($taskCanvas,0x0100,0x0d)
    [void][OmenCurveSmoke]::Message($taskCanvas,0x0101,0x10)
    Start-Sleep -Milliseconds 100
    $taskFlattened=[OmenCurveSmoke]::Items($taskPoints)
    $taskFlatY=$null;$taskFlatCount=0
    for($taskI=0;$taskI-lt127;$taskI++){
        if($taskFlattened[$taskI]-notmatch '^(\d+) mV.*?(\d+) MHz$'){throw 'Cannot parse flattened point.'};$taskEditedX=[int]$Matches[1];$taskEditedY=[int]$Matches[2]
        if($taskEditedX-ge($taskViewXMin+0.7*($taskViewXMax-$taskViewXMin)) -and $taskEditedX-le($taskViewXMin+0.9*($taskViewXMax-$taskViewXMin))){if($null-eq$taskFlatY){$taskFlatY=$taskEditedY};if($taskEditedY-ne$taskFlatY){throw 'Selected band was not flattened.'};$taskFlatCount++}
    }
    if($taskFlatCount-lt2){throw 'No complete voltage band was tested.'}
    [void][OmenCurveSmoke]::Message($taskCanvas,0x0100,0x1b)
    Start-Sleep -Milliseconds 100
    # View commands change the accessible viewport, never draft data.
    $taskViewBefore=[OmenCurveSmoke]::Text($taskCanvas)
    [void][OmenCurveSmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]::new([uint64]523),[IntPtr]::Zero)
    Start-Sleep -Milliseconds 100
    if([OmenCurveSmoke]::Text($taskCanvas)-ceq$taskViewBefore){throw 'Native zoom did not change the viewport.'}
    if(([OmenCurveSmoke]::Items($taskPoints)-join '|')-cne($taskFlattened-join '|')){throw 'Zoom edited the draft.'}
    [void][OmenCurveSmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]::new([uint64]525),[IntPtr]::Zero)
    Start-Sleep -Milliseconds 100
    $taskFitCaption=[OmenCurveSmoke]::Text($taskCanvas)
    if($taskFitCaption-notmatch 'Ansicht:\s*(\d+).(\d+) mV,\s*(\d+).(\d+) MHz\.'){throw 'Missing accessible fitted viewport.'}
    $taskFitLoX=[int]$Matches[1];$taskFitHiX=[int]$Matches[2];$taskFitLoY=[int]$Matches[3];$taskFitHiY=[int]$Matches[4]
    if($taskFitLoX-ne$taskViewXMin-or$taskFitHiX-ne$taskViewXMax){throw 'Fit did not restore the complete voltage grid.'}
    foreach($taskItem in $taskFlattened){if($taskItem-notmatch '^(\d+) mV.*?(\d+) MHz$'){throw 'Missing fitted point.'};if([int]$Matches[2]-lt$taskFitLoY-or[int]$Matches[2]-gt$taskFitHiY){throw 'Fit hid an edited frequency.'}}
    # Separate CPU fan curve, real drag with ordered temperature/duty constraints.
    [void][OmenCurveSmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]::new([uint64]506),[IntPtr]::Zero)
    Wait-OmenText $taskWindow 'CPU-Lüfter'|Out-Null
    [void][OmenCurveSmoke]::Message($taskPoints,0x014E,1)
    [void][OmenCurveSmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]::new([uint64]0x000101FD),[IntPtr]::Zero)
    Start-Sleep -Milliseconds 100
    $taskFanX=[int][OmenCurveSmoke]::Text($taskX);$taskFanY=[int][OmenCurveSmoke]::Text($taskY)
    $taskPx=[int](72+($taskFanX/110)*($taskRect.Right-100));$taskPy=[int]($taskRect.Bottom-54-($taskFanY/100)*($taskRect.Bottom-92))
    [OmenCurveSmoke]::Mouse($taskCanvas,0x0201,$taskPx,$taskPy,1)
    [OmenCurveSmoke]::Mouse($taskCanvas,0x0200,($taskPx+20),($taskPy-20),1)
    [OmenCurveSmoke]::Mouse($taskCanvas,0x0202,($taskPx+20),($taskPy-20),0)
    Start-Sleep -Milliseconds 100
    if([int][OmenCurveSmoke]::Text($taskY)-le$taskFanY){throw 'CPU fan drag did not edit the draft.'}
    [void][OmenCurveSmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]::new([uint64]513),[IntPtr]::Zero)
    Start-Sleep -Milliseconds 100
    if([OmenCurveSmoke]::Message($taskPoints,0x0146)-ne5){throw 'Fan point insertion failed.'}
    [void][OmenCurveSmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]::new([uint64]514),[IntPtr]::Zero)
    Start-Sleep -Milliseconds 100
    if([OmenCurveSmoke]::Message($taskPoints,0x0146)-ne4){throw 'Fan point removal failed.'}
    [void][OmenCurveSmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]::new([uint64]507),[IntPtr]::Zero)
    Wait-OmenText $taskWindow 'GPU-Lüfter'|Out-Null
    [void][OmenCurveSmoke]::Message($taskCanvas,0x0100,0x26)
    Start-Sleep -Milliseconds 100
    [void][OmenCurveSmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]::new([uint64]503),[IntPtr]::Zero)
    Wait-OmenText $taskWindow 'Profil gespeichert. Keine Hardware verändert'|Out-Null
    $taskSaved=Get-Content -LiteralPath $taskProfilePath -Raw|ConvertFrom-Json
    if($taskSaved.settings.Count-ne4){throw 'Other profile settings or separate curve settings were lost.'}
    foreach($taskFeature in @('gpu_vf_curve','cpu_fan_curve','gpu_fan_curve','gpu_memory_offset_mhz')){if(@($taskSaved.settings|Where-Object feature -eq $taskFeature).Count-ne1){throw "Missing saved feature: $taskFeature"}}
    $taskSavedGpu=@($taskSaved.settings|Where-Object feature -eq 'gpu_vf_curve')[0].value.value
    if($taskSavedGpu.Count-ne127 -or @($taskSaved.settings|Where-Object feature -eq 'gpu_core_offset_mhz').Count-ne0){throw 'GPU curve is incomplete or conflicts with a global offset.'}
    for($taskI=0;$taskI-lt127;$taskI++){if($taskSavedGpu[$taskI].voltage_mv-ne($taskCurveBefore.nodes[$taskI].voltage_uv/1000)){throw 'Saved GPU voltage grid changed.'}}
    $taskPlan=Invoke-Omen @('profile-plan',$taskId)
    if($taskPlan.applicable){throw 'Fan draft incorrectly claims hardware-write support.'}
    # Reopen saved state in the same window and verify native persistence.
    [void][OmenCurveSmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]::new([uint64]518),[IntPtr]::Zero)
    Start-Sleep -Milliseconds 150
    Wait-OmenText $taskWindow 'Gespeicherte Kurve'|Out-Null
    $taskAfter=Invoke-Omen @('gpu-capabilities');$taskCurveAfter=Invoke-Omen @('gpu-curve-read')
    if($taskBefore.gpu.core.Ok.current_mhz-ne$taskAfter.gpu.core.Ok.current_mhz -or $taskBefore.gpu.memory.Ok.current_mhz-ne$taskAfter.gpu.memory.Ok.current_mhz){throw 'Hardware offsets changed in the draft-only editor.'}
    $taskControlBefore=@($taskCurveBefore.nodes|ForEach-Object{@($_.index,$_.voltage_uv,$_.delta_khz)})|ConvertTo-Json -Depth 5 -Compress
    $taskControlAfter=@($taskCurveAfter.nodes|ForEach-Object{@($_.index,$_.voltage_uv,$_.delta_khz)})|ConvertTo-Json -Depth 5 -Compress
    if($taskControlBefore-cne$taskControlAfter){throw 'Programmed GPU curve controls changed.'}
    [void][OmenCurveSmoke]::PostMessage($taskWindow,0x0010,[UIntPtr]::Zero,[IntPtr]::Zero)
    if(-not$taskProcess.WaitForExit(3000)-or$taskProcess.ExitCode-ne0){throw 'Editor did not close cleanly.'}
    [pscustomobject]@{measured_at=(Get-Date).ToString('o');binary_sha256=(Get-FileHash -LiteralPath $taskExe).Hash;gpu_drag=$true;shift_group_drag=$true;band_selection_flatten=$true;undo_redo=$true;zoom_fit=$true;fixed_voltage_anchors=$true;fan_drag=$true;keyboard=$true;fan_insert_remove=$true;separate_fan_curves=$true;save_reload=$true;other_settings_preserved=$true;fan_write_support_not_claimed=$true;hardware_controls_unchanged=$true;clean_exit=$true} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRepo '.local\curve-ui-smoke.json') -Encoding UTF8
    'Native GPU/fan curves: mouse, keyboard, immutable anchors, fan insert/remove, separate persistence and no hardware writes passed.'
} finally {
    if($taskProcess){$taskProcess.Refresh();if(-not$taskProcess.HasExited){Stop-Process -Id $taskProcess.Id};$taskProcess.Dispose()}
    if($taskCreated-and(Test-Path -LiteralPath $taskProfilePath)){$taskTarget=(Resolve-Path -LiteralPath $taskProfilePath).Path;if([IO.Path]::GetDirectoryName($taskTarget)-ne$taskProfileRoot){throw 'Test cleanup escaped its profile directory.'};Remove-Item -LiteralPath $taskTarget}
}
