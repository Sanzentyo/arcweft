# As-built API and invariant ledger

Baseline: `0c8cb74dd96116a8b987cc419c9a280b6cabe4a4`.

This file distinguishes exact current signatures from architectural prose.
Fields shown as private remain private; consumers use the existing getters.

## `arcweft-dialogue`

### `DialoguePresentationProfile`

Physical source: `crates/arcweft-dialogue/src/presentation_profile.rs`.

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialoguePresentationProfile {
    view: ViewId,
    style: Option<ViewStyleSheetId>,
    inline_failure: InlineFailurePolicy,
}

impl DialoguePresentationProfile {
    pub fn engine_default() -> Self;
    pub const fn new(
        view: ViewId,
        style: Option<ViewStyleSheetId>,
        inline_failure: InlineFailurePolicy,
    ) -> Self;
    pub const fn view(&self) -> &ViewId;
    pub const fn style(&self) -> Option<&ViewStyleSheetId>;
    pub const fn inline_failure(&self) -> &InlineFailurePolicy;
}
```

`engine_default()` is domain behavior on the owning type, not an endpoint helper.

### `InlineFailurePolicy`

Physical source: `crates/arcweft-dialogue/src/inline_failure.rs`.

```rust
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InlineFailurePolicy {
    FailLine,
    Discard,
    Fallback { fallback: InlineFallback },
}

#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InlineFallback {
    Text { text: String, style: FallbackStylePolicy },
    ExprSource { style: FallbackStylePolicy },
    CallSource { style: FallbackStylePolicy },
    ValuePlain,
}

#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FallbackStylePolicy {
    Plain,
    InheritSurrounding,
    Apply { styles: Vec<CharacterDialogueStyleValue> },
}
```

Deserialization goes through strict mirror enums with
`#[serde(deny_unknown_fields)]` at every tagged level. No permissive serde
flattening or decode-only bridge exists.

### `DialogueProfileRevision`

Physical source: `crates/arcweft-dialogue/src/presentation_revision.rs`.

```rust
pub const fn from_admitted_parts(
    manifest_document: SourceDocumentIdentity,
    topology_sources: SourceSetRevision,
    compiled_sources: SourceSetRevision,
    view_program_id: ViewProgramId,
    view_program_revision: AcceptedViewProgramRevision,
    resource_types: ResourceTypeRegistryDigest,
) -> DialogueProfileRevision;
```

The constructor is public because lower consumers need the serialized type, but
its documentation makes compiler admission the legitimate construction
boundary. New call sites must not fabricate a revision before completing the
cross-product checks.

## `arcweft-launch`

### `DialogueProfileSpec`

Physical source: `crates/arcweft-launch/src/manifest.rs`.

It is crate-private and nested in `ProfileSpec` with default/empty omission. Its
field names are Rust implementation names; serde's kebab-case contract defines
the authored wire.

### `SourceBackedManifest`

Physical source: `crates/arcweft-launch/src/accepted.rs`.

```rust
pub struct SourceBackedManifest {
    document: Arc<SourceDocument>,
    manifest: ArcweftManifestDocument,
    source_map: ManifestSourceMap,
}

pub fn decode(document: Arc<SourceDocument>) -> Result<Self, ManifestReport>;
pub const fn document(&self) -> &Arc<SourceDocument>;
pub const fn manifest(&self) -> &ArcweftManifestDocument;
pub fn resolve_profile(
    &self,
    selection: LaunchProfileSelection<'_>,
) -> Result<ResolvedLaunchProfile, ManifestReport>;
pub fn manifest_token_span(
    &self,
    path: &ManifestTokenPath,
    slot: ManifestTokenSlot,
) -> Option<&SourceSpan>;
```

`decode` verifies both `Arc::ptr_eq` and `SourceDocumentIdentity` equality
between the accepted document and the source map's document.

### `ManifestTokenPath`

Physical source: `crates/arcweft-launch/src/source_map.rs`.

Dialogue projections are:

```rust
ProfileTable { profile: ProfileId }
ProfileDialogueTable { profile: ProfileId }
ProfileDialogueView { profile: ProfileId }
ProfileDialogueStyle { profile: ProfileId }
ProfileDialogueInlineFailureTable { profile: ProfileId }
ProfileDialogueInlineFailureKind { profile: ProfileId }
ProfileDialogueInlineFallbackTable { profile: ProfileId }
ProfileDialogueInlineFallbackKind { profile: ProfileId }
ProfileDialogueInlineFallbackText { profile: ProfileId }
ProfileDialogueInlineFallbackStyleTable { profile: ProfileId }
ProfileDialogueInlineFallbackStyleKind { profile: ProfileId }
ProfileDialogueInlineFallbackStyles { profile: ProfileId }
ProfileDialogueInlineFallbackStyleElement { profile: ProfileId, ordinal: u16 }
```

Slots are exactly `TableHeader`, `FieldKey`, and `Value`.

## `arcweft-compiler`

Physical source: `crates/arcweft-compiler/src/project/dialogue_profile.rs`.

### `CheckedDialogueProfile` getters

```rust
pub const fn owner(&self) -> &DialogueProfileOwner;
pub const fn profile_id(&self) -> Option<&ProfileId>;
pub const fn presentation(&self) -> &DialoguePresentationProfile;
pub const fn revision(&self) -> &DialogueProfileRevision;
pub const fn product(&self) -> &Arc<ValidatedViewProduct>;
pub const fn selected_view_source(&self) -> &SourceSpan;
pub const fn selected_style_source(&self) -> Option<&SourceSpan>;
```

### Admission error enum

```rust
pub enum DialogueProfileAdmissionError {
    ResolvedProfileMismatch { detail: String, primary: SourceSpan },
    ResourceRegistryMismatch { primary: SourceSpan },
    MissingViewProgram { view: ViewId, primary: SourceSpan },
    MissingView { view: ViewId, primary: SourceSpan },
    ViewIsNotDialogue {
        view: ViewId,
        primary: SourceSpan,
        definition: SourceSpan,
    },
    MissingStyle { style: ViewStyleSheetId, primary: SourceSpan },
    MissingSourceProvenance { owner: String, primary: SourceSpan },
    RevisionMismatch { detail: String, primary: SourceSpan },
}
```

The diagnostic conversion adds exactly one secondary source label, and only for
`ViewIsNotDialogue`: `the selected View is defined here`.

## API evolution rule

When this boundary needs new domain behavior, add it to the legitimate owning
type or its existing context. Do not add an ad-hoc helper, extension trait,
string conversion wrapper, endpoint-named adapter, or second enum merely
because a current enum lacks a method.
