$ErrorActionPreference='Stop'
Set-Location -LiteralPath 'D:/git/arcweft'
$taskTemp='C:/Users/sanze/AppData/Local/Temp/arcweft-1011-convergence-resume-cb798c1d'
$root='D:/git/arcweft/docs/implementation/acceptance-evidence'
$output=Join-Path $root 'task-presentation-terminal-request-owner-validation.json'
if(Test-Path -LiteralPath $output){throw 'Evidence already exists'}
if(@(Get-Process -Name cargo,cargo-nextest,rustc,link -ErrorAction SilentlyContinue).Count){throw 'Live Cargo handle'}
. "$taskTemp/convergence-image-git-bytes.ps1"
$owner='presentation-terminal-request-20261031.owner4'
$quality=(Get-Content -LiteralPath "$taskTemp/product-execution-quality.active.txt" -Raw).Trim()
$terminal=Get-Content -LiteralPath "$taskTemp/$owner.runtime.terminal-audit.json" -Raw|ConvertFrom-Json -Depth 20
if(-not $terminal.isPassingRun -or $terminal.executed -ne 2194 -or $terminal.skipped -ne 0 -or $terminal.sourceInputs -ne 4026){throw 'Unexpected owner terminal'}
$freeze=Get-Content -LiteralPath "$taskTemp/$owner.source-freeze.json" -Raw|ConvertFrom-Json -Depth 20
$qualityFreeze=Get-Content -LiteralPath "$taskTemp/$quality.source-freeze.json" -Raw|ConvertFrom-Json -Depth 20
if(@(Compare-Object @($freeze.paths|ForEach-Object {$_.path+' '+$_.sha256}) @($qualityFreeze.paths|ForEach-Object {$_.path+' '+$_.sha256})).Count){throw 'Owner/quality source drift'}
foreach($row in $freeze.paths){if((Get-FileHash -LiteralPath $row.path).Hash.ToLowerInvariant() -ne $row.sha256){throw ('Current source drift: '+$row.path)}}
$checks=@(foreach($kind in @('clippy','default-check','doctest','structure','fmt')){
    $receipt=Get-Content -LiteralPath "$taskTemp/$quality.$kind.json" -Raw|ConvertFrom-Json -Depth 20
    if($receipt.exitCode -ne 0 -or $receipt.sourceDrift -ne 0){throw 'Failed quality'}
    [ordered]@{kind=$kind;receipt=$receipt}
})
if(-not [IO.File]::ReadAllText("$taskTemp/$quality.doctest.log").Contains('78 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out')){throw 'Unexpected doctest summary'}
$publication=Get-Content -LiteralPath "$taskTemp/presentation-terminal-request-repair-source-20261031.verified.json" -Raw|ConvertFrom-Json -Depth 20
$initial=Get-Content -LiteralPath "$taskTemp/presentation-terminal-request-source-20261031.verified.json" -Raw|ConvertFrom-Json -Depth 20
if((git rev-parse HEAD).Trim() -ne $publication.commit -or (git rev-parse origin/main).Trim() -ne $publication.commit){throw 'Publication differs'}
$metrics=@{}
foreach($line in [IO.File]::ReadLines("$taskTemp/$quality.structure.log")){
    if($line -match '^(workspace packages|files scanned|Rust files|Rust physical LOC|review triggers|blocking violations): (\d+)$'){$metrics[$Matches[1]]=[long]$Matches[2]}
}
if($metrics.Count -ne 6 -or $metrics['blocking violations'] -ne 0){throw 'Incomplete structure metrics'}
$paths=Get-Content -LiteralPath "$taskTemp/presentation-terminal-request-source-20261031.owned.txt"
$rows=@(foreach($path in $paths){
    [byte[]]$old=[byte[]]::new(0)
    if(@(git ls-tree -r --name-only $initial.parent -- $path).Count){$old=Read-GitBytes @('show',($initial.parent+':'+$path))}
    $now=[IO.File]::ReadAllBytes((Join-Path 'D:/git/arcweft' $path))
    [ordered]@{path=$path;owner='arcweft-core';classification=$(if($path -like '*/tests/*'){'test'}else{'production'});baseBytes=$old.Length;basePhysicalLoc=([Text.Encoding]::UTF8.GetString($old).Split("`n").Count-1);bytes=$now.Length;physicalLoc=([Text.Encoding]::UTF8.GetString($now).Split("`n").Count-1)}
})
$review=[ordered]@{
    version=1;base=$initial.parent;inspectedMain=$publication.commit;heldTracked=14;heldUntracked=6;rows=$rows
    disposition=@(
        'The selected child transaction already owns its fiber checkpoint, original PresentationBanks and deferred resume journal. Pending terminal intent is non-Serde journal state; no persisted parallel terminal authority is introduced. Suspension producers stage intent for the admitted program and original bank owner to retire before terminalization.'
        'The admitted trap_fiber entry validates whole-contour retirement, then delegates final status and cleanup to the existing VM terminal implementation. No fresh bank, host I/O, public production API, dependency, or wire/version shape is added.'
        'Control failure admission borrows VM observations within the producing-instruction checkpoint. Valid Return/Goto retain their separate ownership-transfer boundary. Accepted failures use the existing main lifecycle helper.'
        'The scheduler stops a refused candidate after its diagnostic instead of retrying it among other retained candidates in the same poll. Existing blocked-owner rotation and later runnable work remain covered by the full owner run.'
        'The new terminal_requests module owns real control/Need/AwaitMany/Host regressions. Its growth above 300 LOC is cohesive with this transaction boundary. It shares explicit private fixtures and the existing complete public-save assertion; no public visibility is introduced for splitting.'
        'product_step.rs retains the selected transaction coordinator: staging, rollback, line reconciliation and commit share its original checkpoint/journal/output. Splitting only by LOC would hide this coupling. suspension.rs prepares protocol-specific resumes without publishing terminal status. vm.rs owns canonical terminal dispatch. No unrelated state cluster or cross-layer traversal was added.'
    )
}
$review|ConvertTo-Json -Depth 12|Set-Content -LiteralPath "$taskTemp/presentation-terminal-request-owner-review.json" -Encoding utf8
$proofs=@()
foreach($attempt in @('focus1','focus2','focus3')){
    $prior=Get-Content -LiteralPath "$taskTemp/presentation-terminal-request-20261031.$attempt.source-freeze.json" -Raw|ConvertFrom-Json -Depth 20
    $mapping=@(foreach($path in $paths){
        $row=$prior.paths|Where-Object path -eq $path
        $source="terminal-request-focus3-source/$path"
        if($attempt -eq 'focus1' -and $path -eq 'crates/arcweft-core/src/awbc/product_step.rs'){$source='terminal-request-focus1.product-source.rs'}
        if($path.EndsWith('/terminal_requests.rs') -and $attempt -eq 'focus2'){$source='terminal-request-focus2.fixture-source.rs'}
        if($path.EndsWith('/terminal_requests.rs') -and $attempt -eq 'focus1'){
            [ordered]@{path=$path;bytes=$row.bytes;sha256=$row.sha256;source=$null;status='Original source-freeze hash and runtime log retained; exact unpublished fixture bytes were not captured.'}
            continue
        }
        $hash=(Get-FileHash -LiteralPath "$taskTemp/$source").Hash.ToLowerInvariant()
        if($hash -ne $row.sha256){throw ('Historical source proof differs: '+$attempt+' '+$path)}
        [ordered]@{path=$path;bytes=$row.bytes;sha256=$hash;source=$source;status='Exact byte/hash match to original source freeze'}
    })
    $proofs+=[ordered]@{attempt=$attempt;ownedSources=$mapping}
}
$proofs|ConvertTo-Json -Depth 12|Set-Content -LiteralPath "$taskTemp/presentation-terminal-request-source-proofs.json" -Encoding utf8
$sources=@('record-presentation-terminal-request-evidence.ps1','run-presentation-trap-quality.ps1','presentation-terminal-request-owner-review.json','presentation-terminal-request-source-proofs.json','terminal-request-focus1.product-source.rs','terminal-request-focus2.fixture-source.rs')
$sources+=@($paths|ForEach-Object {'terminal-request-focus3-source/'+$_})
foreach($prefix in @('presentation-terminal-request-source-20261031','presentation-terminal-request-repair-source-20261031')){
    foreach($suffix in @('owned.txt','prepared.json','verified.json','reviewed.patch','publish.log')){$sources+="$prefix.$suffix"}
}
$attempts=@(foreach($attempt in @('focus1','focus2','focus3','owner4')){
    $stem="presentation-terminal-request-20261031.$attempt"
    $sources+="run-$stem.ps1"
    foreach($suffix in @('source-freeze.json','binaries-before.json','catalog.inventory.json','catalog.json','catalog.log','runtime.json','runtime.log','runtime.terminal-audit.json')){
        if(Test-Path -LiteralPath "$taskTemp/$stem.$suffix"){$sources+="$stem.$suffix"}
    }
    if($attempt -eq 'focus3'){
        [ordered]@{attempt=$attempt;catalog=(Get-Content -LiteralPath "$taskTemp/$stem.catalog.json" -Raw|ConvertFrom-Json);runtime='Not run: compile E0308; bind_publication requires TaskSubmission, not TaskId.'}
    }else{Get-Content -LiteralPath "$taskTemp/$stem.runtime.terminal-audit.json" -Raw|ConvertFrom-Json -Depth 20}
})
foreach($suffix in @('source-freeze.json','clippy.json','clippy.log','default-check.json','default-check.log','doctest.json','doctest.log','structure.json','structure.log','fmt.json','fmt.log')){$sources+="$quality.$suffix"}
foreach($name in @('dependency_edges.csv','file_metrics.csv','findings.md','package_metrics.csv','public_type_duplicates.csv')){$sources+="$quality.structure-report/$name"}
$bytes=0L
$artifacts=@(foreach($source in $sources){
    $name='task-presentation-terminal-request-raw.'+$source.Replace('/','.').Replace('\','.')
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
    scope='Main control failures and staged deferred-child Need/AwaitMany/Host terminal requests under original selected-transaction custody.'
    sourcePublication=$publication;initialCheckpointPublication=$initial
    workingTree=[ordered]@{heldTracked=14;heldUntracked=6;heldPatchSha256=$publication.heldPatchSha256;coreDirty=$false}
    owner=$terminal;ownerBinaryDrift=0;ownerQualitySourceDrift=0;quality=$checks;attempts=$attempts
    attemptNotes=@(
        'focus1: 3 PASS / 2 FAIL / 2136 excluded. The Program child was correctly retired during the same successful step; the test incorrectly expected a retained terminal child. It now asserts actual child removal and the authenticated producer fault. The other failure showed four identical capacity diagnostics: the scheduler retried a refused child while polling retained candidates. It now stops after the refusal diagnostic and preserves normal blocked-child rotation.'
        'focus2: 4 PASS / 1 FAIL / 2136 excluded. Public cold restore correctly changes unacknowledged Program retirement to ReplayPending. The fixture now binds the actual emitted completion to the issuing journal receipt and latches its authenticated acknowledgement before the complete save/restore assertion. No replay contract or snapshot assertion was weakened.'
        'focus3: catalog compile failed E0308, exit 101; no runtime cases executed. The added acknowledgement used TaskId where TaskSubmission is required. The repair obtains the actual accepted journal submission. Initial checkpoint was already published; both full SHAs and raw patches are retained.'
        'owner4: complete Core owner, 2194 PASS / 0 FAIL / 0 skipped. This replaces no previous accepted feature and is not combined with the focused counts.'
    )
    sourceProvenance=$proofs
    doctest=[ordered]@{passed=78;failed=0;ignored=0;filtered=0}
    structure=[ordered]@{packages=$metrics['workspace packages'];files=$metrics['files scanned'];rustFiles=$metrics['Rust files'];rustPhysicalLoc=$metrics['Rust physical LOC'];reviewTriggers=$metrics['review triggers'];blocking=$metrics['blocking violations'];ownerReview='presentation-terminal-request-owner-review.json'}
    accepted=@(
        'Main Panic/Fail/Bail/unresolved Goto preflight the original complete contour before effect publication, retire three genuine nested scopes once, and round-trip the public default-policy owner. Two handles=3/scopes=8 refusal attempts preserve the whole producing-instruction snapshot, budget/cursor, original grants and old queue entry; unchanged-policy retry succeeds.'
        'All deferred suspension manual terminal requests now stage typed intent in the existing resume journal. The selected transaction commits trap through the admitted program and original banks before terminal metadata or input receipts are consumed; refusal restores its checkpoint/journal/queue.'
        'Genuine Program AwaitMany failure and nested Program Need foreign-publication rejection preserve complete selected-step snapshots and the exact queued packet on two bounded refusals. Successful retry retires the original token once. The AwaitMany producer retains its actual fault, and its real accepted HostMetadata completion acknowledgement permits complete public save/restore equality.'
        'Actual LIFO deferred HostCall failures for UnsupportedCapability, Rejected and Failed retain the original receipt through two bounded refusals, then consume it once after admitted resource unwind. Default-policy public save/restore retains custody.'
        'Existing VM/lifecycle/cancellation/checkpoint and unrelated full Core regressions remain passing. Source, binary and catalog drift are zero for this run.'
    )
    notClaimedByThisRun=@('A complete external Product outer step for every failure-packet family; focused negative cases exercise the selected child transaction','All unexpected VM/protocol-error and activation Host failure terminal paths','Frame Return/select/valid Goto and escaping-handle ownership transfer','Native retained contour including actual borrowed parent','Source ViewValue, eleven callables/mount/six methods/lowering/driver','Active-bank replacement, Native/Web image parity and final dependency/feature/target/workspace acceptance')
    currentStatusAuthority='../current-status.md'
    acceptedFeaturesRetained='Generic Match and all prior accepted boundaries remain accepted. Whole-goal incompleteness does not reopen them. String/Bool resource fixtures and modeled retirement queue release do not prove Source ViewValue or real driver ABI.'
    warnings='Existing compiler/Clippy warnings are preserved in raw logs; exit zero is not a zero-warning claim.'
    priorAcceptance=@('task-presentation-lifecycle-trap-owner-validation.json','task-presentation-vm-trap-owner-validation.json','task-presentation-await-cancellation-owner-validation.json')
    artifacts=$artifacts;artifactCount=$artifacts.Count;artifactBytes=$bytes;rawByteDrift=0
}
$record|ConvertTo-Json -Depth 40|Set-Content -LiteralPath $output -Encoding utf8
[ordered]@{record=$output;artifacts=$artifacts.Count;bytes=$bytes;owner=$terminal.passed;sourceInputs=$terminal.sourceInputs;quality=$quality;metrics=$metrics;fmt=($checks|Where-Object kind -eq 'fmt').receipt.paths.Count;review=$rows}|ConvertTo-Json -Depth 10 -Compress
