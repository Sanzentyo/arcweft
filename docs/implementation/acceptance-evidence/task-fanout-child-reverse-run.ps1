$ErrorActionPreference='Stop'
Set-Location -LiteralPath 'D:/git/arcweft'
$evidenceDir='C:/Users/sanze/AppData/Local/Temp/arcweft-1011-convergence-resume-cb798c1d'
$catalogName='task-final-r2842-fanout-child-reverse-catalog'
$runName='task-final-r2843-fanout-child-reverse-runtime'
$receipt=Get-Content (Join-Path $evidenceDir ($catalogName+'.json')) -Raw | ConvertFrom-Json
if($receipt.exitCode -ne 0){throw 'Reverse catalog did not pass'}
& (Join-Path $evidenceDir 'inspect-presentation-catalog.ps1') -Name $catalogName -Required @('program_child_result_fanout_binds_retires_and_survives_each_public_restore','native_fanout_publications_survive_rollback_without_repeating_progress')
$catalog=Get-Content (Join-Path $evidenceDir ($catalogName+'.inventory.json')) -Raw | ConvertFrom-Json
$graph=Get-Content (Join-Path $evidenceDir 'task-fanout-child-reverse-source.graph.json') -Raw | ConvertFrom-Json
$packages=@(for($i=0;$i -lt $receipt.command.Count-1;$i++){if($receipt.command[$i] -eq '-p'){$receipt.command[$i+1]}})
if(@(Compare-Object @($packages | Sort-Object) @($graph.reverseWorkspace | Sort-Object)).Count -or $packages.Count -ne 53){throw 'Catalog package selection differs from the resolved reverse graph'}
$oldNonCore=Get-Content docs/implementation/acceptance-evidence/task-final-r2793-await-many-awbc-reverse-catalog.inventory.json -Raw | ConvertFrom-Json
$oldCore=Get-Content docs/implementation/acceptance-evidence/task-final-r2796-await-many-awbc-unified-core-catalog.inventory.json -Raw | ConvertFrom-Json
$oldKeys=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
foreach($row in @($oldNonCore.rows)+@($oldCore.rows)){[void]$oldKeys.Add($row.binary+"`t"+$row.test)}
$newKeys=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
$added=@(foreach($row in $catalog.rows){$key=$row.binary+"`t"+$row.test; if(-not $newKeys.Add($key)){throw ('Duplicate case: '+$key)}; if(-not $oldKeys.Contains($key)){$row}})
$missing=@($oldKeys | Where-Object {-not $newKeys.Contains($_)})
if($missing.Count -or @($added | Where-Object binary -ne 'arcweft-core').Count){throw 'Review missing previous cases or unexpected non-Core additions'}
$unmatched=@($catalog.rows | Where-Object status -ne 'matches')
if(@($unmatched | Where-Object {-not $_.ignored}).Count){throw 'Non-ignored reverse cases were filtered out'}
$accepted=Get-Content docs/implementation/acceptance-evidence/accepted-generic-c3-c5-closure-audit.json -Raw | ConvertFrom-Json
$selected=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
foreach($row in $catalog.rows | Where-Object status -eq 'matches'){[void]$selected.Add($row.binary+"`t"+$row.test)}
$generic=@(foreach($group in $accepted.groups.PSObject.Properties){foreach($row in $group.Value){$key=$row.binary+"`t"+$row.name;if(-not $selected.Contains($key)){throw ('Missing accepted Generic case: '+$key)}; [ordered]@{group=$group.Name;binary=$row.binary;test=$row.name}}})
if($generic.Count -ne 151){throw 'Accepted Generic inventory drift'}
$freeze=Get-Content (Join-Path $evidenceDir 'task-fanout-child-reverse-source.source-freeze.json') -Raw | ConvertFrom-Json
$drift=@($freeze.paths | Where-Object {(Get-FileHash -LiteralPath $_.path).Hash.ToLowerInvariant() -ne $_.sha256})
if($drift.Count){throw 'Fixed inputs changed while building the reverse catalog'}
$preflight=[ordered]@{version=1;base=$freeze.base;catalog=$catalogName;packages=$packages;allCases=$catalog.all;selectedCases=$catalog.matched;ignored=$unmatched.Count;priorCases=$oldKeys.Count;missingPriorCases=$missing;addedCases=$added;generic=$generic;genericSelected=$generic.Count;sourceInputs=$freeze.paths.Count;sourceDrift=0;at=[DateTime]::UtcNow.ToString('o')}
$preflight | ConvertTo-Json -Depth 6 | Set-Content (Join-Path $evidenceDir 'task-fanout-child-reverse-preflight.json') -Encoding utf8
& (Join-Path $evidenceDir 'pin-catalog-executables.ps1') -Catalog $catalogName -Output 'task-fanout-child-reverse.before-run.pins.json' -Phase before-run
if((Test-Path (Join-Path $evidenceDir ($runName+'.json'))) -or (Test-Path (Join-Path $evidenceDir ($runName+'.log')))){throw 'Runtime name already used; inspect its actual handle/receipt'}
$runArgs=@('nextest','run','--all-features','--locked')
foreach($package in $packages){$runArgs+=@('-p',$package)}
$runArgs+=@('-E','rdeps(=arcweft-core)','--no-fail-fast','--no-tests','fail','--status-level','none','--final-status-level','all')
[ordered]@{catalog=$catalogName;run=$runName;selected=$catalog.matched;ignored=$unmatched.Count;addedCoreCases=$added.Count;missingPriorCases=$missing.Count;genericSelected=$generic.Count;packages=$packages.Count} | ConvertTo-Json -Compress
& (Join-Path $evidenceDir 'task-program-body-command.ps1') -Name $runName -CargoArgs $runArgs