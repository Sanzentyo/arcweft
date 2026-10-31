param([Parameter(Mandatory)][string]$Prefix,[switch]$Refresh)
$ErrorActionPreference='Stop'
$e='C:/Users/sanze/AppData/Local/Temp/arcweft-1011-convergence-resume-cb798c1d'
Set-Location -LiteralPath 'D:/git/arcweft'
. (Join-Path $e 'convergence-image-git-bytes.ps1')
$baseline=Get-Content -LiteralPath (Join-Path $e 'checkpoint-task-view.source-freeze.json') -Raw | ConvertFrom-Json
$owned=@(Get-Content -LiteralPath (Join-Path $e ($Prefix+'.owned.txt')) | Sort-Object -Unique)
$other=@(Get-Content -LiteralPath (Join-Path $e ($Prefix+'.other.txt')) | Where-Object {$_})
$base=(& git rev-parse HEAD).Trim()
if($base -ne $baseline.base -or (& git branch --show-current).Trim() -ne 'main'){throw 'Unexpected base/branch'}
$existingIndex=@(git diff --cached --no-renames --name-only)
if($Refresh){if(@(Compare-Object $owned $existingIndex).Count){throw 'Refresh index differs from the explicit owned set'}}elseif($existingIndex.Count){throw 'Index is not empty'}
$allowed=@($owned+$other+$baseline.held)
if(@(git diff HEAD --name-only | Where-Object {$_ -notin $allowed}).Count){throw 'Unknown changed tracked paths'}
if(@($owned | Where-Object {$_ -in $baseline.held}).Count){throw 'Held owned path'}
$deleted=@($owned|Where-Object {-not(Test-Path -LiteralPath $_)})
foreach($path in $deleted){$oldEntry=@(git ls-tree -r --name-only HEAD -- $path);if($oldEntry.Count -ne 1 -or $oldEntry[0] -ne $path){throw ('Missing path is not a tracked deletion: '+$path)}}
$held=Read-GitBytes (@('diff','--binary','--full-index','--')+$baseline.held)
$heldHash=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($held)).ToLowerInvariant()
if($heldHash -ne $baseline.heldPatchSha256){throw 'Original WIP changed'}
$otherRows=@(foreach($p in $other){[ordered]@{path=$p;sha256=(Get-FileHash -LiteralPath $p).Hash.ToLowerInvariant()}})
$pspec=Join-Path $e ($Prefix+'.pathspec')
[IO.File]::WriteAllText($pspec,($owned -join "`0")+"`0",[Text.UTF8Encoding]::new($false))
& git --literal-pathspecs add ('--pathspec-from-file='+$pspec) --pathspec-file-nul
if($LASTEXITCODE -ne 0){throw 'Explicit staging failed'}
$staged=@(git diff --cached --no-renames --name-only)
if(@(Compare-Object $owned $staged).Count){throw 'Staged set differs'}
$check=@('-c','core.whitespace=trailing-space,space-before-tab,cr-at-eol','diff','--cached','--check','--','.')
foreach($p in ($owned | Where-Object {$_ -like '*.log' -or $_ -like 'docs/implementation/acceptance-evidence/*.patch'})){$check+=(':(exclude)'+$p)}
& git @check
if($LASTEXITCODE -ne 0){throw 'Whitespace check failed'}
$patch=Read-GitBytes @('diff','--cached','--binary','--full-index')
[IO.File]::WriteAllBytes((Join-Path $e ($Prefix+'.reviewed.patch')),$patch)
$rows=@(foreach($line in (& git ls-files --stage)){if($line -notmatch '^[0-9]+ ([0-9a-f]{40}) 0	(.+)$'){throw 'Unexpected index entry'};if($Matches[2] -notin $owned){continue};[ordered]@{path=$Matches[2];blob=$Matches[1]}})
$rows+=@(foreach($path in $deleted){[ordered]@{path=$path;blob=$null}})
if($rows.Count -ne $owned.Count){throw 'Indexed blob set differs from explicit owned paths'}
$prepared=[ordered]@{version=1;base=$base;tree=(& git write-tree).Trim();paths=$rows;other=$otherRows;held=$baseline.held;heldPatchSha256=$heldHash;patchSha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($patch)).ToLowerInvariant();preparedAt=[DateTime]::UtcNow.ToString('o')}
$prepared | ConvertTo-Json -Depth 7 | Set-Content -LiteralPath (Join-Path $e ($Prefix+'.prepared.json')) -Encoding utf8
[ordered]@{base=$base;paths=$rows.Count;other=$otherRows.Count;held=$baseline.held.Count;tree=$prepared.tree;patchSha256=$prepared.patchSha256;heldPatchSha256=$heldHash} | ConvertTo-Json -Compress
& git diff --cached --stat
