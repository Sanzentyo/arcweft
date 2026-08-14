# Decision 02 — public raw data is not operational authority

A public checked constructor proves only that a raw claim is structurally and
canonically representable. It cannot mint project roots, producer roots,
nominal domains, catalog provenance, an admitted generation, an admitted plan,
an admitted AWBC product, or an executor.

The authority cut is observable in the type graph:

```text
public RuntimePlanBuilder/AwbcProgramBuilder -> raw RuntimePlan/AwbcProgram
raw RuntimePlan/AwbcProgram -X-> AdmittedRuntimeGeneration
AdmittedRuntimeGeneration + raw artifact -> admitted wrapper
```

There is no caller-name, source-string, crate-path, feature, or workspace gate.
The real external lowerer is proved by a `trybuild` pass case. Separate
compile-fail cases prove that an unrelated crate cannot use struct literals,
private fields, `Default`, private wire DTOs, unchecked IDs, or admitted-wrapper
constructors. Runtime tests prove that valid raw Serde still yields only raw
data and that changing every self-declared root consistently does not alter the
independently issued generation.

`AUTHORITY_BOUNDARY_TESTS.csv` is the exact focused matrix; all rows are also in
`TEST_MATRIX.csv`.
