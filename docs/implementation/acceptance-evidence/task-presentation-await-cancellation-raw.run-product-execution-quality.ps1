$ErrorActionPreference='Stop'
Set-Location -LiteralPath 'D:/git/arcweft'
$taskTemp='C:/Users/sanze/AppData/Local/Temp/arcweft-1011-convergence-resume-cb798c1d'
$quality='product-execution-quality-'+[DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfff')
if(@(Get-Process -Name cargo,cargo-nextest,rustc,link -ErrorAction SilentlyContinue).Count){throw 'Another Cargo handle is live'}
$previous=Get-Content -LiteralPath (Join-Path $taskTemp 'product-execution-checkpoint.owner.active.txt') -Raw
$input=Get-Content -LiteralPath "$taskTemp/$previous.source-freeze.json" -Raw|ConvertFrom-Json -Depth 20
$freeze=[ordered]@{version=1;base=(git rev-parse HEAD).Trim();previousOwner=$previous;recordedAt=[DateTime]::UtcNow.ToString('o');held=$input.held;paths=@(foreach($row in $input.paths){[ordered]@{path=$row.path;bytes=(Get-Item -LiteralPath $row.path).Length;sha256=(Get-FileHash -LiteralPath $row.path).Hash.ToLowerInvariant()}})}
$freeze|ConvertTo-Json -Depth 15|Set-Content -LiteralPath "$taskTemp/$quality.source-freeze.json" -Encoding utf8
[IO.File]::WriteAllText("$taskTemp/product-execution-quality.active.txt",$quality)
function Assert-Inputs {
 foreach($row in $freeze.paths){if((Get-FileHash -LiteralPath $row.path).Hash.ToLowerInvariant() -ne $row.sha256){throw ('Source drift: '+$row.path)}}
}
function Invoke-Quality([string]$name,[object[]]$arguments) {
 Assert-Inputs
 if(Test-Path -LiteralPath "$taskTemp/$name.json"){throw 'Quality attempt exists'}
 $started=[DateTime]::UtcNow.ToString('o')
 & cargo @arguments *> "$taskTemp/$name.log"
 $code=$LASTEXITCODE
 Assert-Inputs
 [ordered]@{version=1;base=$freeze.base;command=$arguments;exitCode=$code;sourceInputs=$freeze.paths.Count;sourceDrift=0;startedAt=$started;finishedAt=[DateTime]::UtcNow.ToString('o')}|ConvertTo-Json -Depth 8|Set-Content -LiteralPath "$taskTemp/$name.json" -Encoding utf8
 Write-Output "$name exit=$code"
 if($code -ne 0){throw ('Quality failed: '+$name)}
}
Invoke-Quality "$quality.clippy" @('clippy','-p','arcweft-core','--all-targets','--all-features','--locked')
Invoke-Quality "$quality.default-check" @('check','-p','arcweft-core','--all-targets','--locked')
Invoke-Quality "$quality.doctest" @('test','-p','arcweft-core','--doc','--all-features','--locked')
Invoke-Quality "$quality.structure" @('+nightly','-Zscript','tools/structure-audit.rs','--root','.','--fail-on-blocking','--write',"$taskTemp/$quality.structure-report")
$owned=@(git diff --name-only 723e6eb0383a1049ef0faa5a7448360e328bf4db HEAD -- crates/arcweft-core | Where-Object {$_ -like '*.rs'})+@(git diff --name-only -- crates/arcweft-core | Where-Object {$_ -like '*.rs'})
$owned=@($owned|Sort-Object -Unique)
& rustfmt --edition 2024 --config skip_children=true --check @owned *> "$taskTemp/$quality.fmt.log"
$code=$LASTEXITCODE
Assert-Inputs
[ordered]@{version=1;paths=$owned;exitCode=$code;sourceInputs=$freeze.paths.Count;sourceDrift=0}|ConvertTo-Json -Depth 8|Set-Content -LiteralPath "$taskTemp/$quality.fmt.json" -Encoding utf8
if($code -ne 0){throw 'Format failed'}
Write-Output 'quality terminal'