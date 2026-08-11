# Lang-01.5.1 corrected return intake — 2026-08-08

## Scope

This intake classifies three returned design archives against Git commit
`0c8cb74dd96116a8b987cc419c9a280b6cabe4a4`. It changes design authority and
retention records only. It does not claim that Lang-01.5.1.2.1 or
Lang-01.5.1.3 production implementation is complete.

All three archives were copied to `docs/reviews/packages/` after full member
inspection. Their internal manifests matched every recorded byte length and
SHA-256. The Lang-01.5.1.3 `SHA256SUMS` additionally matched all 19 covered
members. Every `OPEN_QUESTIONS.md` is exactly `none\n`.

## Lang-01.5.1.1.1 — accepted as-built confirmation

- package:
  [`arcweft-lang-01.5.1.1.1-dialogue-profile-owner-and-admission-reconciliation-final-contract.zip`](../reviews/packages/zips/arcweft-lang-01.5.1.1.1-dialogue-profile-owner-and-admission-reconciliation-final-contract.zip);
- SHA-256:
  `8B7FE4D8DA08B793AB039E612CCE5A27AF3EC34E39B9FA07533C81C1F901350F`;
- classification: `ACCEPTED_AS_BUILT_CONFIRMATION`;
- package status: `READY_FOR_IMPLEMENTATION`, with
  `CURRENT_MAIN_STATE=SATISFIED_BY_CURRENT_IMPLEMENTATION` and
  `SOURCE_REQUEST_STATE=RESOLVED_DO_NOT_REDISPATCH`.

The return confirms the already selected final owner chain: launch owns the
sole decoder, `SourceBackedManifest`, and generic manifest source map;
dialogue owns the resolved presentation value and the exact six-field
`DialogueProfileRevision`; compiler owns checked admission and retains the
same `Arc<ValidatedViewProduct>`. It authorizes no second map/catalog, no
project-loader-to-runtime-driver dependency, and no resurrection of source
dialogue defaults, `@dialogue.*`, aliases, or dual readers.

This archive does not create another production slice. Current checkout
behavior must still pass its owner-local admission/codec/dependency tests at
the coherent Lang-01.5.1 cut.

## Lang-01.5.1.2.1 — replacement content-root contract

- package:
  [`arcweft-lang-01.5.1.2.1-typed-content-root-admission-and-source-elimination-final-contract.zip`](../reviews/packages/zips/arcweft-lang-01.5.1.2.1-typed-content-root-admission-and-source-elimination-final-contract.zip);
- SHA-256:
  `B1C170170F0EB782FA85E96D933B328F09F4008FAE5EB7BDE20381235DDD7FCB`;
- classification: `ACCEPTED_IMPLEMENTATION_READY_DEFERRED`;
- package status: `READY_FOR_IMPLEMENTATION`; 122 normative test rows.

This package supersedes the content-family decision in
`arcweft-lang-01.5.1.2.1-content-root-family-source-elimination-reconciliation-final-contract-main-5821a3ca.zip`
(`C91C2C...`). The final closed families are exactly `Character`, `Resource`,
and `Activity`. Flow, View, Action, Asset, Signal, Metric, Layer, Source, and
ordinary/generated/external Stream-returning callables are not content roots.

The final authority is one `AcceptedProjectContent` embedded mandatorily in
`ProjectSemanticIndex`, published atomically with `LoadedProfileTopology` as
one `AcceptedProfileProject`. `ProjectTopologyRevision` is the sole accepted
topology identity. Optional absence applies only to an exactly missing
Character manifest with no selected-profile typed reference. Source and the
source `content` declaration are deleted directly; ordinary current grammar
and typed resolution handle old input without a removed-syntax diagnostic,
source gate, compatibility alias, or fallback reader.

Implementation remains ordered after the ordinary-function/generator and
external Stream Source-elimination prerequisites. The accepted binary overlay,
`CharacterPackage`, and topology-revision substrate from earlier work remains
usable; the superseded wider root-family set does not.

## Lang-01.5.1.3 — replacement generated-binding contract

- package:
  [`arcweft-lang-01.5.1.3-generated-artifact-runtime-binding-fail-closed-final-contract.zip`](../reviews/packages/zips/arcweft-lang-01.5.1.3-generated-artifact-runtime-binding-fail-closed-final-contract.zip);
- SHA-256:
  `342D38E521C14F2CCE340355F4F4BC07241C8BFA89DA9B7C324B169869482027`;
- classification: `ACCEPTED_IMPLEMENTATION_READY_DEFERRED`;
- package status: `READY_FOR_IMPLEMENTATION`.

This return supersedes the owner/API selection recorded for the older external
archive with SHA-256 `575DD96C...`. The final shared Sans-I/O owner is
`arcweft-runtime-binding`, not `arcweft-artifact-binding`. `arcweft-id` owns
product-local `GeneratedArtifactBindingId`; the full structural key and
canonical launch product remain outside core; runtime call/function variants
carry only the typed ID. Generated origin is retained directly by
`AdapterFunction` and semantic/runtime evidence, never reconstructed from a
callable, Activity, mount, path, basename, profile, or digest.

The host catalog uses immutable fixed slots and exact complete-key
registration. Missing, stale, unselected, kind-mismatched, mismatched, and
duplicate states fail before callback enqueue, task/scheduler mutation, or
Activity state/registry/event mutation. Catalogs and live bindings are not
serialized. Dynamic-library loading, WASM/process execution, provider
discovery, and successful artifact execution are explicit non-goals.

Implementation remains after the accepted single-manifest/topology, generated
callable publication, typed resource, and content-root prerequisites. The
negative/mismatch/stale/no-partial-work matrix lands before the one in-memory
sentinel success; no provider or loader is inferred.

## Resulting dependency decision

These returns remove Lang-01.5.1.1.1, Lang-01.5.1.2.1, and Lang-01.5.1.3 from
the external-design-wait set. They do not reorder the active Proof/AW-AH-009.3
public switch or authorize skipping Lang-01.5.1.2.1 prerequisites. Older
packages remain retained as immutable audit inputs but their superseded owner
or root-family decisions must not be implemented.
