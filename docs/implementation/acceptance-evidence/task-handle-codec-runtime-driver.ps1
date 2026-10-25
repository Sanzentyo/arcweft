$ErrorActionPreference='Stop'
Set-Location -LiteralPath 'D:/git/arcweft'
$taskEvidence='C:/Users/sanze/AppData/Local/Temp/arcweft-1011-convergence-resume-cb798c1d'
$graph=Get-Content -LiteralPath (Join-Path $taskEvidence 'task-handle-codec-repair.graph.json') -Raw | ConvertFrom-Json
$fullInventory=Get-Content -LiteralPath (Join-Path $taskEvidence 'task-final-r3119-handle-codec-reverse-catalog.inventory.json') -Raw | ConvertFrom-Json
$fullLog=Join-Path $taskEvidence 'task-final-r3119-handle-codec-reverse-catalog.log'
$fullLine=[IO.File]::ReadLines($fullLog) | Where-Object { $_.StartsWith('{"rust-build-meta"') } | Select-Object -First 1
$fullCatalog=$fullLine | ConvertFrom-Json -Depth 100
$ownerLog=Join-Path $taskEvidence 'task-final-r3117-handle-codec-core-catalog.log'
$ownerLine=[IO.File]::ReadLines($ownerLog) | Where-Object { $_.StartsWith('{"rust-build-meta"') } | Select-Object -First 1
$ownerCatalog=$ownerLine | ConvertFrom-Json -Depth 100
$fullPin=Get-Content -LiteralPath (Join-Path $taskEvidence 'task-handle-codec-reverse-executables-before.json') -Raw | ConvertFrom-Json
$ownerPin=Get-Content -LiteralPath (Join-Path $taskEvidence 'task-handle-codec-core-executables-after.json') -Raw | ConvertFrom-Json
$coreSuites=@($fullCatalog.'rust-suites'.PSObject.Properties | ForEach-Object {$_.Value} | Where-Object 'package-name' -eq 'arcweft-core')
$ownerSuites=@($ownerCatalog.'rust-suites'.PSObject.Properties | ForEach-Object {$_.Value} | Where-Object 'package-name' -eq 'arcweft-core')
if(-not $coreSuites.Count -or $coreSuites.Count -ne $ownerSuites.Count){throw 'Incomplete Core binary relation'}
$coreComparison=@(foreach($suite in $coreSuites){
 $old=@($ownerSuites | Where-Object 'binary-id' -eq $suite.'binary-id')
 if($old.Count -ne 1){throw 'Missing Core owner binary'}
 $oldBytes=@($ownerPin.binaries | Where-Object path -eq $old[0].'binary-path')
 $newBytes=@($fullPin.binaries | Where-Object path -eq $suite.'binary-path')
 if($oldBytes.Count -ne 1 -or $newBytes.Count -ne 1){throw 'Missing Core binary hash'}
 [ordered]@{binary=$suite.'binary-id';ownerPath=$oldBytes[0].path;integratedPath=$newBytes[0].path;ownerSha256=$oldBytes[0].sha256;integratedSha256=$newBytes[0].sha256;sameBytes=($oldBytes[0].sha256 -eq $newBytes[0].sha256 -and $oldBytes[0].bytes -eq $newBytes[0].bytes)}
})
$changed=@($coreComparison | Where-Object {-not $_.sameBytes})
$selector=if($changed.Count){'rdeps(=arcweft-core)'}else{'not(package(=arcweft-core))'}
[ordered]@{version=1;base=(& git rev-parse HEAD).Trim();coreBinaryComparison=$coreComparison;changedCoreBinaries=$changed.Count;runtimeSelector=$selector;reason=if($changed.Count){'Integrated dependency-feature closure produced different Core executables; execute Core again with the full consumer build.'}else{'All Core executables match the terminal owner run; reuse that exact Core evidence and execute disjoint consumers.'};sourceFreeze='task-handle-codec-repair.source-freeze.json';sourceInputs=3987} | ConvertTo-Json -Depth 7 | Set-Content -LiteralPath (Join-Path $taskEvidence 'task-handle-codec-runtime-selection.json') -Encoding utf8
[ordered]@{coreBinaries=$coreComparison.Count;changedCoreBinaries=$changed.Count;selector=$selector} | ConvertTo-Json -Compress
$packageArgs=@(foreach($package in $graph.reverseWorkspace){'-p';$package})
$featureArgs=@('--all-targets','--all-features','--locked')
& (Join-Path $taskEvidence 'catalog-preflight-run.ps1') -Name 'task-final-r3120-handle-codec-consumer-catalog' -CargoArguments (@('nextest','list')+$packageArgs+$featureArgs+@('--message-format','json','-E',$selector))
if($LASTEXITCODE -ne 0){exit $LASTEXITCODE}
& (Join-Path $taskEvidence 'inspect-presentation-catalog.ps1') -Name 'task-final-r3120-handle-codec-consumer-catalog'
$runtimeInventory=Get-Content -LiteralPath (Join-Path $taskEvidence 'task-final-r3120-handle-codec-consumer-catalog.inventory.json') -Raw | ConvertFrom-Json
$expected=@($fullInventory.rows | Where-Object { $_.status -eq 'matches' -and ($changed.Count -or $_.package -ne 'arcweft-core') } | ForEach-Object {$_.binary+[char]9+$_.test} | Sort-Object)
$observed=@($runtimeInventory.rows | Where-Object status -eq 'matches' | ForEach-Object {$_.binary+[char]9+$_.test} | Sort-Object)
if($expected.Count -ne $observed.Count -or @(Compare-Object $expected $observed).Count){throw 'Runtime catalog differs from its complete intended closure'}
$required=Get-Content -LiteralPath (Join-Path $taskEvidence 'task-handle-codec-required-cases.json') -Raw | ConvertFrom-Json
foreach($case in $required.cases){if(($case.binary+[char]9+$case.test) -notin $observed){throw ('Required accepted/owner case excluded: '+$case.test)}}
[ordered]@{matched=$observed.Count;required=$required.cases.Count;catalogSetDrift=0} | ConvertTo-Json -Compress
& (Join-Path $taskEvidence 'task-program-body-command.ps1') -Name 'task-final-r3121-handle-codec-consumer-runtime' -CargoArgs (@('nextest','run')+$packageArgs+$featureArgs+@('--no-tests','fail','--no-fail-fast','--status-level','all','--final-status-level','all','-E',$selector))
$runtimeExit=$LASTEXITCODE
& (Join-Path $taskEvidence 'audit-nextest-final-result.ps1') -Run 'task-final-r3121-handle-codec-consumer-runtime' -Catalog 'task-final-r3120-handle-codec-consumer-catalog' -Freeze 'task-handle-codec-repair'
& (Join-Path $taskEvidence 'pin-catalog-executables.ps1') -Catalog 'task-final-r3119-handle-codec-reverse-catalog' -Output 'task-handle-codec-reverse-executables-after.json' -Phase 'after-run'
exit $runtimeExit
