param([string]$Owner='presentation-activation-host-20261031.owner3')
$ErrorActionPreference='Stop'
Set-Location -LiteralPath 'D:/git/arcweft'
$taskTemp='C:/Users/sanze/AppData/Local/Temp/arcweft-1011-convergence-resume-cb798c1d'
$root='D:/git/arcweft/docs/implementation/acceptance-evidence'
$output=Join-Path $root 'task-presentation-activation-host-owner-validation.json'
if(Test-Path -LiteralPath $output){throw 'Evidence exists'}
if(@(Get-Process -Name cargo,cargo-nextest,rustc,link -ErrorAction SilentlyContinue).Count){throw 'Live Cargo handle'}
. "$taskTemp/convergence-image-git-bytes.ps1"
$quality=(Get-Content -LiteralPath "$taskTemp/product-execution-quality.active.txt" -Raw).Trim()
$terminal=Get-Content -LiteralPath "$taskTemp/$Owner.runtime.terminal-audit.json" -Raw|ConvertFrom-Json -Depth 20
if(-not $terminal.isPassingRun -or $terminal.executed -ne 2195 -or $terminal.skipped -ne 0 -or $terminal.sourceInputs -ne 4026){throw 'Unexpected owner terminal'}
$freeze=Get-Content -LiteralPath "$taskTemp/$Owner.source-freeze.json" -Raw|ConvertFrom-Json -Depth 20
$qualityFreeze=Get-Content -LiteralPath "$taskTemp/$quality.source-freeze.json" -Raw|ConvertFrom-Json -Depth 20
if(@(Compare-Object @($freeze.paths|ForEach-Object {$_.path+' '+$_.sha256}) @($qualityFreeze.paths|ForEach-Object {$_.path+' '+$_.sha256})).Count){throw 'Owner/quality source drift'}
foreach($row in $freeze.paths){if((Get-FileHash -LiteralPath $row.path).Hash.ToLowerInvariant() -ne $row.sha256){throw ('Source drift: '+$row.path)}}
$checks=@(foreach($kind in @('clippy','default-check','doctest','structure','fmt')){
    $receipt=Get-Content -LiteralPath "$taskTemp/$quality.$kind.json" -Raw|ConvertFrom-Json -Depth 20
    if($receipt.exitCode -ne 0 -or $receipt.sourceDrift -ne 0){throw 'Quality failed'}
    [ordered]@{kind=$kind;receipt=$receipt}
})
if(-not [IO.File]::ReadAllText("$taskTemp/$quality.doctest.log").Contains('78 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out')){throw 'Unexpected doctest summary'}
$publication=Get-Content -LiteralPath "$taskTemp/presentation-activation-host-source-20261031.verified.json" -Raw|ConvertFrom-Json -Depth 20
if((git rev-parse HEAD).Trim() -ne $publication.commit -or (git rev-parse origin/main).Trim() -ne $publication.commit){throw 'Publication differs'}
$metrics=@{}
foreach($line in [IO.File]::ReadLines("$taskTemp/$quality.structure.log")){
    if($line -match '^(workspace packages|files scanned|Rust files|Rust physical LOC|review triggers|blocking violations): (\d+)$'){$metrics[$Matches[1]]=[long]$Matches[2]}
}
if($metrics.Count -ne 6 -or $metrics['blocking violations'] -ne 0){throw 'Incomplete structure metrics'}
$paths=Get-Content -LiteralPath "$taskTemp/presentation-activation-host-source-20261031.owned.txt"
$review=[ordered]@{
    version=1;base=$publication.parent;inspectedMain=$publication.commit;heldTracked=14;heldUntracked=6
    rows=@(foreach($path in $paths){
        $old=Read-GitBytes @('show',($publication.parent+':'+$path));$now=[IO.File]::ReadAllBytes((Join-Path 'D:/git/arcweft' $path))
        [ordered]@{path=$path;owner='arcweft-core';classification=$(if($path -like '*/tests/*'){'test'}else{'production'});baseBytes=$old.Length;basePhysicalLoc=([Text.Encoding]::UTF8.GetString($old).Split("`n").Count-1);bytes=$now.Length;physicalLoc=([Text.Encoding]::UTF8.GetString($now).Split("`n").Count-1)}
    })
    disposition=@(
        'The existing Product activation checkpoint and original PresentationBanks own Host-failure retirement. The same candidate/VM terminal observation collector and line reconciliation are used as VM traps; no parallel unwind owner or persisted intent is introduced.'
        'PreparedHostResultOutcome owns its context-free failure classification. Deferred and activation consumers use that single rule. Activation commit only verifies the admitted terminal metadata and consumes the exact receipt after command/registry preflight; direct mark_trapped is deleted.'
        'Receipt publication remains the existing one-operation boundary. Host failure neither executes another instruction nor replenishes VM budget. Failure close occurs on the next activation turn, retaining the prior protocol timing and public terminal snapshot.'
        'line.rs remains the existing activation/line reconciliation owner. product_step.rs retains only the shared prepared Host outcome classification and its existing journal consumer. The test module shares actual Dialogue/LoadConst/Nop construction and adds a real HostCall/resume block under the admitted activation function. No public visibility, wire/Cargo edge, host I/O or unrelated owner state was added.'
    )
}
$review|ConvertTo-Json -Depth 12|Set-Content -LiteralPath "$taskTemp/presentation-activation-host-owner-review.json" -Encoding utf8
$proofs=@()
foreach($attempt in @('red1','red2')){
    $prior=Get-Content -LiteralPath "$taskTemp/presentation-activation-host-20261031.$attempt.source-freeze.json" -Raw|ConvertFrom-Json -Depth 20
    foreach($path in @('crates/arcweft-core/src/awbc/product_step/tests/presentation/terminal_requests.rs','crates/arcweft-core/src/awbc/product_step/tests/presentation/trap_lifecycle.rs')){
        $source="activation-host-red1-source/$path"
        if($attempt -eq 'red2' -and $path.EndsWith('/terminal_requests.rs')){$source='activation-host-red2.fixture-source.rs'}
        $row=$prior.paths|Where-Object path -eq $path
        $hash=(Get-FileHash -LiteralPath "$taskTemp/$source").Hash.ToLowerInvariant()
        if($hash -ne $row.sha256){throw 'Historical source drift'}
        $proofs+=[ordered]@{attempt=$attempt;path=$path;source=$source;bytes=$row.bytes;sha256=$hash;drift=0}
    }
}
$proofs|ConvertTo-Json -Depth 10|Set-Content -LiteralPath "$taskTemp/presentation-activation-host-source-proofs.json" -Encoding utf8
$sources=@('record-presentation-activation-host-evidence.ps1','run-presentation-trap-quality.ps1','presentation-activation-host-owner-review.json','presentation-activation-host-source-proofs.json','activation-host-red2.fixture-source.rs')
$sources+=@($proofs.source|Sort-Object -Unique)
$sources=@($sources|Sort-Object -Unique)
foreach($path in $paths){$source='activation-host-owner3-source/'+$path; $row=$freeze.paths|Where-Object path -eq $path; if((Get-FileHash -LiteralPath "$taskTemp/$source").Hash.ToLowerInvariant() -ne $row.sha256){throw 'Final source capture drift'}; $sources+=$source}
foreach($suffix in @('owned.txt','prepared.json','verified.json','reviewed.patch','publish.log')){$sources+="presentation-activation-host-source-20261031.$suffix"}
$attempts=@(foreach($attempt in @('red1','red2','owner3','owner4')){
    $stem="presentation-activation-host-20261031.$attempt"
    if(-not(Test-Path -LiteralPath "$taskTemp/$stem.source-freeze.json")){continue}
    $sources+="run-$stem.ps1"
    foreach($suffix in @('source-freeze.json','binaries-before.json','catalog.inventory.json','catalog.json','catalog.log','runtime.json','runtime.log','runtime.terminal-audit.json')){if(Test-Path -LiteralPath "$taskTemp/$stem.$suffix"){$sources+="$stem.$suffix"}}
    if(Test-Path -LiteralPath "$taskTemp/$stem.runtime.terminal-audit.json"){Get-Content -LiteralPath "$taskTemp/$stem.runtime.terminal-audit.json" -Raw|ConvertFrom-Json -Depth 20}
    else{[ordered]@{attempt=$attempt;catalog=(Get-Content -LiteralPath "$taskTemp/$stem.catalog.json" -Raw|ConvertFrom-Json);runtime='Not run: test fixture referred to ProductStepBudgets at the wrong private module path.'}}
})
foreach($suffix in @('source-freeze.json','clippy.json','clippy.log','default-check.json','default-check.log','doctest.json','doctest.log','structure.json','structure.log','fmt.json','fmt.log')){$sources+="$quality.$suffix"}
foreach($name in @('dependency_edges.csv','file_metrics.csv','findings.md','package_metrics.csv','public_type_duplicates.csv')){$sources+="$quality.structure-report/$name"}
$bytes=0L
$artifacts=@(foreach($source in $sources){
    $name='task-presentation-activation-host-raw.'+$source.Replace('/','.').Replace('\','.')
    $destination=Join-Path $root $name
    if(Test-Path -LiteralPath $destination){throw 'Refusing to overwrite raw evidence'}
    [IO.File]::Copy((Join-Path $taskTemp $source),$destination,$false)
    $hash=(Get-FileHash -LiteralPath $destination).Hash.ToLowerInvariant()
    if($hash -ne (Get-FileHash -LiteralPath (Join-Path $taskTemp $source)).Hash.ToLowerInvariant()){throw 'Raw byte drift'}
    $length=(Get-Item -LiteralPath $destination).Length;$bytes+=$length
    [ordered]@{path=$name;source=$source;bytes=$length;sha256=$hash}
})
$record=[ordered]@{
    version=1;recordedAt=[DateTime]::UtcNow.ToString('o');inspectedMain=$publication.commit
    scope='Actual Dialogue activation Host failure resource admission and exact receipt commit under its original checkpoint.'
    sourcePublication=$publication;workingTree=[ordered]@{heldTracked=14;heldUntracked=6;heldPatchSha256=$publication.heldPatchSha256;coreDirty=$false}
    owner=$terminal;ownerBinaryDrift=0;ownerQualitySourceDrift=0;quality=$checks;attempts=$attempts;sourceProvenance=$proofs
    attemptNotes=@('red1: catalog compile exit 101 (E0433); no runtime. The regression referenced the budget context through the wrong private module. Fixed only that path.','red2: 0 PASS / 1 FAIL / 2141 excluded. An actual Dialogue/HostCall capacity refusal returned successful activation progress instead of typed presentation refusal. Production still deferred direct mark_trapped until receipt consumption. The regression now passes through admitted bank retirement in the original activation checkpoint.','Final owner includes this one regression and all earlier accepted Core cases; focused counts are not added to it.')
    doctest=[ordered]@{passed=78;failed=0;ignored=0;filtered=0}
    structure=[ordered]@{packages=$metrics['workspace packages'];files=$metrics['files scanned'];rustFiles=$metrics['Rust files'];rustPhysicalLoc=$metrics['Rust physical LOC'];reviewTriggers=$metrics['review triggers'];blocking=$metrics['blocking violations']}
    accepted=@('Actual LoadConst/Dialogue/Nop/HostCall execution issues the activation and its exact suspended Host receipt. UnsupportedCapability, Rejected and Failed are exercised with both trusted-default and handles=1/scopes=8 policies.','Two bounded refusals retain the complete executor snapshot including bank, budget, activation revision, grant/body and exact inbound Host receipt. The actual withdrawn transaction is restored without failure close or receipt consumption.','With the same policy after modeled old-retirement acknowledgement, retry retires the original token once, then command/registry preflight precedes exact receipt consumption. Failure code/message and one-operation timing are preserved. Default public save/restore matches the complete terminal owner. The next activation turn closes without reissuing the Host call or retiring the token again.','Deferred and activation Host failures share the prepared outcome owner classification. Prior control failure, deferred terminal request, cancellation and VM/lifecycle cuts remain accepted.')
    notClaimedByThisRun=@('Every external packet family preserves an entire public outer step; the new negative regression exercises the actual activation transaction and its input vector','All unexpected VM/protocol-error terminal paths','Return/select/valid Goto escaping-handle ownership transfer','Native borrowed-parent contour, Source ViewValue/callables/mount/methods/driver, active-bank replacement, Native/Web image parity and final reverse/target/workspace matrix')
    currentStatusAuthority='../current-status.md';priorAcceptance=@('task-presentation-terminal-request-owner-validation.json','task-presentation-lifecycle-trap-owner-validation.json')
    acceptedFeaturesRetained='Generic Match and every prior accepted boundary remain accepted. Resource fixtures and modeled retirement queue release do not prove Source ViewValue or driver ABI.'
    warnings='Existing compiler/Clippy warnings remain in raw logs; exit zero is not zero warnings.'
    artifacts=$artifacts;artifactCount=$artifacts.Count;artifactBytes=$bytes;rawByteDrift=0
}
$record|ConvertTo-Json -Depth 40|Set-Content -LiteralPath $output -Encoding utf8
[ordered]@{record=$output;artifacts=$artifacts.Count;bytes=$bytes;owner=$terminal.passed;sourceInputs=$terminal.sourceInputs;quality=$quality;metrics=$metrics;fmt=($checks|Where-Object kind -eq 'fmt').receipt.paths.Count;review=$review.rows}|ConvertTo-Json -Depth 10 -Compress
