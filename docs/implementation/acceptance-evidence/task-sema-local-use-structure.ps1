$ErrorActionPreference='Stop'
Set-Location -LiteralPath 'D:/git/arcweft'
$e='C:/Users/sanze/AppData/Local/Temp/arcweft-1011-convergence-resume-cb798c1d'
$n='task-final-r3098-sema-local-use-structure'
$log=Join-Path $e ($n+'.log')
$receipt=Join-Path $e ($n+'.json')
if((Test-Path -LiteralPath $log) -or (Test-Path -LiteralPath $receipt)){throw 'Unique structure name required'}
$base=(& git rev-parse HEAD).Trim()
$started=[DateTime]::UtcNow
& just structure-audit-gate *> $log
$actual=$LASTEXITCODE
$finished=[DateTime]::UtcNow
[ordered]@{version=1;base=$base;name=$n;command=@('just','structure-audit-gate');log=$log;exitCode=$actual;startedAt=$started.ToString('o');finishedAt=$finished.ToString('o')} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $receipt -Encoding utf8
& (Join-Path $e 'audit-check-inputs.ps1') -Run $n -Freeze 'task-sema-local-use-reverse'
Get-Content -LiteralPath $log -Tail 4
exit $actual
