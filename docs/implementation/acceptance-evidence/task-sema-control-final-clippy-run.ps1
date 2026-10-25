$ErrorActionPreference='Stop'
Set-Location -LiteralPath 'D:/git/arcweft'
$e='C:/Users/sanze/AppData/Local/Temp/arcweft-1011-convergence-resume-cb798c1d'
& (Join-Path $e 'task-program-body-command.ps1') -Name 'task-final-r3078-sema-control-final-clippy' -CargoArgs @('clippy','-p','arcweft-lang-sema','--all-targets','--all-features','--locked')
$actual=$LASTEXITCODE
& (Join-Path $e 'audit-check-inputs.ps1') -Run 'task-final-r3078-sema-control-final-clippy' -Freeze 'task-sema-control-final-owner'
exit $actual
