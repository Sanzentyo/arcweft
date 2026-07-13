# Request: AW-AH-009 character nominal type identity

Date: 2026-07-14

## Request status and independence

This is a standalone design request. AW-AH-009 is accepted as a real P2 design
defect; the assignee must not need the original audit ZIP. Evidence was
captured at revision `4204d25965129ced50abe82cf5de67d528b483d0` and
implementation targets the current checkout.

## Finding and evidence

AW-AH-009 is a medium-confidence `underspecified` finding. Character-manifest
nominal types are encoded inside the display spelling of
`TypeKind::Named(String)`:

- `crates/arcweft-lang-sema/src/types.rs:621-651` constructs
  `CharacterLook<id>`, `CharacterPart<id>`, and
  `CharacterVariant<character,part>` strings and recovers identity with
  prefix/suffix parsing.
- `crates/arcweft-lang-sema/src/env.rs:480-1000` registers manifest enum
  inventories under those synthesized `TypeKind` values.
- `crates/arcweft-lsp/src/features/completion.rs:64-171` depends on the same
  name construction to offer manifest variants.

Constructor and inverse parser therefore share an incidental string grammar.
Nominal equality, diagnostic display, escaping, qualification, rename, and any
future serialization cannot evolve independently. A concrete current
miscompile was not proven by the static audit, so this request must establish
the identity contract before changing the enum.

## Established substrate that must be preserved

- `arcweft-character` already owns validated `CharacterId`,
  `CharacterLookId`, `CharacterPartId`, and `CharacterVariantId` newtypes.
  `CharacterId` is a `character.*` public identity; the others validate local
  identifiers. Reuse these owners instead of adding sema-local string wrappers.
- `CharacterManifest` is a versioned, validated, deterministic data model with
  duplicate/missing-reference checks. This request concerns semantic type
  identity, not manifest format redesign.
- `TypeKind` owns semantic equality/hash and inherent source labels. Existing
  `Speaker`, `SpeakerPreset`, and `CharacterPatch` typed variants demonstrate
  that character-related behavior belongs on the original enum.
- `SemanticEnvironment` owns manifest registration and enum inventories. LSP
  completion consumes that semantic environment rather than reading manifests
  independently.
- Unreleased internal type contracts may move directly to the final model.
  There is no evidence requiring a dual `Named(String)` reader.

Do not redesign these owners without a concrete defect.

## Design objective

Define dedicated nominal character type identity whose equality is based on
validated IDs and scope, while source labels remain a presentation concern.
The design must cover look, part, and per-part variant types through sema,
diagnostics, completion, caches/codecs if any, and rename behavior.

## Required design decisions

1. Choose the canonical in-memory shape: dedicated `TypeKind` variants or one
   `CharacterNominalType` boundary carried by a `TypeKind` variant. Show exact
   payloads for look, part, and variant families.
2. Use the `arcweft-character` ID newtypes directly where dependency direction
   permits. If a lower-level neutral identity is required, explain ownership
   and provide an owned conversion rather than field-by-field string helpers.
3. Define each identity tuple. At minimum decide whether it contains character
   public ID, part local ID, manifest/package/module identity, and manifest
   schema generation/version.
4. Define nominal scope. State whether two modules loading the same
   `character.akane` refer to one type, whether imports can rename only source
   spelling, and how duplicate/conflicting manifests are diagnosed.
5. Define equality and hashing independently from source aliases and diagnostic
   labels. Qualified versus unqualified spelling must not create two identities
   for the same manifest owner.
6. Define rename semantics. Decide whether changing a character public ID or a
   part ID is a breaking nominal identity change, and whether an IDE rename is
   a source edit rather than a runtime compatibility alias.
7. Define the source-label API on the original owner. Labels must be
   deterministic and unambiguous but are not parsers or serialization keys.
8. Define type checking for look/part/variant assignments, equality,
   collections, function parameters/returns, generics, and diagnostics between
   values from different characters or different parts.
9. Define how `SemanticEnvironment` registers variants and queries them without
   reconstructing a `Named` spelling. Include duplicate manifest and unknown
   owner/part behavior.
10. Define LSP completion, hover, signature help, go-to-definition, and rename
    inputs from the typed identity. LSP must not parse `source_label()`.
11. Identify whether these semantic types cross a persisted query cache,
    compiler interface digest, HIR serialization, AWBC, bundle, save, or plugin
    ABI. For every real boundary, define a stable typed descriptor and version;
    otherwise explicitly keep the type internal and add no codec.
12. Define dependency and crate layering so `arcweft-lang-sema` may use
    `arcweft-character` data types without making the Sans I/O character data
    crate depend on language, LSP, filesystem, or runtime layers.
13. Define hard limits and invalid-ID behavior at decode/manifest registration,
    not after an identity has entered a hash map.

## Ownership and layer constraints

- `arcweft-character` owns validated character/look/part/variant IDs and
  manifest structural validation.
