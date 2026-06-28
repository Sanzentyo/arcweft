# Test execution policy excerpt

Exact native visual golden status classes distinguish optional local discovery from milestone evidence:

- `expected_skip`: local/non-Windows exact run without required pinned evidence.
- `environment_not_pinned`: required job did not assert `ARW_EXACT_NATIVE_GOLDEN_PINNED=1`.
- `environment_blocker`: `imq`, `MS Mincho`, or supported backend evidence is missing.
- `baseline_drift`: capture and dimensions are valid, but MSE/MAE exceed fixture bounds.
- `hard_visual_regression`: capture failure, `imq` failure, dimension mismatch, malformed PNG, or missing artifact.

The pinned milestone exact native visual job must run on Windows with:

```powershell
$env:ARW_EXACT_NATIVE_GOLDEN_REQUIRED = "1"
$env:ARW_EXACT_NATIVE_GOLDEN_PINNED = "1"
$env:ARW_EXACT_NATIVE_GOLDEN_BACKEND = "native_rich_text_observer"
just test-visual-golden
just native-visual-artifacts
```

The job must upload `target/arcweft-native-capture-artifacts/` even on failure.
