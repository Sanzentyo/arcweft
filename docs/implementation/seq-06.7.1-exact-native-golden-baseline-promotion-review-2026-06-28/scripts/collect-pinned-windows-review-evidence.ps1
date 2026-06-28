param(
    [string]$RepoRoot = ".",
    [string]$OutDir = "seq06.7.1-pinned-review-evidence"
)

$ErrorActionPreference = "Stop"

Push-Location $RepoRoot
try {
    $env:ARW_EXACT_NATIVE_GOLDEN_REQUIRED = "1"
    $env:ARW_EXACT_NATIVE_GOLDEN_PINNED = "1"
    $env:ARW_EXACT_NATIVE_GOLDEN_BACKEND = "native_rich_text_observer"

    New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $OutDir "command-logs") | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $OutDir "target-artifacts") | Out-Null

    $log1 = Join-Path $OutDir "command-logs\just-test-visual-golden.log"
    $log2 = Join-Path $OutDir "command-logs\just-native-visual-artifacts.log"

    "# seq06.7.1 pinned Windows evidence collection" | Out-File -Encoding utf8 (Join-Path $OutDir "README.md")
    "Started: $(Get-Date -Format o)" | Add-Content -Encoding utf8 (Join-Path $OutDir "README.md")

    try {
        just test-visual-golden *> $log1
        $testVisualExit = 0
    } catch {
        $testVisualExit = $LASTEXITCODE
        "just test-visual-golden exited with $testVisualExit; baseline_drift can be review input, environment_blocker cannot be promotion evidence." | Add-Content -Encoding utf8 $log1
    }

    just native-visual-artifacts *> $log2
    $nativeArtifactsExit = $LASTEXITCODE
    if ($nativeArtifactsExit -ne 0) {
        throw "just native-visual-artifacts failed with exit code $nativeArtifactsExit"
    }

    Copy-Item -Force target\arcweft-native-capture-artifacts\vertical_tutr_golden.candidate.png (Join-Path $OutDir "target-artifacts\vertical_tutr_golden.candidate.png")
    Copy-Item -Force target\arcweft-native-capture-artifacts\vertical_tutr_golden.observe.json (Join-Path $OutDir "target-artifacts\vertical_tutr_golden.observe.json")
    Copy-Item -Force target\arcweft-native-capture-artifacts\vertical_tutr_golden.imq.json (Join-Path $OutDir "target-artifacts\vertical_tutr_golden.imq.json")
    Copy-Item -Force target\arcweft-native-capture-artifacts\exact-native-golden.environment.json (Join-Path $OutDir "target-artifacts\exact-native-golden.environment.json")

    Get-FileHash -Algorithm SHA256 (Join-Path $OutDir "target-artifacts\*") | Format-Table -AutoSize | Out-String | Out-File -Encoding utf8 (Join-Path $OutDir "SHA256SUMS.windows-artifacts.txt")

    "Completed: $(Get-Date -Format o)" | Add-Content -Encoding utf8 (Join-Path $OutDir "README.md")
    "test_visual_exit=$testVisualExit" | Add-Content -Encoding utf8 (Join-Path $OutDir "README.md")
    "native_artifacts_exit=$nativeArtifactsExit" | Add-Content -Encoding utf8 (Join-Path $OutDir "README.md")
} finally {
    Pop-Location
}
