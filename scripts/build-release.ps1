param()
$ErrorActionPreference = 'Stop'
$taskRepo = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $taskRepo
try {
    cargo build --release --locked -p omen-hub-plus-plus
    if ($LASTEXITCODE -ne 0) { throw 'Release build failed.' }
    $taskDist = Join-Path $taskRepo 'dist'
    New-Item -ItemType Directory -Force -Path $taskDist | Out-Null
    $taskOutput = Join-Path $taskDist 'Omen Hub++.exe'
    Copy-Item -LiteralPath (Join-Path $taskRepo 'target\release\omen-hub-pp.exe') -Destination $taskOutput
    $taskCpuSource = Join-Path $taskRepo '.tools\pawnio\source-0.2.11'
    if (Test-Path -LiteralPath (Join-Path $taskRepo '.tools\pawnio\modules-0.2.11\AMDFamily17.bin')) {
        foreach ($taskFile in @('AMDFamily17.p', 'COPYING', 'source-0.2.11.zip')) {
            if (-not (Test-Path -LiteralPath (Join-Path $taskCpuSource $taskFile))) { throw 'CPU module license/source missing. Run scripts/prepare-cpu-module.ps1.' }
        }
        $taskNoticeDir = Join-Path $taskDist 'third-party\PawnIO.Modules'
        New-Item -ItemType Directory -Force -Path $taskNoticeDir | Out-Null
        Copy-Item -LiteralPath (Join-Path $taskCpuSource 'AMDFamily17.p'), (Join-Path $taskCpuSource 'COPYING'), (Join-Path $taskCpuSource 'source-0.2.11.zip') -Destination $taskNoticeDir
        Copy-Item -LiteralPath (Join-Path $taskRepo 'docs\CPU_SENSORS.md') -Destination $taskNoticeDir
    }
    [pscustomobject]@{
        File = $taskOutput
        Bytes = (Get-Item -LiteralPath $taskOutput).Length
        SHA256 = (Get-FileHash -LiteralPath $taskOutput -Algorithm SHA256).Hash
    } | Format-List
} finally { Pop-Location }
