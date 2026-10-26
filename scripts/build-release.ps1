param()
$ErrorActionPreference = 'Stop'
$taskRepo = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $taskRepo
try {
    $taskMetadata = cargo metadata --format-version 1 --locked --filter-platform x86_64-pc-windows-msvc | Out-String | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'cargo metadata failed.' }
    $taskRootPackage = $taskMetadata.packages | Where-Object { $_.name -eq 'omen-hub-plus-plus' }
    $taskVersion = $taskRootPackage.version
    if ($taskVersion -notmatch '^[0-9]+\.[0-9]+\.[0-9]+(?:[-+][0-9A-Za-z.-]+)?$') { throw 'Invalid package version.' }
    # A fresh, version-specific staging directory excludes old local binaries,
    # backups and unrelated files already present in dist/ from public releases.
    $taskDist = [IO.Path]::GetFullPath((Join-Path $taskRepo ('target\package-staging\' + $taskVersion)))
    $taskStageRoot = [IO.Path]::GetFullPath((Join-Path $taskRepo 'target\package-staging'))
    if ([IO.Path]::GetDirectoryName($taskDist) -cne $taskStageRoot) { throw 'Invalid release staging target.' }
    $taskCheck = $taskDist
    while ($taskCheck.Length -ge $taskRepo.Length) {
        if ((Test-Path -LiteralPath $taskCheck) -and ((Get-Item -LiteralPath $taskCheck -Force).Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Release staging ancestor is redirected.' }
        if ($taskCheck -eq $taskRepo) { break }
        $taskCheck = [IO.Path]::GetDirectoryName($taskCheck)
    }
    if (Test-Path -LiteralPath $taskDist) {
        $taskPendingDirectories = [Collections.Generic.Stack[string]]::new()
        $taskPendingDirectories.Push($taskDist)
        while ($taskPendingDirectories.Count) {
            foreach ($taskEntry in Get-ChildItem -LiteralPath $taskPendingDirectories.Pop() -Force) {
                if ($taskEntry.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Release staging contains a redirected entry.' }
                if ($taskEntry.PSIsContainer) { $taskPendingDirectories.Push($taskEntry.FullName) }
            }
        }
        Remove-Item -LiteralPath $taskDist -Recurse -Force
    }
    cargo build --release --locked -p omen-hub-plus-plus
    if ($LASTEXITCODE -ne 0) { throw 'Release build failed.' }
    New-Item -ItemType Directory -Force -Path $taskDist | Out-Null
    $taskOutput = Join-Path $taskDist 'Omen Hub++.exe'
    Copy-Item -LiteralPath (Join-Path $taskRepo 'target\release\omen-hub-pp.exe') -Destination $taskOutput
    # build.rs embeds icon and version information together; both must be present.
    $taskInfo = (Get-Item -LiteralPath $taskOutput).VersionInfo
    if ($taskInfo.ProductName -ne 'Omen Hub++' -or $taskInfo.ProductVersion -cne $taskVersion -or $taskInfo.IsPreRelease -ne ($taskVersion -match '-')) { throw 'Embedded product/version/prerelease resources differ.' }
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
    Copy-Item -LiteralPath (Join-Path $taskRepo 'docs\HP_CONTROL.md') -Destination $taskDist
    $taskReleaseNotes = Join-Path $taskRepo ('docs\releases\' + $taskVersion + '.md')
    if (Test-Path -LiteralPath $taskReleaseNotes) { Copy-Item -LiteralPath $taskReleaseNotes -Destination (Join-Path $taskDist 'RELEASE_NOTES.md') }

    # License texts of every crate linked into the Windows executable, from the
    # locked graph. Build-only and dev-only dependencies are excluded.
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
    $taskCommit = $null
    $taskDirty = $null
    if (Test-Path -LiteralPath (Join-Path $taskRepo '.git')) {
        $taskCommit = (git rev-parse HEAD | Out-String).Trim()
        if ($LASTEXITCODE -ne 0) { throw 'Cannot identify release source commit.' }
        $taskDirty = @(git status --porcelain).Count -ne 0
        if ($LASTEXITCODE -ne 0) { throw 'Cannot inspect release source state.' }
    }
    [ordered]@{
        schema = 1
        product = 'Omen Hub++'
        version = $taskVersion
        target = 'x86_64-pc-windows-msvc'
        source_commit = $taskCommit
        source_dirty = $taskDirty
        binary_sha256 = $taskHash
        cpu_sensor_module = Test-Path -LiteralPath (Join-Path $taskModules 'AMDFamily17.bin')
        cpu_smu_module = $taskSmu
    } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskDist 'build-info.json') -Encoding UTF8
    $taskZip = Join-Path $taskRepo "target\OmenHubPlusPlus-$taskVersion-x64.zip"
    if (Test-Path -LiteralPath $taskZip) { Remove-Item -LiteralPath $taskZip }
    Get-ChildItem -Recurse -LiteralPath $taskDist | Where-Object { $_.LastWriteTime.Year -lt 1980 } | ForEach-Object { $_.LastWriteTime = (Get-Date) }
    Compress-Archive -Path (Join-Path $taskDist '*') -DestinationPath $taskZip
    # Keep the familiar local output current without including unrelated dist
    # files in the archive. Only the clean staging directory is packaged.
    $taskLocalDist = Join-Path $taskRepo 'dist'
    New-Item -ItemType Directory -Force -Path $taskLocalDist | Out-Null
    Get-ChildItem -LiteralPath $taskDist -Force | Copy-Item -Destination $taskLocalDist -Recurse -Force
    $taskChecksum = Join-Path $taskRepo "target\OmenHubPlusPlus-$taskVersion-SHA256SUMS.txt"
    $taskZipHash = (Get-FileHash -LiteralPath $taskZip -Algorithm SHA256).Hash
    [IO.File]::WriteAllText($taskChecksum,($taskZipHash.ToLowerInvariant() + '  ' + [IO.Path]::GetFileName($taskZip) + "`n"),[Text.UTF8Encoding]::new($false))
    [pscustomobject]@{
        File = Join-Path $taskLocalDist 'Omen Hub++.exe'
        Bytes = (Get-Item -LiteralPath $taskOutput).Length
        SHA256 = $taskHash
        Package = $taskZip
        PackageSHA256 = $taskZipHash
        Checksums = $taskChecksum
    } | Format-List
} finally { Pop-Location }
