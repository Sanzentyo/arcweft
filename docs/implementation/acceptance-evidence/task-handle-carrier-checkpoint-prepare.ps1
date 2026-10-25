param([Parameter(Mandatory)][string]$Prefix)
$ErrorActionPreference='Stop'
$e='C:/Users/sanze/AppData/Local/Temp/arcweft-1011-convergence-resume-cb798c1d'
Set-Location -LiteralPath 'D:/git/arcweft'
. (Join-Path $e 'convergence-image-git-bytes.ps1')
$baseline=Get-Content -LiteralPath (Join-Path $e 'checkpoint-task-view.source-freeze.json') -Raw | ConvertFrom-Json
$partial='crates/arcweft-runtime-host/src/native_task.rs'
$intent=Get-Content -LiteralPath (Join-Path $e 'task-handle-carrier-held-intent.json') -Raw | ConvertFrom-Json
$partialCandidate=Join-Path $e 'task-handle-carrier-native-task-publish.rs'
if((Get-FileHash -LiteralPath $partial).Hash.ToLowerInvariant() -ne $intent.workingSha256 -or (Get-FileHash -LiteralPath $partialCandidate).Hash.ToLowerInvariant() -ne $intent.publishCandidateSha256){throw 'Partially held source or reviewed index candidate changed'}
$owned=@(Get-Content -LiteralPath (Join-Path $e ($Prefix+'.owned.txt')) | Sort-Object -Unique)
$other=@(Get-Content -LiteralPath (Join-Path $e ($Prefix+'.other.txt')) | Where-Object {$_})
$base=(& git rev-parse HEAD).Trim()
if($base -ne $baseline.base -or (& git branch --show-current).Trim() -ne 'main'){throw 'Unexpected base/branch'}
if(@(git diff --cached --name-only).Count){throw 'Index is not empty'}
$allowed=@($owned+$other+$baseline.held)
if(@(git diff HEAD --name-only | Where-Object {$_ -notin $allowed}).Count){throw 'Unknown changed tracked paths'}
if(@($owned | Where-Object {($_ -in $baseline.held -and $_ -ne $partial) -or -not(Test-Path -LiteralPath $_)}).Count){throw 'Held or missing owned path'}
$held=Read-GitBytes (@('diff','--binary','--full-index','--')+$baseline.held)
$heldHash=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($held)).ToLowerInvariant()
if($heldHash -ne $intent.heldFullDiffBeforeStagingSha256){throw 'Original WIP changed'}
$otherRows=@(foreach($p in $other){[ordered]@{path=$p;sha256=(Get-FileHash -LiteralPath $p).Hash.ToLowerInvariant()}})
$pspec=Join-Path $e ($Prefix+'.pathspec')
[IO.File]::WriteAllText($pspec,(($owned | Where-Object {$_ -ne $partial}) -join "`0")+"`0",[Text.UTF8Encoding]::new($false))
& git --literal-pathspecs add ('--pathspec-from-file='+$pspec) --pathspec-file-nul
if($LASTEXITCODE -ne 0){throw 'Explicit staging failed'}
$partialBlob=(& git hash-object -w (('--path=')+$partial) $partialCandidate).Trim()
if($LASTEXITCODE -ne 0 -or $partialBlob -notmatch '^[0-9a-f]{40}$'){throw 'Partial index blob construction failed'}
& git update-index --add --cacheinfo ('100644,'+$partialBlob+','+$partial)
if($LASTEXITCODE -ne 0){throw 'Partial index staging failed'}
$held=Read-GitBytes (@('diff','--binary','--full-index','--')+$baseline.held)
$heldHash=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($held)).ToLowerInvariant()
$staged=@(git diff --cached --name-only)
if(@(Compare-Object $owned $staged).Count){throw 'Staged set differs'}
$check=@('-c','core.whitespace=trailing-space,space-before-tab,cr-at-eol','diff','--cached','--check','--','.')
foreach($p in ($owned | Where-Object {$_ -like '*.log'})){$check+=(':(exclude)'+$p)}
& git @check
if($LASTEXITCODE -ne 0){throw 'Whitespace check failed'}
$patch=Read-GitBytes @('diff','--cached','--binary','--full-index')
[IO.File]::WriteAllBytes((Join-Path $e ($Prefix+'.reviewed.patch')),$patch)
$rows=@(foreach($line in (& git ls-files --stage -- $owned)){if($line -notmatch '^[0-9]+ ([0-9a-f]{40}) 0	(.+)$'){throw 'Unexpected index entry'};[ordered]@{path=$Matches[2];blob=$Matches[1]}})
$prepared=[ordered]@{version=1;base=$base;tree=(& git write-tree).Trim();paths=$rows;other=$otherRows;held=$baseline.held;heldPatchSha256=$heldHash;patchSha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($patch)).ToLowerInvariant();preparedAt=[DateTime]::UtcNow.ToString('o')}
$prepared | ConvertTo-Json -Depth 7 | Set-Content -LiteralPath (Join-Path $e ($Prefix+'.prepared.json')) -Encoding utf8
[ordered]@{base=$base;paths=$rows.Count;other=$otherRows.Count;held=$baseline.held.Count;tree=$prepared.tree;patchSha256=$prepared.patchSha256;heldPatchSha256=$heldHash} | ConvertTo-Json -Compress
& git diff --cached --stat
