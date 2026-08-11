# Mandatory deletion matrix

| ID | Existing/intermediate path to remove | Replacement and acceptance evidence |
|---|---|---|
| D-01 | generated functions inserted with ordinary `with_function_signature` and no origin | `with_generated_function_signature(..., id)`; T-01 |
| D-02 | separate `extend_selected_adapter` result plus `validate_activity_bindings` validation-only pass | one atomic `project_selected_external_modules`; T-03/T-05/T-07 |
| D-03 | generated call lowered through `RuntimeCallTarget::from_label` / `Named(String)` | direct generated variant; P-01 |
| D-04 | generated function reference represented only by callable string | generated `RuntimeFunctionBody`; P-02 |
| D-05 | partial generated call represented only by callable string | same retained ID plus captures; P-03/P-04 |
| D-06 | generated identity reconstructed from mount/path/callable spelling after sema | origin copied in existing callable/evidence owners; P-01–P-04 |
| D-07 | `RuntimeCallTarget::as_label() -> &str` assumption across all variants | variant-aware `named_label()` and direct `Display`; exhaustive compile/tests |
| D-08 | any path/function/Activity/profile/basename/digest fallback resolver | fixed slot lookup by selected ID after exact registration; F-01–F-11 |
| D-09 | provider claim accepted without complete key comparison | registration requires ID + full claimed key; M-series |
| D-10 | catalog construction fails merely because a selected binding is absent | immutable catalog permits empty slot; E-01/E-02 fail at attempt |
| D-11 | catalog reused after source revision/profile change | topology stale gate; S-01–S-06 |
| D-12 | LSP copies catalog/slots to replacement generation | generation-owned fresh lease; S-07/S-08 |
| D-13 | Activity resolution after state/registry/event/task mutation | pre-start resolve gate; N-06–N-08 |
| D-14 | function resolution after callback/task enqueue | pre-dispatch resolve gate; N-01–N-05 |
| D-15 | compatibility schema/serde alias/default/migration reader added during work | strict schema 1 only; W-04–W-23 |
| D-16 | key digest treated as equality/lookup authority | typed structural equality only; M-series |
| D-17 | extension trait/helper matches for Arcweft-owned target/call enums | inherent owner behavior; compile/API review |
| D-18 | parallel path-to-binding or expression-to-binding side map | direct fields in `AdapterFunction`, semantic record, evidence, runtime variants |
| D-19 | binding/catalog serialization into bundle/save | serialize product only; codec/type review |
| D-20 | last-known-good/parent catalog chain | no such type/branch; F-10/S-series |
| D-21 | synthetic `ProfileId`/empty launch product/catalog for `ProjectCompilationContext::accepted_launch_profile == None` | exact `None`; P-11–P-14/R-10 |
| D-22 | Activity implementation or binding ID recovered from Activity/export spelling or an ad hoc side map | product-owned `GeneratedArtifactActivitySelection`; T-03/P-15/F-11 |

A grep showing a name absent is supplemental. The cited typed behavior tests and exhaustive consumer compilation are the primary deletion evidence.
