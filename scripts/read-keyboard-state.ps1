# A bounded, fixed read-only acquisition for the investigated board/BIOS.
# No raw opcode option, setters, heartbeat, elevation or service control.
#requires -Version 5.1
param()
$ErrorActionPreference = 'Stop'
$taskRepo = Split-Path -Parent $PSScriptRoot
$taskLocal = Join-Path $taskRepo '.local\research\keyboard-state'
New-Item -ItemType Directory -Force -Path $taskLocal | Out-Null
$taskOutputPath = Join-Path $taskLocal ((Get-Date -Format 'yyyyMMdd-HHmmss-fff') + '.json')
$taskRecord = [ordered]@{
    schema = 1
    measured_at = (Get-Date).ToString('o')
    powershell_version = $PSVersionTable.PSVersion.ToString()
    process_elevated = ([Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    read_only = $true
    hardware_write_support = $false
    operation = 'inventory'
    bios_calls_attempted = 0
    board = $null
    bios = $null
    keyboard_type = $null
    keyboard_kind = $null
    capability_raw = $null
    legacy_colors_raw = $null
    legacy_brightness_raw = $null
    colors_rgb = $null
    brightness_raw_byte = $null
    error = $null
}

function Read-OmenKeyboardBlock {
    param(
        [Parameter(Mandatory)][Microsoft.Management.Infrastructure.CimInstance]$BiosInstance,
        [Parameter(Mandatory)][ValidateSet('Capability','LegacyColors','LegacyBrightness')][string]$Read,
        [Parameter(Mandatory)][System.Collections.IDictionary]$Record
    )
    # All three cases are GETs traced in the installed manufacturer modules.
    # Sizes differ intentionally; no generic empty/four-byte substitution.
    switch ($Read) {
        'Capability'       { $taskCommand = 0x20008; $taskType = 0x2B; $taskInputBytes = [byte[]]::new(0); $taskSize = 4 }
        'LegacyColors'     { $taskCommand = 0x20009; $taskType = 2; $taskInputBytes = [byte[]]@(0); $taskSize = 128 }
        'LegacyBrightness' { $taskCommand = 0x20009; $taskType = 4; $taskInputBytes = [byte[]]@(0); $taskSize = 128 }
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
    $taskRecord.board = [string](Get-CimInstance Win32_BaseBoard -OperationTimeoutSec 5 | Select-Object -First 1 -ExpandProperty Product)
    $taskRecord.bios = [string](Get-CimInstance Win32_BIOS -OperationTimeoutSec 5 | Select-Object -First 1 -ExpandProperty SMBIOSBIOSVersion)
    if ($taskRecord.board -cne '8E35' -or $taskRecord.bios -cne 'F.13') {
        throw 'Reader is restricted to board 8E35 / BIOS F.13. No firmware operation was sent.'
    }
    $taskRecord.operation = 'bios_instance'
    $taskInstances = @(Get-CimInstance -Namespace 'root\wmi' -ClassName 'hpqBIntM' -OperationTimeoutSec 5)
    if ($taskInstances.Count -ne 1) { throw "Expected one BIOS interface instance; received $($taskInstances.Count)." }
    $taskCapability = (Read-OmenKeyboardBlock -BiosInstance $taskInstances[0] -Read Capability -Record $taskRecord).Data
    $taskRecord.capability_raw = $taskCapability
    $taskTypeNames = @('Normal','FourZoneWithNumpad','FourZoneWithoutNumpad','RgbPerKey','OneZoneWithNumpad','OneZoneWithoutNumpad','FourZoneMCUWithNumpad','FourZoneMCUWithoutNumpad')
    $taskKeyboardType = [int]$taskCapability[0]
    if ($taskKeyboardType -ge $taskTypeNames.Count) { throw "Unknown keyboard type $taskKeyboardType; no further firmware operation was sent." }
    $taskRecord.keyboard_type = $taskKeyboardType
    $taskRecord.keyboard_kind = $taskTypeNames[$taskKeyboardType]
    # Animated/MCU/per-key routes need separate state/restore contracts.
    if ($taskKeyboardType -in 1,2,4,5) {
        $taskColors = (Read-OmenKeyboardBlock -BiosInstance $taskInstances[0] -Read LegacyColors -Record $taskRecord).Data
        $taskRecord.legacy_colors_raw = $taskColors
        $taskBrightness = (Read-OmenKeyboardBlock -BiosInstance $taskInstances[0] -Read LegacyBrightness -Record $taskRecord).Data
        $taskRecord.legacy_brightness_raw = $taskBrightness
        $taskZoneCount = if ($taskKeyboardType -in 4,5) { 1 } else { 4 }
        $taskRecord.colors_rgb = @(
            for ($taskZone = 0; $taskZone -lt $taskZoneCount; $taskZone++) {
                $taskOffset = 25 + 3 * $taskZone
                [pscustomobject]@{ zone = $taskZone; r = $taskColors[$taskOffset]; g = $taskColors[$taskOffset+1]; b = $taskColors[$taskOffset+2] }
            }
        )
        # Bit 7 affects HP default-mode/hotkey logic; this is not a percentage.
        $taskRecord.brightness_raw_byte = [int]$taskBrightness[0]
    }
    $taskRecord.operation = 'complete'
    $taskRecord | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $taskOutputPath -Encoding utf8
    Write-Output "Keyboard type: $($taskRecord.keyboard_kind). Fixed GETs only; no lighting setting was changed."
    Write-Output "Private read-only record: $taskOutputPath"
} catch {
    $taskRecord.error = [ordered]@{ message = $_.Exception.Message; type = $_.Exception.GetType().FullName }
    if ($_.Exception -is [Microsoft.Management.Infrastructure.CimException]) { $taskRecord.error.native_status = [string]$_.Exception.NativeErrorCode }
    $taskRecord | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $taskOutputPath -Encoding utf8
    Write-Error -Message "Keyboard acquisition failed: $($_.Exception.Message). Record: $taskOutputPath" -ErrorAction Continue
    exit 1
}
