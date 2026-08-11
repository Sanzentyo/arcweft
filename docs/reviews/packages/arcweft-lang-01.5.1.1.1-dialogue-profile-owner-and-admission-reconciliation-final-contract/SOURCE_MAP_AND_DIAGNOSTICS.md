# Source map and diagnostics contract

## One source-map authority

`SourceBackedManifest` owns one generic `ManifestSourceMap` bound to the exact
accepted `Arc<SourceDocument>`. The map validates every `SourceSpan` against
that document during construction.

Consumers use:

```rust
manifest_token_span(
    &ManifestTokenPath,
    ManifestTokenSlot,
) -> Option<&SourceSpan>
```

The raw map is `pub(crate)`. This prevents consumers from treating it as an
independently revisioned product.

## Dialogue path/slot table

| Authored token | `ManifestTokenPath` | Slot |
|---|---|---|
| `[profiles.dev]` | `ProfileTable { profile }` | `TableHeader` |
| `[profiles.dev.dialogue]` | `ProfileDialogueTable { profile }` | `TableHeader` |
| `view` key | `ProfileDialogueView { profile }` | `FieldKey` |
| View string including quotes | same | `Value` |
| `style` key | `ProfileDialogueStyle { profile }` | `FieldKey` |
| Style string including quotes | same | `Value` |
| `[...inline-failure]` | `ProfileDialogueInlineFailureTable { profile }` | `TableHeader` |
| policy `kind` | `ProfileDialogueInlineFailureKind { profile }` | `FieldKey` or `Value` |
| `[...inline-failure.fallback]` | `ProfileDialogueInlineFallbackTable { profile }` | `TableHeader` |
| fallback `kind` | `ProfileDialogueInlineFallbackKind { profile }` | `FieldKey` or `Value` |
| fallback `text` | `ProfileDialogueInlineFallbackText { profile }` | `FieldKey` or `Value` |
| `[...fallback.style]` | `ProfileDialogueInlineFallbackStyleTable { profile }` | `TableHeader` |
| style policy `kind` | `ProfileDialogueInlineFallbackStyleKind { profile }` | `FieldKey` or `Value` |
| `styles` | `ProfileDialogueInlineFallbackStyles { profile }` | `FieldKey` or `Value` |
| one style array element | `ProfileDialogueInlineFallbackStyleElement { profile, ordinal }` | `Value` |

For omitted View, the compiler falls back from the dialogue table header to the
profile table header and finally to the manifest start span. For project default
it uses the manifest start span. This is a source-label fallback only; it does
not synthesize a detached range.

## No duplication rule

Forbidden:

- `DialogueManifestSourceMap` or equivalent parallel structure;
- a second TOML scan for dialogue fields;
- copied source text;
- storing `Range<usize>` without `SourceDocumentIdentity`;
- spelling-based range lookup;
- a source map with its own revision separate from the accepted manifest.

## Decoder-owned failures

These failures occur before typed `ViewId`/`ViewStyleSheetId` values exist.

| Failure | Code | Primary range | Related source data |
|---|---|---|---|
| discarded `inline_failure` spelling | `manifest.unknown.field` | exact unknown field key | none |
| malformed nominal ID | `manifest.id.invalid` | exact scalar value | none |
| syntactically valid wrong-family View/Style | `manifest.id.family` | exact scalar value | none |
| malformed/unknown inline policy shape | `manifest.inline-policy.invalid` | exact failing policy token/table | nested strict decoder context only |

Wrong-family references are not represented by a compiler error variant. That
is intentional: a wrong-family string cannot cross the launch boundary as a
typed `ViewId` or `ViewStyleSheetId`.

## Compiler-owned admission failures

All compiler diagnostics have project compile stage
`DialogueProfileAdmission`. The primary label text is:

```text
this launch profile could not be admitted
```

| Internal error | Stable code | Primary range | Related/source data |
|---|---|---|---|
| `MissingViewProgram { view, primary }` | `profile.dialogue.view.missing` | authored View value; profile/dialogue fallback if omitted | typed `ViewId` in message; no secondary |
| `MissingView { view, primary }` | `profile.dialogue.view.missing` | authored View value | typed `ViewId`; no secondary |
| `ViewIsNotDialogue { view, primary, definition }` | `profile.dialogue.view.not-dialogue` | authored View value | one secondary at exact View definition: `the selected View is defined here` |
| `MissingStyle { style, primary }` | `profile.dialogue.style.missing` | authored Style value | typed `ViewStyleSheetId`; no secondary |
| `ResolvedProfileMismatch { detail, primary }` | `profile.dialogue.revision.mismatch` | dialogue/profile table | typed retained/re-resolved comparison summarized by `detail`; no secondary |
| `ResourceRegistryMismatch { primary }` | `profile.dialogue.revision.mismatch` | dialogue/profile table or manifest start | exact Arc/digest check; no secondary |
| `MissingSourceProvenance { owner, primary }` | `profile.dialogue.revision.mismatch` | selected View/Style value | owner ID in message; no secondary |
| `RevisionMismatch { detail, primary }` | `profile.dialogue.revision.mismatch` | dialogue/profile table | exact product/source mismatch in `detail`; no secondary |

The current public diagnostic model carries code, severity, message, primary
label, and optional secondary labels. It does not expose a second ad-hoc
revision payload schema. The typed internal error retains `view`, `style`,
`owner`, or `detail` needed to render the diagnostic.

## Revision mismatch details

At minimum, these two exact source-product mismatches are rejected:

```text
View program source_set_revision != complete product source revision
Style program source_set_revision != complete product source revision
```

Registry mismatch is also classified as revision mismatch when either the
accepted and compiler registry Arcs differ or the accepted product's registry
digest differs.

## CLI/LSP parity

CLI and LSP must receive the same `Diagnostic` and same retained source
document. Tests compare code, severity, primary source identity/range, and all
secondary labels. Presentation text formatting may differ only outside the
structured diagnostic fields.