- `arcweft-lang-sema::TypeKind` owns nominal type variants, equality, and
  inherent identity/source-label behavior.
- `SemanticEnvironment` owns registration and type-to-variant inventories.
- LSP/tooling consume semantic queries. They must not format, strip, split, or
  escape nominal type strings to recover identity.
- Runtime/bundle code receives typed character values only if current
  architecture requires them; this request must not drag LSP or manifest I/O
  into `arcweft-core`.

## Non-goals

- Do not redesign `CharacterManifest`, rendering composition, stage slot
  namespaces, or character asset codecs.
- Do not redesign general user-declared nominal types or replace every
  `TypeKind::Named` use unless the chosen design exposes a concrete shared flaw.
- Do not add `CharacterLook<...>` parsing to another helper, extension trait,
  or compatibility wrapper.
- Do not preserve the old synthesized names as accepted semantic identities.
- Do not invent a stable wire ABI if these types are proven to remain internal.

## Migration order

1. Publish the nominal scope/identity/rename table and identify actual
   persistence boundaries.
2. Add the final typed enum shape and inherent identity/source-label methods to
   the original semantic owner.
3. Switch manifest registration and enum-variant lookup to typed IDs.
4. Switch checker diagnostics and all type comparisons, then LSP completion,
   hover, definition, and rename queries.
5. Add or migrate codecs/caches only for boundaries proven to persist the
   identity, with decode-time validation and explicit version handling.
6. Delete `character_look`, `character_part`, `character_variant`,
   `character_look_character`, and every prefix/suffix parser once callers use
   the typed variants. Rename constructors only if the final inherent API needs
   similarly named typed constructors.
7. Remove tests that bless the synthesized spelling as identity and replace
   them with typed behavior tests.

Do not land a permanent dual representation, deprecated alias, or conversion
through the old string grammar.

## Diagnostics, errors, and codecs

Specify structured errors for:

- invalid character/look/part/variant ID at the owning boundary;
- unknown character or part during manifest registration/type lookup;
- duplicate/conflicting manifest owner in one nominal scope;
- mismatched character look, part, or per-part variant assignment;
- stale or unknown typed identity in a persisted cache/descriptor;
- unsupported descriptor version or unknown kind, if a codec exists.

Diagnostics must retain typed expected/actual identities, show source labels
through inherent methods, and attach source/import/manifest ranges where
available. They must not retain a successfully parsed old synthetic type name.

For each actual codec, use explicit typed discriminants and validated ID fields.
Define canonical ordering, size limits, version behavior, and rejection of
unknown kinds, duplicate fields, malformed IDs, unknown owners/parts, and
noncanonical encodings. No codec should deserialize into `Named(String)` and
then reparse it.

## Required tests

- Same typed identity compares and hashes equally across imports and source
  aliases; different character IDs do not collide.
- Look, part, and variant types differ even when local variant strings match.
- Variants from two parts of one character are not assignment-compatible.
- Valid ID punctuation follows the existing character ID validators; invalid
  whitespace/path punctuation is rejected before registration.
- Manifest registration and enum variant lookup use typed identities for every
  family.
- Function return/argument, collection, generic, and equality checks preserve
  nominal identity.
- Expected/actual mismatch diagnostics show stable, unambiguous labels without
  parsing those labels.
- LSP completion is scoped to the correct character and part; hover,
  go-to-definition, and rename use typed ownership.
- Character/source alias rename does not mutate nominal identity unless the
  documented public-ID rename operation is performed.
- Duplicate/conflicting manifests follow the chosen scope policy.
- Every real cache/codec has exact round-trip plus tampered invalid-ID,
  unknown-owner, wrong-part, unknown-kind/version, duplicate-field, and size
  limit tests.
- Compile tests ensure removed synthetic `Named` construction cannot enter the
  character registration path through public APIs.
- Use a manifest/source corpus with multiple characters, matching local names,
  imports, aliases, and renames. Renderer backend parity is required only if
  the final identity crosses a runtime boundary; otherwise sema/compiler/LSP
  evidence is the correct completion boundary.

Use typed API, compile-fail, manifest codec, and LSP behavior tests. Do not add
a source gate that searches files for `CharacterLook<` or old helper names.

## Expected output

- A normative identity/scope/qualification/rename table.
- Exact `TypeKind` and ID payload shapes with dependency direction.
- Inherent identity/source-label APIs and registration/query contracts.
- Persistence-boundary inventory and codec decision for each boundary.
- Structured diagnostics and LSP behavior.
- Compatibility-free migration/deletion order and a full test matrix.

## Acceptance criteria

The design is implementation-ready only when identity and display spelling are
separate; look/part/variant ownership is unambiguous under imports and rename;
all real persistence boundaries have an explicit typed decision; LSP can query
the inventory without parsing labels; and the final migration deletes every
semantic dependency on the synthesized `TypeKind::Named` grammar.
