# Fixed read-only HP fan acquisition, exact board/BIOS only. No watchdog,
# performance mode, fan SET, service or driver lifecycle operation.
#requires -Version 5.1
param()
$ErrorActionPreference='Stop'
$taskRepo=Split-Path -Parent $PSScriptRoot
$taskRoot=Join-Path $taskRepo '.local/research/fan-control'
New-Item -ItemType Directory -Force -Path $taskRoot|Out-Null
$taskRecord=[ordered]@{schema=1;measured_at=(Get-Date).ToString('o');process_elevated=([Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator);read_only=$true;hardware_write_support=$false;operation='inventory';bios_calls_attempted=0;board=$null;bios=$null;blocks=[ordered]@{};error=$null}
$taskOutput=Join-Path $taskRoot 'read-only-state.json'
function Read-OmenFanBlock {
    param(
        [Parameter(Mandatory)][Microsoft.Management.Infrastructure.CimInstance]$BiosInstance,
        [Parameter(Mandatory)][ValidateSet('MaxFan','Levels','Table','Types','SystemDesign','Surface')][string]$Read,
        [Parameter(Mandatory)][System.Collections.IDictionary]$Record
    )
    # All six cases are GETs traced in the installed manufacturer modules.
    # Sizes differ intentionally; no generic empty/four-byte substitution.
    $taskCommand=0x20008
    $taskInputBytes=[byte[]]@(0,0,0,0)
    switch($Read){
      'MaxFan' {$taskType=0x26;$taskSize=4}
      'Levels' {$taskType=0x2D;$taskSize=128}
      'Table' {$taskType=0x2F;$taskSize=128}
      'Types' {$taskType=0x2C;$taskSize=128}
      'SystemDesign' {$taskType=0x28;$taskSize=128}
      'Surface' {$taskType=0x23;$taskSize=4}
    }
    $Record.operation = "prepare_input:$Read"
    $taskInput = New-CimInstance -Namespace 'root\wmi' -ClassName 'hpqBDataIn' -ClientOnly -Property @{
        Sign = [byte[]]@(0x53,0x45,0x43,0x55)
        Command = [uint32]$taskCommand
        CommandType = [uint32]$taskType
        Size = [uint32]$taskInputBytes.Length
        hpqBData = $taskInputBytes
    }
    $Record.operation = "invoke:$Read"
    $Record.bios_calls_attempted++
    $taskReply = Invoke-CimMethod -InputObject $BiosInstance -MethodName "hpqBIOSInt$taskSize" -Arguments @{ InData = $taskInput } -OperationTimeoutSec 5
    $taskBlock = $taskReply.OutData
    if ($taskBlock -isnot [Microsoft.Management.Infrastructure.CimInstance]) { throw "$Read returned no embedded OutData instance." }
    $taskCode = $taskBlock.CimInstanceProperties['rwReturnCode']
    $taskData = $taskBlock.CimInstanceProperties['Data']
    if ($null -eq $taskCode -or $null -eq $taskCode.Value) { throw "$Read returned no firmware status." }
    if ($taskCode.CimType -ne [Microsoft.Management.Infrastructure.CimType]::UInt32) { throw "$Read returned an unexpected firmware-status type." }
    if ([uint32]$taskCode.Value -ne 0) { throw "$Read firmware status was $($taskCode.Value); no data interpreted." }
    if ($null -eq $taskData -or $null -eq $taskData.Value) { throw "$Read returned no byte array." }
    if ($taskData.CimType -ne [Microsoft.Management.Infrastructure.CimType]::UInt8Array) { throw "$Read returned an unexpected data type." }
    $taskBytes = [byte[]]$taskData.Value
    if ($taskBytes.Length -ne $taskSize) { throw "$Read returned $($taskBytes.Length) bytes; expected exactly $taskSize." }
    return [pscustomobject]@{ Data = $taskBytes }
}

try {
 $taskRecord.board=[string](Get-CimInstance Win32_BaseBoard -OperationTimeoutSec 5).Product
 $taskRecord.bios=[string](Get-CimInstance Win32_BIOS -OperationTimeoutSec 5).SMBIOSBIOSVersion
 if($taskRecord.board-cne'8E35'-or$taskRecord.bios-cne'F.13'){throw 'Reader restricted to 8E35/F.13; no BIOS operation sent.'}
 $taskRecord.operation='bios_instance'
 $taskInstances=@(Get-CimInstance -Namespace 'root/wmi' -ClassName 'hpqBIntM' -OperationTimeoutSec 5|Where-Object Active)
 if($taskInstances.Count-ne1){throw 'Expected one active BIOS interface.'}
 foreach($taskRead in @('SystemDesign','MaxFan','Levels','Types','Table','Surface')){
  try {$taskBlock=(Read-OmenFanBlock -BiosInstance $taskInstances[0] -Read $taskRead -Record $taskRecord).Data;$taskRecord.blocks[$taskRead]=[ordered]@{raw=@($taskBlock);error=$null}}
  catch {$taskRecord.blocks[$taskRead]=[ordered]@{raw=$null;error=$_.Exception.Message}}
 }
 if(@($taskRecord.blocks.Values|Where-Object {$_.error}).Count-gt0){throw 'One or more fixed fan GETs failed; inspect per-block errors.'}
 $taskRecord.operation='complete'
}catch {$taskRecord.error=$_.Exception.Message}
$taskRecord|ConvertTo-Json -Depth 8|Set-Content -LiteralPath $taskOutput -Encoding UTF8
if($taskRecord.error){Write-Error $taskRecord.error;exit 1}
Write-Output $taskOutput
