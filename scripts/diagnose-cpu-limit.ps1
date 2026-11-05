param(
    [Parameter(Mandatory=$true)][string]$Executable,
    [Parameter(Mandatory=$true)][ValidatePattern('^[0-9A-Fa-f]{64}$')][string]$ExpectedSha256,
    [Parameter(Mandatory=$true)][string]$OutputDirectory,
    [ValidateRange(1,5)][int]$Samples=3
)
$ErrorActionPreference='Stop'
# Read-only: fixed existing CLI GETs. No Apply, calibration, service action,
# firmware/driver installation or preference/journal write is invoked here.
$taskExe=(Resolve-Path -LiteralPath $Executable).Path
if ((Get-FileHash -LiteralPath $taskExe -Algorithm SHA256).Hash -ne $ExpectedSha256) { throw 'Reader binary hash differs.' }
if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'The existing PawnIO CPU reader requires an elevated process; no driver will be installed.'
}
$taskOutput=[IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Path $taskOutput -Force | Out-Null
function Read-OmenCpu {
    param([ValidateSet('cpu-smu-probe','cpu-hardware-read')][string]$Command)
    $taskRead=[Diagnostics.Process]::new()
    $taskRead.StartInfo.FileName=$taskExe
    $taskRead.StartInfo.Arguments=$Command
    $taskRead.StartInfo.UseShellExecute=$false
    $taskRead.StartInfo.CreateNoWindow=$true
    $taskRead.StartInfo.RedirectStandardOutput=$true
    $taskRead.StartInfo.RedirectStandardError=$true
    try {
        if (-not $taskRead.Start()) { throw 'Read-only child did not start.' }
        $taskStdout=$taskRead.StandardOutput.ReadToEndAsync()
        $taskStderr=$taskRead.StandardError.ReadToEndAsync()
        if (-not $taskRead.WaitForExit(20000)) {
            # Only this owned GET child, never an existing hardware controller.
            $taskRead.Kill()
            throw 'Read-only child exceeded its 20-second deadline.'
        }
        $taskText=$taskStdout.GetAwaiter().GetResult()
        $taskError=$taskStderr.GetAwaiter().GetResult()
        if ($taskRead.ExitCode -ne 0) { throw $taskError }
        return $taskText | ConvertFrom-Json
    } finally { $taskRead.Dispose() }
}
$taskResults=@()
try {
    for ($taskIndex=0; $taskIndex -lt $Samples; $taskIndex++) {
        $taskSmu=Read-OmenCpu 'cpu-smu-probe'
        $taskMeasured=Read-OmenCpu 'cpu-hardware-read'
        $taskLimit=$null; $taskPpt=$null; $taskSlow=$null
        if ($taskSmu.pm_table_version.Ok -eq 0x540208 -and $taskSmu.pm_table_head.Ok.Count -ge 11) {
            $taskValue=[double]$taskSmu.pm_table_head.Ok[10]
            if (-not [double]::IsNaN($taskValue) -and -not [double]::IsInfinity($taskValue) -and $taskValue -ge 60 -and $taskValue -le 95) { $taskLimit=$taskValue }
            # Same measured 0x540208 contract as cpu_tuning.rs, cells 2 and 4.
            $taskFastValue=[double]$taskSmu.pm_table_head.Ok[2]
            $taskSlowValue=[double]$taskSmu.pm_table_head.Ok[4]
            if (-not [double]::IsNaN($taskFastValue) -and -not [double]::IsInfinity($taskFastValue) -and $taskFastValue -ge 15 -and $taskFastValue -le 160) { $taskPpt=$taskFastValue }
            if (-not [double]::IsNaN($taskSlowValue) -and -not [double]::IsInfinity($taskSlowValue) -and $taskSlowValue -ge 15 -and $taskSlowValue -le 160) { $taskSlow=$taskSlowValue }
        }
        $taskService=Get-CimInstance Win32_Service -Filter "Name='HPOmenCap'"
        $taskResults+= [pscustomobject]@{
            recorded_at_utc=[DateTime]::UtcNow.ToString('o')
            tctl_limit_readback_c=$taskLimit
            ppt_limit_readback_w=$taskPpt
            slow_limit_readback_w=$taskSlow
            measured_package_power_w=$taskMeasured.measured.package_power_w
            measured_tctl_c=$taskMeasured.measured.temperature_c
            hp_controller_state=$taskService.State
            hp_controller_start_mode=$taskService.StartMode
            pm_table_version=$taskSmu.pm_table_version
        }
        [IO.File]::WriteAllText((Join-Path $taskOutput ("sample-$taskIndex.json")),(@{smu=$taskSmu;cpu=$taskMeasured}|ConvertTo-Json -Depth 16),[Text.UTF8Encoding]::new($false))
        if ($taskIndex+1 -lt $Samples) { Start-Sleep -Seconds 2 }
    }
    $taskBoot=$null; $taskTpm=$null
    try { $taskBoot=Confirm-SecureBootUEFI } catch { $taskBoot=$_.Exception.Message }
    try { $taskTpm=Get-Tpm | Select-Object TpmPresent,TpmReady,TpmEnabled } catch { $taskTpm=$_.Exception.Message }
    $taskReport=@{schema=1;reader=$taskExe;reader_sha256=$ExpectedSha256;hardware_applied=$false;samples=$taskResults;secure_boot=$taskBoot;tpm=$taskTpm}
    [IO.File]::WriteAllText((Join-Path $taskOutput 'report.json'),($taskReport|ConvertTo-Json -Depth 10),[Text.UTF8Encoding]::new($false))
} catch {
    [IO.File]::WriteAllText((Join-Path $taskOutput 'error.txt'),$_.Exception.ToString(),[Text.UTF8Encoding]::new($false))
    throw
}
