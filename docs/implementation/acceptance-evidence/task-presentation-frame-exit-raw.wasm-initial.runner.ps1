$ErrorActionPreference='Stop'
Set-Location -LiteralPath 'D:/git/arcweft'
$taskTemp='C:/Users/sanze/AppData/Local/Temp/arcweft-1011-convergence-resume-cb798c1d'
& "$taskTemp/assert-no-arcweft-build.ps1"
$quality=[IO.File]::ReadAllText("$taskTemp/product-execution-quality.active.txt")
$freeze=Get-Content -LiteralPath "$taskTemp/$quality.source-freeze.json" -Raw|ConvertFrom-Json -Depth 20
if(Test-Path -LiteralPath "$taskTemp/$quality.wasm-check.json"){throw 'Wasm attempt exists'}
function Assert-Inputs {
    if(@($freeze.paths|Where-Object {(Get-FileHash -LiteralPath $_.path).Hash.ToLowerInvariant() -ne $_.sha256}).Count){throw 'Wasm source drift'}
}
Assert-Inputs
$arguments=@('check','-p','arcweft-core','-p','arcweft-web-player','--target','wasm32-unknown-unknown','--locked')
$started=[DateTime]::UtcNow.ToString('o')
& cargo @arguments *> "$taskTemp/$quality.wasm-check.log"
$code=$LASTEXITCODE
Assert-Inputs
[ordered]@{version=1;base=(git rev-parse HEAD).Trim();command=$arguments;exitCode=$code;sourceInputs=$freeze.paths.Count;sourceDrift=0;startedAt=$started;finishedAt=[DateTime]::UtcNow.ToString('o')}|ConvertTo-Json -Depth 8|Set-Content -LiteralPath "$taskTemp/$quality.wasm-check.json" -Encoding utf8
Write-Output "Wasm exit=$code"
exit $code
