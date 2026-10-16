# Fixed, read-only qualification. Run in an administrator PowerShell.
# No SET, heartbeat, service control or driver installation is dispatched.
#requires -Version 5.1
param()
$ErrorActionPreference='Stop'
# UI labels below are German; pin the app language so the checks stay stable.
$env:OMEN_HUB_LANG = 'de'
$taskRepo=Split-Path -Parent $PSScriptRoot
$taskExe=Join-Path $taskRepo 'dist\Omen Hub++.exe'
$taskOutput=Join-Path $taskRepo '.local\research\fan-control\cooling-read-check.json'
$taskPrincipal=[Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())
if(-not $taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Read qualification requires an administrator PowerShell.'}
function Read-CoolingJson {
    param([ValidateSet('cooling-read','fan-read','gpu-curve-read','hp-read')][string]$Command)
    $taskRaw=& $taskExe $Command | Out-String
    $taskCode=$LASTEXITCODE
    if([string]::IsNullOrWhiteSpace($taskRaw)){throw "No JSON from $Command (exit $taskCode)."}
    [pscustomobject]@{code=$taskCode;value=($taskRaw|ConvertFrom-Json)}
}
function Read-ControlDigest {
    $taskCurve=Read-CoolingJson 'gpu-curve-read'
    if($taskCurve.code-ne0){throw 'GPU control comparison could not be read.'}
    @($taskCurve.value.nodes|ForEach-Object{@($_.index,$_.voltage_uv,$_.delta_khz)})|ConvertTo-Json -Depth 5 -Compress
}
function Read-MetadataHashes {
    $taskData=Join-Path ([Environment]::GetFolderPath('UserProfile')) '.omen-hub-pp'
    foreach($taskName in @('control-journal.json','startup.json')){
        $taskPath=Join-Path $taskData $taskName
        if(Test-Path -LiteralPath $taskPath){(Get-FileHash -LiteralPath $taskPath -Algorithm SHA256).Hash}else{'absent'}
    }
}
$taskMetadataBefore=@(Read-MetadataHashes)
$taskControlsBefore=Read-ControlDigest
$taskPowerBefore=Read-CoolingJson 'hp-read'
$taskFanBefore=Read-CoolingJson 'fan-read'
$taskSamples=@()
for($taskIndex=0;$taskIndex-lt3;$taskIndex++){
    $taskSamples+=Read-CoolingJson 'cooling-read'
    if($taskIndex-lt2){Start-Sleep -Milliseconds 1000}
}
$taskControlsAfter=Read-ControlDigest
$taskPowerAfter=Read-CoolingJson 'hp-read'
$taskFanAfter=Read-CoolingJson 'fan-read'
$taskMetadataAfter=@(Read-MetadataHashes)
$taskRecord=[ordered]@{
    measured_at=(Get-Date).ToString('o');binary_sha256=(Get-FileHash -LiteralPath $taskExe -Algorithm SHA256).Hash
    binary_bytes=(Get-Item -LiteralPath $taskExe).Length;samples=$taskSamples
    gpu_controls_unchanged=($taskControlsBefore-ceq$taskControlsAfter)
    hp_power_status_unchanged=($taskPowerBefore.code-eq0-and$taskPowerAfter.code-eq0-and(($taskPowerBefore.value|ConvertTo-Json -Compress)-ceq($taskPowerAfter.value|ConvertTo-Json -Compress)))
    max_fan_unchanged=($taskFanBefore.code-eq0-and$taskFanAfter.code-eq0-and(($taskFanBefore.value.max_fan|ConvertTo-Json -Compress)-ceq($taskFanAfter.value.max_fan|ConvertTo-Json -Compress)))
    metadata_unchanged=(($taskMetadataBefore-join',')-ceq($taskMetadataAfter-join','))
    hardware_writes=$false;service_control=$false;driver_installation=$false
}
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $taskOutput)|Out-Null
$taskRecord|ConvertTo-Json -Depth 20|Set-Content -LiteralPath $taskOutput -Encoding UTF8
$taskGoodCount=0
$taskStaleCount=0
foreach($taskSample in $taskSamples){
    # Validate both usable batches and a truthful rejection of a slow batch.
    # Driver initialization and OS scheduling can exceed the fixed age bound.
    $taskValue=$taskSample.value
    if($taskValue.maximum_sample_age_ms-ne2000){throw 'Unexpected freshness policy.'}
    $taskExpectedFresh=$true
    foreach($taskRead in @($taskValue.fans,$taskValue.cpu,$taskValue.gpu)){
        if($null-eq$taskRead.result.Ok){throw 'Required reader failed; qualification incomplete.'}
        $taskWindow=$taskRead.window
        if($taskWindow.started_ms-gt$taskWindow.completed_ms-or$taskWindow.completed_ms-gt$taskValue.elapsed_ms){throw 'Invalid monotonic sample window.'}
        if(($taskValue.elapsed_ms-$taskWindow.started_ms)-gt2000){$taskExpectedFresh=$false}
    }
    if($null-eq$taskValue.fans.result.Ok.ir_temperature.Ok){throw 'IR decoder rejected the sample.'}
    if($null-eq$taskValue.power_before.ac_connected-or$null-eq$taskValue.power_after.ac_connected-or$taskValue.power_before.ac_connected-ne$taskValue.power_after.ac_connected){throw 'Power changed/unavailable during qualification.'}
    $taskExpectedCode=if($taskExpectedFresh){0}else{1}
    if($taskSample.code-ne$taskExpectedCode-or$taskValue.temperature_batch_valid-ne$taskExpectedFresh){throw 'CLI result disagrees with independently checked sample age.'}
    if($taskExpectedFresh){
        if($taskValue.sensor_faults.Count-ne0){throw 'Usable batch retained sensor faults.'}
        $taskGoodCount++
    }else{
        if($taskValue.sensor_faults.Count-eq0-or@($taskValue.sensor_faults|Where-Object {$_-notmatch'acquisition is older than the diagnostic freshness bound$'}).Count-ne0){throw 'Slow batch lacks its specific freshness error.'}
        $taskStaleCount++
    }
    if($taskSample.value.hardware_applied-or$taskSample.value.software_curve_write_supported-or$null-ne$taskSample.value.firmware_performance_mode-or$null-ne$taskSample.value.firmware_fan_owner){throw 'Read diagnostic inferred an unsupported control state.'}
    if($null-ne$taskSample.value.cpu.result.Ok.package_power_w){throw 'One-shot CPU baseline must not invent package watts.'}
}
if($taskGoodCount-eq0){throw 'No fresh batch acquired; qualification incomplete.'}
if(-not$taskRecord.gpu_controls_unchanged-or-not$taskRecord.hp_power_status_unchanged-or-not$taskRecord.max_fan_unchanged-or-not$taskRecord.metadata_unchanged){throw 'Read-only invariants changed during the qualification; inspect private record.'}
"Cooling qualification passed: $taskGoodCount usable batches, $taskStaleCount slow batches correctly rejected; checked controls and metadata unchanged."
