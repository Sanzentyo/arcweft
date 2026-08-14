# Decision 06 — pair, driver, hot-swap, restore, and replay ownership

`AdmittedRuntimeProduct` owns the admitted plan and AWBC product. Both wrappers
own clones of the same `AdmittedRuntimeGeneration` Arc; construction calls the
public `AdmittedRuntimeGeneration::require_same_parent`, whose implementation
is only `Arc::ptr_eq` on private inners. It cannot reconstruct same-parent
identity from a scalar, and separately issued byte-equal parents fail.

`arcweft_runtime_driver::generation_runtime::RuntimeDriverGeneration` owns:

- one `AdmittedRuntimeProduct`;
- one `AdmittedCharacterDialogueCatalogs` containing immutable `Arc` catalog
  owners and a clone of the same admitted-generation parent.

The driver generation has no raw plan/AWBC field and no replacement setter.
Executors borrow or clone the admitted AWBC wrapper. Same-generation state-
preserving replacement is accepted only by the admitted product-step method;
cross-generation replacement uses `prepare_generation_swap` and an atomic
`commit_generation_swap` guarded by a clone of the exact still-current
admitted parent. The prepared swap does not store only
`RuntimeGenerationIdentity`, so an ABA replacement is detectable.

Bundle/restore/replay order is fixed by `RESTORE_ORDER.csv` and
`BUNDLE_RUNTIME_GENERATION_SECTIONS.md`. The existing
`arcweft_bundle::container::BundleSectionKind` owner receives
`RuntimeGenerationFacts = 23` and `RuntimePlan = 24` in its original enum and
inherent `encoded`/`from_encoded`/policy methods; AWBC remains the existing
`ProgramBytecode = 1` section. All three required sections use schema version
`1`. `arcweft-bundle` first verifies the product AWFB header/TOC, unique
required section set, bounds, overlap, lengths, stored/content/index/manifest
digests, and external payloads, and returns an owned private-field
`VerifiedRuntimeGenerationSections` token. The token exposes checked decode
methods, never replaceable byte access. Only the independent generation-fact
section and catalogs are decoded first and the generation is issued. Plan/AWBC
decode and admission follow; snapshot/replay value payloads are last and use
contexts issued by the admitted product. A fixed snapshot header may be read
for version/generation comparison, but no serialized `RuntimeValue`, plan, or
AWBC payload is decoded before the parent exists.
