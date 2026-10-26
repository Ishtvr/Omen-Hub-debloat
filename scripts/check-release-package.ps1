# Reads a built ZIP; no installation, executable launch or hardware access.
#requires -Version 5.1
param(
    [Parameter(Mandatory=$true)][string]$Package,
    [Parameter(Mandatory=$true)][string]$Version,
    [ValidatePattern('^$|^[a-fA-F0-9]{40}$')][string]$ExpectedCommit='',
    [switch]$RequireCleanSource
)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem
$releaseZipPath=(Resolve-Path -LiteralPath $Package).Path
$releaseArchive=[IO.Compression.ZipFile]::OpenRead($releaseZipPath)
$releaseTemp=Join-Path ([IO.Path]::GetTempPath()) ('OmenReleaseCheck-'+[Guid]::NewGuid().ToString('N'))
$releaseExe=Join-Path $releaseTemp 'Omen Hub++.exe'
$releaseRequired=@('Omen Hub++.exe','Install.cmd','install-app.ps1','install-startup.ps1','install-fan-service.ps1','LICENSE','THIRD_PARTY.md','RECOVERY.md','HARDWARE_MATRIX.md','SYSTEM_CONTROLS.md','FAN_HANDOVER.md','VALIDATION.md','POWER_LIFECYCLE.md','HP_CONTROL.md','RELEASE_NOTES.md','build-info.json','third-party/RUST-DEPENDENCIES.md')
try {
    $releaseNames=@($releaseArchive.Entries | Where-Object {$_.Name} | ForEach-Object {$_.FullName.Replace('\','/')})
    if(@($releaseNames|Select-Object -Unique).Count-ne$releaseNames.Count){throw 'Duplicate package entries.'}
    foreach($releaseName in $releaseNames){
        if($releaseName.StartsWith('/')-or$releaseName.Contains(':')-or$releaseName.Split('/')-contains'..'){throw 'Unsafe archive entry path.'}
        if($releaseName-cin$releaseRequired){continue}
        if($releaseName-match'^third-party/rust/[^/]+/(LICEN[CS]E[^/]*|COPYING[^/]*|NOTICE[^/]*|UNLICENSE[^/]*|COPYRIGHT[^/]*)$'){continue}
        if($releaseName-cmatch'^third-party/PawnIO\.Modules/(AMDFamily17\.p|RyzenSMU\.p|COPYING|source-0\.2\.11\.zip|CPU_SENSORS\.md)$'){continue}
        throw "Unexpected release payload: $releaseName"
    }
    foreach($releaseName in $releaseRequired){if($releaseName-cnotin$releaseNames){throw "Required release file absent: $releaseName"}}
    if(@($releaseNames|Where-Object {$_-like'third-party/rust/*'}).Count-eq0){throw 'Rust license texts are absent.'}
    $releaseInfoEntry=$releaseArchive.GetEntry('build-info.json')
    if($releaseInfoEntry.Length-gt16384){throw 'Build information exceeds its bound.'}
    $releaseReader=[IO.StreamReader]::new($releaseInfoEntry.Open())
    try{$releaseBuild=$releaseReader.ReadToEnd()|ConvertFrom-Json}finally{$releaseReader.Dispose()}
    if($releaseBuild.schema-ne1-or$releaseBuild.product-cne'Omen Hub++'-or$releaseBuild.version-cne$Version-or$releaseBuild.target-cne'x86_64-pc-windows-msvc'){throw 'Build identity differs.'}
    if($ExpectedCommit-and$releaseBuild.source_commit-cne$ExpectedCommit){throw 'Package source commit differs.'}
    if($RequireCleanSource-and$releaseBuild.source_dirty-cne$false){throw 'Public release requires a clean source checkout.'}
    if($releaseBuild.cpu_sensor_module-or$releaseBuild.cpu_smu_module){
        foreach($releaseName in @('third-party/PawnIO.Modules/AMDFamily17.p','third-party/PawnIO.Modules/COPYING','third-party/PawnIO.Modules/source-0.2.11.zip','third-party/PawnIO.Modules/CPU_SENSORS.md')){
            if($releaseName-cnotin$releaseNames){throw 'Embedded module license/corresponding source is missing.'}
        }
    }
    $releaseEntry=$releaseArchive.GetEntry('Omen Hub++.exe')
    if($releaseEntry.Length-eq0-or$releaseEntry.Length-gt32MB){throw 'Unexpected application size.'}
    New-Item -ItemType Directory -Path $releaseTemp | Out-Null
    $releaseInput=$releaseEntry.Open();$releaseOutput=[IO.File]::Create($releaseExe)
    try{$releaseInput.CopyTo($releaseOutput)}finally{$releaseInput.Dispose();$releaseOutput.Dispose()}
    $releaseHash=(Get-FileHash -LiteralPath $releaseExe).Hash
    if($releaseHash-cne$releaseBuild.binary_sha256){throw 'Packaged application hash differs from its manifest.'}
    $releaseInfo=[Diagnostics.FileVersionInfo]::GetVersionInfo($releaseExe)
    if($releaseInfo.ProductName-cne'Omen Hub++'-or$releaseInfo.ProductVersion-cne$Version-or$releaseInfo.FileVersion-cne$Version-or$releaseInfo.IsPreRelease-ne($Version-match'-')){throw 'Packaged Windows version/prerelease metadata differs.'}
    [pscustomobject]@{
        passed=$true;version=$Version;files=$releaseNames.Count;bytes=$releaseEntry.Length
        binary_sha256=$releaseHash;package_sha256=(Get-FileHash -LiteralPath $releaseZipPath).Hash
        source_commit=$releaseBuild.source_commit;source_dirty=$releaseBuild.source_dirty
        cpu_sensor_module=$releaseBuild.cpu_sensor_module;cpu_smu_module=$releaseBuild.cpu_smu_module
        installed=$false;executed=$false
    }|ConvertTo-Json
} finally {
    $releaseArchive.Dispose()
    # Only the one file and directory created here; no recursive removal.
    if(Test-Path -LiteralPath $releaseExe){Remove-Item -LiteralPath $releaseExe}
    if(Test-Path -LiteralPath $releaseTemp){Remove-Item -LiteralPath $releaseTemp}
}
