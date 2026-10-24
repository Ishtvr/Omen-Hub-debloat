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
    # build.rs embeds icon and version information together; both must be present.
    if ((Get-Item -LiteralPath $taskOutput).VersionInfo.ProductName -ne 'Omen Hub++') { throw 'Embedded icon/version resources are missing.' }
    $taskCpuSource = Join-Path $taskRepo '.tools\pawnio\source-0.2.11'
    $taskModules = Join-Path $taskRepo '.tools\pawnio\modules-0.2.11'
    $taskSmu = Test-Path -LiteralPath (Join-Path $taskModules 'RyzenSMU.bin')
    if ((Test-Path -LiteralPath (Join-Path $taskModules 'AMDFamily17.bin')) -or $taskSmu) {
        foreach ($taskFile in @('AMDFamily17.p', 'COPYING', 'source-0.2.11.zip')) {
            if (-not (Test-Path -LiteralPath (Join-Path $taskCpuSource $taskFile))) { throw 'CPU module license/source missing. Run scripts/prepare-cpu-module.ps1.' }
        }
        $taskNoticeDir = Join-Path $taskDist 'third-party\PawnIO.Modules'
        New-Item -ItemType Directory -Force -Path $taskNoticeDir | Out-Null
        Copy-Item -LiteralPath (Join-Path $taskCpuSource 'AMDFamily17.p'), (Join-Path $taskCpuSource 'COPYING'), (Join-Path $taskCpuSource 'source-0.2.11.zip') -Destination $taskNoticeDir
        # The embedded RyzenSMU module comes from the same LGPL release.
        if ($taskSmu -and (Test-Path -LiteralPath (Join-Path $taskCpuSource 'RyzenSMU.p'))) { Copy-Item -LiteralPath (Join-Path $taskCpuSource 'RyzenSMU.p') -Destination $taskNoticeDir }
        Copy-Item -LiteralPath (Join-Path $taskRepo 'docs\CPU_SENSORS.md') -Destination $taskNoticeDir
    }

    # Installation package: installer, double-click wrapper, licenses and recovery guide.
    Copy-Item -LiteralPath (Join-Path $taskRepo 'scripts\install-app.ps1'), (Join-Path $taskRepo 'scripts\install-startup.ps1'), (Join-Path $taskRepo 'scripts\install-fan-service.ps1') -Destination $taskDist
    Set-Content -LiteralPath (Join-Path $taskDist 'Install.cmd') -Encoding ASCII -Value @(
        '@echo off',
        'powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0install-app.ps1" %*',
        'if errorlevel 1 pause'
    )
    Copy-Item -LiteralPath (Join-Path $taskRepo 'LICENSE'), (Join-Path $taskRepo 'THIRD_PARTY.md'), (Join-Path $taskRepo 'docs\RECOVERY.md'), (Join-Path $taskRepo 'docs\HARDWARE_MATRIX.md'), (Join-Path $taskRepo 'docs\SYSTEM_CONTROLS.md'), (Join-Path $taskRepo 'docs\FAN_HANDOVER.md'), (Join-Path $taskRepo 'docs\VALIDATION.md') -Destination $taskDist

    Copy-Item -LiteralPath (Join-Path $taskRepo 'docs\POWER_LIFECYCLE.md') -Destination $taskDist

    # License texts of every crate linked into the Windows executable, from the
    # locked graph. Build-only and dev-only dependencies are excluded.
    $taskMetadata = cargo metadata --format-version 1 --locked --filter-platform x86_64-pc-windows-msvc | Out-String | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'cargo metadata failed.' }
    $taskPackages = @{}
    foreach ($taskPackage in $taskMetadata.packages) { $taskPackages[$taskPackage.id] = $taskPackage }
    $taskNodes = @{}
    foreach ($taskNode in $taskMetadata.resolve.nodes) { $taskNodes[$taskNode.id] = $taskNode }
    $taskRoot = ($taskMetadata.packages | Where-Object { $_.name -eq 'omen-hub-plus-plus' }).id
    $taskSeen = @{}
    $taskPending = [Collections.Generic.Stack[string]]::new()
    $taskPending.Push($taskRoot)
    while ($taskPending.Count -gt 0) {
        $taskId = $taskPending.Pop()
        if ($taskSeen.ContainsKey($taskId)) { continue }
        $taskSeen[$taskId] = $true
        foreach ($taskDep in $taskNodes[$taskId].deps) {
            if (@($taskDep.dep_kinds | Where-Object { $null -eq $_.kind }).Count -gt 0) { $taskPending.Push($taskDep.pkg) }
        }
    }
    $taskRustNotices = Join-Path $taskDist 'third-party\rust'
    if (Test-Path -LiteralPath $taskRustNotices) { Remove-Item -LiteralPath $taskRustNotices -Recurse -Force }
    New-Item -ItemType Directory -Force -Path $taskRustNotices | Out-Null
    $taskRows = @('# Rust dependencies', '', 'Crates in the locked graph of the Windows executable, including compile-time procedural macros. License files are copied from each crate source.', '', '| Crate | Version | License | License files |', '| --- | --- | --- | --- |')
    $taskMissing = @()
    foreach ($taskId in ($taskSeen.Keys | Sort-Object { $taskPackages[$_].name }, { $taskPackages[$_].version })) {
        if ($taskMetadata.workspace_members -contains $taskId) { continue }
        $taskPackage = $taskPackages[$taskId]
        $taskDir = Split-Path -Parent $taskPackage.manifest_path
        $taskTarget = Join-Path $taskRustNotices ($taskPackage.name + '-' + $taskPackage.version)
        $taskFiles = @(Get-ChildItem -LiteralPath $taskDir -File | Where-Object { $_.Name -match '^(LICEN[CS]E|COPYING|NOTICE|UNLICENSE|COPYRIGHT)' })
        if ($taskFiles.Count -eq 0) { $taskMissing += $taskPackage.name }
        New-Item -ItemType Directory -Force -Path $taskTarget | Out-Null
        foreach ($taskFile in $taskFiles) { Copy-Item -LiteralPath $taskFile.FullName -Destination $taskTarget }
        $taskRows += "| $($taskPackage.name) | $($taskPackage.version) | $($taskPackage.license) | $(($taskFiles | ForEach-Object Name) -join ', ') |"
    }
    if ($taskMissing.Count -gt 0) {
        $taskRows += ''
        $taskRows += 'No license file is shipped inside these crate sources; their SPDX license applies: ' + ($taskMissing -join ', ') + '.'
    }
    Set-Content -LiteralPath (Join-Path $taskDist 'third-party\RUST-DEPENDENCIES.md') -Encoding UTF8 -Value $taskRows

    $taskHash = (Get-FileHash -LiteralPath $taskOutput -Algorithm SHA256).Hash
    $taskVersion = ($taskMetadata.packages | Where-Object { $_.id -eq $taskRoot }).version
    $taskZip = Join-Path $taskRepo "target\OmenHubPlusPlus-$taskVersion-x64.zip"
    if (Test-Path -LiteralPath $taskZip) { Remove-Item -LiteralPath $taskZip }
    Get-ChildItem -Recurse -LiteralPath $taskDist | Where-Object { $_.LastWriteTime.Year -lt 1980 } | ForEach-Object { $_.LastWriteTime = (Get-Date) }
    Compress-Archive -Path (Join-Path $taskDist '*') -DestinationPath $taskZip
    [pscustomobject]@{
        File = $taskOutput
        Bytes = (Get-Item -LiteralPath $taskOutput).Length
        SHA256 = $taskHash
        Package = $taskZip
        PackageSHA256 = (Get-FileHash -LiteralPath $taskZip -Algorithm SHA256).Hash
    } | Format-List
} finally { Pop-Location }
