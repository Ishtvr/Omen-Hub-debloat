# Metadata-only integration test scoped to a newly launched Omen Hub++ process.
# Never invokes apply, restore, startup enable or any hardware setter.
#requires -Version 5.1
param()
$ErrorActionPreference = 'Stop'
$taskRepo = Split-Path -Parent $PSScriptRoot
$taskExe = Join-Path $taskRepo 'dist\Omen Hub++.exe'
$taskProfileName = 'Omen GPU UI Test ' + [Guid]::NewGuid().ToString('N')
$taskStorage = & $taskExe profile-storage | Out-String | ConvertFrom-Json
$taskProfiles = [IO.Path]::GetFullPath((Join-Path $taskStorage.directory 'profiles'))
$taskCreated = $null
if (-not ('OmenGpuUiSmoke' -as [type])) {
    Add-Type @'
using System;
using System.Text;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class OmenGpuUiSmoke {
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
    public static IntPtr Find(uint process) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((hwnd,arg) => { uint owner; GetWindowThreadProcessId(hwnd,out owner); var name = new StringBuilder(128); GetClassName(hwnd,name,name.Capacity);
            if (owner == process && name.ToString() == "OmenHubPPGpuProfiles") { found = hwnd; return false; } return true; },IntPtr.Zero);
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
$taskBefore = (& $taskExe gpu-capabilities | Out-String | ConvertFrom-Json).gpu
$taskCurveBefore = & $taskExe gpu-curve-read | Out-String | ConvertFrom-Json
$taskProcess = Start-Process -FilePath $taskExe -ArgumentList 'gpu-ui' -PassThru -WindowStyle Hidden
try {
    $taskDeadline = (Get-Date).AddSeconds(12)
    $taskWindow = [IntPtr]::Zero
    do {
        $taskProcess.Refresh()
        if ($taskProcess.HasExited) { throw "GPU editor exited: $($taskProcess.ExitCode)" }
        $taskWindow = [OmenGpuUiSmoke]::Find([uint32]$taskProcess.Id)
        $taskText = if ($taskWindow -ne [IntPtr]::Zero) { [OmenGpuUiSmoke]::Text($taskWindow) } else { '' }
        if ($taskText -match 'NVML-MHz' -and [OmenGpuUiSmoke]::IsWindowEnabled([OmenGpuUiSmoke]::GetDlgItem($taskWindow,303))) { break }
        Start-Sleep -Milliseconds 100
    } while ((Get-Date) -lt $taskDeadline)
    if ($taskText -notmatch 'NVML-MHz' -or $taskText -notmatch 'RTX 5070 Laptop GPU') { throw "GPU editor did not become ready: $taskText" }
    if ($taskText -notmatch 'V/F-&Kurve verwenden' -or $taskText -notmatch 'kein hartes Spannungslimit' -or $taskText -notmatch 'VRAM-Spannung: kein verifizierter') { throw 'Curve scope or unsupported voltage controls are not explained.' }
    $taskCore = [OmenGpuUiSmoke]::GetDlgItem($taskWindow,310)
    $taskMemory = [OmenGpuUiSmoke]::GetDlgItem($taskWindow,311)
    $taskName = [OmenGpuUiSmoke]::GetDlgItem($taskWindow,309)
    $taskApply = [OmenGpuUiSmoke]::GetDlgItem($taskWindow,304)
    $taskUsesActiveCurve=-not[OmenGpuUiSmoke]::IsWindowEnabled($taskCore)
    $taskInvalidField=if($taskUsesActiveCurve){$taskMemory}else{$taskCore}
    [void][OmenGpuUiSmoke]::SetWindowText($taskInvalidField,'keinezahl')
    $taskInvalidNotification=if($taskUsesActiveCurve){0x02000137}else{0x02000136}
    [void][OmenGpuUiSmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]::new([uint64]$taskInvalidNotification),[IntPtr]::Zero)
    Start-Sleep -Milliseconds 150
    if ([OmenGpuUiSmoke]::IsWindowEnabled($taskApply)) { throw 'Apply remains enabled for an unsaved/invalid draft.' }
    $taskExpectedError=if($taskUsesActiveCurve){'VRAM-Offset als ganze MHz-Zahl'}else{'GPU-Offset als ganze MHz-Zahl'}
    if ([OmenGpuUiSmoke]::Text($taskWindow) -notmatch $taskExpectedError) { throw ('Inline numeric validation is missing: ' + [OmenGpuUiSmoke]::Text($taskWindow)) }
    [void][OmenGpuUiSmoke]::SetWindowText($taskName,$taskProfileName)
    [void][OmenGpuUiSmoke]::SetWindowText($taskCore,'0')
    [void][OmenGpuUiSmoke]::SetWindowText($taskMemory,'0')
    [void][OmenGpuUiSmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]::new([uint64]303),[IntPtr]::Zero)
    $taskDeadline = (Get-Date).AddSeconds(12)
    do {
        $taskText = [OmenGpuUiSmoke]::Text($taskWindow)
        if ($taskText -match 'Profil gespeichert') { break }
        Start-Sleep -Milliseconds 100
    } while ((Get-Date) -lt $taskDeadline)
    if ($taskText -notmatch 'Profil gespeichert') { throw "Profile save failed: $taskText" }
    foreach ($taskFile in (Get-ChildItem -LiteralPath $taskProfiles -File -Filter '*.json')) {
        $taskProfile = Get-Content -LiteralPath $taskFile.FullName -Raw | ConvertFrom-Json
        if ($taskProfile.name -eq $taskProfileName) { $taskCreated = $taskFile.FullName; break }
    }
    if (-not $taskCreated -or $taskProfile.schema -ne 1 -or $taskProfile.settings.Count -ne 2) { throw 'Saved typed profile does not match the draft.' }
    if($taskUsesActiveCurve){
        $taskInitialCurve=@($taskProfile.settings|Where-Object feature -eq 'gpu_vf_curve')
        if($taskInitialCurve.Count-ne1-or$taskInitialCurve[0].value.value.Count-ne127-or@($taskProfile.settings|Where-Object feature -eq 'gpu_core_offset_mhz').Count-ne0){throw 'Active individual curve was not preserved.'}
        # Boost observations can change between reads. Compare anchors here;
        # complete point preservation is checked below against an imported draft.
        for($taskI=0;$taskI-lt127;$taskI++){if($taskInitialCurve[0].value.value[$taskI].voltage_mv-ne($taskCurveBefore.nodes[$taskI].voltage_uv/1000)){throw 'Saving the active curve altered a voltage anchor.'}}
    }elseif(@($taskProfile.settings | Where-Object { $_.value.value -ne 0 }).Count -ne 0){throw 'Saved scalar offsets do not match the draft.'}
    if (-not [OmenGpuUiSmoke]::IsWindowEnabled($taskApply)) { throw 'Verified supported saved profile did not enable Apply.' }
    $taskCurveCheck = [OmenGpuUiSmoke]::GetDlgItem($taskWindow,312)
    $taskVoltage = [OmenGpuUiSmoke]::GetDlgItem($taskWindow,313)
    $taskFrequency = [OmenGpuUiSmoke]::GetDlgItem($taskWindow,314)
    if(-not$taskUsesActiveCurve){[void][OmenGpuUiSmoke]::Message($taskCurveCheck,0x00F5)}
    if ([OmenGpuUiSmoke]::IsWindowEnabled($taskCore) -or -not [OmenGpuUiSmoke]::IsWindowEnabled($taskFrequency)) { throw 'Curve mode did not exclude global core-offset editing.' }
    [void][OmenGpuUiSmoke]::SetWindowText($taskFrequency,'keinezahl')
    [void][OmenGpuUiSmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]::new([uint64]0x0200013A),[IntPtr]::Zero)
    Start-Sleep -Milliseconds 100
    if ([OmenGpuUiSmoke]::Text($taskWindow) -notmatch 'Frequenzziel als ganze MHz-Zahl') { throw 'Curve target validation is missing.' }
    $taskLast = $taskCurveBefore.nodes[-1]
    [void][OmenGpuUiSmoke]::Message($taskVoltage,0x014E,[uint64]($taskCurveBefore.nodes.Count - 1))
    [void][OmenGpuUiSmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]::new([uint64]0x00010139),[IntPtr]::Zero)
    [void][OmenGpuUiSmoke]::SetWindowText($taskFrequency,([string]($taskLast.frequency_khz / 1000)))
    [void][OmenGpuUiSmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]::new([uint64]303),[IntPtr]::Zero)
    $taskDeadline = (Get-Date).AddSeconds(12)
    do {
        Start-Sleep -Milliseconds 100
        $taskText = [OmenGpuUiSmoke]::Text($taskWindow)
    } while ($taskText -notmatch 'Profil gespeichert' -and (Get-Date) -lt $taskDeadline)
    if ($taskText -notmatch 'Profil gespeichert') { throw "Curve save failed: $taskText" }
    $taskProfile = Get-Content -LiteralPath $taskCreated -Raw | ConvertFrom-Json
    $taskSavedCurve = @($taskProfile.settings | Where-Object feature -eq 'gpu_vf_curve')
    if ($taskSavedCurve.Count -ne 1 -or $taskSavedCurve[0].value.value.Count -ne 127 -or @($taskProfile.settings | Where-Object feature -eq 'gpu_core_offset_mhz').Count -ne 0) { throw 'Saved curve profile lost points or contains a conflicting global core offset.' }
    # A distinct, full custom curve must survive name-only editing. This is metadata.
    foreach ($taskPoint in $taskSavedCurve[0].value.value) { $taskPoint.frequency_mhz -= 15 }
    $taskExpectedCurve = $taskSavedCurve[0].value | ConvertTo-Json -Depth 8 -Compress
    # Windows PowerShell 5.1's Set-Content UTF8 emits a BOM; the strict profile
    # parser expects ordinary UTF-8 JSON, as written by the production store.
    [IO.File]::WriteAllText($taskCreated, ($taskProfile | ConvertTo-Json -Depth 10), [Text.UTF8Encoding]::new($false))
    [void][OmenGpuUiSmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]::new([uint64]308),[IntPtr]::Zero)
    $taskDeadline = (Get-Date).AddSeconds(12)
    do { Start-Sleep -Milliseconds 100 } while (-not [OmenGpuUiSmoke]::IsWindowEnabled([OmenGpuUiSmoke]::GetDlgItem($taskWindow,303)) -and (Get-Date) -lt $taskDeadline)
    [void][OmenGpuUiSmoke]::SetWindowText($taskName,($taskProfileName + ' curve'))
    [void][OmenGpuUiSmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]::new([uint64]303),[IntPtr]::Zero)
    $taskDeadline = (Get-Date).AddSeconds(12)
    do { Start-Sleep -Milliseconds 100; $taskText = [OmenGpuUiSmoke]::Text($taskWindow) } while ($taskText -notmatch 'Profil gespeichert' -and (Get-Date) -lt $taskDeadline)
    $taskProfile = Get-Content -LiteralPath $taskCreated -Raw | ConvertFrom-Json
    if ($taskProfile.name -ne ($taskProfileName + ' curve') -or ((@($taskProfile.settings | Where-Object feature -eq 'gpu_vf_curve')[0].value | ConvertTo-Json -Depth 8 -Compress) -cne $taskExpectedCurve)) { throw 'Name-only editing changed the imported custom curve.' }
    # Max-Fan is profile intent only. Save/reload must preserve the full custom
    # curve, and a cooling override must never become eligible for logon apply.
    $taskMax=[OmenGpuUiSmoke]::GetDlgItem($taskWindow,316)
    if($taskMax-eq[IntPtr]::Zero-or-not[OmenGpuUiSmoke]::IsWindowEnabled($taskMax)){throw 'Qualified Max-Fan profile field is missing.'}
    foreach($taskChoice in @(1,2,0)){
        [void][OmenGpuUiSmoke]::Message($taskMax,0x014E,[uint64]$taskChoice)
        [void][OmenGpuUiSmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]::new([uint64]0x0001013C),[IntPtr]::Zero)
        Start-Sleep -Milliseconds 100
        if([OmenGpuUiSmoke]::IsWindowEnabled($taskApply)){throw 'Max-Fan draft did not gate Apply.'}
        [void][OmenGpuUiSmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]::new([uint64]303),[IntPtr]::Zero)
        $taskDeadline=(Get-Date).AddSeconds(12)
        do{Start-Sleep -Milliseconds 100;$taskText=[OmenGpuUiSmoke]::Text($taskWindow)}while($taskText-notmatch'Profil gespeichert'-and(Get-Date)-lt$taskDeadline)
        $taskProfile=Get-Content -LiteralPath $taskCreated -Raw|ConvertFrom-Json
        $taskMaxSetting=@($taskProfile.settings|Where-Object feature -eq 'max_fan')
        if($taskChoice-eq0){if($taskMaxSetting.Count-ne0){throw 'Nicht aendern retained a Max-Fan write instruction.'}}
        elseif($taskMaxSetting.Count-ne1-or$taskMaxSetting[0].value.kind-cne'boolean'-or$taskMaxSetting[0].value.value-ne($taskChoice-eq1)){throw 'Max-Fan boolean did not persist exactly.'}
        if((@($taskProfile.settings|Where-Object feature -eq 'gpu_vf_curve')[0].value|ConvertTo-Json -Depth 8 -Compress)-cne$taskExpectedCurve){throw 'Max-Fan save changed the full custom GPU curve.'}
        if($taskChoice-ne0-and[OmenGpuUiSmoke]::IsWindowEnabled([OmenGpuUiSmoke]::GetDlgItem($taskWindow,306))){throw 'Unqualified Max-Fan lifecycle enabled logon application.'}
        [void][OmenGpuUiSmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]::new([uint64]308),[IntPtr]::Zero)
        Start-Sleep -Milliseconds 100
        $taskDeadline=(Get-Date).AddSeconds(12)
        while(-not[OmenGpuUiSmoke]::IsWindowEnabled($taskMax)-and(Get-Date)-lt$taskDeadline){Start-Sleep -Milliseconds 80}
        $taskActual=[OmenGpuUiSmoke]::Message($taskMax,0x0147)
        if($taskActual-ne$taskChoice){throw 'Max-Fan profile reload lost its choice.'}
    }
    $taskAfter = (& $taskExe gpu-capabilities | Out-String | ConvertFrom-Json).gpu
    if ($taskBefore.core.Ok.current_mhz -ne $taskAfter.core.Ok.current_mhz -or $taskBefore.memory.Ok.current_mhz -ne $taskAfter.memory.Ok.current_mhz) { throw 'GPU offsets changed during a metadata-only test.' }
    $taskCurveAfter = & $taskExe gpu-curve-read | Out-String | ConvertFrom-Json
    $taskControlsBefore=@($taskCurveBefore.nodes | ForEach-Object { @($_.index,$_.voltage_uv,$_.delta_khz) }) | ConvertTo-Json -Depth 5 -Compress
    $taskControlsAfter=@($taskCurveAfter.nodes | ForEach-Object { @($_.index,$_.voltage_uv,$_.delta_khz) }) | ConvertTo-Json -Depth 5 -Compress
    if ($taskControlsBefore -cne $taskControlsAfter -or $taskCurveBefore.minimum_delta_khz -ne $taskCurveAfter.minimum_delta_khz -or $taskCurveBefore.maximum_delta_khz -ne $taskCurveAfter.maximum_delta_khz) { throw 'GPU curve controls changed during a metadata-only test.' }
    $taskText | Set-Content -LiteralPath (Join-Path $taskRepo '.local\gpu-ui-smoke-text.txt') -Encoding UTF8
    [void][OmenGpuUiSmoke]::PostMessage($taskWindow,0x0010,[UIntPtr]::Zero,[IntPtr]::Zero)
    if (-not $taskProcess.WaitForExit(3000) -or $taskProcess.ExitCode -ne 0) { throw 'GPU editor did not close cleanly.' }
    [pscustomobject]@{ measured_at=(Get-Date).ToString('o'); binary_sha256=(Get-FileHash -LiteralPath $taskExe).Hash; read_and_save=$true; curve_save=$true; custom_curve_preserved=$true; max_fan_metadata_roundtrip=$true;max_fan_startup_blocked=$true; inline_validation=$true; apply_disabled_for_draft=$true; offsets_unchanged=$true; curve_unchanged=$true; hardware_writes=$false; clean_exit=$true } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRepo '.local\gpu-ui-smoke.json') -Encoding UTF8
    'Native GPU editor, truthful support labels, inline validation, save-only persistence, draft gating and clean exit passed; GPU offsets unchanged.'
} finally {
    $taskProcess.Refresh()
    if (-not $taskProcess.HasExited) { Stop-Process -Id $taskProcess.Id }
    $taskProcess.Dispose()
    if ($taskCreated) {
        $taskResolved = (Resolve-Path -LiteralPath $taskCreated).Path
        if ([IO.Path]::GetDirectoryName($taskResolved) -ne $taskProfiles) { throw 'Test cleanup target escaped its profile directory.' }
        Remove-Item -LiteralPath $taskResolved
    }
}
