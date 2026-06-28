# Seq 06.8 visual evidence CI integration disabled design

Date: 2026-06-28
Package: `arcweft-seq06.8-visual-evidence-ci-integration-disabled-design-2026-06-28.zip`
Package SHA-256: `4D560910921804FBC8D541F824B23BEB58FC72B71A848337150ED573BF404AB4`

## Goal

Seq06.8 defines the CI integration boundary for Arcweft visual evidence without enabling GitHub Actions. It consumes seq06.5 selected-capture metadata, seq06.6 visual smoke/golden fixtures, seq06.7 exact native golden drift stabilization, the current test execution policy, exact native golden policy JSON, native drift metadata, existing Justfile recipes, and current disabled CI conventions.

## Applied scope

The package is design-only and disabled by default. Per the package `README.md`, the repository application copies only this implementation note into `docs/implementation/`.

The package also contains a disabled workflow draft, command map, artifact retention design, activation checklist, artifact manifest schema, and example manifest. Those files remain package evidence and are not copied into `.github/`, `Justfile`, or active repository workflow locations by this seq06.8 application.

No repository workflow is enabled by this package.

## CI lane decisions

| Lane | Commands | CI status |
| --- | --- | --- |
| Fast visual smoke | `just test-visual-smoke`; optionally `just test-cli-native` for native CLI capture changes | Future fast PR/local lane. No exact pixels. |
| Workspace fast validation | `just verify` or constituent fast commands; `just test-workspace` remains the workspace fast path | Does not run exact native goldens. |
| Milestone/release exact native golden | pinned Windows env plus `just test-visual-golden` and `just native-visual-artifacts` | Required evidence only when Windows/font/`imq`/backend contract is satisfied. |
| Manual evidence | `just native-visual-artifacts` or `just fixture-refresh-native-capture-candidates` | Diagnostic unless pinned requirements are satisfied and reviewed. |
| Slow Agent/MCP capture | `just test-slow-mcp`, `just test-slow-agent-observe`, or `just test-tier2` | Explicit Tier 2/risky-change lane. |

## Smoke versus exact boundary

- `just test-visual-smoke`, `just test-cli-native`, and `just fixture-refresh-native-capture-check` are not exact native golden gates.
- `just test-visual-golden`, `just native-visual-artifacts`, `just fixture-refresh-native-capture-candidates`, `just test-tier2`, and `just verify-full` include or support exact image comparison evidence.
- Exact native golden remains outside `just test-workspace`.

## Selected-capture metadata handling

Selected object/layer capture metadata is reviewed as protocol evidence. Future CI artifacts should upload scrubbed selected-capture metadata where available, preserve the `image.selected_capture` schema, and review it alongside image dimensions, crop bounds, renderer/scope/composition labels, and source role. Exact pixels are not required for this fast smoke review lane.

## Exact native golden status mapping

| Status | CI meaning |
| --- | --- |
| `expected_skip` | Optional/local diagnostic only; not milestone evidence. |
| `environment_not_pinned` | Required exact job did not assert pinned contract; fail milestone/release. |
| `environment_blocker` | Missing `imq`, pinned font, or backend evidence; fail milestone/release as infrastructure blocker. |
| `baseline_drift` | Valid capture/dimensions but MSE/MAE exceed gates; fail and review artifacts. |
| `hard_visual_regression` | Capture failure, `imq` failure, dimension mismatch, malformed PNG, or missing artifact; fail hard. |

## Mandatory exact native golden environment

A milestone/release exact native golden job counts as evidence only on Windows with:

```powershell
$env:ARW_EXACT_NATIVE_GOLDEN_REQUIRED = "1"
$env:ARW_EXACT_NATIVE_GOLDEN_PINNED = "1"
$env:ARW_EXACT_NATIVE_GOLDEN_BACKEND = "native_rich_text_observer"
```

Additional requirements: `MS Mincho` font probe passes, `imq` is on `PATH`, viewport is `1280x720`, device scale is raw `1.0`, PNG is produced by `arcw agent observe --image png`, and `exact-native-golden.environment.json` is uploaded.

## Artifact groups

| Group | Path |
| --- | --- |
| `visual-smoke-metadata` | smoke logs and selected-capture metadata manifests when generated. |
| `exact-native-golden-run` | `target/arcweft-native-capture-artifacts/`. |
| `exact-native-golden-drift-fixture` | `target/arcweft-native-golden-drift/test-visual-golden/<fixture-id>/`. |
| `native-capture-refresh-candidates` | `target/arcweft-native-capture-refresh/`. |
| `agent-mcp-capture-tier2` | slow Agent/MCP logs and generated capture/resource artifacts. |

Recommended exact fixture artifact name:

```text
arcweft-exact-native-golden-<fixture-id>-<status-class>-<short-sha>-<run-id>-<attempt>
```

## Disabled workflow storage

Workflow draft storage is package-local only:

```text
ci-drafts/visual-evidence.disabled.yml
```

The draft is deliberately not copied into this repository as an active GitHub Actions workflow. A future activation PR must copy or rewrite it into `.github/workflows/`, add real workflow syntax and a deliberate event policy, keep smoke/exact jobs separate, and run validation.

## Justfile decision

No Justfile alias is needed now. Current recipes already map cleanly to the design, and adding aliases in seq06.8 would be unnecessary without activating CI.

## Baseline promotion independence

This design works whether seq06.7.1 promotes a new PNG baseline or defers it. The CI boundary depends on fixture IDs, thresholds, status classes, artifact roots, and environment fingerprints, not on a specific candidate hash.

## Validation status

Package-generation validation checked required files, confirmed no `.github/workflows/` path exists inside the zip, confirmed the disabled draft has no active workflow keys, checked trailing whitespace/final newline, and verified `SHA256SUMS.txt`.

Repository validation after applying this overlay-only note:

```bash
git diff --check
```

Additional source scan confirmed no `.github/workflows/*.yml` or `.github/workflows/*.yaml` file was added or modified by this application.

## Future activation boundary

Future CI activation is a separate human-reviewed change. It must intentionally add or modify the workflow under `.github/workflows/`, convert the package draft into valid workflow syntax, choose event triggers deliberately, upload exact visual artifacts on failure, and update this note plus `docs/implementation/test-execution-policy.md` if behavior changes.
