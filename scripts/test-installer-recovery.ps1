# Executes only the actual installers' recovery-record guard AST on isolated
# temporary JSON files. Never runs an installer, service command or task action.
#requires -Version 5.1
$ErrorActionPreference='Stop'
$testRoot=Join-Path ([IO.Path]::GetTempPath()) ('OmenInstallerGuard-'+[Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $testRoot | Out-Null
$taskInstallRoot=$testRoot
$fanRoot=$testRoot
$appRoot=$testRoot
$testPassed=0
try {
    foreach($testInstaller in @('install-startup.ps1','install-fan-service.ps1','install-app.ps1')) {
        $testTokens=$null;$testErrors=$null
        $testAst=[Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot $testInstaller),[ref]$testTokens,[ref]$testErrors)
        if($testErrors.Count){throw 'Installer syntax failed.'}
        $testLoop=$testAst.Find({param($node) $node -is [Management.Automation.Language.ForEachStatementAst] -and $node.Variable.VariablePath.UserPath -eq 'recoveryFile'},$true)
        if($null-eq$testLoop){throw 'Installer recovery guard is missing.'}
        $testGuard=[ScriptBlock]::Create($testLoop.Extent.Text)
        $testMutations=$testAst.FindAll({param($node) $node -is [Management.Automation.Language.CommandAst] -and $node.GetCommandName() -in @('Copy-Item','Stop-ScheduledTask','Stop-Process','Unregister-ScheduledTask','Set-Acl')},$true)
        foreach($testAction in $testMutations){if($testAction.Extent.StartOffset-lt$testLoop.Extent.EndOffset){throw 'Installer mutates state before its recovery guard.'}}
        foreach($testName in @('controller-ownership.json','controller-cooling.json')) {
            $testPath=Join-Path $testRoot $testName
            foreach($testCase in @('absent','restored','superseded','owned','pending_maximum','return_requested','restore_pending','corrupt','oversized','unknown')) {
                if(Test-Path -LiteralPath $testPath){Remove-Item -LiteralPath $testPath}
                if($testCase-ne'absent') {
                    $testValue=switch($testCase){
                        'corrupt' {'{'}
                        'oversized' {'x'*16385}
                        'unknown' {'{}'}
                        default { @{schema=1;phase=$testCase;owner=@{session=('a'*64)}} | ConvertTo-Json -Depth 3 -Compress }
                    }
                    [IO.File]::WriteAllText($testPath,$testValue,[Text.UTF8Encoding]::new($false))
                }
                $testRejected=$false
                try{ & $testGuard }catch{ $testRejected=$true }
                $testExpected=$testCase-notin@('absent','restored','superseded')
                if($testRejected-ne$testExpected){throw "Unexpected guard result: $testInstaller / $testName / $testCase"}
                $testPassed++
            }
            Remove-Item -LiteralPath $testPath
        }
    }
    [pscustomobject]@{passed=$testPassed;real_installers_run=$false;services_changed=$false}|ConvertTo-Json
} finally {
    foreach($testName in @('controller-ownership.json','controller-cooling.json')) {
        $testPath=Join-Path $testRoot $testName
        if(Test-Path -LiteralPath $testPath){Remove-Item -LiteralPath $testPath}
    }
    Remove-Item -LiteralPath $testRoot
}
