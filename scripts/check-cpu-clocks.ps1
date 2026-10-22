# Fixed read-only hardware validation; does not install/start a driver or tune CPU/GPU.
#requires -Version 5.1
#requires -RunAsAdministrator
param()
$ErrorActionPreference='Stop'
$taskRepo=Split-Path -Parent $PSScriptRoot
$taskExe=Join-Path $taskRepo 'dist\Omen Hub++.exe'
$taskOutput=Join-Path $taskRepo '.local\research\cpu-clocks'
New-Item -ItemType Directory -Force -Path $taskOutput|Out-Null
$taskRecord=[ordered]@{
 started_at=(Get-Date).ToString('o')
 binary_sha256=(Get-FileHash -LiteralPath $taskExe).Hash
 success=$false;error=$null;hardware_tuning_writes=$false
}
function Get-PublicGpuControls {
 $taskGpu=& $taskExe gpu-capabilities|Out-String
 if($LASTEXITCODE-ne0){throw 'Public GPU GET failed.'}
 $taskGpu=$taskGpu|ConvertFrom-Json
 if($null-eq$taskGpu.gpu.core.Ok-or$null-eq$taskGpu.gpu.memory.Ok){throw 'Public GPU offsets are unavailable; no comparison is possible.'}
 return (@($taskGpu.gpu.identity,$taskGpu.gpu.core,$taskGpu.gpu.memory)|ConvertTo-Json -Depth 8 -Compress)
}
function Get-ControlMetadata {
 $taskStore=Join-Path ([Environment]::GetFolderPath('UserProfile')) '.omen-hub-pp'
 return (@('control-journal.json','startup.json','fan-mode.json')|ForEach-Object{
  $taskPath=Join-Path $taskStore $_
  if(Test-Path -LiteralPath $taskPath){(Get-FileHash -LiteralPath $taskPath).Hash}else{'absent'}
 }) -join ':'
}
try{
 $taskGpuBefore=Get-PublicGpuControls
 $taskMetaBefore=Get-ControlMetadata
 $taskDriversBefore=Get-CimInstance Win32_SystemDriver -Filter "Name='PawnIO' OR Name='AMDRyzenMasterDriverV31'"|Select-Object Name,State,PathName
 # Optional independent HWCR comparison uses only the exact previously reviewed
 # fixed 32-slot GET probe, and only if its already-running driver is unchanged.
 $taskHwcrExe=Join-Path $taskRepo '.local\research\cpu-sdk\probe-hwcr.exe'
 $taskHwcrDriver='C:\Program Files\AMD\RyzenMasterSDK\bin\AMDRyzenMasterDriver.sys'
 $taskHwcrAvailable=(Test-Path -LiteralPath $taskHwcrExe)-and(Test-Path -LiteralPath $taskHwcrDriver)-and
  @($taskDriversBefore|Where-Object {$_.Name-ceq'AMDRyzenMasterDriverV31'-and$_.State-ceq'Running'}).Count-eq1
 if($taskHwcrAvailable){
  $taskHwcrAvailable=(Get-FileHash -LiteralPath $taskHwcrExe).Hash-ceq'67023DBA0AB02D0B613351A77F8F47D96C39A53D2CAF671E6670B8F2A50AD9DE'-and
   (Get-FileHash -LiteralPath $taskHwcrDriver).Hash-ceq'7250DDCB9B04E7E00694C9D5AE3AA2B37CAAE367A4125BB84D8AC0B0DFAB8624'
 }
 if($taskHwcrAvailable){
  $taskHwcrBefore=& $taskHwcrExe|Out-String
  if($LASTEXITCODE-ne0){throw 'Independent HWCR baseline GET failed.'}
 }
 $taskText=& $taskExe cpu-hardware-read|Out-String
 if($LASTEXITCODE-ne0){throw ('Fixed CPU GET failed: '+$taskText)}
 $taskText|Set-Content -LiteralPath (Join-Path $taskOutput 'samples.json') -Encoding UTF8
 $taskSamples=$taskText|ConvertFrom-Json
 if($taskSamples.board-cne'8E35'-or$taskSamples.bios-cne'F.13'){throw 'Unexpected device binding.'}
 if($null-ne$taskSamples.baseline.effective_clocks-or$null-ne$taskSamples.baseline.package_power_w){throw 'First baseline fabricated a clock/power value.'}
 $taskClocks=$taskSamples.measured.effective_clocks
 if($taskClocks.source-cne'amd_aperf_read_only'-or$taskClocks.logical_processor_count-ne32-or$taskClocks.processors.Count-ne32){throw 'Incomplete CPU clock coverage/provenance.'}
 for($taskIndex=0;$taskIndex-lt32;$taskIndex++){
  $taskClock=$taskClocks.processors[$taskIndex]
  if($taskClock.logical_processor-ne$taskIndex-or$null-eq$taskClock.effective_mhz-or[Double]::IsNaN($taskClock.effective_mhz)-or$taskClock.effective_mhz-lt0-or$taskClock.effective_mhz-gt8000-or$taskClock.interval_ms-lt200-or$taskClock.interval_ms-gt10000){throw 'Invalid per-processor clock sample.'}
 }
 if($taskSamples.measured.warnings.Count-ne0-or$null-eq$taskSamples.measured.package_power_w-or$null-eq$taskSamples.measured.temperature_c){throw 'Clock extension degraded package watts/Tctl or reported a warning.'}
 if($taskHwcrAvailable){
  $taskHwcrAfter=& $taskHwcrExe|Out-String
  if($LASTEXITCODE-ne0-or$taskHwcrBefore-cne$taskHwcrAfter){throw 'Independent HWCR readback changed.'}
  $taskHwcrAfter|Set-Content -LiteralPath (Join-Path $taskOutput 'hwcr-after.jsonl') -Encoding UTF8
 }
 $taskDriversAfter=Get-CimInstance Win32_SystemDriver -Filter "Name='PawnIO' OR Name='AMDRyzenMasterDriverV31'"|Select-Object Name,State,PathName
 if(($taskDriversBefore|ConvertTo-Json -Compress)-cne($taskDriversAfter|ConvertTo-Json -Compress)){throw 'Driver status/identity changed.'}
 if($taskGpuBefore-cne(Get-PublicGpuControls)-or$taskMetaBefore-cne(Get-ControlMetadata)){throw 'Public GPU controls or profile/startup metadata changed.'}
 $taskRecord.board=$taskSamples.board;$taskRecord.bios=$taskSamples.bios
 $taskRecord.driver_sha256=$taskSamples.driver_sha256;$taskRecord.module_sha256=$taskSamples.module_sha256
 $taskRecord.logical_processors=32;$taskRecord.first_sample_without_clocks_or_watts=$true
 $taskRecord.max_effective_mhz=($taskClocks.processors.effective_mhz|Measure-Object -Maximum).Maximum
 $taskRecord.min_interval_ms=($taskClocks.processors.interval_ms|Measure-Object -Minimum).Minimum
 $taskRecord.max_interval_ms=($taskClocks.processors.interval_ms|Measure-Object -Maximum).Maximum
 $taskRecord.package_power_w=$taskSamples.measured.package_power_w;$taskRecord.tctl_c=$taskSamples.measured.temperature_c
 $taskRecord.hwcr_independently_checked=$taskHwcrAvailable
 $taskRecord.hwcr_unchanged=if($taskHwcrAvailable){$true}else{$null}
 $taskRecord.public_gpu_offsets_unchanged=$true;$taskRecord.control_metadata_unchanged=$true
 $taskRecord.driver_status_unchanged=$true;$taskRecord.actual_suspend_or_load_benchmark=$false
 $taskRecord.success=$true
}catch{$taskRecord.error=$_.Exception.Message}
finally{
 $taskRecord.finished_at=(Get-Date).ToString('o')
 $taskRecord|ConvertTo-Json -Depth 6|Set-Content -LiteralPath (Join-Path $taskOutput 'validation.json') -Encoding UTF8
}
if(-not$taskRecord.success){throw $taskRecord.error}
$taskRecord|ConvertTo-Json -Depth 6
