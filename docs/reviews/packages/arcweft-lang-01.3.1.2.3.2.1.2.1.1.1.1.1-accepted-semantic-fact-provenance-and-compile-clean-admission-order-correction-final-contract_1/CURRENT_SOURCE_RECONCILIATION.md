# Current-source reconciliation

## Baseline

The latest GitHub `main` history row is `35d42efdd89fef8fde73f62be2a3e38fd5e81e52` and equals the user-provided
confirmed SHA. All source observations in this package are pinned to that full
commit. The current workspace manifest contains `arcweft-core`,
`arcweft-runtime-plan`, `arcweft-compiler`, `arcweft-bundle`,
`arcweft-runtime-driver`, `arcweft-runtime-codegen`,
`arcweft-lang-jit-cranelift`, `arcweft-runtime-accelerator`, `arcweft-save`, and
`arcweft-verify`. It contains no `arcweft-aot`, `arcweft-lang-aot-rust`, or
`arcweft-lang-vm` package. This contract therefore assigns VM ownership to the
core AWBC VM plus runtime-driver publication, and AOT/native generation to
`arcweft-runtime-codegen`.

## Implemented substrate retained verbatim

No new competing owner is introduced for:

- HIR runtime semantic owner inventory;
- accepted expression, pattern, and local normalized type facts;
- canonical runtime local declaration table;
- `RuntimePatternBindingCoordinate` and its exact v1 tags;
- `RuntimeValuePath` / `RuntimeCheckedTypePath`;
- checked/operational `RuntimePlanTypeKind` classification;
- `RuntimePlanTypeId` and its first-seen plan-local interner;
- normalized variant case tables;
- opaque owner/value/payload ownership paths;
- runtime value outer-shape classification;
- project/producer root scalar projection;
- Character/View canonical catalog digests;
- record-field identity, nominal record layout, anonymous/column admission; or
- any Arcweft-owned version number other than `1` (none is allocated).

## Residual source mismatch closed here

Current `RuntimePlan` and `AwbcProgram` still expose raw construction surfaces;
current final lowerers still return raw expression/pattern nodes; synthetic
nodes have no retained accepted-type coordinate; the nominal field projection
lacks a public checked constructor; the AWBC nominal-domain table is absent;
and no layer-correct verified publication token joins compiler, bundle, driver,
VM/JIT/AOT, swap, restore, and replay. Those—not the landed substrate—are the
only production cuts specified by this contract.
