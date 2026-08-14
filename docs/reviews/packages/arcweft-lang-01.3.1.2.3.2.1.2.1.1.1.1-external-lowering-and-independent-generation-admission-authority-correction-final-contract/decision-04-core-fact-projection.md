# Decision 04 — core-owned lower-layer fact projection

`arcweft-core` does not name HIR, sema, runtime-plan, dialogue, View, or compiler
types. It consumes owned, non-Serde projection rows:

- `RuntimeProjectTypeProjection`;
- `RuntimeProducerTypeProjection`;
- `RuntimeNominalRecordProjection`;
- `RuntimeGenerationCatalogProjection`;
- `RuntimeGenerationAdmissionProjection`.

Projection rows contain only core IDs, semantic identities, checked or closed
operational shapes, layouts, ordered field IDs, producer identities, roots,
and canonical catalog digests. They contain no source text, HIR IDs, spans,
raw artifact declaration, borrowed catalog object, or callback.

The compiler assembly borrows accepted owners only while constructing rows; the
projection owns the copied values and is consumed once. There is no `Clone`,
Serde, `Default`, public field, `From<RuntimePlan>`, `From<AwbcProgram>`, or
`TryFrom` from either raw declaration table.

Input order is canonical and checked, never repaired: project rows strictly
increase by semantic identity; producer rows by `(producer, semantic_identity)`;
nominal rows by `(nominal, semantic_identity)`. Equality is `Duplicate*`; a
decrease is `NonCanonical*Order`. Root IDs are recomputed losslessly from the
semantic ID; caller-provided roots do not exist. Exact producer rows require an
`ExactIdentity` opaque owner. Error precedence is fixed in
`ERROR_PRECEDENCE.csv`.
