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
    [pscustomobject]@{
        File = $taskOutput
        Bytes = (Get-Item -LiteralPath $taskOutput).Length
        SHA256 = (Get-FileHash -LiteralPath $taskOutput -Algorithm SHA256).Hash
    } | Format-List
} finally { Pop-Location }
