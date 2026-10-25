$ErrorActionPreference='Stop'
Set-Location -LiteralPath 'D:/git/arcweft'
$e='C:/Users/sanze/AppData/Local/Temp/arcweft-1011-convergence-resume-cb798c1d'
& (Join-Path $e 'task-program-body-command.ps1') -Name 'task-final-r3096-sema-local-use-workspace-check' -CargoArgs @('check','--workspace','--all-targets','--all-features','--locked')
$checked=$LASTEXITCODE
& (Join-Path $e 'audit-check-inputs.ps1') -Run 'task-final-r3096-sema-local-use-workspace-check' -Freeze 'task-sema-local-use-reverse'
if($checked -ne 0){exit $checked}
& (Join-Path $e 'task-program-body-command.ps1') -Name 'task-final-r3097-sema-local-use-workspace-clippy' -CargoArgs @('clippy','--workspace','--all-targets','--all-features','--locked')
$lint=$LASTEXITCODE
& (Join-Path $e 'audit-check-inputs.ps1') -Run 'task-final-r3097-sema-local-use-workspace-clippy' -Freeze 'task-sema-local-use-reverse'
exit $lint
