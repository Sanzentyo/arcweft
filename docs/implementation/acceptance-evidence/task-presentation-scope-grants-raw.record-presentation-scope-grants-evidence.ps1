$ErrorActionPreference='Stop'
Set-Location -LiteralPath 'D:/git/arcweft'
$taskTemp='C:/Users/sanze/AppData/Local/Temp/arcweft-1011-convergence-resume-cb798c1d'
& "$taskTemp/assert-no-arcweft-build.ps1"
. "$taskTemp/convergence-image-git-bytes.ps1"
$root='D:/git/arcweft/docs/implementation/acceptance-evidence'
$output=Join-Path $root 'task-presentation-scope-grants-owner-validation.json'
if(Test-Path -LiteralPath $output){throw 'Evidence exists'}
$owner='presentation-scope-grants-20261031.owner5'
$quality=(Get-Content -LiteralPath "$taskTemp/product-execution-quality.active.txt" -Raw).Trim()
$terminal=Get-Content -LiteralPath "$taskTemp/$owner.runtime.terminal-audit.json" -Raw|ConvertFrom-Json -Depth 20
if(-not $terminal.isPassingRun -or $terminal.executed -ne 2198 -or $terminal.skipped -ne 0 -or $terminal.sourceInputs -ne 4028){throw 'Unexpected owner terminal'}
$freeze=Get-Content -LiteralPath "$taskTemp/$owner.source-freeze.json" -Raw|ConvertFrom-Json -Depth 20
$qualityFreeze=Get-Content -LiteralPath "$taskTemp/$quality.source-freeze.json" -Raw|ConvertFrom-Json -Depth 20
if(@(Compare-Object @($freeze.paths|ForEach-Object {$_.path+' '+$_.sha256}) @($qualityFreeze.paths|ForEach-Object {$_.path+' '+$_.sha256})).Count){throw 'Owner/quality input drift'}
foreach($row in $freeze.paths){if((Get-FileHash -LiteralPath $row.path).Hash.ToLowerInvariant() -ne $row.sha256){throw ('Source drift: '+$row.path)}}
$checks=@(foreach($kind in @('workspace-check','clippy','default-check','doctest','structure','fmt')){
    $receipt=Get-Content -LiteralPath "$taskTemp/$quality.$kind.json" -Raw|ConvertFrom-Json -Depth 20
    if($receipt.exitCode -ne 0 -or $receipt.sourceDrift -ne 0){throw ('Failed quality: '+$kind)}
    [ordered]@{kind=$kind;receipt=$receipt}
})
if(-not [IO.File]::ReadAllText("$taskTemp/$quality.doctest.log").Contains('78 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out')){throw 'Unexpected doctest summary'}
$initial=Get-Content -LiteralPath "$taskTemp/presentation-scope-grants-source-20261031.verified.json" -Raw|ConvertFrom-Json -Depth 20
$publication=Get-Content -LiteralPath "$taskTemp/presentation-scope-grants-repair-source-20261031.verified.json" -Raw|ConvertFrom-Json -Depth 20
if((git rev-parse HEAD).Trim() -ne $publication.commit -or (git rev-parse origin/main).Trim() -ne $publication.commit){throw 'Publication differs'}
$coverage=Get-Content -LiteralPath "$taskTemp/presentation-scope-grants-workspace-source-coverage.json" -Raw|ConvertFrom-Json -Depth 10
if($coverage.frozenInputs -ne 4028 -or @($coverage.unfrozenCrateRust).Count){throw 'Workspace source coverage missing'}
$metrics=@{}
foreach($line in [IO.File]::ReadLines("$taskTemp/$quality.structure.log")){
    if($line -match '^(workspace packages|files scanned|Rust files|Rust physical LOC|review triggers|blocking violations): (\d+)$'){$metrics[$Matches[1]]=[long]$Matches[2]}
}
if($metrics.Count -ne 6 -or $metrics['blocking violations'] -ne 0){throw 'Incomplete structure metrics'}
$paths=@(@($initial.paths.path)+@($publication.paths.path)|Sort-Object -Unique)
$review=[ordered]@{
    version=1;base=$initial.parent;inspectedMain=$publication.commit;heldTracked=14;heldUntracked=6
    rows=@(foreach($path in $paths){
        [byte[]]$old=[byte[]]::new(0);[byte[]]$now=[byte[]]::new(0)
        if(@(git ls-tree -r --name-only $initial.parent -- $path).Count){$old=Read-GitBytes @('show',($initial.parent+':'+$path))}
        if(Test-Path -LiteralPath $path){$now=[IO.File]::ReadAllBytes((Join-Path 'D:/git/arcweft' $path))}
        [ordered]@{path=$path;owner=$(if($path.StartsWith('docs/')){'specification'}else{'arcweft-core'});classification=$(if($path -like '*/tests/*' -or $path.EndsWith('/tests.rs')){'test'}elseif($path.StartsWith('docs/')){'specification'}else{'production'});baseBytes=$old.Length;basePhysicalLoc=([Text.Encoding]::UTF8.GetString($old).Split("`n").Count-1);bytes=$now.Length;physicalLoc=([Text.Encoding]::UTF8.GetString($now).Split("`n").Count-1)}
    })
    disposition=@(
        'The complete domain permits a physical scope to receive resources from multiple issuing banks. Keeping one grant or renaming tokens would prevent real cross-bank Return/Dialogue transfer. The same scope binding now owns a bank-indexed grant map; it is the only grant authority and the ledger remains the sole body/lease owner.'
        'The new scope.rs owns scope identities, anchors, borrowed grant targets and live/inert scope admission. Its growth above 300 LOC is cohesive with this domain; ledger slot/lease behavior stays in resources.rs. No public API widening was added for physical splitting.'
        'The original bank-row codec was moved to the common resource owner and reused for both bank and scope maps. Duplicate keys are rejected before reading the next owned record. The old path and old single-grant reader are removed; the enclosing contract stays version 1.'
        'Registration bank authority is independent of the banks of held grants. The shared execution role owns Root/Dialogue bank checks. AWBC intrinsic roots, actual existing/staged line-child owners, and quiescent Native root execution use that rule. Program parent links remain the actual execution contour responsibility.'
        'Every grant across the whole actual scope contour and total pending-body capacity is checked before retirement. Retirements follow scope unwind order and bank order within a scope. Native env and AWBC live/inert snapshots retain the same grant grammar and original program lease.'
        'This cut is the receiving-scope foundation. It does not implement Return/select/Goto transfer, Native borrowed-parent contour, Source ViewValue ABI or final reverse/target acceptance. No fallback owner, wire version fork, new Cargo edge or I/O boundary was added.'
    )
}
$review|ConvertTo-Json -Depth 12|Set-Content -LiteralPath "$taskTemp/presentation-scope-grants-owner-review.json" -Encoding utf8
$proofs=@()
foreach($attempt in @('owner1','owner2')){
    $prior=Get-Content -LiteralPath "$taskTemp/presentation-scope-grants-20261031.$attempt.source-freeze.json" -Raw|ConvertFrom-Json -Depth 20
    $prefix="scope-grants-$attempt-source/"
    foreach($file in Get-ChildItem -LiteralPath "$taskTemp/scope-grants-$attempt-source" -Recurse -File){
        $path=[IO.Path]::GetRelativePath("$taskTemp/scope-grants-$attempt-source",$file.FullName).Replace('\','/')
        $row=$prior.paths|Where-Object path -eq $path
        $hash=(Get-FileHash -LiteralPath $file.FullName).Hash.ToLowerInvariant()
        if($null -eq $row -or $hash -ne $row.sha256 -or $file.Length -ne $row.bytes){throw 'Historical source drift'}
        $proofs+=[ordered]@{attempt=$attempt;path=$path;source=($prefix+$path);bytes=$row.bytes;sha256=$hash;drift=0}
    }
}
if($proofs.Count -ne 26){throw 'Incomplete historical captures'}
$proofs|ConvertTo-Json -Depth 10|Set-Content -LiteralPath "$taskTemp/presentation-scope-grants-source-proofs.json" -Encoding utf8
$sources=@('record-presentation-scope-grants-evidence.ps1','run-presentation-scope-workspace-quality.ps1','assert-no-arcweft-build.ps1','checkpoint-explicit-deletions-prepare.ps1','checkpoint-explicit-deletions-publish.ps1','presentation-scope-grants-owner-review.json','presentation-scope-grants-source-proofs.json','presentation-scope-grants-workspace-source-coverage.json')+@($proofs.source)
foreach($prefix in @('presentation-scope-grants-source-20261031','presentation-scope-grants-repair-source-20261031')){
    foreach($suffix in @('owned.txt','prepared.json','verified.json','reviewed.patch','publish.log')){$sources+="$prefix.$suffix"}
}
$attempts=@(foreach($attempt in @('owner1','owner2','focus3','owner4','owner5')){
    $stem="presentation-scope-grants-20261031.$attempt";$sources+="run-$stem.ps1"
    foreach($suffix in @('source-freeze.json','binaries-before.json','catalog.inventory.json','catalog.json','catalog.log','runtime.json','runtime.log','runtime.terminal-audit.json')){if(Test-Path -LiteralPath "$taskTemp/$stem.$suffix"){$sources+="$stem.$suffix"}}
    if(Test-Path -LiteralPath "$taskTemp/$stem.runtime.terminal-audit.json"){Get-Content -LiteralPath "$taskTemp/$stem.runtime.terminal-audit.json" -Raw|ConvertFrom-Json -Depth 20}
    else{[ordered]@{attempt=$attempt;catalog=(Get-Content -LiteralPath "$taskTemp/$stem.catalog.json" -Raw|ConvertFrom-Json);runtime='Not run: two old test calls retained the removed registration-bank argument.'}}
})
$sources+="$quality.source-freeze.json"
foreach($kind in @('workspace-check','clippy','default-check','doctest','structure','fmt')){foreach($ext in @('json','log')){$sources+="$quality.$kind.$ext"}}
foreach($name in @('dependency_edges.csv','file_metrics.csv','findings.md','package_metrics.csv','public_type_duplicates.csv')){$sources+="$quality.structure-report/$name"}
$bytes=0L
$artifacts=@(foreach($source in $sources){
    $name='task-presentation-scope-grants-raw.'+$source.Replace('/','.').Replace('\','.')
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
    scope='Bank-indexed grants of one physical scope, shared live/inert admission, atomic multibank retirement and independent registration-role checks. Receiving-scope foundation only.'
    sourcePublication=$publication;initialCheckpointPublication=$initial
    workingTree=[ordered]@{heldTracked=14;heldUntracked=6;heldPatchSha256=$publication.heldPatchSha256;coreDirty=$false}
    owner=$terminal;ownerBinaryDrift=0;ownerQualitySourceDrift=0;quality=$checks;workspaceSourceCoverage=$coverage;attempts=$attempts;sourceProvenance=$proofs
    attemptNotes=@('owner1: catalog compile 101, runtime not run; two old private trait arity calls migrated without changing assertions.','owner2: 2194 PASS / 4 FAIL / 0 skipped. Migrated the old single-grant wire mutation to the actual bank row. Separating held grants from registration bank exposed three protections that had relied on equality of those distinct roles. Root and actual Dialogue child role checks now own those protections. All original rejection assertions remain.','focus3: 7 PASS / 2138 excluded; three new scope tests and the four earlier failures. These counts are not added to final owner5.','owner4: 2196 PASS / 2 FAIL / 0 skipped, exact published checkpoint 935fa74803b1200f19eaf73dd08447a0a1b7dabd source. The independently valid foreign-root fixture used an Entry in a Dialogue bank; it now uses an independently admitted foreign Function root while retaining the exact activation-root error. The joined-child cancellation fixture copied a Root fiber; it now uses the real allocated Dialogue child factory. All cancellation and failure assertions remain.','owner5: full 2198 PASS / 0 FAIL / 0 skipped, with catalog/source/binary drift zero. Workspace gates use the same 4028 frozen inputs, including all 3115 crate Rust files; held WIP remains part of that observed checkout.')
    doctest=[ordered]@{passed=78;failed=0;ignored=0;filtered=0}
    structure=[ordered]@{packages=$metrics['workspace packages'];files=$metrics['files scanned'];rustFiles=$metrics['Rust files'];rustPhysicalLoc=$metrics['Rust physical LOC'];reviewTriggers=$metrics['review triggers'];blocking=$metrics['blocking violations']}
    accepted=@('One actual Native environment scope holds both Root and Dialogue issuing-bank grants, with distinct unchanged tokens even when local issuance coordinates match. Shared live admission, inert scope wire and bank rollback preserve both grants and exact bodies. Whole-scope retirement removes both grants in bank order and retires each body once.','Two capacity refusals under the same handles=2/scopes=4 policy preserve the complete bank image and both scope grants. After modeled old-retirement acknowledgement, the same policy retires both original tokens in deterministic order.','Duplicate bank wire is rejected before reading its second invalid grant. An empty grant capsule is rejected during admission and poisons the same retained allowance. Old reader/path are deleted, contract version remains 1.','Root registration and actual existing/staged Dialogue child registration are authenticated separately from held resource banks. Existing bank-forgery, source/anchor and original activation-root rejection coverage remains passing.')
    notClaimedByThisRun=@('VM Return/select/static or dynamic Goto escaping-handle transfer or ordinary Product root result retention','Native complete actual borrowed-parent contour or Source ViewValue/callables/mount/methods/driver','Active-bank replacement, Native/Web image parity and final reverse-dependency/feature/target/workspace runtime acceptance')
    currentStatusAuthority='../current-status.md';priorAcceptance=@('task-presentation-activation-host-owner-validation.json','task-presentation-terminal-request-owner-validation.json')
    acceptedFeaturesRetained='Generic Match and prior accepted cancellation/VM/lifecycle/control/Host boundaries remain accepted. A receiving-scope prerequisite is not completion of Return/Goto. Whole-goal incompleteness does not reopen accepted features.'
    warnings='Existing compiler/Clippy warnings remain in raw logs; exit zero is not zero warnings.'
    artifacts=$artifacts;artifactCount=$artifacts.Count;artifactBytes=$bytes;rawByteDrift=0
}
$record|ConvertTo-Json -Depth 40|Set-Content -LiteralPath $output -Encoding utf8
[ordered]@{record=$output;artifacts=$artifacts.Count;bytes=$bytes;owner=$terminal.passed;sourceInputs=$terminal.sourceInputs;quality=$quality;metrics=$metrics;fmt=($checks|Where-Object kind -eq 'fmt').receipt.paths.Count;review=$review.rows}|ConvertTo-Json -Depth 10 -Compress
