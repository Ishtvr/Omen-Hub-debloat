# Fixed GPU-clock qualification, with independent full-control comparisons.
#requires -Version 5.1
#requires -RunAsAdministrator
param([Parameter(Mandatory=$true)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$ExpectedBinarySha256)
$ErrorActionPreference='Stop'
$taskRepo=Split-Path -Parent $PSScriptRoot
$taskExe=Join-Path $taskRepo 'target\release\omen-hub-pp.exe'
$taskOutput=Join-Path $taskRepo '.local\research\nvidia-61742'
New-Item -ItemType Directory -Force -Path $taskOutput|Out-Null
$taskRecord=[ordered]@{started_at=(Get-Date).ToString('o');binary_sha256=$ExpectedBinarySha256;success=$false;steps=@();error=$null;power_or_voltage_fields_written=$false}
function Invoke-GpuProbe {param([string]$Command)
 $taskText=& $taskExe $Command 2>&1|Out-String
 $taskCode=$LASTEXITCODE
 if($taskCode-ne0){throw ($Command+': '+$taskText)}
 return ($taskText|ConvertFrom-Json)
}
function Get-CurveControls {param($Curve)
 return (@($Curve.nodes|ForEach-Object {@($_.index,$_.voltage_uv,$_.delta_khz)})|ConvertTo-Json -Compress -Depth 4)
}
function Get-GpuControls {param($Gpu)
 return (@($Gpu.identity,$Gpu.core,$Gpu.memory)|ConvertTo-Json -Compress -Depth 8)
}
function Get-MetadataHashes {param([string]$Store)
 return (@('control-journal.json','startup.json','fan-mode.json','app-settings.json')|ForEach-Object{
  $taskFile=Join-Path $Store $_
  if(Test-Path -LiteralPath $taskFile){(Get-FileHash -LiteralPath $taskFile).Hash}else{'absent'}
 }) -join ':'
}
try{
 if((Get-FileHash -LiteralPath $taskExe).Hash-cne$ExpectedBinarySha256.ToUpperInvariant()){throw 'Qualification binary changed.'}
 if(Get-Process -Name MSIAfterburner -ErrorAction SilentlyContinue){throw 'MSI Afterburner is running; no clock trial started.'}
 $taskStorage=(Invoke-GpuProbe 'profile-storage').directory
 $taskMetaBefore=Get-MetadataHashes $taskStorage
 $taskBefore=(Invoke-GpuProbe 'gpu-capabilities').gpu
 $taskBeforeCurve=Invoke-GpuProbe 'gpu-curve-read'
 if($taskBefore.identity.driver-cne'617.42'-or$taskBefore.identity.vbios-cne'98.06.2a.40.da'-or$taskBeforeCurve.nodes.Count-ne127){throw 'Unexpected driver/VBIOS/curve domain.'}
 if($null-eq$taskBefore.core.Ok-or$null-eq$taskBefore.memory.Ok-or$taskBefore.core.Ok.current_mhz-ne0-or$taskBefore.memory.Ok.current_mhz-ne0){throw 'Existing nonzero global offsets are not reset by the fixed trial.'}
 $taskRecord.before_gpu=$taskBefore;$taskRecord.before_curve=$taskBeforeCurve
 foreach($taskCommand in @('gpu-validate-curve','gpu-validate-memory-curve','gpu-validate')){
  if(Get-Process -Name MSIAfterburner -ErrorAction SilentlyContinue){throw 'Foreign clock controller restarted; remaining probes cancelled.'}
  $taskResult=Invoke-GpuProbe $taskCommand
  $taskAfter=(Invoke-GpuProbe 'gpu-capabilities').gpu
  $taskAfterCurve=Invoke-GpuProbe 'gpu-curve-read'
  if((Get-GpuControls $taskBefore)-cne(Get-GpuControls $taskAfter)-or(Get-CurveControls $taskBeforeCurve)-cne(Get-CurveControls $taskAfterCurve)){
   throw 'Independent readback did not match every original control; do not continue.'
  }
  if($taskMetaBefore-cne(Get-MetadataHashes $taskStorage)){throw 'Production recovery/startup/UI metadata changed.'}
  $taskRecord.steps += [pscustomobject]@{command=$taskCommand;result=$taskResult;independent_full_restore=$true;metadata_unchanged=$true}
 }
 $taskRecord.success=$true
}catch{$taskRecord.error=$_.Exception.Message}
finally{
 $taskRecord.finished_at=(Get-Date).ToString('o')
 $taskRecord|ConvertTo-Json -Depth 22|Set-Content -LiteralPath (Join-Path $taskOutput ('trials-'+$ExpectedBinarySha256.Substring(0,16)+'.json')) -Encoding UTF8
}
if($taskRecord.error){throw $taskRecord.error}
'Fixed curve, VRAM preservation and +/-15/+/-24 MHz trials restored all original controls and metadata.'
