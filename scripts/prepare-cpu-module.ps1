# Stage the exact upstream signed module for build-time embedding; no driver API.
#requires -Version 5.1
param()
$ErrorActionPreference = 'Stop'
$taskRepo = Split-Path -Parent $PSScriptRoot
$taskRoot = Join-Path $taskRepo '.tools\pawnio'
$taskModuleDir = Join-Path $taskRoot 'modules-0.2.11'
$taskModule = Join-Path $taskModuleDir 'AMDFamily17.bin'
$taskModuleHash = 'DAE74615761B78BDF064DFB3E136252DDCC6FC727D88F14738D0E5800D427A91'
if (Test-Path -LiteralPath $taskModule) {
    if ((Get-FileHash -LiteralPath $taskModule).Hash -cne $taskModuleHash) { throw 'Staged CPU module differs from the pinned signed release.' }
} else {
    New-Item -ItemType Directory -Force -Path $taskRoot | Out-Null
    $taskZip = Join-Path $taskRoot 'release_0_2_11.zip'
    if (-not (Test-Path -LiteralPath $taskZip)) {
        Invoke-WebRequest -UseBasicParsing -Uri 'https://github.com/namazso/PawnIO.Modules/releases/download/0.2.11/release_0_2_11.zip' -OutFile $taskZip
    }
    if ((Get-FileHash -LiteralPath $taskZip).Hash -cne '43608CB89BC84247FEF1368A139013F7D043E17DB6D6C8DFC9B46BF0905A81F4') { throw 'CPU module archive hash differs.' }
    Expand-Archive -LiteralPath $taskZip -DestinationPath $taskModuleDir -Force
    if ((Get-FileHash -LiteralPath $taskModule).Hash -cne $taskModuleHash) { throw 'Extracted signed CPU module hash differs.' }
}
# Corresponding LGPL source and license accompany builds. Downloads occur only
# in this explicitly invoked preparation script; runtime has no network code.
$taskSource = Join-Path $taskRoot 'source-0.2.11'
New-Item -ItemType Directory -Force -Path $taskSource | Out-Null
foreach ($taskName in @('AMDFamily17.p', 'COPYING')) {
    $taskPath = Join-Path $taskSource $taskName
    if (-not (Test-Path -LiteralPath $taskPath)) {
        Invoke-WebRequest -UseBasicParsing -Uri ('https://raw.githubusercontent.com/namazso/PawnIO.Modules/52a7e536dff3e53c96917a28caac5e0fa6510696/' + $taskName) -OutFile $taskPath
    }
}
$taskSourceArchive = Join-Path $taskSource 'source-0.2.11.zip'
if (-not (Test-Path -LiteralPath $taskSourceArchive)) {
    Invoke-WebRequest -UseBasicParsing -Uri 'https://github.com/namazso/PawnIO.Modules/archive/52a7e536dff3e53c96917a28caac5e0fa6510696.zip' -OutFile $taskSourceArchive
}
[pscustomobject]@{ Module = $taskModule; SHA256 = $taskModuleHash; DriverInstalled = $false; HardwareAccess = $false }
