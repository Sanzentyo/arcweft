param([Parameter(Mandatory)][string]$Prefix)
$ErrorActionPreference='Stop'
$e='C:/Users/sanze/AppData/Local/Temp/arcweft-1011-convergence-resume-cb798c1d'
Set-Location -LiteralPath 'D:/git/arcweft'
. (Join-Path $e 'convergence-image-git-bytes.ps1')
$p=Get-Content -LiteralPath (Join-Path $e ($Prefix+'.prepared.json')) -Raw | ConvertFrom-Json
if((& git rev-parse HEAD).Trim() -ne $p.base -or (& git branch --show-current).Trim() -ne 'main' -or (& git write-tree).Trim() -ne $p.tree){throw 'Reviewed base/tree changed'}
& git fetch origin main
if($LASTEXITCODE -ne 0 -or (& git rev-parse origin/main).Trim() -ne $p.base){throw 'Fetch failed or main moved; reconcile before publishing'}
& git commit --file (Join-Path $e ($Prefix+'.message.txt'))
if($LASTEXITCODE -ne 0){throw 'Commit failed'}
$head=(& git rev-parse HEAD).Trim()
& git push origin HEAD:main
if($LASTEXITCODE -ne 0){throw 'Non-forced push failed'}
$remote=((& git ls-remote origin refs/heads/main) -split '\s+')[0]
if($remote -ne $head -or (& git rev-parse 'HEAD^').Trim() -ne $p.base -or (& git rev-parse 'HEAD^{tree}').Trim() -ne $p.tree){throw 'Published identity mismatch'}
$patch=Read-GitBytes @('diff','--binary','--full-index',$p.base,$head)
if([Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($patch)).ToLowerInvariant() -ne $p.patchSha256){throw 'Published patch differs'}
$blobs=@{}
foreach($line in (& git ls-tree -r HEAD -- $p.paths.path)){if($line -notmatch '^[0-9]+ blob ([0-9a-f]{40})	(.+)$'){throw 'Unexpected tree entry'};$blobs[$Matches[2]]=$Matches[1]}
foreach($row in $p.paths){if($blobs[$row.path] -ne $row.blob){throw ('Published blob mismatch: '+$row.path)}}
foreach($row in $p.other){if((Get-FileHash -LiteralPath $row.path).Hash.ToLowerInvariant() -ne $row.sha256){throw 'Separate pending work changed'}}
$held=Read-GitBytes (@('diff','--binary','--full-index','--')+$p.held)
if([Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($held)).ToLowerInvariant() -ne $p.heldPatchSha256){throw 'Original WIP changed'}
if(@(git diff --cached --name-only).Count){throw 'Index is not empty'}
$verified=[ordered]@{version=1;commit=$head;parent=$p.base;tree=$p.tree;remote=$remote;paths=$p.paths;blobDrift=0;patchSha256=$p.patchSha256;heldPatchSha256=$p.heldPatchSha256;separatelyHeld=$p.other;indexEmpty=$true;at=[DateTime]::UtcNow.ToString('o')}
$verified | ConvertTo-Json -Depth 7 | Set-Content -LiteralPath (Join-Path $e ($Prefix+'.verified.json')) -Encoding utf8
$baseline=Get-Content -LiteralPath (Join-Path $e 'checkpoint-task-view.source-freeze.json') -Raw | ConvertFrom-Json
$updated=@{}
foreach($row in $baseline.paths){$updated[$row.path]=$row}
foreach($row in $p.paths){$updated[$row.path]=[ordered]@{path=$row.path;bytes=(Get-Item -LiteralPath $row.path).Length;sha256=(Get-FileHash -LiteralPath $row.path).Hash.ToLowerInvariant()}}
$baseline.base=$head
$baseline.heldPatchSha256=$p.heldPatchSha256
$baseline.paths=@($updated.Values | Sort-Object path)
$baseline | ConvertTo-Json -Depth 7 | Set-Content -LiteralPath (Join-Path $e 'checkpoint-task-view.source-freeze.json') -Encoding utf8
[ordered]@{commit=$head;parent=$p.base;tree=$p.tree;paths=$p.paths.Count;blobDrift=0;originalWip=$p.held.Count;separatelyHeld=$p.other.Count;patchSha256=$p.patchSha256;indexEmpty=$true} | ConvertTo-Json -Compress
