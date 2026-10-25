$ErrorActionPreference='Stop'
Set-Location -LiteralPath 'D:/git/arcweft'
$e='C:/Users/sanze/AppData/Local/Temp/arcweft-1011-convergence-resume-cb798c1d'
$g=Get-Content -LiteralPath (Join-Path $e 'task-handle-carrier.graph.json') -Raw | ConvertFrom-Json
$packages=@(foreach($p in $g.reverseWorkspace){'-p';$p})
$full='task-final-r3100-handle-carrier-full-catalog'
$core='task-final-r3101-handle-carrier-core-catalog'
$owner='task-final-r3102-handle-carrier-core-runtime'
$reverse='task-final-r3103-handle-carrier-consumer-catalog'
$runtime='task-final-r3104-handle-carrier-consumer-runtime'
$baseList=@('nextest','list')+$packages+@('--all-targets','--all-features','--locked','--message-format','json')
& (Join-Path $e 'catalog-preflight-run.ps1') -Name $full -CargoArguments $baseList
$receipt=Get-Content -LiteralPath (Join-Path $e ($full+'.json')) -Raw | ConvertFrom-Json
if($receipt.exitCode -ne 0){exit $receipt.exitCode}
& (Join-Path $e 'inspect-presentation-catalog.ps1') -Name $full -Required @('equal_payloads_with_different_carriers_have_distinct_task_and_need_identities','host_requests_cannot_issue_or_restore_thread_handles_with_the_same_payload','thread_source_uses_original_capture_order_and_retains_only_an_admitted_source','saved_producer_metadata_is_recovered_only_from_the_original_derivation_input','deeply_nested_unannotated_diverging_branches_keep_the_never_result')
& (Join-Path $e 'pin-catalog-executables.ps1') -Catalog $full -Output 'task-handle-carrier-executables-before.json' -Phase 'before-run'
& (Join-Path $e 'catalog-preflight-run.ps1') -Name $core -CargoArguments ($baseList+@('-E','package(=arcweft-core)'))
$coreReceipt=Get-Content -LiteralPath (Join-Path $e ($core+'.json')) -Raw | ConvertFrom-Json
if($coreReceipt.exitCode -ne 0){exit $coreReceipt.exitCode}
& (Join-Path $e 'inspect-presentation-catalog.ps1') -Name $core -Required @('equal_payloads_with_different_carriers_have_distinct_task_and_need_identities','host_requests_cannot_issue_or_restore_thread_handles_with_the_same_payload','thread_source_uses_original_capture_order_and_retains_only_an_admitted_source','saved_producer_metadata_is_recovered_only_from_the_original_derivation_input')
$baseRun=@('nextest','run')+$packages+@('--all-targets','--all-features','--locked','--no-tests','fail','--no-fail-fast','--status-level','all','--final-status-level','all')
& (Join-Path $e 'task-program-body-command.ps1') -Name $owner -CargoArgs ($baseRun+@('-E','package(=arcweft-core)'))
$ownerExit=$LASTEXITCODE
& (Join-Path $e 'audit-nextest-final-result.ps1') -Run $owner -Catalog $core -Freeze 'task-handle-carrier'
& (Join-Path $e 'pin-catalog-executables.ps1') -Catalog $full -Output 'task-handle-carrier-executables-after-core.json' -Phase 'after-run'
if($ownerExit -ne 0){exit $ownerExit}
& (Join-Path $e 'catalog-preflight-run.ps1') -Name $reverse -CargoArguments ($baseList+@('-E','not package(=arcweft-core)'))
$reverseReceipt=Get-Content -LiteralPath (Join-Path $e ($reverse+'.json')) -Raw | ConvertFrom-Json
if($reverseReceipt.exitCode -ne 0){exit $reverseReceipt.exitCode}
& (Join-Path $e 'inspect-presentation-catalog.ps1') -Name $reverse -Required @('deeply_nested_unannotated_diverging_branches_keep_the_never_result','source_traversal_does_not_enter_the_callback_body')
& (Join-Path $e 'task-program-body-command.ps1') -Name $runtime -CargoArgs ($baseRun+@('-E','not package(=arcweft-core)'))
$actual=$LASTEXITCODE
& (Join-Path $e 'pin-catalog-executables.ps1') -Catalog $full -Output 'task-handle-carrier-executables-after.json' -Phase 'after-run'
& (Join-Path $e 'audit-nextest-final-result.ps1') -Run $runtime -Catalog $reverse -Freeze 'task-handle-carrier'
exit $actual
