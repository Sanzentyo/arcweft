param([string]$Owner='presentation-trap-20261031.owner2',[string]$PublicationPrefix='presentation-trap-budget-source-20261031')
$ErrorActionPreference='Stop'
Set-Location -LiteralPath 'D:/git/arcweft'
$taskTemp='C:/Users/sanze/AppData/Local/Temp/arcweft-1011-convergence-resume-cb798c1d'
$root='D:/git/arcweft/docs/implementation/acceptance-evidence'
$output=Join-Path $root 'task-presentation-vm-trap-owner-validation.json'
if(Test-Path -LiteralPath $output){throw 'Evidence already exists'}
if(@(Get-Process -Name cargo,cargo-nextest,rustc,link -ErrorAction SilentlyContinue).Count){throw 'Live Cargo handle'}
$quality=(Get-Content -LiteralPath "$taskTemp/product-execution-quality.active.txt" -Raw).Trim()
$terminal=Get-Content -LiteralPath "$taskTemp/$Owner.runtime.terminal-audit.json" -Raw|ConvertFrom-Json -Depth 15
if(-not $terminal.isPassingRun -or $terminal.executed -ne 2183 -or $terminal.sourceInputs -ne 4023){throw 'Unexpected owner result'}
$checks=@(foreach($kind in @('clippy','default-check','doctest','structure','fmt')){
    $row=Get-Content -LiteralPath "$taskTemp/$quality.$kind.json" -Raw|ConvertFrom-Json -Depth 20
    if($row.exitCode -ne 0 -or $row.sourceDrift -ne 0){throw 'Quality failed'}
    [ordered]@{kind=$kind;receipt=$row}
})
$freeze=Get-Content -LiteralPath "$taskTemp/$Owner.source-freeze.json" -Raw|ConvertFrom-Json -Depth 20
$qualityFreeze=Get-Content -LiteralPath "$taskTemp/$quality.source-freeze.json" -Raw|ConvertFrom-Json -Depth 20
if(@(Compare-Object @($freeze.paths|ForEach-Object {$_.path+' '+$_.sha256}) @($qualityFreeze.paths|ForEach-Object {$_.path+' '+$_.sha256})).Count){throw 'Owner and quality inputs differ'}
foreach($row in $freeze.paths){if((Get-FileHash -LiteralPath $row.path).Hash.ToLowerInvariant() -ne $row.sha256){throw 'Source drift'}}
$publication=Get-Content -LiteralPath "$taskTemp/$PublicationPrefix.verified.json" -Raw|ConvertFrom-Json -Depth 15
if((git rev-parse HEAD).Trim() -ne $publication.commit){throw 'Unexpected inspected main'}
$initialPublication=Get-Content -LiteralPath "$taskTemp/presentation-trap-source-20261031.verified.json" -Raw|ConvertFrom-Json -Depth 15
$firstAttempt=Get-Content -LiteralPath "$taskTemp/presentation-trap-20261031.owner1.runtime.terminal-audit.json" -Raw|ConvertFrom-Json -Depth 15
if($firstAttempt.executed -ne 2183 -or $firstAttempt.passed -ne 2181 -or $firstAttempt.failed -ne 2){throw 'Unexpected original failure history'}
$metrics=@{}
foreach($line in [IO.File]::ReadLines("$taskTemp/$quality.structure.log")){
    if($line -match '^(workspace packages|files scanned|Rust files|Rust physical LOC|review triggers|blocking violations): (\d+)$'){$metrics[$Matches[1]]=[long]$Matches[2]}
}
if($metrics.Count -ne 6 -or $metrics['blocking violations'] -ne 0){throw 'Incomplete structure measurement'}
$sources=@('record-presentation-trap-evidence.ps1','presentation-trap-owner-review.json','run-presentation-trap-quality.ps1',"run-$Owner.ps1")
foreach($suffix in @('owned.txt','prepared.json','verified.json','reviewed.patch','publish.log')){$sources+="$PublicationPrefix.$suffix"}
foreach($suffix in @('owned.txt','prepared.json','verified.json','reviewed.patch','publish.log')){$sources+="presentation-trap-source-20261031.$suffix"}
foreach($suffix in @('source-freeze.json','binaries-before.json','catalog.inventory.json','catalog.json','catalog.log','runtime.json','runtime.log','runtime.terminal-audit.json')){$sources+="presentation-trap-20261031.owner1.$suffix"}
$sources+='run-presentation-trap-20261031.owner1.ps1'
foreach($suffix in @('source-freeze.json','binaries-before.json','catalog.inventory.json','catalog.json','catalog.log','runtime.json','runtime.log','runtime.terminal-audit.json')){$sources+="$Owner.$suffix"}
foreach($suffix in @('source-freeze.json','clippy.json','clippy.log','default-check.json','default-check.log','doctest.json','doctest.log','structure.json','structure.log','fmt.json','fmt.log')){$sources+="$quality.$suffix"}
foreach($name in @('dependency_edges.csv','file_metrics.csv','findings.md','package_metrics.csv','public_type_duplicates.csv')){$sources+="$quality.structure-report/$name"}
$cleanName='workspace-cargo-clean-20261031T210118273.log'
$sources+=@($cleanName,"$cleanName.result.json")
$clean=Get-Content -LiteralPath "$taskTemp/$cleanName.result.json" -Raw|ConvertFrom-Json
if($clean.exit -ne 0 -or $clean.target -ne 'D:\git\arcweft\target'){throw 'Clean receipt mismatch'}
$bytes=0L
$artifacts=@(foreach($source in $sources){
    $name='task-presentation-vm-trap-raw.'+$source.Replace('/','.')
    $destination=Join-Path $root $name
    if(Test-Path -LiteralPath $destination){throw 'Refusing to overwrite raw evidence'}
    [IO.File]::Copy((Join-Path $taskTemp $source),$destination,$false)
    $hash=(Get-FileHash -LiteralPath $destination).Hash.ToLowerInvariant()
    if($hash -ne (Get-FileHash -LiteralPath (Join-Path $taskTemp $source)).Hash.ToLowerInvariant()){throw 'Raw byte drift'}
    $length=(Get-Item -LiteralPath $destination).Length
    $bytes+=$length
    [ordered]@{path=$name;source=$source;bytes=$length;sha256=$hash}
})
$record=[ordered]@{
    version=1;recordedAt=[DateTime]::UtcNow.ToString('o');inspectedMain=$publication.commit
    scope='Actual AWBC VM trap unwind through borrowed execution resources; Product main custody refusal preserves its original complete continuation.'
    sourcePublication=$publication
    initialCheckpointPublication=$initialPublication
    workingTree=[ordered]@{heldTracked=14;heldUntracked=6;heldPatchSha256=$publication.heldPatchSha256;coreDirty=$false}
    clean=$clean
    cleanOutput='Removed 198113 files, 244.6GiB total; source and held WIP retained. This was the explicitly requested whole-workspace cargo clean, not an incremental-only deletion.'
    owner=$terminal;ownerBinaryDrift=0;ownerQualitySourceDrift=0;quality=$checks
    doctest=[ordered]@{passed=78;failed=0;ignored=0;filtered=0}
    structure=[ordered]@{packages=$metrics['workspace packages'];files=$metrics['files scanned'];rustFiles=$metrics['Rust files'];rustPhysicalLoc=$metrics['Rust physical LOC'];reviewTriggers=$metrics['review triggers'];blocking=$metrics['blocking violations'];disposition='The private VM execution resource context borrows the existing program context and original bank aggregate for one slice, without persisted ownership or a new ledger. Scope contour validation and retirement stay on the bank aggregate. Product main owns complete checkpoint restoration and typed custody refusal. First-in-slice budget replenishment now occurs inside that existing checkpoint and is not repeated by later instructions. The new focused test module covers trap routes, actual call/lexical identities, bounded refusal and checkpoint/save behavior; the accompanying file metrics retain actual physical LOC. No public API, Cargo edge, wire grammar or layer direction changed.'}
    accepted=@(
        'Explicit Trap, Unreachable, instruction-local SequenceGet bounds failure, and evaluation-error division by zero all use the same original-bank unwind before terminal mutation; the regression executes each through Product one-op steps and preserves its precise trap code.'
        'A real CallFunction and EnterScope create two actual frame identities and one lexical scope. Retirement order is callee lexical, callee root, caller root, with original Bool bodies and exact tokens retained in that order.'
        'The complete successful nested-frame snapshot survives public save/restore under the actual default policy. The original private execution checkpoint restores the complete pre-trap snapshot, including original program, body, grants, cursor, budget and queue.'
        'With handles=3/scopes=8, three live bodies and one old retirement exceed aggregate capacity. Product main refuses twice with a diagnostic and full snapshot equality, including instruction budget, without a second terminal transition. After modeling driver acknowledgement of the old queue item, the identical instruction retries under the unchanged policy and retires all three bodies once.'
        'A standalone VM without a borrowed bank rejects actual outstanding grants with typed MissingLedger; it leaves all three grants and live bodies intact. Its original Product checkpoint also restores consumed instruction budget; standalone stepping itself is not claimed to own that transaction.'
    )
    warnings='Compiler and Clippy warnings remain in raw logs. Successful exit is not a zero-warning claim.'
    attempts=@(
        [ordered]@{result=$firstAttempt;diagnosis='The four-route test incorrectly indexed default Unit as UInt. The bounded public-step test exposed pre-checkpoint budget replenishment: refused state remaining=64 differed from original remaining=63. Corrected typed row references and moved first-in-slice replenishment inside the original checkpoint; kept the full snapshot assertions.'}
        [ordered]@{result=$terminal}
    )
    priorAcceptance=@('task-presentation-await-cancellation-owner-validation.json','task-presentation-line-cancellation-owner-validation.json','task-presentation-lexical-vm-owner-validation.json','task-product-execution-checkpoint-owner-validation.json')
    currentStatusAuthority='../current-status.md'
    notClaimedByThisRun=@('Frame return/select/goto and escaping-handle ownership transfer','Product lifecycle traps outside the VM and complete child/Dialogue refusal integration','Native retained contour including the actual borrowed parent','Source ViewValue construction and eleven callables/mount/six methods/lowering/driver','Active-bank replacement and Native/Web image parity','Final reverse-dependency, feature/target and workspace acceptance matrix')
    acceptedFeaturesRetained='Generic Match and all prior accepted checkpoints, lexical exits and cancellation cuts remain accepted. Whole-goal incompleteness does not reopen them. Bool resource bodies are custody evidence, not Source ViewValue or driver ABI evidence.'
    artifacts=$artifacts;artifactCount=$artifacts.Count;artifactBytes=$bytes;rawByteDrift=0
}
$record|ConvertTo-Json -Depth 35|Set-Content -LiteralPath $output -Encoding utf8
[ordered]@{record=$output;artifacts=$artifacts.Count;bytes=$bytes;owner=$terminal.passed;sourceInputs=$terminal.sourceInputs;quality=$quality;metrics=$metrics}|ConvertTo-Json -Depth 8 -Compress
