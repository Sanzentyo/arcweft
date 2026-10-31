param([string]$Owner='presentation-lifecycle-trap-20261031.owner9',[string]$PublicationPrefix='presentation-lifecycle-trap-repair-source-20261031')
$ErrorActionPreference='Stop'
Set-Location -LiteralPath 'D:/git/arcweft'
$taskTemp='C:/Users/sanze/AppData/Local/Temp/arcweft-1011-convergence-resume-cb798c1d'
$root='D:/git/arcweft/docs/implementation/acceptance-evidence'
$output=Join-Path $root 'task-presentation-lifecycle-trap-owner-validation.json'
if(Test-Path -LiteralPath $output){throw 'Evidence already exists'}
if(@(Get-Process -Name cargo,cargo-nextest,rustc,link -ErrorAction SilentlyContinue).Count){throw 'Live Cargo handle'}
$quality=(Get-Content -LiteralPath "$taskTemp/product-execution-quality.active.txt" -Raw).Trim()
$terminal=Get-Content -LiteralPath "$taskTemp/$Owner.runtime.terminal-audit.json" -Raw|ConvertFrom-Json -Depth 15
if(-not $terminal.isPassingRun -or $terminal.executed -ne 2189 -or $terminal.sourceInputs -ne 4025){throw 'Unexpected owner result'}
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
$metrics=@{}
foreach($line in [IO.File]::ReadLines("$taskTemp/$quality.structure.log")){
    if($line -match '^(workspace packages|files scanned|Rust files|Rust physical LOC|review triggers|blocking violations): (\d+)$'){$metrics[$Matches[1]]=[long]$Matches[2]}
}
if($metrics.Count -ne 6 -or $metrics['blocking violations'] -ne 0){throw 'Incomplete structure measurement'}
$sources=@('record-presentation-lifecycle-trap-evidence.ps1','presentation-lifecycle-trap-owner-review.json','run-presentation-trap-quality.ps1','presentation-lifecycle-trap-fixture-history.json','presentation-lifecycle-trap-owner2.fixture-source.rs','presentation-lifecycle-trap-owner3.fixture-source.rs','presentation-lifecycle-trap-focus4.fixture-source.rs','presentation-lifecycle-trap-focus5.fixture-source.rs','presentation-lifecycle-trap-focus6.fixture-source.rs')
$sources+=@('presentation-lifecycle-trap-focus7.fixture-source.rs','presentation-lifecycle-trap-focus7.candidate.patch','presentation-lifecycle-trap-focus8.fixture-source.rs')
foreach($prefix in @('presentation-lifecycle-trap-source-20261031','presentation-lifecycle-trap-repair-source-20261031')){
    if(Test-Path -LiteralPath "$taskTemp/$prefix.verified.json"){
        foreach($suffix in @('owned.txt','prepared.json','verified.json','reviewed.patch','publish.log')){$sources+="$prefix.$suffix"}
    }
}
$attempts=@(foreach($attempt in 1..9){
    $stem="presentation-lifecycle-trap-20261031.owner$attempt"
    if(Test-Path -LiteralPath "$taskTemp/$stem.runtime.terminal-audit.json"){
        $sources+="run-$stem.ps1"
        foreach($suffix in @('source-freeze.json','binaries-before.json','catalog.inventory.json','catalog.json','catalog.log','runtime.json','runtime.log','runtime.terminal-audit.json')){$sources+="$stem.$suffix"}
        Get-Content -LiteralPath "$taskTemp/$stem.runtime.terminal-audit.json" -Raw|ConvertFrom-Json -Depth 15
    }
})
$focusAttempts=@(foreach($attempt in 4..9){
    $stem="presentation-lifecycle-trap-20261031.focus$attempt"
    if(Test-Path -LiteralPath "$taskTemp/$stem.runtime.terminal-audit.json"){
        $sources+="run-$stem.ps1"
        foreach($suffix in @('source-freeze.json','binaries-before.json','catalog.inventory.json','catalog.json','catalog.log','runtime.json','runtime.log','runtime.terminal-audit.json')){$sources+="$stem.$suffix"}
        Get-Content -LiteralPath "$taskTemp/$stem.runtime.terminal-audit.json" -Raw|ConvertFrom-Json -Depth 15
    }
})
foreach($suffix in @('source-freeze.json','clippy.json','clippy.log','default-check.json','default-check.log','doctest.json','doctest.log','structure.json','structure.log','fmt.json','fmt.log')){$sources+="$quality.$suffix"}
foreach($name in @('dependency_edges.csv','file_metrics.csv','findings.md','package_metrics.csv','public_type_duplicates.csv')){$sources+="$quality.structure-report/$name"}
$bytes=0L
$artifacts=@(foreach($source in $sources){
    $name='task-presentation-lifecycle-trap-raw.'+$source.Replace('/','.')
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
    scope='Product main lifecycle trap custody, actual independent-child VM custody refusal rollback and actual Dialogue activation VM refusal without failure close.'
    sourcePublication=$publication
    initialCheckpointPublication=(Get-Content -LiteralPath "$taskTemp/presentation-lifecycle-trap-source-20261031.verified.json" -Raw|ConvertFrom-Json -Depth 15)
    workingTree=[ordered]@{heldTracked=14;heldUntracked=6;heldPatchSha256=$publication.heldPatchSha256;coreDirty=$false}
    owner=$terminal;ownerBinaryDrift=0;ownerQualitySourceDrift=0;quality=$checks;attempts=$attempts;focusAttempts=$focusAttempts
    attemptNotes=@('owner1: 2187 PASS / 1 new fixture FAIL. The root dialogue target was set only after admission; verifier correctly rejected uninitialized register 0. Replaced runtime seeding with the actual typed opaque LoadConst instruction.','owner2: 2187 PASS / 1 new fixture FAIL. The self-resume edge was a loop without a loop safe point. Added a genuine separate Root resume/Return block and migrated actual function block ranges. Loop checks and complete snapshot assertions were retained.','owner3: 2187 PASS / 1 new fixture FAIL. The old private-helper content label entry.main was not a valid production line ID. Migrated the fixture to a canonical line.* content ID.','focus4: 0 PASS / 1 FAIL / 2134 excluded. Appending that ID broke strict canonical string-table ordering. Reused the fixture String row in sorted order without relaxing the verifier. Source bytes for these unpublished fixture epochs match their original source-freeze SHA256 receipts.')
    additionalAttemptNotes=@('focus5: 0 PASS / 1 FAIL / 2134 excluded. Registration made the activation visible at cursor 0 before Nop ran. The fixture now waits for the exact cursor 1 after actual Nop execution; its assertion, 8-step construction bound and one-op runtime budget remain unchanged.')
    productionFailureNotes=@('focus6: 0 PASS / 1 FAIL / 2134 excluded. Valid refusal preserved banks/grants/cursors but spent two Dialogue revisions (4 to 6) and replenished budget (63 to 64). Removed the old ingress budget commit; one host-step budget context now replenishes inside the actual execution checkpoint. Zero elapsed time still authenticates Duration suspension but restores its unchanged transaction without spending a revision.','focus7: 0 PASS / 1 FAIL / 2134 excluded. Full refusal snapshot assertions passed, but successful retry had no retirement: the observation collector treated a valid VM Trap as a collection error and restored its checkpoint. Terminal Trap now returns typed progress with its admitted cleanup batch to failure-close preparation.','focus8: all six targeted lifecycle cases passed; 2130 library cases excluded. This focused result is not added to the final full Core run.')
    doctest=[ordered]@{passed=78;failed=0;ignored=0;filtered=0}
    structure=[ordered]@{packages=$metrics['workspace packages'];files=$metrics['files scanned'];rustFiles=$metrics['Rust files'];rustPhysicalLoc=$metrics['Rust physical LOC'];reviewTriggers=$metrics['review triggers'];blocking=$metrics['blocking violations'];disposition='The admitted program supplies the general unwind contour for cancellation and traps. The bank aggregate remains the only scope retirement authority. Product lifecycle checks its actual contour before committing AwaitMany dialogue reconciliation. Product VM transaction owners classify typed presentation refusal and restore existing checkpoints/journals/actual Dialogue transactions before returning diagnostics. No second owner, fallback ledger, wire shape, Cargo edge or public production API was added. The new focused lifecycle regression module and reused nested-program fixture are reviewed in the raw owner review.'}
    accepted=@(
        'The Product main lifecycle termination helper preflights legacy cleanup inputs and the whole original scope contour before reconciliation commands, then retires actual grants/bodies before cleanup dispatch and terminal marking. Three real nested scopes retire in unwind order exactly once, with public save/restore equality under default policy.'
        'With handles=3/scopes=8, two refused lifecycle trap attempts preserve the complete snapshot, all three grants and the old queue item, and emit no line command or terminal flow event. The unchanged policy permits retry after modeling acknowledgement of the old queue item. Missing bank custody refuses with MissingLedger and retains all original grants.'
        'An actual independent child uses the executor-allocated identity and real EnterScope instruction. A VM trap capacity refusal restores the selected child, queue and original pre-replenishment instruction budget twice; the child is not marked trapped or removed. Under the same handles=1/scopes=8 policy, retry retires the original token once after the old queue item is acknowledged.'
        'An actual typed opaque LoadConst and Dialogue terminator start the activation; one real Nop instruction parks the initializer immediately before Trap. A genuine Root resume/Return block avoids a fabricated loop. The issued activation bank owns the real grant. VM capacity refusal preserves the complete executor snapshot twice and emits no close commands; the real withdrawn transaction is restored instead of entering Dialogue failure close. Same-policy retry retires the exact token.'
        'The one-host-step budget context retains only replenishment requests and never enters persisted state. Main and activation quanta replenish at most once, inside their original execution checkpoints. A valid terminal activation Trap carries its cleanup execution batch into failure close instead of being rolled back as a failed observation.'
        'Two zero-operation public Dialogue steps preserve the complete snapshot, budget, revision, actual grant and body. Under the default trusted policy the actual active Dialogue resource snapshot round-trips public save/restore without losing custody.'
        'All original cancellation consumers now use the same general admitted-program unwind API. Existing accepted VM trap routes, checkpoint, lexical exit and cancellation regressions remain in the full Core owner run.'
    )
    currentStatusAuthority='../current-status.md'
    notClaimedByThisRun=@('All externally supplied failure packets preserve a complete outer Product step; lifecycle helper admission alone is not that broader claim','Manual deferred-child Need/Host/AwaitMany trap marking outside the VM and all child-owner families','Frame return/select/goto and escaping-handle ownership transfer','Native retained contour including the actual borrowed parent','Source ViewValue and eleven callables/mount/six methods/lowering/driver','Active-bank replacement, Native/Web image parity and final reverse-dependency/feature/target/workspace matrix')
    warnings='Existing compiler and Clippy warnings remain in raw logs; exit zero is not a zero-warning claim.'
    priorAcceptance=@('task-presentation-vm-trap-owner-validation.json','task-presentation-await-cancellation-owner-validation.json','task-product-execution-checkpoint-owner-validation.json')
    acceptedFeaturesRetained='Generic Match and all prior accepted cuts remain accepted. Whole-goal incompleteness does not reopen them. Bool bodies and modeled queue acknowledgement do not establish Source ViewValue or real driver ABI.'
    artifacts=$artifacts;artifactCount=$artifacts.Count;artifactBytes=$bytes;rawByteDrift=0
}
$record|ConvertTo-Json -Depth 35|Set-Content -LiteralPath $output -Encoding utf8
[ordered]@{record=$output;artifacts=$artifacts.Count;bytes=$bytes;owner=$terminal.passed;sourceInputs=$terminal.sourceInputs;quality=$quality;metrics=$metrics}|ConvertTo-Json -Depth 8 -Compress
