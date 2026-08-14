# Decision 01 — `CharacterCatalog` runtime digest

## Complete owner and transcript

Owner: `crates/arcweft-character/src/catalog.rs`, module `arcweft_character::catalog`.

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CharacterCatalogRuntimeDigest([u8; 32]);

impl CharacterCatalogRuntimeDigest {
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32];
}

#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum CharacterCatalogRuntimeDigestError {
    #[error(transparent)]
    Catalog(#[from] CharacterCatalogError),
    #[error("character catalog contains {observed} rows; maximum is {maximum}")]
    EntryLimit { observed: usize, maximum: usize },
    #[error("character catalog key {key} does not equal manifest owner {owner}")]
    KeyOwnerMismatch { key: CharacterId, owner: CharacterId },
    #[error("character {character} field {field:?} has {bytes} UTF-8 bytes; maximum is {maximum}")]
    StringLength {
        character: CharacterId,
        field: CharacterCatalogStringField,
        bytes: usize,
        maximum: u32,
    },
    #[error("character {character} field {field:?} has {observed} rows; maximum is {maximum}")]
    SequenceLength {
        character: CharacterId,
        field: CharacterCatalogSequenceField,
        observed: usize,
        maximum: u32,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterCatalogStringField {
    CharacterId,
    DefaultLookId,
    PartId,
    VariantId,
    AssetPath,
    LookId,
    SelectionPartId,
    SelectionVariantId,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterCatalogSequenceField {
    CatalogRows,
    Parts,
    Variants,
    Looks,
    LookSelections,
}

impl CharacterCatalog {
    pub const MAX_RUNTIME_DIGEST_ROWS: usize = 65_536;

    pub fn runtime_digest_v1(
        &self,
    ) -> Result<CharacterCatalogRuntimeDigest, CharacterCatalogRuntimeDigestError>;
}
```

Fields remain private. The digest type has no `Serialize`, `Deserialize`, `Default`, `From<[u8; 32]>`, or caller-supplied constructor. `CharacterCatalog::runtime_digest_v1` is the only issuer.

## Local validation and duplicate order

1. Reject `self.len() > 65_536`.
2. Iterate the private `BTreeMap<CharacterId, CharacterManifest>` in key order.
3. Call `CharacterManifest::validate()` for the row before any canonical bytes are committed for that row.
4. Require map key equality with `manifest.character()`.
5. Require every UTF-8 byte length and nested sequence count used by the existing manifest fingerprint to fit `u32`; report the first field in the transcript order below.
6. Existing catalog construction already rejects duplicate character owners; existing manifest validation rejects duplicate parts, variants, looks, asset paths, and per-look part selections in its deterministic source order. No digest routine silently sorts away a duplicate.
7. Only after all local validation succeeds is the complete candidate transcript hashed.

## BLAKE3 domain and scalar grammar

Domain bytes, including the final NUL:

```text
arcweft.character-catalog.runtime.v1\0
```

All fixed-width integers are little-endian. `str32(s)` is `u32_le(UTF8_byte_len) || UTF8_bytes`. `seq32(n)` is `u32_le(n)`. Boolean is one byte, `0x00` or `0x01`. There is no Serde, Debug, Display, JSON, TOML, source path, map iteration, or Rust enum-layout input.

Complete catalog transcript:

```text
DOMAIN
u32_le(1)
u32_le(character_row_count)
repeat rows in CharacterId Ord order:
    str32(character_id)
    [u8; 32] manifest.semantic_fingerprint_v1
```

The per-manifest fingerprint is the current source-owned `CharacterManifest::semantic_fingerprint_v1`; this contract retains its exact existing version-1 transcript instead of copying it into a second catalog encoder:

```text
DOMAIN = "arcweft-character-manifest-fingerprint-v1\0"
u32_le(1)
str32(character_id)
u32_le(canvas.width)
u32_le(canvas.height)
i32_le(anchor.x)
i32_le(anchor.y)
str32(default_look_id)
seq32(parts sorted by CharacterPartId)
  str32(part.id)
  i32_le(part.z)
  seq32(variants sorted by CharacterVariantId)
    str32(variant.id)
    str32(variant.asset)
    i32_le(rect.x)
    i32_le(rect.y)
    u32_le(rect.width)
    u32_le(rect.height)
    u8(opacity)
    u32_le(CharacterBlendMode::stable_code())
    u8(clipping: false=0, true=1)
seq32(looks sorted by CharacterLookId)
  str32(look.id)
  seq32(selections sorted by CharacterPartId)
    str32(selection.part)
    str32(selection.variant)
```

## Included and excluded semantics

Included: character identity, canvas, anchor, default look, all parts, z order, variants, asset paths, rectangles, opacity, stable blend code, clipping, looks, and every part-to-variant selection. Therefore changing any runtime-rendering or dialogue-look resolution field changes the catalog digest.

Excluded: manifest `format` and `version` after they have been validated as the sole accepted values; `source`, source warnings, importer identity, source file, source hash, and `source_layer` provenance. They are source-only evidence and do not change runtime dialogue admission. The current model has no alias table, so aliases are unrepresentable rather than ignored. There is no character retirement/tombstone row; removal of a manifest removes a live row and changes row count and digest. A future retired-row model would require a new in-place version-1 contract decision before implementation; it cannot be inferred by this encoder.

## Projection into the generation contract

`arcweft-runtime-driver` losslessly copies `*CharacterCatalogRuntimeDigest::as_bytes()` into the existing raw `RuntimeCharacterCatalogDigest::from_bytes`. That scalar is only assertion evidence inside the existing generation declaration; the operational catalog remains the borrowed admitted wrapper in Decision 03.
