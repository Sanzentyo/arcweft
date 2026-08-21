# Repository evidence

## 1. Baseline

- Repository: `Sanzentyo/arcweft`
- Request-observed commit: `6e17c9fafe7c254b27e99f51af52ccc109a3a41d`
- Inspected current `origin/main`: `9138efeeabdfca56809e8ad9c16fc85380ae18c5`
- Latest commit subject observed through the GitHub connector: `Advance task await fixture semantics`

The design uses the newer SHA because current main had advanced.

## 2. Instructions read

The following were read at `9138efeeabdfca56809e8ad9c16fc85380ae18c5`:

- `AGENTS.md`
- `crates/AGENTS.md`
- `docs/AGENTS.md`
- `docs/reviews/AGENTS.md`
- `docs/implementation/AGENTS.md`
- `docs/README.md`
- `docs/reviews/README.md`
- `docs/00-overview/crate-map.md`

The local `Rust Skill.txt` was read through its final line. The design follows domain-owned inherent behavior, typed IDs/errors, deterministic collections, compile-clean deletion, no `mod.rs`, and no unnecessary compatibility surfaces.

## 3. Request and fixture evidence

- Request: `2026-08-21-lang-01.1.1.3.2-suspended-function-runtime-emission-and-opaque-nominal-layout-reconciliation.md`
- Fixture: `tests/fixtures/arcw/current_pass/check/013_task_fn_await_shape.arcw`
- Progress note: `docs/implementation/2026-08-21-task-fn-await-positive-fixture-sema-progress.md`

Observed fixture structure:

- `OpeningAssets` is a project struct with `bg: ImageHandle`;
- `load_opening_assets()` performs `try await load_bg()`;
- `flow main()` returns a constant string and does not call the function.

## 4. Source evidence table

| Path | Blob SHA observed | Relevant current behavior |
|---|---|---|
| `crates/arcweft-lang-hir/src/final_project/runtime_semantic_owners.rs` | `2024338e1b03e8a47c164ed47ed26eb24bf5b794` | Builds all owner sets and subtracts View/Style presentation closure; ordinary function reachability is not considered. |
| `crates/arcweft-lang-hir/src/final_project/selected_expressions.rs` | `372171a127956574252b064521343a1ae67310e0` | HIR owns selected-expression traversal and accepts semantic callbacks for selected postfix/call disposition. |
| `crates/arcweft-lang-sema/src/final_analysis/nominal_schema.rs` | `9d28f585e3f375f0240021585f78f05d70705c75` | Explicitly describes itself as persistence schema projection; unsupported checked types become `InvalidShape` string errors. |
| `crates/arcweft-lang-sema/src/final_analysis/model.rs` | `fbf61b149910c5321e0b67562013c9d85f4d810e` | Owns `CheckedFunctionExecution`, `CheckedSuspensionRole`, and `CheckedItemRole::Function`. |
| `crates/arcweft-compiler/src/lower.rs` | `74b6d1f971b02b5eda14506e10d9b3894ec53393` | `runtime_nominal` calls `project_nominal_schema`, converts to `RuntimeSchemaProjection`, hashes layout, and then constructs `RuntimeResolvedNominal`; this is the concrete blocker. |
| `crates/arcweft-core/src/awbc/schema.rs` | `c33d695e1bfe7fb74d62583bce1806d490299245` | `AWBC_ABI_VERSION=1`, `AWBC_CODEC_VERSION=1`; Nominal/NominalRecord have layout bytes and Opaque has producer/semantic/admission/arguments. |
| `crates/arcweft-runtime-driver/src/session_save.rs` | `3de21cd04e75b18c22657423307641c218f79b0f` | Bundle session save schema version is 1 and AWBC snapshots are validated against the active program. |

## 5. Maintained contracts inspected

- ordinary function role closure and direct suspension implementation/status docs;
- checked callable catalog authority package;
- opaque composite checked-type owner package;
- nominal record/record sequence owner package;
- nominal runtime external admission package;
- current runtime semantic facts/final Flow planning source;
- nominal record and opaque value owners;
- AWBC type projection/schema/verifier/VM consumers;
- AWBC save snapshot and runtime-driver session save.

The accepted nominal external-admission contract retains `RuntimeNominalRecordLayout`, defining order, `RuntimeNominalRecordValue`, and schema-derived layout identity. This evidence is why this design does not introduce a second transient project nominal layout merely to accommodate an unreachable function.

## 6. Evidence-derived conclusion

The narrow defect is publication scope: compiler runtime projection currently receives every non-presentation HIR owner, while the runtime planner already avoids turning every ordinary declaration into an executable. A generation-bound reachability product aligns those layers and makes fixture 013 succeed without weakening nominal or opaque contracts.
