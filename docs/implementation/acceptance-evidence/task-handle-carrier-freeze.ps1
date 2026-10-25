$ErrorActionPreference='Stop'
Set-Location -LiteralPath 'D:/git/arcweft'
$e='C:/Users/sanze/AppData/Local/Temp/arcweft-1011-convergence-resume-cb798c1d'
$baseline=Get-Content -LiteralPath (Join-Path $e 'checkpoint-task-view.source-freeze.json') -Raw | ConvertFrom-Json
$graph=Get-Content -LiteralPath (Join-Path $e 'task-handle-carrier.graph.json') -Raw | ConvertFrom-Json
if((& git rev-parse HEAD).Trim() -ne $graph.base -or $baseline.base -ne $graph.base){throw 'Unexpected source construction base'}
$owned=@($graph.owned)
$all=@(($baseline.paths.path | Where-Object {$_ -notlike 'docs/*'})+$owned | Sort-Object -Unique)
$previous=@{};foreach($r in $baseline.paths){$previous[$r.path]=$r}
$rows=@(foreach($p in $all){$hash=(Get-FileHash -LiteralPath $p).Hash.ToLowerInvariant();if($p -notin $owned -and $hash -ne $previous[$p].sha256){throw ('Outside-owned source drift: '+$p)};[ordered]@{path=$p;bytes=(Get-Item -LiteralPath $p).Length;sha256=$hash}})
$metaReceipt=Get-Content -LiteralPath (Join-Path $e 'task-final-r2285.metadata-receipt.json') -Raw | ConvertFrom-Json
foreach($m in $metaReceipt.manifests){if((Get-FileHash -LiteralPath $m.path).Hash.ToLowerInvariant() -ne $m.sha256){throw 'Manifests/lock changed; metadata needs refreshing'}}
if((Get-FileHash -LiteralPath $graph.metadataSource).Hash.ToLowerInvariant() -ne $graph.metadataSha256){throw 'Metadata bytes changed'}
[ordered]@{version=1;base=$graph.base;branch='main';recordedAt=[DateTime]::UtcNow.ToString('o');paths=$rows;owned=$owned;deleted=@();held=$baseline.held;outsideOwnedDrift=0;heldIntentAudit='task-handle-carrier-held-intent.json';manifestInputs=$metaReceipt.manifests.Count;manifestLockDrift=0} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $e 'task-handle-carrier.source-freeze.json') -Encoding utf8
[ordered]@{base=$graph.base;sourceInputs=$rows.Count;owned=$owned.Count;reversePackages=$graph.packageCount;manifestInputs=$metaReceipt.manifests.Count;outsideOwnedDrift=0} | ConvertTo-Json -Compress
