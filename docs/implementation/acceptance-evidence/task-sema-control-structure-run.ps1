$ErrorActionPreference='Stop'
Set-Location -LiteralPath 'D:/git/arcweft'
$taskRoot='C:/Users/sanze/AppData/Local/Temp/arcweft-1011-convergence-resume-cb798c1d'
$run='task-final-r3073-sema-control-structure'
$log=Join-Path $taskRoot ($run+'.log')
if(Test-Path -LiteralPath $log){throw 'Unique structure run required'}
$started=[DateTime]::UtcNow
& just structure-audit-gate *> $log
$actual=$LASTEXITCODE
[ordered]@{version=1;base=(& git rev-parse HEAD).Trim();name=$run;command=@('just','structure-audit-gate');log=$log;exitCode=$actual;startedAt=$started.ToString('o');finishedAt=[DateTime]::UtcNow.ToString('o')} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot ($run+'.json')) -Encoding utf8
& (Join-Path $taskRoot 'audit-check-inputs.ps1') -Run $run -Freeze 'task-sema-control-checkpoint-owner'
exit $actual
