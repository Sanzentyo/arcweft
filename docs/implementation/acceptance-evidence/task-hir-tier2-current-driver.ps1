$ErrorActionPreference='Stop'
Set-Location -LiteralPath 'D:/git/arcweft'
$taskEvidence='C:/Users/sanze/AppData/Local/Temp/arcweft-1011-convergence-resume-cb798c1d'
$inputs=Get-Content -LiteralPath (Join-Path $taskEvidence 'task-hir-tier2-current-input-audit.json') -Raw | ConvertFrom-Json
& (Join-Path $taskEvidence 'inspect-presentation-catalog.ps1') -Name 'task-final-r3126-hir-tier2-production-catalog' -ExpectedMatches 8 -Required $inputs.required.test
& (Join-Path $taskEvidence 'pin-catalog-executables.ps1') -Catalog 'task-final-r3126-hir-tier2-production-catalog' -Output 'task-hir-tier2-current-executables-before.json' -Phase 'before-run'
& (Join-Path $taskEvidence 'task-program-body-command.ps1') -Name 'task-final-r3127-hir-tier2-production-runtime' -CargoArgs @('nextest','run','--binaries-metadata',(Join-Path $taskEvidence 'task-hir-tier2-current.binaries.json'),'--cargo-metadata',(Join-Path $taskEvidence 'task-final-r2285.metadata.json'),'--run-ignored','ignored-only','--test-threads','1','--no-tests','fail','--no-fail-fast','--status-level','all','--final-status-level','all')
$runtimeExit=$LASTEXITCODE
& (Join-Path $taskEvidence 'audit-nextest-final-result.ps1') -Run 'task-final-r3127-hir-tier2-production-runtime' -Catalog 'task-final-r3126-hir-tier2-production-catalog' -Freeze 'task-handle-codec-repair'
& (Join-Path $taskEvidence 'pin-catalog-executables.ps1') -Catalog 'task-final-r3126-hir-tier2-production-catalog' -Output 'task-hir-tier2-current-executables-after.json' -Phase 'after-run'
exit $runtimeExit
