param([string]$AssemblyDirectory)
$ErrorActionPreference = 'Stop'
$taskRepo = Split-Path -Parent $PSScriptRoot
if (-not $AssemblyDirectory) {
    $taskPackage = Get-AppxPackage '*OMENCommandCenter*' | Select-Object -First 1
    if (-not $taskPackage) { throw 'OMEN Gaming Hub not found. Specify -AssemblyDirectory.' }
    $AssemblyDirectory = Join-Path $taskPackage.InstallLocation 'OmenCommandCenterApp'
}
$taskInput = (Resolve-Path -LiteralPath $AssemblyDirectory).Path
$taskDestination = Join-Path $taskRepo '.local\research'
New-Item -ItemType Directory -Force -Path $taskDestination | Out-Null
$taskTool = Join-Path $taskRepo '.tools\ilspy\ilspycmd.exe'
if (-not (Test-Path -LiteralPath $taskTool)) {
    throw 'Install ilspycmd locally: dotnet tool install ilspycmd --version 11.1.0.9782 --tool-path .tools/ilspy'
}
$taskFiles = @('HP.Omen.Background.PerformanceControl.dll', 'HP.Omen.Background.FanControlBg.dll', 'HP.Omen.PerformanceControlModule.dll', 'HP.Omen.Core.Common.dll', 'HP.Omen.CommonData.dll')
$taskManifest = @()
foreach ($taskName in $taskFiles) {
    $taskPath = Join-Path $taskInput $taskName
    if (-not (Test-Path -LiteralPath $taskPath)) { Write-Warning "Missing: $taskName"; continue }
    $taskInfo = Get-Item -LiteralPath $taskPath
    $taskHash = (Get-FileHash -LiteralPath $taskPath -Algorithm SHA256).Hash
    $taskOutput = Join-Path $taskDestination $taskName
    & $taskTool -o $taskOutput $taskPath
    $taskExit = $LASTEXITCODE
    $taskManifest += [pscustomobject]@{ Name = $taskName; Version = $taskInfo.VersionInfo.FileVersion; SHA256 = $taskHash; DecompilerExit = $taskExit }
    if ($taskExit -ne 0) { Write-Warning "Decompilation reported errors for $taskName; inspect the output." }
}
$taskManifest | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $taskDestination 'assembly-manifest.json') -Encoding utf8
$taskManifest | Format-Table Name, Version, DecompilerExit
Write-Output "Local-only analysis saved to $taskDestination. Do not commit decompiled sources."
