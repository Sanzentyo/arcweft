$ErrorActionPreference='Stop'
Set-Location -LiteralPath 'D:/git/arcweft'
$taskTemp='C:/Users/sanze/AppData/Local/Temp/arcweft-1011-convergence-resume-cb798c1d'
$stem='presentation-dialogue-receipt-reverse-20261101'
& "$taskTemp/assert-no-arcweft-build.ps1"
if(Test-Path -LiteralPath "$taskTemp/$stem.source-freeze.json"){throw 'Reverse epoch already exists'}
$quality=[IO.File]::ReadAllText("$taskTemp/product-execution-quality.active.txt")
$freeze=Get-Content -LiteralPath "$taskTemp/$quality.source-freeze.json" -Raw|ConvertFrom-Json -Depth 30
function Assert-Inputs {
    foreach($row in $freeze.paths){if((Get-FileHash -LiteralPath $row.path).Hash.ToLowerInvariant() -ne $row.sha256){throw ('Reverse source drift: '+$row.path)}}
}
Assert-Inputs
$freeze.base=(git rev-parse HEAD).Trim()
$freeze|ConvertTo-Json -Depth 30|Set-Content -LiteralPath "$taskTemp/$stem.source-freeze.json" -Encoding utf8
function Invoke-Recorded([string]$name,[object[]]$arguments) {
    if(Test-Path -LiteralPath "$taskTemp/$name.log"){throw 'Attempt already exists'}
    Assert-Inputs
    $started=[DateTime]::UtcNow.ToString('o')
    & cargo @arguments *> "$taskTemp/$name.log"
    $code=$LASTEXITCODE
    Assert-Inputs
    [ordered]@{version=1;base=$freeze.base;command=$arguments;log="$taskTemp/$name.log";exitCode=$code;sourceInputs=$freeze.paths.Count;sourceDrift=0;startedAt=$started;finishedAt=[DateTime]::UtcNow.ToString('o')}|ConvertTo-Json -Depth 10|Set-Content -LiteralPath "$taskTemp/$name.json" -Encoding utf8
    return $code
}
$metaExit=Invoke-Recorded "$stem.metadata" @('metadata','--format-version','1','--all-features','--locked')
if($metaExit -ne 0){throw 'Resolved metadata unavailable'}
$metadataLine=[IO.File]::ReadLines("$taskTemp/$stem.metadata.log")|Where-Object {$_.StartsWith('{"packages"')}|Select-Object -Last 1
if(-not $metadataLine){throw 'Cargo metadata JSON missing'}
$metadata=$metadataLine|ConvertFrom-Json -Depth 100
$workspace=[Collections.Generic.HashSet[string]]::new([string[]]$metadata.workspace_members)
$names=@{};foreach($p in $metadata.packages){$names[$p.id]=$p.name}
$root=@($metadata.packages|Where-Object {$_.name -eq 'arcweft-core' -and $workspace.Contains($_.id)})
if($root.Count -ne 1){throw 'Expected one actual Core package'}
$reverse=[Collections.Generic.HashSet[string]]::new();[void]$reverse.Add($root[0].id)
do {
    $added=$false
    foreach($node in $metadata.resolve.nodes){if(@($node.dependencies|Where-Object {$reverse.Contains($_)}).Count -and $reverse.Add($node.id)){$added=$true}}
} while($added)
$packages=@($reverse|Where-Object {$workspace.Contains($_)}|ForEach-Object {$names[$_]}|Sort-Object)
if('arcweft-cli' -notin $packages){throw 'Core reverse closure unexpectedly omits CLI'}
$graph=[ordered]@{version=1;base=$freeze.base;root=$root[0].id;workspacePackages=$workspace.Count;reversePackages=$packages;selection='actual Cargo resolve transitive reverse closure, all features; repository test-workspace library/integration/CLI surfaces';sourceInputs=$freeze.paths.Count;sourceDrift=0}
$graph|ConvertTo-Json -Depth 12|Set-Content -LiteralPath "$taskTemp/$stem.graph.json" -Encoding utf8
$groups=@(
    [pscustomobject]@{name='libraries';selection=@('--lib','--tests')+@($packages|Where-Object {$_ -ne 'arcweft-cli'}|ForEach-Object {'-p';$_})},
    [pscustomobject]@{name='cli-unit';selection=@('-p','arcweft-cli','--lib','--bins')},
    [pscustomobject]@{name='cli-integration';selection=@('-p','arcweft-cli','--test','runtime_native_options','--test','check_core_cli','--test','native_style_parity_sample','--test','release_trust_json','--test','arcw_fixtures_check_run','--test','seq04_8_4_persistent_cache_build_cli_goldens')}
)
$rows=[Collections.Generic.List[object]]::new()
$state=[ordered]@{version=1;base=$freeze.base;status='running';sourceFreeze="$stem.source-freeze.json";graph="$stem.graph.json";currentGroup='';stage='';rows=@();startedAt=[DateTime]::UtcNow.ToString('o')}
function Save-State { $state.rows=$rows.ToArray();$state|ConvertTo-Json -Depth 30|Set-Content -LiteralPath "$taskTemp/$stem.matrix.json" -Encoding utf8 }
foreach($group in $groups) {
    $state.currentGroup=$group.name;$state.stage='catalog';Save-State
    $name="$stem.$($group.name)"
    $selection=@($group.selection)+@('--all-features','--locked','-E','rdeps(=arcweft-core)')
    $catalog=Invoke-Recorded "$name.catalog" (@('nextest','list')+$selection+@('--message-format','json'))
    if($catalog -ne 0){$rows.Add([ordered]@{group=$group.name;catalogExit=$catalog;runtime='not run'});Save-State;continue}
    $line=[IO.File]::ReadLines("$taskTemp/$name.catalog.log")|Where-Object {$_.StartsWith('{"rust-build-meta"')}|Select-Object -Last 1
    if(-not $line){throw 'Nextest catalog JSON missing'}
    $catalogJson=$line|ConvertFrom-Json -Depth 100
    $cases=@(foreach($suite in $catalogJson.'rust-suites'.PSObject.Properties){foreach($case in $suite.Value.testcases.PSObject.Properties){[pscustomobject]@{package=$suite.Value.'package-name';binary=$suite.Value.'binary-id';test=$case.Name;ignored=$case.Value.ignored;status=$(if($case.Value.ignored){'ignored'}else{$case.Value.'filter-match'.status})}}})
    $inventory=[ordered]@{version=1;name="$name.catalog";all=$cases.Count;matched=@($cases|Where-Object status -eq 'matches').Count;excluded=@($cases|Where-Object status -ne 'matches').Count;rows=$cases}
    if(-not $inventory.matched){throw 'Empty reverse selection'}
    $inventory|ConvertTo-Json -Depth 15|Set-Content -LiteralPath "$taskTemp/$name.catalog.inventory.json" -Encoding utf8
    $unexpected=@($cases|Where-Object {$_.status -ne 'matches' -and -not $_.ignored})
    if($unexpected.Count){throw 'Unexpected nonignored reverse exclusions'}
    $pins=@(foreach($suite in $catalogJson.'rust-suites'.PSObject.Properties){$path=$suite.Value.'binary-path';[ordered]@{binary=$suite.Value.'binary-id';path=$path;sha256=(Get-FileHash -LiteralPath $path).Hash.ToLowerInvariant()}})
    $pins|ConvertTo-Json -Depth 8|Set-Content -LiteralPath "$taskTemp/$name.binaries-before.json" -Encoding utf8
    $state.stage='runtime';Save-State
    $runtime=Invoke-Recorded "$name.runtime" (@('nextest','run')+$selection+@('--no-tests','fail','--no-fail-fast','--status-level','none','--final-status-level','all'))
    if(@($pins|Where-Object {(Get-FileHash -LiteralPath $_.path).Hash.ToLowerInvariant() -ne $_.sha256}).Count){throw 'Reverse executable drift'}
    if($runtime -in @(0,100)) {
        & "$taskTemp/audit-nextest-final-result.ps1" -Run "$name.runtime" -Catalog "$name.catalog" -Freeze $stem *> "$taskTemp/$name.audit.log"
        $terminal=Get-Content -LiteralPath "$taskTemp/$name.runtime.terminal-audit.json" -Raw|ConvertFrom-Json -Depth 20
        $rows.Add([ordered]@{group=$group.name;catalogExit=$catalog;runtimeExit=$runtime;terminal=$terminal})
        Write-Output "$($group.name): $($terminal.passed) passed, $($terminal.failed) failed, $($terminal.skipped) skipped"
    } else {$rows.Add([ordered]@{group=$group.name;catalogExit=$catalog;runtimeExit=$runtime;terminal=$null})}
    Save-State
}
Assert-Inputs
$state.status='terminal';$state.stage='closed';$state.finishedAt=[DateTime]::UtcNow.ToString('o');Save-State
Write-Output 'reverse matrix terminal'
