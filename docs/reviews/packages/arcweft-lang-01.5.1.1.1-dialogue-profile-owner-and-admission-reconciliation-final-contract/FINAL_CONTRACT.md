# Final contract

## 1. Scope

This contract governs only the profile-level selection and admission of the
Dialogue View, optional Style sheet, and inline interpolation failure policy.
It does not redesign Character identity, View grammar, Style cascade,
prepared-text geometry, dialogue reveal/advance/input-wait state, renderer
backends, or resource-extension manifest syntax.

## 2. Final owner table

| Authority | Exact owner | Contract |
|---|---|---|
| Neutral manifest identities/hashes/wire primitives | `arcweft-manifest-model` | No presentation dependency and no decoder |
| Schema-1 document/spec/sole decoder/generic source map | `arcweft-launch` | One `SourceBackedManifest` from one immutable source document |
| Dialogue presentation value and strict inline policy | `arcweft-dialogue` | Domain-owned typed behavior and defaults |
| Reusable six-field revision value | `arcweft-dialogue` | Reachable by compiler, runtime-plan, codecs, save/replay without a cycle |
| Validated View/Style compilation product | compiler/bundle product path | One immutable product, not a project-loader or runtime-driver shadow catalog |
| Cross-product checked admission | `arcweft-compiler` | Sole `CheckedDialogueProfile::try_admit` operation |
| Frozen source topology | project-loader/project source boundary | Does not compile View programs |
| Runtime display selection | runtime-plan/runtime consumers | Consume the checked result; never reparse or re-resolve the manifest |

## 3. Launch-owned source model

`DialogueProfileSpec` remains a strict, crate-private schema record in
`arcweft-launch`:

```rust
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub(crate) struct DialogueProfileSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) view: Option<ViewId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) style: Option<ViewStyleSheetId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) inline_failure: Option<InlineFailurePolicy>,
}
```

The Rust field `inline_failure` is serialized/deserialized by the enclosing
kebab-case schema as `inline-failure`. There is no alias.

## 4. Dialogue-owned resolved value

`DialoguePresentationProfile` is immutable and contains exactly:

```rust
view: ViewId,
style: Option<ViewStyleSheetId>,
inline_failure: InlineFailurePolicy,
```

Pure resolution is field-wise:

```text
view            = authored view or ViewId::standard_dialogue()
style           = authored optional style
inline_failure  = authored policy or InlineFailurePolicy::FailLine
```

The engine default is therefore `std.view.dialogue`, no profile base style, and
`fail_line`.

## 5. Exact checked owner

`arcweft-compiler` owns:

```rust
pub struct CheckedDialogueProfile {
    owner: DialogueProfileOwner,
    presentation: DialoguePresentationProfile,
    revision: DialogueProfileRevision,
    product: Arc<ValidatedViewProduct>,
    selected_view_source: SourceSpan,
    selected_style_source: Option<SourceSpan>,
}
```

`DialogueProfileOwner` is exactly:

```rust
pub enum DialogueProfileOwner {
    Launch(ProfileId),
    ProjectDefault,
}
```

No unchecked constructor is part of the consumer contract. The single
construction operation is compiler-internal `try_admit`, after the compiler has
one `CompiledViewProduct`.

## 6. Exact admission input and operation

```rust
pub(crate) enum DialogueProfileAdmissionInput<'a> {
    Launch(&'a AcceptedLaunchProfileInput),
    ProjectDefault {
        manifest: &'a SourceDocument,
        topology_sources: SourceSetRevision,
    },
}

pub(crate) fn try_admit(
    input: DialogueProfileAdmissionInput<'_>,
    views: &CompiledViewProduct,
    compiler_resource_types: &Arc<ResourceTypeRegistry>,
) -> Result<CheckedDialogueProfile, DialogueProfileAdmissionError>;
```

The operation is Sans-I/O. It reads no path, source text, TOML, second manifest,
or runtime catalog.

## 7. Admission invariants

For a launch-owned profile, admission must perform all of the following in one
operation:

1. Re-resolve the selected `ProfileId` from the same retained
   `SourceBackedManifest`.
2. Require the re-resolved value and retained resolved value to be equal.
3. Require `Arc::ptr_eq(accepted_registry, compiler_registry)`.
4. Require the registry digest to equal the digest recorded by the compiled
   View product.
