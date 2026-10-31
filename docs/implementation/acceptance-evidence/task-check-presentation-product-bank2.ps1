$ErrorActionPreference='Stop'
Set-Location -LiteralPath 'D:/git/arcweft'
$arcTemp='C:/Users/sanze/AppData/Local/Temp/arcweft-1011-convergence-resume-cb798c1d'
$arcFreeze=Get-Content -LiteralPath "$arcTemp/presentation-product-bank.owner2.source-freeze.json" -Raw|ConvertFrom-Json -Depth 20
if(@(Get-Process -Name cargo,cargo-nextest,rustc,link -ErrorAction SilentlyContinue).Count){throw 'Another Cargo/rustc/link handle is live'}
function Assert-BankInputs {
 if(@($arcFreeze.paths|Where-Object {(Get-FileHash -LiteralPath $_.path).Hash.ToLowerInvariant() -ne $_.sha256}).Count){throw 'Bank input drift'}
}
$arcCommands=@(
 [pscustomobject]@{name='check';arguments=@('check','-p','arcweft-core','--all-targets','--all-features','--locked')},
 [pscustomobject]@{name='clippy';arguments=@('clippy','-p','arcweft-core','--all-targets','--all-features','--locked')},
 [pscustomobject]@{name='doctest';arguments=@('test','-p','arcweft-core','--doc','--all-features','--locked')},
 [pscustomobject]@{name='default-check';arguments=@('check','-p','arcweft-core','--all-targets','--locked')}
)
foreach($arcCommand in $arcCommands){
 $arcName='presentation-product-bank.owner2.'+$arcCommand.name
 if(Test-Path -LiteralPath "$arcTemp/$arcName.json"){throw 'Bank check receipt exists'}
 Assert-BankInputs
 $arcStarted=[DateTime]::UtcNow.ToString('o')
 $arcArguments=$arcCommand.arguments
 & cargo @arcArguments *> "$arcTemp/$arcName.log"
 $arcExit=$LASTEXITCODE
 Assert-BankInputs
 [ordered]@{version=1;base=$arcFreeze.base;command=$arcCommand.arguments;log="$arcTemp/$arcName.log";exitCode=$arcExit;sourceInputs=$arcFreeze.paths.Count;sourceDrift=0;startedAt=$arcStarted;finishedAt=[DateTime]::UtcNow.ToString('o')}|ConvertTo-Json -Depth 8|Set-Content -LiteralPath "$arcTemp/$arcName.json" -Encoding utf8
 if($arcExit -ne 0){throw ('Bank '+$arcCommand.name+' failed')}
}
'Bank check/Clippy/doctest/default-check terminal: PASS'
