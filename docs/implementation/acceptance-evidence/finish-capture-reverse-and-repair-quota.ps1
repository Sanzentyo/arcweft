$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath 'D:/git/arcweft'
$taskTemp = 'C:/Users/sanze/AppData/Local/Temp/arcweft-1011-convergence-resume-cb798c1d'
$taskOriginalProcess = Get-CimInstance Win32_Process -Filter 'ProcessId=45820'
if ($taskOriginalProcess -and -not $taskOriginalProcess.CommandLine.Contains('run-program-capture-reverse.ps1')) { throw 'Original runtime driver PID was reused' }
$taskOriginalCreation = $taskOriginalProcess.CreationDate
while ($taskOriginalProcess) {
    Start-Sleep -Seconds 15
    $taskOriginalProcess = Get-CimInstance Win32_Process -Filter 'ProcessId=45820'
    if ($taskOriginalProcess -and $taskOriginalProcess.CreationDate -ne $taskOriginalCreation) { break }
}
$taskAudit = Get-Content -LiteralPath "$taskTemp/task-final-r3177-program-capture-reverse-runtime.terminal-audit.json" -Raw | ConvertFrom-Json
if ($taskAudit.executed -ne 7364 -or $taskAudit.sourceDrift -ne 0 -or $taskAudit.actualExitCode -notin @(0, 100)) { throw 'Original reverse runtime has no complete fixed-input audit' }
$taskBefore = Get-Content -LiteralPath "$taskTemp/task-program-capture-reverse.executables-before.json" -Raw | ConvertFrom-Json
$taskAfter = Get-Content -LiteralPath "$taskTemp/task-program-capture-reverse.executables-after.json" -Raw | ConvertFrom-Json
if ($taskBefore.binaries.Count -ne 227 -or ($taskBefore.binaries | ConvertTo-Json -Depth 8 -Compress) -cne ($taskAfter.binaries | ConvertTo-Json -Depth 8 -Compress)) { throw 'Original reverse executable drift' }
& "$taskTemp/seal-program-capture-reverse.ps1"
if (-not $?) { throw 'Original reverse evidence sealing failed' }
$taskBaseline = Get-Content -LiteralPath "$taskTemp/checkpoint-task-view.source-freeze.json" -Raw | ConvertFrom-Json
if ((git branch --show-current).Trim() -ne 'main' -or (git rev-parse HEAD).Trim() -ne $taskBaseline.base -or @(git diff --cached --name-only).Count) { throw 'Unexpected checkout/index before quota fixture repair' }
if (@(git diff --name-only | Where-Object { $_ -notin $taskBaseline.held }).Count) { throw 'Unrelated tracked edits appeared before quota fixture repair' }
. "$taskTemp/convergence-image-git-bytes.ps1"
$taskHeldBytes = Read-GitBytes (@('diff', '--binary', '--full-index', '--') + $taskBaseline.held)
if ([Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($taskHeldBytes)).ToLowerInvariant() -ne $taskBaseline.heldPatchSha256) { throw 'Original user WIP changed' }
$taskTarget = 'crates/arcweft-compiler/tests/flow_include.rs'
$taskFreeze = Get-Content -LiteralPath "$taskTemp/task-program-capture-reverse.source-freeze.json" -Raw | ConvertFrom-Json
$taskInput = @($taskFreeze.paths | Where-Object path -eq $taskTarget)
if ($taskInput.Count -ne 1 -or (Get-FileHash -LiteralPath $taskTarget -Algorithm SHA256).Hash.ToLowerInvariant() -ne $taskInput[0].sha256) { throw 'Quota fixture differs from audited original input' }
$taskOldHash = $taskInput[0].sha256
$taskBytes = [IO.File]::ReadAllBytes((Join-Path 'D:/git/arcweft' $taskTarget))
$taskBom = $taskBytes.Length -ge 3 -and $taskBytes[0] -eq 239 -and $taskBytes[1] -eq 187 -and $taskBytes[2] -eq 191
$taskText = [IO.File]::ReadAllText((Join-Path 'D:/git/arcweft' $taskTarget))
$taskOld = 'diagnostic.message.contains("Thread start")'
$taskNew = 'diagnostic.message.contains("Program task start")'
if ([regex]::Matches($taskText, [regex]::Escape($taskOld)).Count -ne 1) { throw 'Quota diagnostic expectation is not unique' }
[IO.File]::WriteAllText((Join-Path 'D:/git/arcweft' $taskTarget), $taskText.Replace($taskOld, $taskNew), [Text.UTF8Encoding]::new($taskBom))
$taskInput[0].sha256 = (Get-FileHash -LiteralPath $taskTarget -Algorithm SHA256).Hash.ToLowerInvariant()
$taskInput[0].bytes = (Get-Item -LiteralPath $taskTarget).Length
$taskFreeze.base = (git rev-parse HEAD).Trim()
$taskFreeze.recordedAt = [DateTime]::UtcNow.ToString('o')
$taskFreeze | Add-Member -NotePropertyName owned -NotePropertyValue @($taskTarget) -Force
$taskFreeze | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath "$taskTemp/task-program-quota-expectation.source-freeze.json" -Encoding utf8
[ordered]@{version=1;base=$taskFreeze.base;path=$taskTarget;beforeSha256=$taskOldHash;afterSha256=$taskInput[0].sha256;oldExpectation=$taskOld;newExpectation=$taskNew;reason='ProgramTask generalized the diagnostic owner; budget/category/no-child/no-task assertions are retained';recordedAt=[DateTime]::UtcNow.ToString('o')} | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath "$taskTemp/task-program-quota-expectation.repair.json" -Encoding utf8
$taskGraph = Get-Content -LiteralPath "$taskTemp/task-program-capture-reverse.graph.json" -Raw | ConvertFrom-Json
$taskPackages = @(foreach ($taskPackage in $taskGraph.reverseWorkspace) { '-p'; $taskPackage })
$taskFilter = 'binary(=flow_include)'
$taskCatalog = 'task-final-r3178-program-quota-expectation-catalog'
$taskRun = 'task-final-r3179-program-quota-expectation-runtime'
& "$taskTemp/catalog-preflight-run.ps1" -Name $taskCatalog -CargoArguments (@('nextest', 'list') + $taskPackages + @('--all-targets', '--all-features', '--locked', '-E', $taskFilter, '--message-format', 'json'))
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
& "$taskTemp/audit-check-inputs.ps1" -Run $taskCatalog -Freeze 'task-program-quota-expectation'
& "$taskTemp/inspect-presentation-catalog.ps1" -Name $taskCatalog -ExpectedMatches 18 -Required @('native_thread_quota_rejection_publishes_no_child_or_task')
$taskCatalogReceipt = Get-Content -LiteralPath "$taskTemp/$taskCatalog.json" -Raw | ConvertFrom-Json
$taskCatalogMetadata = ([IO.File]::ReadLines($taskCatalogReceipt.log) | Where-Object { $_.StartsWith('{"rust-build-meta"') } | Select-Object -First 1) | ConvertFrom-Json -Depth 100
$taskMatchedSuites = @($taskCatalogMetadata.'rust-suites'.PSObject.Properties | Where-Object { @($_.Value.testcases.PSObject.Properties | Where-Object { $_.Value.'filter-match'.status -eq 'matches' }).Count })
if ($taskMatchedSuites.Count -ne 1 -or $taskMatchedSuites[0].Value.'binary-id' -ne 'arcweft-compiler::flow_include') { throw 'Unexpected focused executable selection' }
function Write-QuotaExecutablePin([string]$phase) {
    $taskBinaryPath = [IO.Path]::GetFullPath($taskMatchedSuites[0].Value.'binary-path')
    $taskTargetRoot = [IO.Path]::GetFullPath($taskCatalogMetadata.'rust-build-meta'.'target-directory').TrimEnd([char[]]'\/') + [IO.Path]::DirectorySeparatorChar
    if (-not $taskBinaryPath.StartsWith($taskTargetRoot, [StringComparison]::OrdinalIgnoreCase)) { throw 'Selected executable escapes catalog target' }
    [ordered]@{version=1;catalog=$taskCatalog;phase=$phase;binaries=@([ordered]@{path=$taskBinaryPath;bytes=(Get-Item -LiteralPath $taskBinaryPath).Length;sha256=(Get-FileHash -LiteralPath $taskBinaryPath -Algorithm SHA256).Hash.ToLowerInvariant()});recordedAt=[DateTime]::UtcNow.ToString('o')} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath "$taskTemp/task-program-quota-expectation.executables-$phase.json" -Encoding utf8
}
Write-QuotaExecutablePin 'before'
& "$taskTemp/task-program-body-command.ps1" -Name $taskRun -CargoArgs (@('nextest', 'run') + $taskPackages + @('--all-targets', '--all-features', '--locked', '-E', $taskFilter, '--no-fail-fast', '--no-tests', 'fail'))
$taskRuntimeExit = $LASTEXITCODE
& "$taskTemp/audit-nextest-final-result.ps1" -Run $taskRun -Catalog $taskCatalog -Freeze 'task-program-quota-expectation'
Write-QuotaExecutablePin 'after'
$taskBefore = Get-Content -LiteralPath "$taskTemp/task-program-quota-expectation.executables-before.json" -Raw | ConvertFrom-Json
$taskAfter = Get-Content -LiteralPath "$taskTemp/task-program-quota-expectation.executables-after.json" -Raw | ConvertFrom-Json
if (($taskBefore.binaries | ConvertTo-Json -Depth 6 -Compress) -cne ($taskAfter.binaries | ConvertTo-Json -Depth 6 -Compress)) { throw 'Focused runtime executable drift' }
exit $taskRuntimeExit
