$Phase='green'
$ErrorActionPreference='Stop'
Set-Location -LiteralPath 'D:/git/arcweft'
$arcTemp='C:/Users/sanze/AppData/Local/Temp/arcweft-1011-convergence-resume-cb798c1d'
$arcStem='presentation-trap-20261031.owner1'
if(Test-Path -LiteralPath "$arcTemp/$arcStem.source-freeze.json"){throw 'Bank attempt exists'}
$arcRecovery=Get-Content -LiteralPath "$arcTemp/presentation-awbc-scopes.recovery-reverse-matrix.json" -Raw|ConvertFrom-Json -Depth 30
if($arcRecovery.status -ne 'terminal'){throw 'Source-frozen consumer recovery is still live or unclosed'}
if(@(Get-Process -Name cargo,cargo-nextest,rustc,link -ErrorAction SilentlyContinue).Count){throw 'Another Cargo/rustc/link handle is live'}
$arcTemplate=Get-Content -LiteralPath "$arcTemp/presentation-banks-product-connect-20261031T080632957.owner1.inputs.json" -Raw|ConvertFrom-Json -Depth 20
$arcHeld=Get-Content -LiteralPath "$arcTemp/checkpoint-task-view.source-freeze.json" -Raw|ConvertFrom-Json -Depth 20
$arcFreeze=[ordered]@{version=1;base=(git rev-parse HEAD).Trim();phase=$Phase;recordedAt=[DateTime]::UtcNow.ToString('o');held=$arcHeld.held;paths=@(foreach($arcInput in (@($arcTemplate.paths) + @(git ls-files -- crates/arcweft-core/src/awbc/product_step/presentation.rs crates/arcweft-core/src/presentation/resources/execution.rs crates/arcweft-core/src/presentation/resources/execution/tests.rs crates/arcweft-core/src/line_task/activation/contour.rs crates/arcweft-core/src/line_task/activation/contour/tests.rs | ForEach-Object { [pscustomobject]@{path=$_} }) + @(git ls-files -- crates/arcweft-core/src/awbc/product_step/checkpoint.rs | ForEach-Object { [pscustomobject]@{path=$_} }))){[ordered]@{path=$arcInput.path;bytes=(Get-Item -LiteralPath $arcInput.path).Length;sha256=(Get-FileHash -LiteralPath $arcInput.path).Hash.ToLowerInvariant()}})}
$arcAdditional=@('crates/arcweft-core/src/awbc/product_step/tests/presentation/cancellation_resume.rs','crates/arcweft-core/src/awbc/product_step/tests/presentation/fanout-cancellation.awbc','crates/arcweft-core/src/awbc/product_step/tests/presentation/fanout-need-cancellation.awbc','crates/arcweft-core/src/awbc/vm/resources.rs','crates/arcweft-core/src/awbc/product_step/tests/presentation/traps.rs')
$arcFreeze.paths+=@(foreach($arcPath in $arcAdditional){[ordered]@{path=$arcPath;bytes=(Get-Item -LiteralPath $arcPath).Length;sha256=(Get-FileHash -LiteralPath $arcPath).Hash.ToLowerInvariant()}})
if($arcFreeze.paths.Count -ne 4023 -or @($arcFreeze.paths|ForEach-Object {$_.path}|Group-Object|Where-Object Count -ne 1).Count){throw 'Incomplete or duplicate expanded source input set'}
$arcFreeze|ConvertTo-Json -Depth 15|Set-Content -LiteralPath "$arcTemp/$arcStem.source-freeze.json" -Encoding utf8
function Assert-BankInputs {
 if(@($arcFreeze.paths|Where-Object {(Get-FileHash -LiteralPath $_.path).Hash.ToLowerInvariant() -ne $_.sha256}).Count){throw 'Bank input drift'}
}
function Invoke-BankCargo([string]$arcName,[object[]]$arcArguments){
 $arcStarted=[DateTime]::UtcNow.ToString('o')
 & cargo @arcArguments *> "$arcTemp/$arcName.log"
 $arcExit=$LASTEXITCODE
 Assert-BankInputs
 [ordered]@{version=1;base=$arcFreeze.base;command=$arcArguments;log="$arcTemp/$arcName.log";exitCode=$arcExit;startedAt=$arcStarted;finishedAt=[DateTime]::UtcNow.ToString('o')}|ConvertTo-Json -Depth 8|Set-Content -LiteralPath "$arcTemp/$arcName.json" -Encoding utf8
 return $arcExit
}
$arcSelection=@('-p','arcweft-core','--all-features','--locked')
if($Phase -eq 'red'){$arcSelection+=@('--lib','-E','test(live_and_inert_scope_headers_observe_identical_metadata_bank_boundaries)')}
$arcCatalogName=$arcStem+'.catalog'
$arcCatalogExit=Invoke-BankCargo $arcCatalogName (@('nextest','list')+$arcSelection+@('--message-format','json'))
if($arcCatalogExit -ne 0){throw 'Bank catalog failed'}
$arcLine=[IO.File]::ReadLines("$arcTemp/$arcCatalogName.log")|Where-Object {$_.StartsWith('{"rust-build-meta"')}|Select-Object -Last 1
$arcCatalog=$arcLine|ConvertFrom-Json -Depth 100
$arcCases=@(foreach($arcSuite in $arcCatalog.'rust-suites'.PSObject.Properties){foreach($arcCase in $arcSuite.Value.testcases.PSObject.Properties){[pscustomobject]@{package=$arcSuite.Value.'package-name';binary=$arcSuite.Value.'binary-id';test=$arcCase.Name;ignored=$arcCase.Value.ignored;status=$(if($arcCase.Value.ignored){'ignored'}else{$arcCase.Value.'filter-match'.status})}}})
[ordered]@{rows=$arcCases;excluded=@($arcCases|Where-Object status -ne 'matches').Count}|ConvertTo-Json -Depth 10|Set-Content -LiteralPath "$arcTemp/$arcCatalogName.inventory.json" -Encoding utf8
$arcExpected=2183
if(@($arcCases|Where-Object status -eq 'matches').Count -ne $arcExpected){throw 'Unexpected bank case selection'}
$arcBinaries=@(foreach($arcSuite in $arcCatalog.'rust-suites'.PSObject.Properties){$arcPath=$arcSuite.Value.'binary-path';[ordered]@{binary=$arcSuite.Value.'binary-id';path=$arcPath;sha256=(Get-FileHash -LiteralPath $arcPath).Hash.ToLowerInvariant()}})
$arcBinaries|ConvertTo-Json -Depth 8|Set-Content -LiteralPath "$arcTemp/$arcStem.binaries-before.json" -Encoding utf8
$arcRuntimeName=$arcStem+'.runtime'
$arcExit=Invoke-BankCargo $arcRuntimeName (@('nextest','run')+$arcSelection+@('--no-tests','fail','--no-fail-fast','--status-level','all','--final-status-level','all'))
if($arcExit -notin @(0,100)){throw 'Bank runtime has no valid test terminal'}
if(@($arcBinaries|Where-Object {(Get-FileHash -LiteralPath $_.path).Hash.ToLowerInvariant() -ne $_.sha256}).Count){throw 'Bank executable drift'}
& "$arcTemp/audit-nextest-final-result.ps1" -Run $arcRuntimeName -Catalog $arcCatalogName -Freeze $arcStem
exit $arcExit
