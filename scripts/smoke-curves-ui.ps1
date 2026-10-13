# Own-process native editor test. All mutations are profile metadata, never SETs.
#requires -Version 5.1
param()
$ErrorActionPreference='Stop'
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
    $taskCanvas=[OmenCurveSmoke]::GetDlgItem($taskWindow,519)
    $taskPoints=[OmenCurveSmoke]::GetDlgItem($taskWindow,509)
    $taskX=[OmenCurveSmoke]::GetDlgItem($taskWindow,510)
    $taskY=[OmenCurveSmoke]::GetDlgItem($taskWindow,511)
    if([OmenCurveSmoke]::Message($taskPoints,0x0146)-ne127){throw 'Complete GPU grid not present.'}
    # Numeric voltage-anchor tampering is rejected even outside the readonly UI.
    $taskAnchor=[int][OmenCurveSmoke]::Text($taskX)
    [OmenCurveSmoke]::SetText($taskX,([string]($taskAnchor+5)))
    [void][OmenCurveSmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]512,[IntPtr]::Zero)
    Wait-OmenText $taskWindow 'GPU-Spannungsanker bleiben unverändert'|Out-Null
    [void][OmenCurveSmoke]::Message($taskCanvas,0x0100,0x28)
    Start-Sleep -Milliseconds 120
    if([int][OmenCurveSmoke]::Text($taskX)-ne$taskAnchor){throw 'GPU voltage anchor changed.'}
    # Exercise real mouse messages in the GPU canvas; horizontal drag stays fixed.
    $taskRect=New-Object OmenCurveSmoke+Rect
    [void][OmenCurveSmoke]::GetClientRect($taskCanvas,[ref]$taskRect)
    $taskGpuY=[int][OmenCurveSmoke]::Text($taskY)
    $taskMax=[int]([math]::Ceiling(($taskCurveBefore.nodes[-1].frequency_khz/1000+$taskCurveBefore.maximum_delta_khz/1000)/1000)*1000)
    $taskPx=$taskRect.Right-28
    $taskPy=[int]($taskRect.Bottom-64-($taskGpuY/$taskMax)*($taskRect.Bottom-98))
    [OmenCurveSmoke]::Mouse($taskCanvas,0x0201,$taskPx,$taskPy,1)
    [OmenCurveSmoke]::Mouse($taskCanvas,0x0200,($taskPx-25),($taskPy-2),1)
    [OmenCurveSmoke]::Mouse($taskCanvas,0x0202,($taskPx-25),($taskPy-2),0)
    Start-Sleep -Milliseconds 120
    if([int][OmenCurveSmoke]::Text($taskX)-ne$taskAnchor){throw 'GPU horizontal drag altered the voltage grid.'}
    # Separate CPU fan curve, real drag with ordered temperature/duty constraints.
    [void][OmenCurveSmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]506,[IntPtr]::Zero)
    Wait-OmenText $taskWindow 'CPU-Lüfter'|Out-Null
    [void][OmenCurveSmoke]::Message($taskPoints,0x014E,1)
    [void][OmenCurveSmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]0x000101FD,[IntPtr]::Zero)
    Start-Sleep -Milliseconds 100
    $taskFanX=[int][OmenCurveSmoke]::Text($taskX);$taskFanY=[int][OmenCurveSmoke]::Text($taskY)
    $taskPx=[int](72+($taskFanX/110)*($taskRect.Right-100));$taskPy=[int]($taskRect.Bottom-64-($taskFanY/100)*($taskRect.Bottom-98))
    [OmenCurveSmoke]::Mouse($taskCanvas,0x0201,$taskPx,$taskPy,1)
    [OmenCurveSmoke]::Mouse($taskCanvas,0x0200,($taskPx+20),($taskPy-20),1)
    [OmenCurveSmoke]::Mouse($taskCanvas,0x0202,($taskPx+20),($taskPy-20),0)
    Start-Sleep -Milliseconds 100
    if([int][OmenCurveSmoke]::Text($taskY)-le$taskFanY){throw 'CPU fan drag did not edit the draft.'}
    [void][OmenCurveSmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]513,[IntPtr]::Zero)
    Start-Sleep -Milliseconds 100
    if([OmenCurveSmoke]::Message($taskPoints,0x0146)-ne5){throw 'Fan point insertion failed.'}
    [void][OmenCurveSmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]514,[IntPtr]::Zero)
    Start-Sleep -Milliseconds 100
    if([OmenCurveSmoke]::Message($taskPoints,0x0146)-ne4){throw 'Fan point removal failed.'}
    [void][OmenCurveSmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]507,[IntPtr]::Zero)
    Wait-OmenText $taskWindow 'GPU-Lüfter'|Out-Null
    [void][OmenCurveSmoke]::Message($taskCanvas,0x0100,0x26)
    Start-Sleep -Milliseconds 100
    [void][OmenCurveSmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]503,[IntPtr]::Zero)
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
    [void][OmenCurveSmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]518,[IntPtr]::Zero)
    Start-Sleep -Milliseconds 150
    Wait-OmenText $taskWindow 'Gespeicherte Kurve'|Out-Null
    $taskAfter=Invoke-Omen @('gpu-capabilities');$taskCurveAfter=Invoke-Omen @('gpu-curve-read')
    if($taskBefore.gpu.core.Ok.current_mhz-ne$taskAfter.gpu.core.Ok.current_mhz -or $taskBefore.gpu.memory.Ok.current_mhz-ne$taskAfter.gpu.memory.Ok.current_mhz){throw 'Hardware offsets changed in the draft-only editor.'}
    $taskControlBefore=@($taskCurveBefore.nodes|ForEach-Object{@($_.index,$_.voltage_uv,$_.delta_khz)})|ConvertTo-Json -Depth 5 -Compress
    $taskControlAfter=@($taskCurveAfter.nodes|ForEach-Object{@($_.index,$_.voltage_uv,$_.delta_khz)})|ConvertTo-Json -Depth 5 -Compress
    if($taskControlBefore-cne$taskControlAfter){throw 'Programmed GPU curve controls changed.'}
    [void][OmenCurveSmoke]::PostMessage($taskWindow,0x0010,[UIntPtr]::Zero,[IntPtr]::Zero)
    if(-not$taskProcess.WaitForExit(3000)-or$taskProcess.ExitCode-ne0){throw 'Editor did not close cleanly.'}
    [pscustomobject]@{measured_at=(Get-Date).ToString('o');binary_sha256=(Get-FileHash -LiteralPath $taskExe).Hash;gpu_drag=$true;fixed_voltage_anchors=$true;fan_drag=$true;keyboard=$true;fan_insert_remove=$true;separate_fan_curves=$true;save_reload=$true;other_settings_preserved=$true;fan_write_support_not_claimed=$true;hardware_controls_unchanged=$true;clean_exit=$true} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRepo '.local\curve-ui-smoke.json') -Encoding UTF8
    'Native GPU/fan curves: mouse, keyboard, immutable anchors, fan insert/remove, separate persistence and no hardware writes passed.'
} finally {
    if($taskProcess){$taskProcess.Refresh();if(-not$taskProcess.HasExited){Stop-Process -Id $taskProcess.Id};$taskProcess.Dispose()}
    if($taskCreated-and(Test-Path -LiteralPath $taskProfilePath)){$taskTarget=(Resolve-Path -LiteralPath $taskProfilePath).Path;if([IO.Path]::GetDirectoryName($taskTarget)-ne$taskProfileRoot){throw 'Test cleanup escaped its profile directory.'};Remove-Item -LiteralPath $taskTarget}
}
