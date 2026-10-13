param([ValidateRange(5, 55)][int]$DurationSeconds = 30)
$ErrorActionPreference = 'Stop'
$taskRepo = Split-Path -Parent $PSScriptRoot
$taskExe = Join-Path $taskRepo 'dist\Omen Hub++.exe'
$taskProcess = Start-Process -FilePath $taskExe -ArgumentList '--tray' -PassThru -WindowStyle Hidden
try {
    Start-Sleep -Seconds 5
    $taskProcess.Refresh()
    if ($taskProcess.HasExited) { throw 'Tray process failed during startup.' }
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
