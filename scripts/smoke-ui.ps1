param([string]$Executable)
$ErrorActionPreference = 'Stop'
$taskRepo = Split-Path -Parent $PSScriptRoot
if (-not $Executable) { $Executable = Join-Path $taskRepo 'dist\Omen Hub++.exe' }
$taskExe = (Resolve-Path -LiteralPath $Executable).Path
if (-not ('OmenUiSmoke' -as [type])) {
    Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class OmenUiSmoke {
    public delegate bool EnumWindow(IntPtr hwnd, IntPtr param);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumWindow callback, IntPtr param);
    [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr parent, EnumWindow callback, IntPtr param);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetWindowText(IntPtr hwnd, StringBuilder text, int capacity);
    [DllImport("user32.dll", EntryPoint="SendMessageTimeoutW", CharSet=CharSet.Unicode)] static extern IntPtr ReadText(IntPtr hwnd, uint message, UIntPtr capacity, StringBuilder text, uint flags, uint timeout, out UIntPtr result);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetClassName(IntPtr hwnd, StringBuilder text, int capacity);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hwnd, uint message, UIntPtr wparam, IntPtr lparam);
    [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr hwnd, int id);
    [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr hwnd, uint message, UIntPtr wparam, IntPtr lparam);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
    public static IntPtr Find(uint owner) {
        IntPtr result = IntPtr.Zero;
        EnumWindows((hwnd, param) => {
            uint pid; GetWindowThreadProcessId(hwnd, out pid);
            var name = new StringBuilder(128); GetClassName(hwnd, name, name.Capacity);
            if (pid == owner && name.ToString() == "OmenHubPlusPlusWindow") { result = hwnd; return false; }
            return true;
        }, IntPtr.Zero);
        return result;
    }
    public static string ChildrenText(IntPtr hwnd) {
        var values = new List<string>();
        EnumChildWindows(hwnd, (child, param) => {
            var text = new StringBuilder(16384); UIntPtr result;
            ReadText(child, 0x000D, (UIntPtr)text.Capacity, text, 2, 500, out result);
            values.Add(text.ToString()); return true;
        }, IntPtr.Zero);
        return string.Join("\n", values);
    }
}
'@
}
$taskProcess = Start-Process -FilePath $taskExe -PassThru -WindowStyle Hidden
try {
    $taskDeadline = (Get-Date).AddSeconds(12)
    $taskWindow = [IntPtr]::Zero
    do {
        $taskProcess.Refresh()
        if ($taskProcess.HasExited) { throw "Application exited during startup: $($taskProcess.ExitCode)" }
        $taskWindow = [OmenUiSmoke]::Find([uint32]$taskProcess.Id)
        $taskText = if ($taskWindow -ne [IntPtr]::Zero) { [OmenUiSmoke]::ChildrenText($taskWindow) } else { '' }
        if ($taskText -match 'Board: 8E35') { break }
        Start-Sleep -Milliseconds 100
    } while ((Get-Date) -lt $taskDeadline)
    if ($taskWindow -eq [IntPtr]::Zero -or $taskText -notmatch 'Board: 8E35') { throw "Native window or inventory did not become ready. Window text: $taskText" }
    if ($taskText -notmatch 'GPU · NVIDIA') { throw 'GPU sensor section is missing.' }
    if ($taskText -notmatch 'CPU-Temperatur \(Tctl\)' -or $taskText -notmatch 'Windows-Takt: \d+ MHz') { throw 'CPU sensor section or qualified Windows clock is missing.' }
    if ($taskText -notmatch 'CPU-Paketleistung\s+Nicht verfügbar' -or $taskText -notmatch 'CPU-Temperatur \(Tctl\)\s+Nicht verfügbar') { throw 'Unvalidated CPU hardware measurements must be shown as unavailable.' }
    # The first reading establishes a baseline; the next polling interval must
    # provide real busy-time data rather than leave the CPU permanently loading.
    $taskCpuDeadline = (Get-Date).AddSeconds(5)
    while ($taskText -notmatch 'Auslastung: \d+[\.,]\d+ %' -and (Get-Date) -lt $taskCpuDeadline) {
        Start-Sleep -Milliseconds 100
        $taskText = [OmenUiSmoke]::ChildrenText($taskWindow)
    }
    if ($taskText -notmatch 'Auslastung: \d+[\.,]\d+ %') { throw 'CPU busy-time measurement did not become ready.' }
    $taskCombo = [OmenUiSmoke]::GetDlgItem($taskWindow, 102)
    [void][OmenUiSmoke]::SendMessage($taskCombo, 0x014E, [UIntPtr]::new([uint64]2), [IntPtr]::Zero)
    [void][OmenUiSmoke]::PostMessage($taskWindow, 0x0111, [UIntPtr]::new([uint64]0x00090066), [IntPtr]::Zero)
    Start-Sleep -Milliseconds 150
    $taskText = [OmenUiSmoke]::ChildrenText($taskWindow)
    if ($taskText -notmatch 'Performance \(Default\): noch nicht verfügbar.') { throw 'Profile preview did not update.' }
    $taskLocal = Join-Path $taskRepo '.local'
    New-Item -ItemType Directory -Force -Path $taskLocal | Out-Null
    $taskText | Set-Content -LiteralPath (Join-Path $taskLocal 'ui-smoke-text.txt') -Encoding utf8
    [void][OmenUiSmoke]::PostMessage($taskWindow, 0x0010, [UIntPtr]::Zero, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 200
    if ([OmenUiSmoke]::IsWindowVisible($taskWindow)) { throw 'Close did not hide the application in tray.' }
    $taskProcess.Refresh()
    if ($taskProcess.HasExited) { throw 'Close unexpectedly exited the tray application.' }
    if ([OmenUiSmoke]::GetDlgItem($taskWindow,106)-ne[IntPtr]::Zero) { throw 'Hidden dashboard did not release its native controls.' }
    [void][OmenUiSmoke]::PostMessage($taskWindow,0x0111,[UIntPtr]::new([uint64]201),[IntPtr]::Zero)
    $taskReturnDeadline=(Get-Date).AddSeconds(5)
    do {$taskText=[OmenUiSmoke]::ChildrenText($taskWindow);if($taskText-match'Board: 8E35'-and$taskText-match'Performance \(Default\): noch nicht verfügbar.'){break};Start-Sleep -Milliseconds 80}while((Get-Date)-lt$taskReturnDeadline)
    $taskRestoredCombo=[OmenUiSmoke]::GetDlgItem($taskWindow,102)
    if($taskText-notmatch'Performance \(Default\): noch nicht verfügbar.'-or[OmenUiSmoke]::SendMessage($taskRestoredCombo,0x0147,[UIntPtr]::Zero,[IntPtr]::Zero).ToInt64()-ne2){throw 'Rebuilt dashboard lost its preview selection.'}
    if(-not[OmenUiSmoke]::IsWindowVisible($taskWindow)-or[OmenUiSmoke]::GetDlgItem($taskWindow,103)-eq[IntPtr]::Zero){throw 'Tray return did not rebuild operable controls.'}
    [void][OmenUiSmoke]::PostMessage($taskWindow, 0x0111, [UIntPtr]::new([uint64]202), [IntPtr]::Zero)
    if (-not $taskProcess.WaitForExit(3000)) { throw 'Application did not exit within 3 seconds.' }
    if ($taskProcess.ExitCode -ne 0) { throw "Application failed: $($taskProcess.ExitCode)" }
    Write-Output 'Native window, inventory, CPU/GPU telemetry, profile preview, close-to-tray and clean exit passed.'
} finally {
    $taskProcess.Refresh()
    if (-not $taskProcess.HasExited) { Stop-Process -Id $taskProcess.Id }
    $taskProcess.Dispose()
}
