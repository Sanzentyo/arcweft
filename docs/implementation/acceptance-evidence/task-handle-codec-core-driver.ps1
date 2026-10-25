$ErrorActionPreference='Stop'
$taskEvidence='C:/Users/sanze/AppData/Local/Temp/arcweft-1011-convergence-resume-cb798c1d'
Set-Location -LiteralPath 'D:/git/arcweft'
& (Join-Path $taskEvidence 'inspect-presentation-catalog.ps1') -Name 'task-final-r3117-handle-codec-core-catalog' -Required @('thread_source_codec_retains_program_definition_and_capture_roles','host_template_sources_cannot_be_relabelled_as_program_work','need_request_values_share_live_and_snapshot_resource_limits_in_native_and_awbc','native_fanout_values_borrow_the_enclosing_resource_bank','native_waits_share_live_inert_and_parent_node_limits')
& (Join-Path $taskEvidence 'pin-catalog-executables.ps1') -Catalog 'task-final-r3117-handle-codec-core-catalog' -Output 'task-handle-codec-core-executables-before.json' -Phase 'before-run'
& (Join-Path $taskEvidence 'task-program-body-command.ps1') -Name 'task-final-r3118-handle-codec-core-runtime' -CargoArgs @('nextest','run','-p','arcweft-core','--all-targets','--all-features','--locked','--no-tests','fail','--no-fail-fast','--status-level','all','--final-status-level','all')
$runtimeExit=$LASTEXITCODE
& (Join-Path $taskEvidence 'audit-nextest-final-result.ps1') -Run 'task-final-r3118-handle-codec-core-runtime' -Catalog 'task-final-r3117-handle-codec-core-catalog' -Freeze 'task-handle-codec-repair'
& (Join-Path $taskEvidence 'pin-catalog-executables.ps1') -Catalog 'task-final-r3117-handle-codec-core-catalog' -Output 'task-handle-codec-core-executables-after.json' -Phase 'after-run'
exit $runtimeExit