5. Require the accepted product to contain a View program.
6. Require the View program source-set revision to equal the complete product
   source revision.
7. If a Style program exists, require its source-set revision to equal the same
   complete product source revision.
8. Require the selected nominal `ViewId` to resolve in that View program.
9. Require that definition to accept the canonical dialogue input role.
10. Require exact source provenance for the selected View.
11. If a Style is selected, require its nominal ID to resolve in the accepted
    Style program and require exact source provenance.
12. Build the six-field revision only after every check passes.
13. Retain an `Arc` clone of the exact accepted `ValidatedViewProduct`.

Project-default admission uses the same product and compiler registry, but
builds `DialoguePresentationProfile::engine_default()` and uses the supplied
manifest document/start span and topology revision.

## 8. Six-field revision authority

The reusable lower value is exactly:

```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueProfileRevision {
    manifest_document: SourceDocumentIdentity,
    topology_sources: SourceSetRevision,
    compiled_sources: SourceSetRevision,
    view_program_id: ViewProgramId,
    view_program_revision: AcceptedViewProgramRevision,
    resource_types: ResourceTypeRegistryDigest,
}
```

Equality is derived structural equality over all six fields. There is no
ID-only, digest-only, source-only, or “compatible enough” equality. Unknown,
missing, malformed, uppercase-noncanonical, or legacy wire facts are rejected
by the strict typed codec.

## 9. Source-map authority

All profile dialogue ranges are projections over the one generic
`ManifestSourceMap` retained by `SourceBackedManifest`:

```rust
pub fn manifest_token_span(
    &self,
    path: &ManifestTokenPath,
    slot: ManifestTokenSlot,
) -> Option<&SourceSpan>;
```

The raw map remains crate-private. Consumers name a typed path and slot. They
must not receive a copied dialogue map, detached byte range, source string, or
independently revisioned projection.

## 10. Wire authority

- profile table: `[profiles.<id>.dialogue]`
- fields: `view`, `style`
- inline policy table: `[profiles.<id>.dialogue.inline-failure]`
- policy discriminator: `kind`
- policy variants: `fail_line`, `discard`, `fallback`
- fallback variants: `text`, `expr_source`, `call_source`, `value_plain`
- fallback style variants: `plain`, `inherit_surrounding`, `apply`

Every tagged level denies unknown fields. The discarded underscore spelling is
handled by ordinary strict unknown-field diagnostics only.

## 11. Diagnostic authority

Manifest shape/family errors belong to `arcweft-launch`. Cross-product
availability, capability, provenance, registry, and revision errors belong to
compiler stage `DialogueProfileAdmission`. No runtime consumer invents a
replacement diagnostic.

The stable compiler codes are:

```text
profile.dialogue.view.missing
profile.dialogue.view.not-dialogue
profile.dialogue.style.missing
profile.dialogue.revision.mismatch
```

The stable strict decoder codes relevant here are:

```text
manifest.unknown.field
manifest.id.invalid
manifest.id.family
manifest.inline-policy.invalid
```

## 12. Runtime and tooling consumption

`CompiledProject` must contain one checked dialogue profile. Runtime-plan
lowering receives that checked value and carries the selected View, optional
Style, inline policy, and exact revision into each relevant display plan.
CLI and LSP consume the same compiled candidate and source document; runtime,
native, Web, headless, Agent, and MCP consume the same checked selection and
revision through their existing typed paths. None reparse the manifest or
construct another catalog.

## 13. Atomic publication

A candidate is publishable only after manifest decode, source-topology freeze,
View/Style/resource product acceptance, checked dialogue admission, runtime-plan
construction, codec validation, and generation construction all succeed.

On any failure:

- no new manifest/profile/product/catalog/revision subset becomes visible;
- the previous complete `ProgramGeneration` remains current;
- save/replay identity remains bound to the previous six-field revision; and
- diagnostics refer to the rejected candidate's retained source document.

## 14. Direct deletion

The final contract contains no source `dialogue defaults`,
`DialogueDefaultsItem`, `@dialogue.*`, `.say`, speaker-line sugar, authored
external-module syntax, source `content`/`source` compatibility fields, CSS,
Takumi, or alternate View/Style/policy reader. Removed source forms fail through
ordinary parser/recovery behavior and produce no typed node.
