param([Parameter(Mandatory)][string]$Run,[Parameter(Mandatory)][string]$Catalog,[Parameter(Mandatory)][string]$Freeze)
$ErrorActionPreference='Stop'
$taskEvidence='C:/Users/sanze/AppData/Local/Temp/arcweft-1011-convergence-resume-cb798c1d'
$taskReceipt=Get-Content -LiteralPath (Join-Path $taskEvidence ($Run+'.json')) -Raw | ConvertFrom-Json
$taskCatalog=Get-Content -LiteralPath (Join-Path $taskEvidence ($Catalog+'.inventory.json')) -Raw | ConvertFrom-Json
$taskSelected=@($taskCatalog.rows | Where-Object status -eq 'matches')
$taskLines=[IO.File]::ReadAllLines($taskReceipt.log)
$taskSummaryIndex=[Array]::FindIndex($taskLines,[Predicate[string]]{param($line) $line -cmatch '^\s*Summary\s+\['})
if($taskSummaryIndex -lt 0){throw 'Missing terminal summary; execution is not complete evidence'}
$taskCases=@(foreach($taskLine in $taskLines[($taskSummaryIndex+1)..($taskLines.Length-1)]){
 if($taskLine -cmatch '^\s*(PASS|SLOW|FAIL|ABORT)\s+\[.*?\]\s+\(\s*\d+/\d+\)\s+(\S+)\s+(\S+)\s*$'){
  [pscustomobject]@{status=$Matches[1];binary=$Matches[2];test=$Matches[3]}
 }
})
$taskCaseRowSource='final-status-after-summary'
if($taskCases.Count -ne $taskSelected.Count){
 $taskCases=@(foreach($taskLine in $taskLines[0..($taskSummaryIndex-1)]){
  if($taskLine -cmatch '^\s*(PASS|SLOW|FAIL|ABORT)\s+\[.*?\]\s+\(\s*\d+/\d+\)\s+(\S+)\s+(\S+)\s*$'){
   [pscustomobject]@{status=$Matches[1];binary=$Matches[2];test=$Matches[3]}
  }
 })
 $taskCaseRowSource='complete-live-status-before-terminal-summary'
}
$taskExpected=@($taskSelected | ForEach-Object {$_.binary+"`t"+$_.test} | Sort-Object)
$taskObserved=@($taskCases | ForEach-Object {$_.binary+"`t"+$_.test} | Sort-Object)
if($taskCases.Count -ne $taskSelected.Count -or @(Compare-Object $taskExpected $taskObserved).Count){throw 'Incomplete or changed terminal catalog set'}
$taskPassed=@($taskCases | Where-Object status -in @('PASS','SLOW')).Count
$taskFailures=@($taskCases | Where-Object status -notin @('PASS','SLOW'))
$taskSelectedPackages=@(for($taskIndex=0;$taskIndex -lt $taskReceipt.command.Count-1;$taskIndex++){if($taskReceipt.command[$taskIndex] -eq '-p'){$taskReceipt.command[$taskIndex+1]}})
$taskBinaryMetadataIndex=[Array]::IndexOf([string[]]$taskReceipt.command,'--binaries-metadata')
if($taskBinaryMetadataIndex -ge 0){
 $taskCargoMetadataIndex=[Array]::IndexOf([string[]]$taskReceipt.command,'--cargo-metadata')
 if($taskCargoMetadataIndex -lt 0){throw 'Reused binaries have no recorded Cargo metadata'}
 $taskBinaryMetadata=Get-Content -LiteralPath $taskReceipt.command[$taskBinaryMetadataIndex+1] -Raw | ConvertFrom-Json -Depth 100
 $taskCargoMetadata=Get-Content -LiteralPath $taskReceipt.command[$taskCargoMetadataIndex+1] -Raw | ConvertFrom-Json -Depth 100
 $taskPackageIds=@($taskBinaryMetadata.'rust-binaries'.PSObject.Properties | ForEach-Object {$_.Value.'package-id'} | Sort-Object -Unique)
 $taskSelectedPackages=@($taskCargoMetadata.packages | Where-Object {$_.id -in $taskPackageIds} | ForEach-Object name | Sort-Object -Unique)
 if($taskSelectedPackages.Count -ne $taskPackageIds.Count){throw 'Reused binary package IDs do not resolve in their Cargo metadata'}
}
$taskMatchedPackages=@($taskSelected.package | Sort-Object -Unique)
$taskNoMatchingSelectedTests=@($taskSelectedPackages | Where-Object {$_ -notin $taskMatchedPackages})
$taskPattern='^\s*Summary\s+\[.*?\]\s+'+$taskSelected.Count+' tests? run: '+$taskPassed+' passed(?: \(\d+ slow\))?'
if($taskFailures.Count){$taskPattern+=', '+$taskFailures.Count+' failed'}
if($taskCatalog.excluded){
 $taskPattern+=', '+$taskCatalog.excluded+' skipped\s*$'
}else{
 $taskPattern+='(?:, 0 skipped)?\s*$'
}
$taskSummary=@($taskLines | Where-Object {$_ -cmatch $taskPattern})
if($taskSummary.Count -ne 1){throw 'Summary differs from actual terminal cases'}
if(($taskFailures.Count -eq 0 -and $taskReceipt.exitCode -ne 0) -or ($taskFailures.Count -gt 0 -and $taskReceipt.exitCode -ne 100)){throw 'Exit code differs from the terminal result'}
$taskFreeze=Get-Content -LiteralPath (Join-Path $taskEvidence ($Freeze+'.source-freeze.json')) -Raw | ConvertFrom-Json
if (@($taskFreeze.deleted | Where-Object { $_ -and (Test-Path -LiteralPath $_) }).Count) { throw 'Deleted frozen source was recreated' }
$taskDrift=@($taskFreeze.paths | Where-Object {(Get-FileHash -LiteralPath $_.path -Algorithm SHA256).Hash.ToLowerInvariant() -ne $_.sha256})
if($taskDrift.Count){throw ('Frozen source changed: '+($taskDrift.path -join ', '))}
[ordered]@{version=1;base=$taskReceipt.base;run=$Run;actualExitCode=$taskReceipt.exitCode;summary=$taskSummary[0].Trim();caseRowSource=$taskCaseRowSource;executed=$taskCases.Count;passed=$taskPassed;failed=$taskFailures.Count;skipped=$taskCatalog.excluded;selectedPackages=$taskSelectedPackages.Count;matchedPackages=$taskMatchedPackages.Count;noMatchingSelectedTests=$taskNoMatchingSelectedTests;catalogCaseDrift=0;sourceInputs=$taskFreeze.paths.Count;sourceDrift=0;held=$taskFreeze.held.Count;failureSet=$taskFailures;isPassingRun=($taskReceipt.exitCode -eq 0);recordedAt=[DateTime]::UtcNow.ToString('o')} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $taskEvidence ($Run+'.terminal-audit.json')) -Encoding utf8
Get-Content -LiteralPath (Join-Path $taskEvidence ($Run+'.terminal-audit.json')) -Raw
