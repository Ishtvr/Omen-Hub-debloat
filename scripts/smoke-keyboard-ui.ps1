# Metadata-only renders of the production lighting window. No UAC or hardware SET.
#requires -Version 5.1
param()
$ErrorActionPreference='Stop'
$taskRepo=Split-Path -Parent $PSScriptRoot
$taskExe=Join-Path $taskRepo 'dist\Omen Hub++.exe'
$taskId='keyboard-preview-test-'+[Guid]::NewGuid().ToString('N')
$taskStorage=& $taskExe profile-storage|Out-String|ConvertFrom-Json
$taskRoot=[IO.Path]::GetFullPath((Join-Path $taskStorage.directory 'profiles'))
$taskPath=Join-Path $taskRoot ($taskId+'.json')
$taskOut=Join-Path $taskRepo '.local\keyboard-ui-smoke'
New-Item -ItemType Directory -Force -Path $taskOut|Out-Null
function Invoke-KeyboardJson {param([string[]]$Arguments);$taskRaw=& $taskExe @Arguments|Out-String;if($LASTEXITCODE-ne0){throw $taskRaw};$taskRaw|ConvertFrom-Json}
$taskFiles=@((Join-Path $taskStorage.directory 'control-journal.json'),(Join-Path $taskStorage.directory 'startup.json'),(Join-Path $taskStorage.directory 'keyboard-control\control-journal.json'))
$taskBefore=@($taskFiles|ForEach-Object{if(Test-Path -LiteralPath $_){(Get-FileHash -LiteralPath $_).Hash}else{'absent'}})
$taskCreated=$false
try {
 if(Test-Path -LiteralPath $taskPath){throw 'Temporary profile exists.'}
 Invoke-KeyboardJson @('profile-create',$taskId,'Lighting capture fixture')|Out-Null;$taskCreated=$true
 $taskProfile=Get-Content -LiteralPath $taskPath -Raw|ConvertFrom-Json
 $taskProfile.settings=@([pscustomobject]@{feature='keyboard_colors';value=[pscustomobject]@{kind='colors';value=@(@(185,213,248),@(202,224,255),@(181,227,221),@(220,218,250))}},[pscustomobject]@{feature='keyboard_effect';value=[pscustomobject]@{kind='choice';value='static'}})
 $taskCaptures=@()
 foreach($taskMode in @('static','off')){
  $taskProfile.settings[1].value.value=$taskMode
  [IO.File]::WriteAllText($taskPath,($taskProfile|ConvertTo-Json -Depth 12),[Text.UTF8Encoding]::new($false))
  foreach($taskSize in @(@(1000,590),@(760,480))){
   $taskBmp=Join-Path $taskOut ($taskMode+'-'+$taskSize[0]+'.bmp')
   $taskArgs='keyboard-preview '+$taskId+' "'+$taskBmp+'" '+($taskSize-join' ')
   $taskProcess=Start-Process -FilePath $taskExe -ArgumentList $taskArgs -PassThru -WindowStyle Hidden
   try{if(-not$taskProcess.WaitForExit(12000)){Stop-Process -Id $taskProcess.Id;throw 'Read-only preview timed out.'};if($taskProcess.ExitCode-ne0){throw 'Lighting preview failed.'}}finally{$taskProcess.Dispose()}
   if(-not(Test-Path -LiteralPath $taskBmp)-or(Get-Item -LiteralPath $taskBmp).Length-lt500000){throw 'Incomplete own-client lighting render.'}
   $taskCaptures+=$taskBmp
  }
 }
 $taskAfter=@($taskFiles|ForEach-Object{if(Test-Path -LiteralPath $_){(Get-FileHash -LiteralPath $_).Hash}else{'absent'}})
 if(($taskBefore-join',')-cne($taskAfter-join',')){throw 'Read-only previews changed a hardware/startup journal.'}
 [pscustomobject]@{measured_at=(Get-Date).ToString('o');binary_sha256=(Get-FileHash -LiteralPath $taskExe).Hash;on_off_renders=$true;minimum_size_renders=$true;captures=$taskCaptures;hardware_writes=$false;journals_startup_unchanged=$true;clean_exit=$true}|ConvertTo-Json -Depth 4|Set-Content -LiteralPath (Join-Path $taskRepo '.local\keyboard-ui-smoke.json') -Encoding UTF8
 'Production keyboard on/off and minimum-size metadata renders passed; journals unchanged.'
} finally {
 if($taskCreated-and(Test-Path -LiteralPath $taskPath)){$taskResolved=(Resolve-Path -LiteralPath $taskPath).Path;if([IO.Path]::GetDirectoryName($taskResolved)-ne$taskRoot){throw 'Cleanup escaped the temporary profile directory.'};Remove-Item -LiteralPath $taskResolved}
}