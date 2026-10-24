$ErrorActionPreference='Stop'
Set-Location -LiteralPath 'D:/git/arcweft'
$taskEvidence='C:/Users/sanze/AppData/Local/Temp/arcweft-1011-convergence-resume-cb798c1d'
$taskFreezeName='task-presentation-role-repaired-source'
$taskFreeze=Get-Content -LiteralPath "$taskEvidence/$taskFreezeName.source-freeze.json" -Raw | ConvertFrom-Json
$taskGraph=Get-Content -LiteralPath "$taskEvidence/$taskFreezeName.graph.json" -Raw | ConvertFrom-Json
foreach($taskName in @('task-final-r2851-presentation-role-workspace-clippy','task-final-r2852-presentation-role-reverse-catalog')){
 if((Test-Path -LiteralPath "$taskEvidence/$taskName.json") -or (Test-Path -LiteralPath "$taskEvidence/$taskName.log")){throw ('Name already used: '+$taskName)}
}
$taskRun='task-final-r2850-presentation-role-workspace-check'
$taskReceipt=Get-Content -LiteralPath "$taskEvidence/$taskRun.json" -Raw | ConvertFrom-Json
if($taskReceipt.exitCode -ne 0){throw 'Workspace check failed'}
$taskDrift=@($taskFreeze.paths | Where-Object {(Get-FileHash -LiteralPath $_.path).Hash.ToLowerInvariant() -ne $_.sha256})
if($taskDrift.Count){throw 'Frozen workspace check inputs changed'}
[ordered]@{version=1;base=$taskFreeze.base;run=$taskRun;exitCode=0;sourceInputs=$taskFreeze.paths.Count;sourceDrift=0;configuration=$taskReceipt.command;at=[DateTime]::UtcNow.ToString('o')} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath "$taskEvidence/$taskRun.input-audit.json" -Encoding utf8
& "$taskEvidence/task-program-body-command.ps1" -Name task-final-r2851-presentation-role-workspace-clippy -CargoArgs @('clippy','--workspace','--all-targets','--all-features','--locked')
if($LASTEXITCODE -ne 0){throw 'Workspace Clippy failed; preserve original receipt'}
$taskRun='task-final-r2851-presentation-role-workspace-clippy'
$taskReceipt=Get-Content -LiteralPath "$taskEvidence/$taskRun.json" -Raw | ConvertFrom-Json
if($taskReceipt.exitCode -ne 0){throw 'Workspace Clippy receipt failed'}
$taskDrift=@($taskFreeze.paths | Where-Object {(Get-FileHash -LiteralPath $_.path).Hash.ToLowerInvariant() -ne $_.sha256})
if($taskDrift.Count){throw 'Frozen workspace Clippy inputs changed'}
[ordered]@{version=1;base=$taskFreeze.base;run=$taskRun;exitCode=0;sourceInputs=$taskFreeze.paths.Count;sourceDrift=0;configuration=$taskReceipt.command;at=[DateTime]::UtcNow.ToString('o')} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath "$taskEvidence/$taskRun.input-audit.json" -Encoding utf8
$taskArgs=@('nextest','list','--all-features','--locked')
foreach($taskPackage in $taskGraph.reverseWorkspace){$taskArgs+=@('-p',$taskPackage)}
$taskArgs+=@('-E','rdeps(=arcweft-lang-sema)','--message-format','json')
& "$taskEvidence/catalog-preflight-run.ps1" -Name task-final-r2852-presentation-role-reverse-catalog -CargoArguments $taskArgs
if($LASTEXITCODE -ne 0){throw 'Reverse catalog failed; preserve original receipt'}
[ordered]@{version=1;phase='Workspace gates and reverse22 catalog terminal; inspect before runtime';catalog='task-final-r2852-presentation-role-reverse-catalog';nextUnusedRun=2853;freeze=$taskFreezeName;at=[DateTime]::UtcNow.ToString('o')} | ConvertTo-Json | Set-Content -LiteralPath "$taskEvidence/task-presentation-role-shared-catalog.complete.json" -Encoding utf8
