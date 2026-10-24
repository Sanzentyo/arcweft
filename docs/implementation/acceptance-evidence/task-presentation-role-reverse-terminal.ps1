$ErrorActionPreference='Stop'
Set-Location -LiteralPath 'D:/git/arcweft'
$taskEvidence='C:/Users/sanze/AppData/Local/Temp/arcweft-1011-convergence-resume-cb798c1d'
$taskRun='task-final-r2853-presentation-role-reverse-runtime'
$taskCatalogName='task-final-r2852-presentation-role-reverse-catalog'
$taskFreezeName='task-presentation-role-repaired-source'
if(-not (Test-Path -LiteralPath "$taskEvidence/$taskRun.json")){throw 'Runtime has not produced its terminal receipt; keep observing the original session'}
$taskAuditText=& "$taskEvidence/audit-nextest-final-result.ps1" -Run $taskRun -Catalog $taskCatalogName -Freeze $taskFreezeName
$taskAudit=Get-Content -LiteralPath "$taskEvidence/$taskRun.terminal-audit.json" -Raw | ConvertFrom-Json
$taskReceipt=Get-Content -LiteralPath "$taskEvidence/$taskRun.json" -Raw | ConvertFrom-Json
$taskLines=[IO.File]::ReadAllLines($taskReceipt.log)
$taskSummaryIndex=[Array]::FindIndex($taskLines,[Predicate[string]]{param($line) $line -cmatch '^\s*Summary\s+\['})
$taskCases=@(foreach($taskLine in $taskLines[($taskSummaryIndex+1)..($taskLines.Length-1)]){
 if($taskLine -cmatch '^\s*(PASS|SLOW|FAIL)\s+\[.*?\]\s+\(\s*\d+/\d+\)\s+(\S+)\s+(\S+)\s*$'){
  [pscustomobject]@{status=$Matches[1];binary=$Matches[2];test=$Matches[3]}
 }
})
$taskActual=@{}
foreach($taskCase in $taskCases){
 $taskKey=$taskCase.binary+"`t"+$taskCase.test
 if($taskActual.ContainsKey($taskKey)){throw 'Duplicate terminal case'}
 $taskActual[$taskKey]=$taskCase
}
$taskOld=Get-Content -LiteralPath 'docs/implementation/acceptance-evidence/task-await-many-awbc-reverse-combined-execution.json' -Raw | ConvertFrom-Json
$taskOldFailures=@($taskOld.failureSet | ForEach-Object {$_.binary+"`t"+$_.test} | Sort-Object)
$taskNewFailures=@($taskAudit.failureSet | ForEach-Object {$_.binary+"`t"+$_.test} | Sort-Object)
$taskNewFailureKeys=@($taskNewFailures | Where-Object {$_ -notin $taskOldFailures})
$taskNoLongerFailing=@($taskOldFailures | Where-Object {$_ -notin $taskNewFailures})
$taskGeneric=Get-Content -LiteralPath 'docs/implementation/acceptance-evidence/accepted-generic-c3-c5-closure-audit.json' -Raw | ConvertFrom-Json
$taskGenericRows=@(foreach($taskGroup in $taskGeneric.groups.PSObject.Properties){foreach($taskCase in $taskGroup.Value){
 $taskKey=$taskCase.binary+"`t"+$taskCase.name
 if(-not $taskActual.ContainsKey($taskKey)){throw ('Missing accepted Generic case: '+$taskKey)}
 [ordered]@{group=$taskGroup.Name;binary=$taskCase.binary;test=$taskCase.name;status=$taskActual[$taskKey].status}
}})
if($taskGenericRows.Count -ne 151){throw 'Accepted Generic inventory differs from 151'}
$taskOwnerNames=@(
 'presentation_schemas_retain_typed_argument_roles_for_every_callable',
 'unchecked_admission_commits_the_value_consumer',
 'unchecked_presentation_roles_cannot_alias_each_other_or_an_ordinary_value',
 'character_nominal_show_checker_signature_primary_and_schema_equal'
)
$taskOwnerRows=@($taskCases | Where-Object {($_.test -split '::')[-1] -in $taskOwnerNames})
if($taskOwnerRows.Count -ne 4){throw 'Missing required Presentation owner tests'}
$taskPins=Get-Content -LiteralPath "$taskEvidence/task-presentation-role-reverse.before-run.pins.json" -Raw | ConvertFrom-Json
$taskPinRows=@(foreach($taskPin in $taskPins.binaries){
 $taskFile=Get-Item -LiteralPath $taskPin.path
 $taskHash=(Get-FileHash -LiteralPath $taskPin.path -Algorithm SHA256).Hash.ToLowerInvariant()
 [ordered]@{path=$taskPin.path;bytes=$taskFile.Length;nonTest=$taskPin.nonTest;sha256=$taskHash;unchanged=($taskHash -eq $taskPin.sha256 -and $taskFile.Length -eq $taskPin.bytes)}
})
$taskPinDrift=@($taskPinRows | Where-Object {-not $_.unchanged})
[ordered]@{version=1;run=$taskRun;catalog=$taskCatalogName;binaries=$taskPinRows;binaryCount=$taskPinRows.Count;binaryDrift=$taskPinDrift.Count;artifactCopyMove=$false;at=[DateTime]::UtcNow.ToString('o')} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath "$taskEvidence/task-presentation-role-reverse.executable-audit.json" -Encoding utf8
if($taskPinDrift.Count){throw 'Pinned binary changed during execution'}
$taskInventory=Get-Content -LiteralPath "$taskEvidence/$taskCatalogName.inventory.json" -Raw | ConvertFrom-Json
$taskGraph=Get-Content -LiteralPath "$taskEvidence/$taskFreezeName.graph.json" -Raw | ConvertFrom-Json
$taskCatReceipt=Get-Content -LiteralPath "$taskEvidence/$taskCatalogName.json" -Raw | ConvertFrom-Json
$taskCatLine=[IO.File]::ReadLines($taskCatReceipt.log) | Where-Object {$_.StartsWith('{"rust-build-meta"')} | Select-Object -First 1
$taskCatalog=$taskCatLine | ConvertFrom-Json -Depth 100
$taskCoverage=@(foreach($taskSuite in $taskCatalog.'rust-suites'.PSObject.Properties.Value){
 $taskRows=@($taskInventory.rows | Where-Object binary -eq $taskSuite.'binary-id')
 $taskSelected=@($taskRows | Where-Object status -eq 'matches')
 $taskExecuted=@(foreach($taskRow in $taskSelected){$taskActual[$taskRow.binary+"`t"+$taskRow.test]})
 [ordered]@{package=$taskSuite.'package-name';binary=$taskSuite.'binary-id';kind=$taskSuite.kind;catalog=$taskRows.Count;selected=$taskSelected.Count;executed=$taskExecuted.Count;passed=@($taskExecuted | Where-Object status -in @('PASS','SLOW')).Count;failed=@($taskExecuted | Where-Object status -eq 'FAIL').Count;ignored=@($taskRows | Where-Object ignored).Count;buildOnly=($taskRows.Count -eq 0);status=$taskSuite.status}
})
$taskPackages=@(foreach($taskPackage in $taskGraph.reverseWorkspace){
 $taskTargets=@($taskCoverage | Where-Object package -eq $taskPackage)
 [ordered]@{package=$taskPackage;targets=$taskTargets.Count;executed=($taskTargets.executed | Measure-Object -Sum).Sum;passed=($taskTargets.passed | Measure-Object -Sum).Sum;failed=($taskTargets.failed | Measure-Object -Sum).Sum;ignored=($taskTargets.ignored | Measure-Object -Sum).Sum;buildOnlyTargets=@($taskTargets | Where-Object buildOnly).Count}
})
[ordered]@{version=1;base=$taskReceipt.base;run=$taskRun;catalog=$taskCatalogName;selectedPackages=$taskGraph.reverseWorkspace;packages=$taskPackages;targets=$taskCoverage;allCases=$taskInventory.all;selected=$taskInventory.matched;ignored=$taskInventory.excluded;ownerOnlyReuse=$false;duplicateCases=0;at=[DateTime]::UtcNow.ToString('o')} | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath "$taskEvidence/task-presentation-role-reverse-coverage.json" -Encoding utf8
[ordered]@{version=1;base=$taskReceipt.base;sourcePublication=$null;unpublishedSourcePaths=7;run=$taskRun;catalog=$taskCatalogName;actualExitCode=$taskReceipt.exitCode;summary=$taskAudit.summary;executed=$taskAudit.executed;passed=$taskAudit.passed;failed=$taskAudit.failed;ignored=$taskAudit.skipped;selectedPackages=$taskGraph.reverseWorkspace.Count;catalogCaseDrift=$taskAudit.catalogCaseDrift;sourceInputs=$taskAudit.sourceInputs;sourceDrift=$taskAudit.sourceDrift;failureSet=$taskAudit.failureSet;baselineFailureCount=$taskOldFailures.Count;newFailureKeys=$taskNewFailureKeys;noLongerFailingKeys=$taskNoLongerFailing;genericRows=$taskGenericRows;genericPassed=@($taskGenericRows | Where-Object status -in @('PASS','SLOW')).Count;requiredPresentation=$taskOwnerRows;binaryCount=$taskPinRows.Count;binaryDrift=$taskPinDrift.Count;ownerOnlyReuse=$false;at=[DateTime]::UtcNow.ToString('o')} | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath "$taskEvidence/task-presentation-role-reverse-acceptance.json" -Encoding utf8
[ordered]@{executed=$taskAudit.executed;passed=$taskAudit.passed;failed=$taskAudit.failed;ignored=$taskAudit.skipped;newFailures=$taskNewFailureKeys.Count;noLongerFailing=$taskNoLongerFailing.Count;genericPassed=@($taskGenericRows | Where-Object status -in @('PASS','SLOW')).Count;requiredPresentation=$taskOwnerRows.Count;sourceInputs=$taskAudit.sourceInputs;sourceDrift=$taskAudit.sourceDrift;binaryCount=$taskPinRows.Count;binaryDrift=$taskPinDrift.Count;targets=$taskCoverage.Count;packages=$taskPackages.Count} | ConvertTo-Json -Compress
