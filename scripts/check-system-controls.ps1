# Fixed, opt-in conservative hardware qualification, never an ordinary smoke.
# Close tuning applications and pause Omen Hub++'s own fan task beforehand.
# Every implemented trial captures, reads back and immediately restores its
# complete original. An incomplete recovery stops the sequence and is retained.
#requires -Version 5.1
#requires -RunAsAdministrator
param(
    [Parameter(Mandatory=$true)][ValidatePattern('^[0-9a-fA-F]{64}$')][string]$ExpectedBinarySha256,
    [string]$SourceBinary='',
    [string]$Case='all',
    [switch]$CoolCpu
)
$ErrorActionPreference='Stop'
$taskRepo=Split-Path -Parent $PSScriptRoot
$taskExe=if($SourceBinary){[IO.Path]::GetFullPath($SourceBinary)}else{Join-Path $taskRepo 'target\release\omen-hub-pp.exe'}
if((Get-FileHash -LiteralPath $taskExe -Algorithm SHA256).Hash -ine $ExpectedBinarySha256){throw 'Qualification executable differs from the reviewed build.'}
$taskConflicts=@('MSIAfterburner','OmenCommandCenterBackground','HP.Omen.Application.Background.Tasks.Container','Universal x86 Tuning Utility','Omen Hub++ Fan')
if(Get-Process -Name $taskConflicts -ErrorAction SilentlyContinue){throw 'Close the other tuning applications and pause the own fan task before qualification.'}
$taskOut=Join-Path $taskRepo ('.local\research\system-tests\'+(Get-Date -Format 'yyyyMMdd-HHmmss'))
New-Item -ItemType Directory -Path $taskOut -Force|Out-Null
function Invoke-FixedProbe {param([string]$Label,[string[]]$Arguments)
    $taskPreference=$ErrorActionPreference;$ErrorActionPreference='Continue'
    $taskText=& $taskExe @Arguments 2>(Join-Path $taskOut ($Label+'-stderr.log'))|Out-String;$taskCode=$LASTEXITCODE
    $ErrorActionPreference=$taskPreference
    $taskText|Set-Content -LiteralPath (Join-Path $taskOut ($Label+'.log')) -Encoding UTF8
    if($taskCode-ne0){throw ($Label+' returned '+$taskCode+'; inspect its retained report.')}
    return ($taskText|ConvertFrom-Json)
}
$taskRoot=(Invoke-FixedProbe 'storage' @('profile-storage')).directory
$taskMetadata=@('control-journal.json','startup.json','fan-mode.json','active-fan-curve.json','ccd-affinity.json','graphics-mode.json')
function Read-MetadataHashes {
    @($taskMetadata|ForEach-Object{
        $taskPath=Join-Path $taskRoot $_
        if(Test-Path -LiteralPath $taskPath){(Get-FileHash -LiteralPath $taskPath -Algorithm SHA256).Hash}else{'absent'}
    })
}
$taskBefore=Read-MetadataHashes
$taskRecord=[ordered]@{started_at=(Get-Date).ToString('o');binary_sha256=$ExpectedBinarySha256;case=$Case;error=$null;reports=@()}
$taskCooling=$null
if($CoolCpu){
    # Same documented power-policy contract as the production adapter. This
    # outer guard prevents compiler/background boost from heating the CPU
    # during the small SMU trials. Save both sources before changing either.
    Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class OmenSafeCooling {
 [DllImport("powrprof.dll")] static extern uint PowerGetActiveScheme(IntPtr root,out IntPtr scheme);
 [DllImport("kernel32.dll")] static extern IntPtr LocalFree(IntPtr value);
 [DllImport("powrprof.dll")] static extern uint PowerReadACValueIndex(IntPtr root,ref Guid scheme,ref Guid group,ref Guid setting,out uint value);
 [DllImport("powrprof.dll")] static extern uint PowerReadDCValueIndex(IntPtr root,ref Guid scheme,ref Guid group,ref Guid setting,out uint value);
 [DllImport("powrprof.dll")] static extern uint PowerWriteACValueIndex(IntPtr root,ref Guid scheme,ref Guid group,ref Guid setting,uint value);
 [DllImport("powrprof.dll")] static extern uint PowerWriteDCValueIndex(IntPtr root,ref Guid scheme,ref Guid group,ref Guid setting,uint value);
 [DllImport("powrprof.dll")] static extern uint PowerSetActiveScheme(IntPtr root,ref Guid scheme);
 static Guid group=new Guid("54533251-82be-4824-96c1-47b60b740d00"),setting=new Guid("be337238-0d82-4146-a960-4f3749d470c7");
 static void Check(uint code){if(code!=0)throw new Exception("Windows boost policy error "+code);}
 public static Guid Active(){IntPtr p;Check(PowerGetActiveScheme(IntPtr.Zero,out p));try{return (Guid)Marshal.PtrToStructure(p,typeof(Guid));}finally{LocalFree(p);}}
 public static uint[] Read(Guid scheme){uint ac,dc;Check(PowerReadACValueIndex(IntPtr.Zero,ref scheme,ref group,ref setting,out ac));Check(PowerReadDCValueIndex(IntPtr.Zero,ref scheme,ref group,ref setting,out dc));return new uint[]{ac,dc};}
 public static void Write(Guid scheme,uint ac,uint dc){Check(PowerWriteACValueIndex(IntPtr.Zero,ref scheme,ref group,ref setting,ac));Check(PowerWriteDCValueIndex(IntPtr.Zero,ref scheme,ref group,ref setting,dc));if(Active()==scheme)Check(PowerSetActiveScheme(IntPtr.Zero,ref scheme));}
}
'@
    $taskCoolingScheme=[OmenSafeCooling]::Active();$taskCoolingValues=[OmenSafeCooling]::Read($taskCoolingScheme)
    $taskCooling=[pscustomobject]@{scheme=$taskCoolingScheme.ToString();ac=$taskCoolingValues[0];dc=$taskCoolingValues[1]}
    $taskCooling|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $taskOut 'temporary-boost-original.json') -Encoding UTF8
}
try {
    if($taskCooling){[OmenSafeCooling]::Write([guid]$taskCooling.scheme,0,0);Start-Sleep -Seconds 20}
    $taskRecord.controls=Invoke-FixedProbe 'controls' @('system-validate-safe',$Case,'--confirm-safe-writes')
    if($Case-eq'all'){
        foreach($taskCommand in @('fan-validate-max','keyboard-validate-colors','keyboard-validate-lighting','cooling-read')){
            $taskRecord.reports+=@{command=$taskCommand;report=(Invoke-FixedProbe $taskCommand @($taskCommand))}
        }
    }
}catch{$taskRecord.error=$_.Exception.Message}
finally{
    if($taskCooling){
        try {
            $taskLive=[OmenSafeCooling]::Read([guid]$taskCooling.scheme)
            if(($taskLive[0]-ne0-and$taskLive[0]-ne$taskCooling.ac)-or($taskLive[1]-ne0-and$taskLive[1]-ne$taskCooling.dc)){throw 'Temporary boost policy changed externally; inspect the recorded original.'}
            [OmenSafeCooling]::Write([guid]$taskCooling.scheme,$taskCooling.ac,$taskCooling.dc)
            $taskRestored=[OmenSafeCooling]::Read([guid]$taskCooling.scheme)
            $taskRecord.temporary_boost_restored=($taskRestored[0]-eq$taskCooling.ac-and$taskRestored[1]-eq$taskCooling.dc)
            if(-not$taskRecord.temporary_boost_restored){throw 'Temporary Windows boost did not return to both original values.'}
        }catch{$taskRecord.error=$_.Exception.Message;$taskRecord.temporary_boost_restored=$false}
    }
    $taskControlsPath=Join-Path $taskOut 'controls.log'
    if(Test-Path -LiteralPath $taskControlsPath){$taskRecord.controls=Get-Content -LiteralPath $taskControlsPath -Raw|ConvertFrom-Json}
    $taskAfter=Read-MetadataHashes
    $taskRecord.metadata_preserved=(($taskBefore-join',')-ceq($taskAfter-join','))
    $taskRecord.finished_at=(Get-Date).ToString('o')
    $taskRecord|ConvertTo-Json -Depth 30|Set-Content -LiteralPath (Join-Path $taskOut 'summary.json') -Encoding UTF8
    [pscustomobject]@{OutputDirectory=$taskOut;Error=$taskRecord.error;MetadataPreserved=$taskRecord.metadata_preserved;Cases=@($taskRecord.controls.cases|Select-Object case,status,full_originals_restored)}|ConvertTo-Json -Depth 5
}
if($taskRecord.error-or-not$taskRecord.metadata_preserved){exit 1}
