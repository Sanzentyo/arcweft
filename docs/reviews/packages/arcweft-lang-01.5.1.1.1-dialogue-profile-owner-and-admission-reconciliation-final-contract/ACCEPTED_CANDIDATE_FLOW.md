# Accepted candidate construction and publication flow

## Transaction rule

“Candidate” below names the existing transaction aggregate; it does not require
a new public wrapper type. All steps operate on typed objects and immutable
`Arc` ownership. No step reconstructs source strings or IDs.

## Exact construction order

### 1. Immutable manifest source

Create one `Arc<SourceDocument>` with a stable `SourceDocumentIdentity`.

### 2. Sole manifest decode

Call exactly once:

```rust
SourceBackedManifest::decode(Arc::clone(&manifest_document))
```

The result contains the same source document, one typed
`ArcweftManifestDocument`, and one generic `ManifestSourceMap`. Decode checks
that the map and accepted object share both `Arc` identity and document
identity.

### 3. Pure launch resolution

Select a `ProfileId` and call `SourceBackedManifest::resolve_profile`. The
operation performs no I/O and no second parse. It produces the typed
`DialoguePresentationProfile`, applying `std.view.dialogue`/none/`fail_line`
for omitted fields.

### 4. Source topology freeze

Project loading accepts manifest/source modules, package manifests, generated
external-module metadata, adapter facts, and resource inputs, then freezes the
source topology and records `SourceSetRevision topology_sources`.

Project-loader stops here for this concern. It does not compile View programs
or validate View capability.

### 5. Compiler product construction

The compiler lowers and validates View and Style definitions into one
`CompiledViewProduct` retaining one `Arc<ValidatedViewProduct>`, source
provenance, the complete product `SourceSetRevision`, View program identity and
accepted revision, Style program if present, and resource-registry digest.

### 6. Checked dialogue admission

The compiler calls `CheckedDialogueProfile::try_admit` with either the retained
`AcceptedLaunchProfileInput` or a project-default input, the compiled product,
and the compiler transaction's `Arc<ResourceTypeRegistry>`.

Admission re-resolves the launch profile from the same `SourceBackedManifest`
and checks exact registry object identity, product revisions, nominal
existence, dialogue capability, and source provenance.

### 7. Revision construction

Only after all checks pass, construct:

```text
DialogueProfileRevision {
  manifest_document,
  topology_sources,
  compiled_sources,
  view_program_id,
  view_program_revision,
  resource_types,
}
```

Every field comes directly from an already accepted typed owner. No field is
recomputed from string serialization.

### 8. `CompiledProject` closure

The compiler returns one `CompiledProject` containing the checked dialogue
profile and the same accepted View product. A compiler error at this stage has
stage `DialogueProfileAdmission` and prevents project construction.

### 9. Runtime-plan lowering

Runtime-plan lowering consumes the checked profile and emits line/display plans
that carry the selected View, optional Style, inline policy, and exact dialogue
profile revision. It does not take raw profile options.

### 10. Codec and bundle closure

AWBC/bundle codecs serialize and deserialize the typed revision and display
facts with strict unknown-field rejection. Round trip must preserve structural
identity and must not drop the product/program revision.

### 11. Program generation

The runtime transaction builds a complete `ProgramGeneration` from the compiled
project, runtime plan, View/Style catalog, resources, and the exact profile
revision. Hot replacement/restore compares the typed revision rather than an ID
or loose digest subset.

### 12. Atomic publication

Publish only the complete generation. Publication must be one commit point.
Before that point, the current generation remains untouched.

## Equality and coherence rules

The candidate is coherent only when:

```text
resolved_profile == pure_re_resolve(same SourceBackedManifest, same ProfileId)
accepted_registry Arc is compiler_registry Arc
accepted_registry.digest == compiled_product.resource_registry_digest
view_program.source_set_revision == compiled_product.complete_source_revision
style_program.source_set_revision == compiled_product.complete_source_revision
selected View exists and accepts dialogue
selected Style, if any, exists
selected View/Style provenance belongs to compiled source identities
all six DialogueProfileRevision fields match the candidate
```

`DialogueProfileRevision::eq` is the cross-consumer equality authority. `Arc`
identity checks remain admission-time object-coherence checks and are not
weakened into digest equality.

## Rejection and rollback

If any step fails:

1. return structured diagnostics bound to the rejected manifest/source;
2. do not mutate the current runtime catalog or profile;
3. do not publish a partial manifest/product/resource revision combination;
4. retain the previous complete `ProgramGeneration` and its six-field revision;
5. do not update save/replay identity; and
6. allow a later candidate to retry from a new complete transaction.

A test must hold the previous generation by identity, reject a deliberately
mismatched candidate, and then assert that every observable backend and save
header still reports the previous revision.

## CLI/LSP sharing rule

CLI and LSP receive the same accepted manifest document and same
`CompiledProject` result. They may render different presentations of the same
structured diagnostics, but they may not call `SourceBackedManifest::decode`
again or independently check View/Style IDs.
