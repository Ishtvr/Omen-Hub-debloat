# Fixture validation of evidence classification; never requests a power change.
#requires -Version 5.1
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'check-power-lifecycle.ps1') -FunctionsOnly
function New-LifecycleFixture {
 [pscustomobject]@{boot_unix_seconds=1000;recorded_at_unix_seconds=2000;binary_sha256='build';profile_digest='profile';live_profile_verified=$true;startup_ready=$true;dashboard_ready=$true;cpu_diagnostic_ok=$true;gpu_diagnostic_ok=$true;app_pid=10;reader_pid=11;power_generation=4;startup_result_time=0;startup_result_ok=$false;startup_task_result=0;startup_task_time=0}
}
function Assert-LifecycleVerdict {param([bool]$Expected,$Before,$After,[string]$Kind,$Events)
 $taskVerdict=Test-OmenLifecycleEvidence $Before $After $Kind $Events
 if($taskVerdict.passed-ne$Expected){throw ('Unexpected fixture verdict: '+($taskVerdict|ConvertTo-Json -Compress))}
}
$taskBefore=New-LifecycleFixture;$taskAfter=New-LifecycleFixture;$taskAfter.power_generation=8;$taskAfter.recorded_at_unix_seconds=2300
$taskPair=@([pscustomobject]@{id=506;record_id=50;unix_seconds=2100},[pscustomobject]@{id=507;record_id=51;unix_seconds=2200})
Assert-LifecycleVerdict $true $taskBefore $taskAfter AfterStandby $taskPair
Assert-LifecycleVerdict $false $taskBefore $taskAfter AfterStandby @()
Assert-LifecycleVerdict $false $taskBefore $taskAfter AfterStandby @($taskPair[0])
$taskWrongOrder=@([pscustomobject]@{id=507;record_id=50;unix_seconds=2100},[pscustomobject]@{id=506;record_id=51;unix_seconds=2200})
Assert-LifecycleVerdict $false $taskBefore $taskAfter AfterStandby $taskWrongOrder
$taskOldPair=@([pscustomobject]@{id=506;record_id=1;unix_seconds=1900},[pscustomobject]@{id=507;record_id=2;unix_seconds=1950})
Assert-LifecycleVerdict $false $taskBefore $taskAfter AfterStandby $taskOldPair
$taskAfter.reader_pid=12
Assert-LifecycleVerdict $false $taskBefore $taskAfter AfterStandby $taskPair
$taskAfter=New-LifecycleFixture;$taskAfter.boot_unix_seconds=4000;$taskAfter.recorded_at_unix_seconds=4020;$taskAfter.startup_result_time=4010;$taskAfter.startup_result_ok=$true;$taskAfter.startup_task_time=4010
Assert-LifecycleVerdict $true $taskBefore $taskAfter AfterReboot @()
$taskAfter.startup_result_time=3999
Assert-LifecycleVerdict $false $taskBefore $taskAfter AfterReboot @()
$taskAfter.startup_result_time=4010;$taskAfter.startup_task_result=1
Assert-LifecycleVerdict $false $taskBefore $taskAfter AfterReboot @()
$taskAfter.startup_task_result=0;$taskAfter.binary_sha256='different build'
Assert-LifecycleVerdict $false $taskBefore $taskAfter AfterReboot @()
$taskAfter.binary_sha256='build';$taskAfter.profile_digest='different profile'
Assert-LifecycleVerdict $false $taskBefore $taskAfter AfterReboot @()
$taskAfter.profile_digest='profile';$taskAfter.live_profile_verified=$false
Assert-LifecycleVerdict $false $taskBefore $taskAfter AfterReboot @()
$taskBefore=New-LifecycleFixture;$taskAfter=New-LifecycleFixture;$taskAfter.recorded_at_unix_seconds=2300;$taskAfter.power_generation=8
$taskFuturePair=@([pscustomobject]@{id=506;record_id=60;unix_seconds=2400},[pscustomobject]@{id=507;record_id=61;unix_seconds=2500})
Assert-LifecycleVerdict $false $taskBefore $taskAfter AfterStandby $taskFuturePair
$taskBefore.dashboard_ready=$false
Assert-LifecycleVerdict $false $taskBefore $taskAfter AfterStandby $taskPair
$taskBefore.dashboard_ready=$true;$taskBefore.cpu_diagnostic_ok=$false
Assert-LifecycleVerdict $false $taskBefore $taskAfter AfterStandby $taskPair
$taskBefore.cpu_diagnostic_ok=$true;$taskBefore.reader_pid=0;$taskAfter.reader_pid=0
Assert-LifecycleVerdict $false $taskBefore $taskAfter AfterStandby $taskPair
$taskBefore.reader_pid=11;$taskAfter.reader_pid=11;$taskAfter.recorded_at_unix_seconds=1900
Assert-LifecycleVerdict $false $taskBefore $taskAfter AfterStandby $taskPair
$taskAfter.boot_unix_seconds=900;$taskAfter.recorded_at_unix_seconds=2400;$taskAfter.startup_result_time=2390;$taskAfter.startup_result_ok=$true;$taskAfter.startup_task_time=2390
Assert-LifecycleVerdict $false $taskBefore $taskAfter AfterReboot @()
'18 lifecycle evidence fixtures passed; no hardware, standby or reboot was invoked.'
