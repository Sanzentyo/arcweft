# Decision 03 — independent admitted-generation issuer

The standard accepted-world orchestrator is
`arcweft_compiler::project::runtime_generation::ProjectRuntimeGenerationAssembly<'project>`.
It is compiler-internal (`pub(crate)`), non-Serde, and borrows these already
accepted owners:

- project semantic facts: `arcweft_runtime_plan::RuntimePlanSemanticFacts`;
- nominal catalog/world: `arcweft_lang_sema::AcceptedNominalWorld`;
- exact opaque producers: `AcceptedRuntimeOpaqueProducerRegistry` owned by
  `arcweft-lang-sema::registration::runtime_producers`;
- CharacterDialogue role and custom-field registries from the same registered
  nominal world;
- `CharacterCatalog`, `ViewRegistry`, and their canonical digest APIs;
- the accepted compiler project/snapshot inventory that determines the one
  generation transcript.

The assembly copies only lower-layer scalar/checked facts into a core-owned
`RuntimeGenerationAdmissionProjection`, then consumes that projection through
`AdmittedRuntimeGeneration::try_issue`. Core computes the version-1 generation
transcript and declaration, validates all rows, and publishes one immutable
`Arc<AdmittedRuntimeGenerationInner>`.

No raw plan/AWBC object is accepted by the assembly or projection builder. The
issued generation exists before either artifact is lowered. Exact APIs and
owner fields are in `GENERATION_ISSUANCE_API.md` and
`ACCEPTED_WORLD_OWNER_MAP.csv`.
