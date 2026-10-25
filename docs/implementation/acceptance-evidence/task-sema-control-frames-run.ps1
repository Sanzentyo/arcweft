$ErrorActionPreference='Stop'
Set-Location -LiteralPath 'D:/git/arcweft'
$e='C:/Users/sanze/AppData/Local/Temp/arcweft-1011-convergence-resume-cb798c1d'
$c='task-final-r3032-hir-control-stack-catalog'
$r='task-final-r3033-hir-control-stack-runtime'
if(Test-Path -LiteralPath (Join-Path $e ($c+'.log'))){throw 'Unique catalog required'}
$filter='test(deeply_nested_unannotated_diverging_branches_keep_the_never_result)'
& (Join-Path $e 'catalog-preflight-run.ps1') -Name $c -CargoArguments @('nextest','list','-p','arcweft-lang-sema','--lib','--all-features','--locked','-E',$filter,'--message-format','json')
$receipt=Get-Content -LiteralPath (Join-Path $e ($c+'.json')) -Raw | ConvertFrom-Json
if($receipt.exitCode -ne 0){throw 'Owner catalog build failed'}
& (Join-Path $e 'inspect-presentation-catalog.ps1') -Name $c -ExpectedMatches 1 -Required @('deeply_nested_unannotated_diverging_branches_keep_the_never_result')
& (Join-Path $e 'pin-catalog-executables.ps1') -Catalog $c -Output 'task-sema-control-frames-executables-before.json' -Phase 'before-run'
& (Join-Path $e 'task-program-body-command.ps1') -Name $r -CargoArgs @('nextest','run','-p','arcweft-lang-sema','--lib','--all-features','--locked','-E',$filter,'--no-tests','fail','--no-fail-fast','--status-level','all','--final-status-level','all')
$actual=$LASTEXITCODE
& (Join-Path $e 'pin-catalog-executables.ps1') -Catalog $c -Output 'task-sema-control-frames-executables-after.json' -Phase 'after-run'
& (Join-Path $e 'audit-nextest-final-result.ps1') -Run $r -Catalog $c -Freeze 'task-sema-control-frames'
exit $actual
