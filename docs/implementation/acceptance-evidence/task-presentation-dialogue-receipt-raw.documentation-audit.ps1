$ErrorActionPreference='Stop'
Set-Location -LiteralPath 'D:/git/arcweft'
$repoPath=(Get-Location).Path
$taskDir=Join-Path $env:TEMP 'arcweft-docs-reconcile-20261027'
$proofDir=Join-Path $env:TEMP 'arcweft-1011-convergence-resume-cb798c1d'
. (Join-Path $proofDir 'convergence-image-git-bytes.ps1')
$before=Get-Content -LiteralPath (Join-Path $taskDir 'before.json') -Raw|ConvertFrom-Json
$base=(git rev-parse HEAD).Trim()
$preservation=Get-Content -LiteralPath (Join-Path $proofDir 'task-dialogue-prefix.cli-preservation-proof.json') -Raw|ConvertFrom-Json -Depth 15
if(-not $preservation.originalUserPatchPreserved -or $preservation.heldCanonicalBlobDrift -ne 0 -or (git hash-object -- $preservation.path).Trim() -ne $preservation.workingBlob){throw 'Original CLI user patch proof differs'}
$cliHeld=@($before.held|Where-Object {$_.path.Replace('\','/') -match '(^|/)crates/arcweft-cli/tests/check/cli_runtime_bench\.rs$'})
if($cliHeld.Count -ne 1){throw 'Expected one preserved partial source'}
$cliHeld[0].sha256=(Get-FileHash -LiteralPath $cliHeld[0].path).Hash.ToLowerInvariant()
$heldDrift=@($before.held|Where-Object {-not(Test-Path -LiteralPath $_.path) -or (Get-FileHash -LiteralPath $_.path).Hash.ToLowerInvariant() -ne $_.sha256})
if($heldDrift.Count){throw 'Original WIP changed'}
$maintained=@(git ls-files -- 'docs/00-overview/*.md' 'docs/01-language/*.md' 'docs/02-runtime/*.md' 'docs/03-presentation/*.md' 'docs/04-tooling/*.md' 'docs/05-build-and-security/*.md' 'docs/examples/*.md' 'docs/schemas/*.md')+@('docs/README.md','docs/implementation/README.md','docs/implementation/current-status.md','docs/implementation/documentation-cleanup.md','docs/reviews/README.md')
$links=0;$broken=@();$fences=@()
foreach($path in $maintained){
 if(-not(Test-Path -LiteralPath $path)){continue}
 $inFence=$false;$fenceChar='';$fenceLength=0
 foreach($line in [IO.File]::ReadAllLines((Join-Path $repoPath $path))){
  $fence=[regex]::Match($line,'^\s*(\x60{3,}|~{3,})')
  if($fence.Success){$marker=$fence.Groups[1].Value;if(-not $inFence){$inFence=$true;$fenceChar=$marker.Substring(0,1);$fenceLength=$marker.Length}elseif($marker.StartsWith($fenceChar) -and $marker.Length -ge $fenceLength){$inFence=$false};continue}
  if($inFence){continue}
  $proseLine=[regex]::Replace($line,'\x60+[^\x60]*\x60+','')
  foreach($link in [regex]::Matches($proseLine,'!?\[[^\]]*\]\((?<target><[^>]+>|[^\s)]+)(?:\s+"[^"]*")?\)')){
   $target=$link.Groups['target'].Value.Trim('<','>')
   if($target -match '^([a-zA-Z][a-zA-Z0-9+.-]*:|#|/)'){continue}
   $target=[Uri]::UnescapeDataString(($target -split '#|\?',2)[0]);if(-not $target){continue}
   $links++;$resolved=Join-Path (Split-Path (Join-Path $repoPath $path)) $target
   if(-not(Test-Path -LiteralPath $resolved)){$broken+="$path -> $target"}
  }
 }
 if($inFence){$fences+=$path}
}
$changed=@(git diff HEAD --name-only -- docs|Where-Object {$_ -like '*.md' -and $_ -notlike 'docs/implementation/acceptance-evidence/*'})
$history=[IO.File]::ReadAllText((Join-Path $repoPath 'docs/implementation/convergence-evidence-history.md'))+[IO.File]::ReadAllText((Join-Path $repoPath 'docs/implementation/evidence-history.md'))+[IO.File]::ReadAllText((Join-Path $repoPath 'docs/implementation/documentation-cleanup.md'))
$missingIds=@()
foreach($path in $changed){
 $oldText=[Text.Encoding]::UTF8.GetString((Read-GitBytes @('show',($base+':'+$path))))
 $newText=if(Test-Path -LiteralPath $path){[IO.File]::ReadAllText((Join-Path $repoPath $path))}else{''}
 foreach($id in @([regex]::Matches($oldText,'(?<![0-9a-fA-F])[0-9a-fA-F]{40}(?![0-9a-fA-F])')|ForEach-Object Value|Sort-Object -Unique)){if(-not $newText.Contains($id) -and -not $history.Contains($id)){$missingIds+="$path : $id"}}
}
$statusText=[IO.File]::ReadAllText((Join-Path $repoPath 'docs/implementation/current-status.md'))
$commitIds=@([regex]::Matches($statusText,'(?<![0-9a-fA-F])[0-9a-fA-F]{40}(?![0-9a-fA-F])')|ForEach-Object Value|Sort-Object -Unique)
foreach($id in $commitIds){$kind=(git cat-file -t $id).Trim();if($LASTEXITCODE -ne 0 -or $kind -ne 'commit'){throw "Not a commit: $id"};git merge-base --is-ancestor $id $base;if($LASTEXITCODE -ne 0){throw "Not an inspected-main ancestor: $id"}}
$historyPath='docs/implementation/convergence-evidence-history.md'
$oldBytes=Read-GitBytes @('show',($base+':'+$historyPath));$newBytes=[IO.File]::ReadAllBytes((Join-Path $repoPath $historyPath))
if($newBytes.Length -lt $oldBytes.Length){throw 'Historical content was truncated'}
$prefix=[byte[]]::new($oldBytes.Length);[Array]::Copy($newBytes,$prefix,$oldBytes.Length)
if([Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($prefix)) -ne [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($oldBytes))){throw 'Historical prefix changed'}
git -c core.whitespace=trailing-space,space-before-tab,cr-at-eol diff --check -- docs
if($LASTEXITCODE -ne 0){throw 'Documentation whitespace check failed'}
$rawChanges=@(git diff HEAD --name-status -- docs/implementation/acceptance-evidence docs/implementation/evidence-history.md docs/reviews/evidence docs/reviews/evidence-history.md|Where-Object {$_ -notmatch '^A\s'})
$result=[ordered]@{date=[DateTime]::Now.ToString('yyyy-MM-dd');base=$base;changed=$changed;checkedDocuments=$maintained.Count;relativeLinks=$links;brokenLinks=$broken;unclosedFences=$fences;statusCommitIds=$commitIds.Count;unretainedIds=$missingIds;historicalRawChanges=$rawChanges;historicalPrefixPreserved=$true;heldFiles=$before.held.Count;heldDrift=$heldDrift.Count;partialCliOriginalWipProof=$true;rustCargoRun=$false}
$result|ConvertTo-Json -Depth 6|Set-Content -LiteralPath (Join-Path $taskDir 'task-wait-reverse-doc-audit.json') -Encoding utf8
$result|ConvertTo-Json -Depth 6 -Compress
if($broken.Count -or $fences.Count -or $missingIds.Count -or $rawChanges.Count){throw 'Documentation audit failed'}
