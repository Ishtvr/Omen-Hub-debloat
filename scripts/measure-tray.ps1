param([ValidateRange(5, 55)][int]$DurationSeconds = 30)
$ErrorActionPreference = 'Stop'
$taskRepo = Split-Path -Parent $PSScriptRoot
$taskExe = Join-Path $taskRepo 'dist\Omen Hub++.exe'
if(-not('OmenTrayVisibility'-as[type])){Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class OmenTrayVisibility {
 public delegate bool Callback(IntPtr hwnd,IntPtr other);
 [DllImport("user32.dll")] static extern bool EnumWindows(Callback cb,IntPtr other);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd,out uint pid);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetClassName(IntPtr hwnd,StringBuilder text,int capacity);
 [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr hwnd);
 public static bool Hidden(uint pid){bool found=false;bool hidden=false;EnumWindows((h,a)=>{uint owner;GetWindowThreadProcessId(h,out owner);var text=new StringBuilder(128);GetClassName(h,text,text.Capacity);if(owner==pid&&text.ToString()=="OmenHubPlusPlusWindow"){found=true;hidden=!IsWindowVisible(h);return false;}return true;},IntPtr.Zero);return found&&hidden;}
}
'@}
$taskProcess = Start-Process -FilePath $taskExe -ArgumentList '--tray' -PassThru -WindowStyle Hidden
try {
    Start-Sleep -Seconds 5
    $taskProcess.Refresh()
    if ($taskProcess.HasExited) { throw 'Tray process failed during startup.' }
    if(-not[OmenTrayVisibility]::Hidden([uint32]$taskProcess.Id)){throw 'Owned app is not hidden; refuse a cold-tray label.'}
    $taskBaselineCpu = $taskProcess.TotalProcessorTime.TotalSeconds
    Start-Sleep -Seconds ($DurationSeconds - 5)
    $taskProcess.Refresh()
    if ($taskProcess.HasExited) { throw 'Tray process exited during measurement.' }
    $taskRecord = [pscustomobject]@{
        MeasuredAt = (Get-Date).ToString('o')
        DurationSeconds = $DurationSeconds
        ExecutableBytes = (Get-Item -LiteralPath $taskExe).Length
        ExecutableSHA256 = (Get-FileHash -LiteralPath $taskExe).Hash
        WorkingSetMiB = [math]::Round($taskProcess.WorkingSet64 / 1MB, 2)
        PrivateMemoryMiB = [math]::Round($taskProcess.PrivateMemorySize64 / 1MB, 2)
        CpuSeconds = $taskProcess.TotalProcessorTime.TotalSeconds
        CpuDeltaAfter5Seconds = $taskProcess.TotalProcessorTime.TotalSeconds - $taskBaselineCpu
        Responding = $taskProcess.Responding
        WindowHidden = [OmenTrayVisibility]::Hidden([uint32]$taskProcess.Id)
    }
    $taskLocal = Join-Path $taskRepo '.local'
    New-Item -ItemType Directory -Force -Path $taskLocal | Out-Null
    $taskRecord | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskLocal 'tray-measurement.json') -Encoding utf8
    $taskRecord | ConvertTo-Json
} finally {
    $taskProcess.Refresh()
    if (-not $taskProcess.HasExited) { Stop-Process -Id $taskProcess.Id }
    $taskProcess.Dispose()
}
