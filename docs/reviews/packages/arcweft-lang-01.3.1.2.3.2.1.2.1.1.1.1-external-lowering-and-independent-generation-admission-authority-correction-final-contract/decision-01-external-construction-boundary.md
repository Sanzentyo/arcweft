# Decision 01 — external checked construction boundary

## Selected owners

- plan primitive/wrapper owner: `arcweft_core::plan::typed_sites`;
- plan aggregate builder owner: `arcweft_core::plan::construction`;
- AWBC primitive/wrapper owner: `arcweft_core::awbc::typed_sites`;
- AWBC aggregate builder owner: `arcweft_core::awbc::construction`;
- legitimate caller: `arcweft-runtime-plan` final lowerer.

All invariant-bearing raw fields are private. Public read-only accessors expose
finished data. Public checked constructors and builders are intentionally
callable across the crate boundary. They validate shape, index/path grammar,
cardinality, canonical ordering, duplicate IDs, table references, and the
closed typed-node/slot vocabulary; they do not admit execution.

`RuntimePlanBuilder::finish` and `AwbcProgramBuilder::finish` consume staging
state and return the finished raw aggregate only after all checks pass. Failure
publishes no aggregate. Custom version-1 `Deserialize` implementations decode a
private `*WireV1` candidate and call the same finish path. `Default`, public
struct literals, post-finish mutation, unchecked IDs, and a second wire DTO are
removed.

Exact declarations and every builder method are in `RAW_CONSTRUCTION_API.md`
and `RAW_CONSTRUCTION_SURFACE.csv`.
