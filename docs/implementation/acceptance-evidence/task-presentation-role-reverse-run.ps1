$ErrorActionPreference='Stop'
Set-Location -LiteralPath 'D:/git/arcweft'
$taskEvidence='C:/Users/sanze/AppData/Local/Temp/arcweft-1011-convergence-resume-cb798c1d'
$taskCatalogName='task-final-r2852-presentation-role-reverse-catalog'
$taskRunName='task-final-r2853-presentation-role-reverse-runtime'
$taskReceipt=Get-Content -LiteralPath "$taskEvidence/$taskCatalogName.json" -Raw | ConvertFrom-Json
if($taskReceipt.exitCode -ne 0){throw 'Reverse catalog has not passed'}
& "$taskEvidence/inspect-presentation-catalog.ps1" -Name $taskCatalogName -Required @('presentation_schemas_retain_typed_argument_roles_for_every_callable','unchecked_admission_commits_the_value_consumer','unchecked_presentation_roles_cannot_alias_each_other_or_an_ordinary_value','character_nominal_show_checker_signature_primary_and_schema_equal')
$taskCatalog=Get-Content -LiteralPath "$taskEvidence/$taskCatalogName.inventory.json" -Raw | ConvertFrom-Json
$taskGraph=Get-Content -LiteralPath "$taskEvidence/task-presentation-role-repaired-source.graph.json" -Raw | ConvertFrom-Json
$taskPackages=@(for($taskIndex=0;$taskIndex -lt $taskReceipt.command.Count-1;$taskIndex++){if($taskReceipt.command[$taskIndex] -eq '-p'){$taskReceipt.command[$taskIndex+1]}})
if($taskPackages.Count -ne 22 -or @(Compare-Object @($taskPackages | Sort-Object) @($taskGraph.reverseWorkspace | Sort-Object)).Count){throw 'Catalog packages differ from resolved reverse22 graph'}
$taskOldCatalog=Get-Content -LiteralPath 'docs/implementation/acceptance-evidence/task-final-r2842-fanout-child-reverse-catalog.inventory.json' -Raw | ConvertFrom-Json
$taskOldKeys=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
foreach($taskRow in $taskOldCatalog.rows | Where-Object {$_.package -in $taskPackages}){[void]$taskOldKeys.Add($taskRow.binary+"`t"+$taskRow.test)}
$taskKeys=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
$taskSelected=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
$taskAdded=@(foreach($taskRow in $taskCatalog.rows){$taskKey=$taskRow.binary+"`t"+$taskRow.test;if(-not $taskKeys.Add($taskKey)){throw ('Duplicate catalog case: '+$taskKey)};if($taskRow.status -eq 'matches'){[void]$taskSelected.Add($taskKey)};if(-not $taskOldKeys.Contains($taskKey)){$taskRow}})
$taskMissing=@($taskOldKeys | Where-Object {-not $taskKeys.Contains($_)})
$taskExpectedMissing="arcweft-lang-sema`tcallable::digest::tests::unchecked_admission_golden_bytes_have_no_consumer_payload"
if($taskMissing.Count -ne 1 -or $taskMissing[0] -ne $taskExpectedMissing){throw ('Unexpected removed baseline cases: '+($taskMissing -join ', '))}
$taskExpectedAdded=@('callable::digest::tests::unchecked_admission_commits_the_value_consumer','callable::digest::tests::unchecked_presentation_roles_cannot_alias_each_other_or_an_ordinary_value','callable::schema::families::tests::presentation_schemas_retain_typed_argument_roles_for_every_callable')
if($taskAdded.Count -ne 3 -or @($taskAdded | Where-Object {$_.binary -ne 'arcweft-lang-sema' -or $_.test -notin $taskExpectedAdded}).Count){throw 'Unexpected new catalog cases'}
$taskExcluded=@($taskCatalog.rows | Where-Object status -ne 'matches')
if(@($taskExcluded | Where-Object {-not $_.ignored}).Count){throw 'A non-ignored reverse case was filtered out'}
$taskAccepted=Get-Content -LiteralPath 'docs/implementation/acceptance-evidence/accepted-generic-c3-c5-closure-audit.json' -Raw | ConvertFrom-Json
$taskGeneric=@(foreach($taskGroup in $taskAccepted.groups.PSObject.Properties){foreach($taskRow in $taskGroup.Value){$taskKey=$taskRow.binary+"`t"+$taskRow.name;if(-not $taskSelected.Contains($taskKey)){throw ('Missing accepted Generic case: '+$taskKey)};[ordered]@{group=$taskGroup.Name;binary=$taskRow.binary;test=$taskRow.name}}})
if($taskGeneric.Count -ne 151){throw 'Accepted Generic inventory drift'}
$taskOldFailures=(Get-Content -LiteralPath 'docs/implementation/acceptance-evidence/task-fanout-child-reverse-acceptance.json' -Raw | ConvertFrom-Json).failureSet
foreach($taskFailure in $taskOldFailures){if(-not $taskSelected.Contains($taskFailure.binary+"`t"+$taskFailure.test)){throw 'A known failure was omitted from the reverse selection'}}
$taskFreeze=Get-Content -LiteralPath "$taskEvidence/task-presentation-role-repaired-source.source-freeze.json" -Raw | ConvertFrom-Json
$taskDrift=@($taskFreeze.paths | Where-Object {(Get-FileHash -LiteralPath $_.path).Hash.ToLowerInvariant() -ne $_.sha256})
if($taskDrift.Count){throw 'Inputs changed while catalog built'}
[ordered]@{version=1;base=$taskFreeze.base;catalog=$taskCatalogName;packages=$taskPackages;allCases=$taskCatalog.all;selectedCases=$taskCatalog.matched;ignored=$taskExcluded.Count;baselineCases=$taskOldKeys.Count;removedCases=$taskMissing;removalReason='The old unchecked admission golden asserted that the consumer was absent. Its replacement commits the actual consumer in version 1 and adds role collision rejection.';addedCases=$taskAdded;generic=$taskGeneric;genericSelected=$taskGeneric.Count;baselineFailuresSelected=$taskOldFailures.Count;sourceInputs=$taskFreeze.paths.Count;sourceDrift=0;at=[DateTime]::UtcNow.ToString('o')} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath "$taskEvidence/task-presentation-role-reverse-preflight.json" -Encoding utf8
& "$taskEvidence/pin-catalog-executables.ps1" -Catalog $taskCatalogName -Output task-presentation-role-reverse.before-run.pins.json -Phase before-run
if((Test-Path -LiteralPath "$taskEvidence/$taskRunName.log") -or (Test-Path -LiteralPath "$taskEvidence/$taskRunName.json")){throw 'Runtime name already used; inspect its actual session or receipt'}
$taskArgs=@('nextest','run','--all-features','--locked')
foreach($taskPackage in $taskPackages){$taskArgs+=@('-p',$taskPackage)}
$taskArgs+=@('-E','rdeps(=arcweft-lang-sema)','--no-fail-fast','--no-tests','fail','--status-level','none','--final-status-level','all')
[ordered]@{run=$taskRunName;selected=$taskCatalog.matched;ignored=$taskExcluded.Count;packages=$taskPackages.Count;acceptedGeneric=$taskGeneric.Count;oldFailuresIncluded=$taskOldFailures.Count;sourceInputs=$taskFreeze.paths.Count;sourceDrift=0} | ConvertTo-Json -Compress
& "$taskEvidence/task-program-body-command.ps1" -Name $taskRunName -CargoArgs $taskArgs
