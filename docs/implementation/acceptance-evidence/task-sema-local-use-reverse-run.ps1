$ErrorActionPreference='Stop'
Set-Location -LiteralPath 'D:/git/arcweft'
$e='C:/Users/sanze/AppData/Local/Temp/arcweft-1011-convergence-resume-cb798c1d'
$g=Get-Content -LiteralPath (Join-Path $e 'task-sema-local-use-reverse.graph.json') -Raw | ConvertFrom-Json
$c='task-final-r3094-traversal-sema-reverse-catalog'
$r='task-final-r3095-traversal-sema-reverse-runtime'
if(Test-Path -LiteralPath (Join-Path $e ($c+'.log'))){throw 'Unique reverse catalog required'}
$packages=@(foreach($p in $g.reverseWorkspace){'-p';$p})
$listArgs=@('nextest','list')+$packages+@('--all-targets','--all-features','--locked','--message-format','json')
& (Join-Path $e 'catalog-preflight-run.ps1') -Name $c -CargoArguments $listArgs
$receipt=Get-Content -LiteralPath (Join-Path $e ($c+'.json')) -Raw | ConvertFrom-Json
if($receipt.exitCode -ne 0){throw 'Reverse catalog build failed'}
& (Join-Path $e 'inspect-presentation-catalog.ps1') -Name $c -Required @('deeply_nested_unannotated_diverging_branches_keep_the_never_result','inferred_control_result_preserves_polymorphic_prefix_schemes','e29_named_block_preserves_valid_and_invalid_present_name_but_normalizes_omission','source_traversal_keeps_owned_collections_and_callbacks_without_starting_children','source_traversal_retains_computed_source_and_callback_operands','source_traversal_does_not_enter_the_callback_body','imported_bindable_parameters_solve_locally_without_mutating_or_exporting_the_parent','imported_rigid_and_future_parameters_cannot_be_specialized_by_a_child')
& (Join-Path $e 'pin-catalog-executables.ps1') -Catalog $c -Output 'task-sema-local-use-reverse-executables-before.json' -Phase 'before-run'
$runArgs=@('nextest','run')+$packages+@('--all-targets','--all-features','--locked','--no-tests','fail','--no-fail-fast','--status-level','all','--final-status-level','all')
& (Join-Path $e 'task-program-body-command.ps1') -Name $r -CargoArgs $runArgs
$actual=$LASTEXITCODE
& (Join-Path $e 'pin-catalog-executables.ps1') -Catalog $c -Output 'task-sema-local-use-reverse-executables-after.json' -Phase 'after-run'
& (Join-Path $e 'audit-nextest-final-result.ps1') -Run $r -Catalog $c -Freeze 'task-sema-local-use-reverse'
exit $actual
