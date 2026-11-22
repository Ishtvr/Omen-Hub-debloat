<#
.SYNOPSIS
Read-only battery measurement for Omen Hub++ (German or English Windows).

.DESCRIPTION
Samples the power source, the battery discharge rate, the dGPU power state
from the device tree (never a driver query, so the dGPU is not woken), the
active power plan and the running Omen processes. It changes nothing: no
profile, service, driver or power setting is written.

Run one phase per state and compare the mean discharge rate, for example:

  .\scripts\measure-battery.ps1 -Label closed      # app and fan helper ended
  .\scripts\measure-battery.ps1 -Label tray        # tray only
  .\scripts\measure-battery.ps1 -Label window      # window open

Unplug the charger and any monitor on the dGPU first, wait a minute, then
start. Results go to .local\battery-measure\<time>-<label>.csv.
#>
[CmdletBinding()]
param(
    [string]$Label = 'run',
    [ValidateRange(10, 3600)][int]$Seconds = 180,
    [ValidateRange(1, 60)][int]$Interval = 2
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$directory = Join-Path $root '.local\battery-measure'
New-Item -ItemType Directory -Force -Path $directory | Out-Null
$file = Join-Path $directory ('{0}-{1}.csv' -f (Get-Date -Format 'yyyyMMdd-HHmmss'), ($Label -replace '[^A-Za-z0-9_-]', '_'))

$nvidia = Get-PnpDevice -Class Display -PresentOnly |
    Where-Object InstanceId -Like 'PCI\VEN_10DE*' |
    Select-Object -First 1 -ExpandProperty InstanceId

function Get-GpuState {
    if (-not $nvidia) { return 'none' }
    try {
        $data = (Get-PnpDeviceProperty -InstanceId $nvidia -KeyName 'DEVPKEY_Device_PowerData').Data
        switch ([BitConverter]::ToInt32($data, 4)) { 1 { 'D0' } 2 { 'D1' } 3 { 'D2' } 4 { 'D3' } default { 'unknown' } }
    } catch { 'unknown' }
}

$rows = New-Object System.Collections.Generic.List[object]
$end = (Get-Date).AddSeconds($Seconds)
Write-Host "Measuring '$Label' for $Seconds s. Nothing is changed."
while ((Get-Date) -lt $end) {
    $battery = Get-CimInstance -Namespace root\wmi -ClassName BatteryStatus -ErrorAction SilentlyContinue | Select-Object -First 1
    $plan = (powercfg /getactivescheme) -replace '^.*:\s*([0-9a-fA-F-]{36}).*$', '$1'
    $processes = Get-Process -ErrorAction SilentlyContinue |
        Where-Object { $_.ProcessName -match '^(Omen Hub\+\+|Omen Hub\+\+ Fan|OmenCommandCenterBackground)$' } |
        ForEach-Object ProcessName | Sort-Object -Unique
    $row = [pscustomobject]@{
        time             = Get-Date -Format 'HH:mm:ss'
        on_ac            = if ($battery) { $battery.PowerOnline } else { '' }
        discharge_w      = if ($battery) { [math]::Round($battery.DischargeRate / 1000, 2) } else { '' }
        charge_w         = if ($battery) { [math]::Round($battery.ChargeRate / 1000, 2) } else { '' }
        remaining_wh     = if ($battery) { [math]::Round($battery.RemainingCapacity / 1000, 2) } else { '' }
        dgpu             = Get-GpuState
        power_plan       = $plan
        omen_processes   = $processes -join '+'
    }
    $rows.Add($row)
    Start-Sleep -Seconds $Interval
}
$rows | Export-Csv -LiteralPath $file -NoTypeInformation -Encoding UTF8

$discharge = $rows | Where-Object { $_.discharge_w -ne '' -and $_.on_ac -eq $false } | ForEach-Object { [double]$_.discharge_w }
$asleep = @($rows | Where-Object dgpu -In 'D1', 'D2', 'D3').Count
[pscustomobject]@{
    label               = $Label
    samples             = $rows.Count
    samples_on_battery  = @($discharge).Count
    mean_discharge_w    = if ($discharge) { [math]::Round(($discharge | Measure-Object -Average).Average, 2) } else { $null }
    max_discharge_w     = if ($discharge) { ($discharge | Measure-Object -Maximum).Maximum } else { $null }
    dgpu_asleep_percent = if ($rows.Count) { [math]::Round(100 * $asleep / $rows.Count) } else { $null }
    power_plans         = ($rows.power_plan | Sort-Object -Unique) -join ', '
    processes           = ($rows.omen_processes | Sort-Object -Unique) -join ' | '
    file                = $file
} | Format-List
