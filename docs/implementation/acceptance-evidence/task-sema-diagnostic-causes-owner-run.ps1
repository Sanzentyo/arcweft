$ErrorActionPreference='Stop'
Set-Location -LiteralPath 'D:/git/arcweft'
$e='C:/Users/sanze/AppData/Local/Temp/arcweft-1011-convergence-resume-cb798c1d'
$c='task-final-r3082-control-result-closure-catalog'
$r='task-final-r3083-control-result-closure-runtime'
if(Test-Path -LiteralPath (Join-Path $e ($c+'.log'))){throw 'Unique catalog required'}
$filter='test(=final_analysis::tests::generic_calls::traversal_await_match_infers_an_empty_error_branch_without_a_result_annotation)'
& (Join-Path $e 'catalog-preflight-run.ps1') -Name $c -CargoArguments @('nextest','list','-p','arcweft-lang-sema','--all-targets','--all-features','--locked','--message-format','json')
$receipt=Get-Content -LiteralPath (Join-Path $e ($c+'.json')) -Raw | ConvertFrom-Json
if($receipt.exitCode -ne 0){throw 'Owner catalog build failed'}
& (Join-Path $e 'inspect-presentation-catalog.ps1') -Name $c -ExpectedMatches 1357 -Required @('inferred_control_result_preserves_polymorphic_prefix_schemes','traversal_await_match_infers_an_empty_error_branch_without_a_result_annotation','unannotated_control_results_share_context_in_both_branch_orders','control_result_inference_replays_generic_calls_with_the_original_capture','control_result_inference_rejects_conflicts_and_missing_element_evidence','contextual_results_constrain_direct_and_nested_generic_arguments_before_probing','deeply_nested_unannotated_diverging_branches_keep_the_never_result','generic_nested_result_mismatch_still_checks_each_outer_candidate_source','failed_implicit_branch_synthesis_restores_scope_before_contextual_recheck','nested_call_is_rechecked_inside_each_outer_candidate_pass')
& (Join-Path $e 'pin-catalog-executables.ps1') -Catalog $c -Output 'task-sema-diagnostic-causes-owner-executables-before.json' -Phase 'before-run'
& (Join-Path $e 'task-program-body-command.ps1') -Name $r -CargoArgs @('nextest','run','-p','arcweft-lang-sema','--all-targets','--all-features','--locked','--no-tests','fail','--no-fail-fast','--status-level','all','--final-status-level','all')
$actual=$LASTEXITCODE
& (Join-Path $e 'pin-catalog-executables.ps1') -Catalog $c -Output 'task-sema-diagnostic-causes-owner-executables-after.json' -Phase 'after-run'
& (Join-Path $e 'audit-nextest-final-result.ps1') -Run $r -Catalog $c -Freeze 'task-sema-diagnostic-causes-owner'
exit $actual
