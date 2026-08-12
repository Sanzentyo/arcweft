# Lang-01.3.1.2.3.2.1.1.1 — external opaque-producer declaration authority correction

## Sequence position and precedence

This is Lang-01.3.1.2.3.2.1.1.1. It is a narrow mandatory correction to the
returned Lang-01.3.1.2.3.2.1.1 opaque-composite checked-type owner contract.
It must return before that package's A1.2 producer/projection gate can be
accepted.

The retained-byte parent authority is
`docs/reviews/packages/zips/arcweft-lang-01.3.1.2.3.2.1.1-opaque-composite-checked-type-owner-reconciliation-correction-final-contract.zip`.
Its searchable frozen mirror is the sibling package directory. The ZIP
SHA-256 is
`93af482a2914ca4a9e6b985aa7a09c040f569bd71141611dcaa4d579ac01640c`.

The implementation audit used Git commit
`7636b61a1c4c8e81127cb81a8fd27ef765d5ce2a` on `main`, equal to
`origin/main`. The working tree also contained preserved, unstaged A1
production work. This is a design-only request; it must not return a
production patch or modify that work.

The following accepted substrate must not be redesigned without a concrete
repository-evidenced defect:

- `RuntimeOpaqueTypeProducerId`, `RuntimeOpaqueTypeAdmission`,
  `RuntimeOpaqueTypeOwner`, `RuntimeOpaqueValue`, and their accepted native
  owner/acceptance relation;
- mandatory producer-bearing `AcceptedNominalSemantics::Opaque` and
  `AcceptedNominalType`;
- exact admission for accepted opaque nominals and producer-wide admission
  only for the canonical `CharacterDialogue::Any` semantic top;
- the fixed `std.*` producer IDs in the parent package;
- opaque atomic recursion, complete recursive composite owners, and typed
  projection paths/errors;
- ABI 1, AWBC codec 11, canonical runtime-value tag 16, opaque AWBC type tag
  23, opaque constant tag 18, and session-save schema 3; and
- the parent nominal-record layout, identity/slot/path, activation, View, and
  Stream decisions outside this gap.

## Split reason

The parent says that every opaque accepted nominal record carries a mandatory
validated producer and that a Rust export declaring a new opaque type must
declare that producer in its accepted descriptor before catalog publication.
Current production has no such declaration authority for external adapter or
Rust-export rows:

- `AdapterNominalDeclaration` contains only path, arity, visibility, and source
  label;
- `ArcweftRustTypeDecl` contains path, Rust path, parameters, and structural
  Rust kind;
- `AdapterRustType` is derived from an `ArcweftRustTypeDecl` after a package
  mount and has no independent authored descriptor;
- `AcceptedNominalInventoryInput` contains identity, arity, visibility,
  origin, source, and publication item, but no producer;
- `accepted_external_environment` currently publishes every external
  inventory row as producerless `AcceptedNominalSemantics::Opaque`; and
- the adapter manifest codec, Rust ABI codec/derive, registration digest, and
  generated registration source have no producer field.

Names, paths, package IDs, Rust metadata hashes, structural Rust metadata, and
semantic identities are provenance or type identity. None is producer
authority. Deriving a producer from any of them would recreate the forbidden
name/path fallback and would make an external author unable to declare that
several exact nominal identities belong to one producer domain.

Adding the missing authority is not a local Rust detail. It changes two public
descriptor models, two serialized schemas, a proc-macro contract, registration
digests, generated source, typed errors, and external fixture migration. It is
therefore large enough to require an independently throwable correction
rather than being guessed inside A1.2 production work.

## Directional decisions already fixed by repository evidence

The returned design shall close exact APIs and wire details within these
constraints rather than reopening the owner model.

1. Adapter-native nominal declarations own a mandatory validated producer in
   `arcweft-adapter-context`. The layer-correct type is an adapter-owned
   `AdapterOpaqueTypeProducerId`; `arcweft-adapter-context` must not depend on
   `arcweft-core`.
2. A Rust-export declaration is authored on
   `arcweft-rust-abi::ArcweftRustTypeDecl`, not on derived
   `AdapterRustType`. The Rust ABI owns its own validated producer ID type.
   `AdapterRustType` may expose the declaration's producer but must not copy or
   override it.
3. Producer IDs are explicit descriptor data. They are never derived from a
   display name, accepted path, Rust path, package identity, metadata digest,
   nominal identity, or schema/layout.
4. Both project-local adapter manifest schema 1 and Rust ABI schema 1 move to
   schema 2 as hard cuts. There is no default producer, migration map, dual
   reader, legacy success path, or compatibility-only carrier.
5. Version rejection precedes interpretation of schema-2 required fields.
   JSON and TOML adapter manifest decoding, and every serialized Rust ABI
   decoding surface, must preflight the schema header and reject schema 1 with
   the version error before reporting a missing producer field. A header
   preflight is not a schema-1 reader.
6. External adapter and Rust-export rows always project exact admission.
   External descriptors do not carry an admission field. Producer-wide
   admission remains reserved for the fixed CharacterDialogue semantic top.
7. One producer may intentionally own multiple exact nominal identities,
   including generic instantiations. Duplicate producer strings are therefore
   not catalog collisions and no producer-only unique index or side table is
   introduced.
8. External descriptors may not claim the reserved `std.` namespace. The
   fixed standard and CharacterDialogue constructors remain the only owners
   allowed to construct those producer IDs. Other equal external producer IDs
   are allowed and mean an explicitly shared producer domain.
9. `arcweft-adapter-sema` is the single layer boundary that converts either
   external descriptor ID into core `RuntimeOpaqueTypeProducerId` and places
   it in mandatory `AcceptedNominalInventoryInput` evidence.
10. Registration digests and generated registration source must include the
    producer explicitly. The semantic type identity digest remains nominal
    identity plus arguments and is not replaced by the producer.

## Required exact decisions

1. Give the exact Rust declarations, modules, visibility, derives,
   constructors, accessors, and error types for
   `AdapterOpaqueTypeProducerId` and the Rust-ABI-owned producer ID. Fix their
   spelling validation, maximum length if any, empty/control handling, and
   reserved-namespace validation owner.
2. Give the final `AdapterNominalDeclaration` field and constructor signature,
   the exact schema-2 `nominal_types[].opaque_producer` JSON/TOML spelling, and
   codec/model error mapping.
3. Give the final `ArcweftRustTypeDecl` field and schema-2 serialized spelling.
   Define the exact `#[derive(ArcweftType)]` input syntax, including duplicate,
   missing, empty, control-character, and malformed-attribute diagnostics.
   The attribute must be mandatory for exported nominal types and must not be
   inferred.
4. Define the exact schema-header preflight representation and precedence for
   adapter JSON, adapter TOML, and Rust ABI decoding. Pin unsupported-version,
   malformed-header, missing-header, missing-producer, invalid-producer, and
   otherwise-invalid-body precedence without retaining a schema-1 success
   path.
5. Define how programmatic `ArcweftRustManifest` construction and
   `AdapterManifest::try_with_rust_manifest` validate and retain producer
   evidence, including whether a manifest with no type declarations needs any
   producer data.
6. Give the exact accessor by which `AdapterRustType` exposes the producer from
   its `decl`, and prove there is no second mutable/copy authority.
7. Give the final mandatory producer fields and constructor/accessor changes
   for `AcceptedNominalInventoryInput`, `AcceptedNominalSemantics::Opaque`,
   `AcceptedNominalRecord::try_new_opaque`, and `AcceptedNominalType`. Define
   substitution/instantiation preservation.
8. Define the exact adapter-sema conversion, typed error variants, source
   attachment, and precedence for an invalid or reserved producer. Do not map
   it to an unstructured string.
9. Allocate and define all affected canonical digest domains/versions and row
   encoding. At minimum close adapter registration manifest digest, external
   type-input digest if affected, accepted nominal catalog digest, and any Rust
   ABI manifest hash or generated artifact digest. State explicitly which
   semantic identity digests remain unchanged.
10. Define the generated registration-source representation and escaping for
    producer IDs, including deterministic ordering and source-map ownership.
11. Inventory every standard adapter manifest, desktop/Rust export, test
    fixture, LSP/verify fixture, loader fixture, macro pass/fail fixture, and
    direct struct literal that must gain an explicit producer. Give the rule
    for choosing fixture producer IDs without deriving production IDs.
12. State the exact deletion set: producerless constructors/variants, schema-1
    writers/readers/goldens, derive success without the attribute, and any
    temporary fallback or post-build overlay.

## Error precedence

The returned contract must close, at minimum, these orders:

1. raw syntax/header decode;
2. schema version support;
3. schema-2 required-field presence;
4. producer spelling validity;
5. reserved namespace;
6. remaining descriptor/model validation;
7. package mount and Rust ABI validation;
8. nominal duplicate/capacity/work accounting; and
9. atomic catalog publication.

If repository evidence requires different ordering for one codec, name that
codec, give the exact typed error, and explain why the ordering remains
deterministic. Missing producer must be impossible after validated catalog
publication.

## Required producer and consumer inventory

Inspect and close at least:

- `arcweft-adapter-context::{manifest, manifest::nominal, codec, standard}`;
- `arcweft-rust-abi::{model, validation, display, tests}` and every
  encode/decode entry point;
- `arcweft-rust-abi-macros` attribute parsing, expansion, and trybuild tests;
- `arcweft-adapter-sema::registration::{input, input::digest,
  input::source, registrar, tests}`;
- `arcweft-lang-sema::registration::environment_input`, accepted nominal
  catalog construction/digest, instantiation, substitution, and Rust metadata;
- standard/desktop adapter manifests, project loader, compiler, LSP,
  verify-LSP, and direct `ArcweftRustTypeDecl`/`AdapterNominalDeclaration`
  constructors; and
- maintained JSON/TOML manifests, generated source goldens, digest goldens,
  and Rust derive examples.

## Required tests

- adapter schema-2 JSON and TOML accept a valid explicit producer;
- schema 1 is rejected as unsupported before missing-producer validation;
- missing, empty, control-containing, and reserved `std.` external producer
  IDs fail with exact typed errors and source evidence;
- two exact external nominals may explicitly share one non-reserved producer;
- equal producer plus unequal semantic identity remains two exact owners;
- external descriptors cannot request producer-wide admission;
- Rust ABI schema 2 and `ArcweftType` derive require the explicit producer;
- macro duplicate/missing/malformed producer attributes fail deterministically;
- `AdapterRustType` exposes exactly the producer authored by its
  `ArcweftRustTypeDecl`;
- adapter-sema publishes mandatory producer evidence for both adapter-native
  and Rust-export rows;
- accepted nominal instantiation and generic substitution retain the producer;
- manifest/catalog/generated-source digests change when only the producer
  changes, while semantic type identity remains governed by nominal identity
  and arguments;
- all schema-1 writers/readers/goldens and producerless success fixtures are
  absent at the final cut; and
- focused adapter-context, Rust ABI, macro, adapter-sema, lang-sema,
  compiler-entry, LSP, workspace check, and workspace Clippy commands pass at
  every stated compile-clean gate.

## Implementation order required from the return

Return a compile-clean, deletion-driven order that begins with the lower Rust
ABI and adapter-context descriptor owners, then moves through adapter-sema
publication and lang-sema accepted catalog evidence, and only afterward
resumes the parent A1.2 runtime projection. Identify exact atomic subgates if
the two schema hard cuts cannot safely be one commit. Do not place a
producerless compatibility interval between subgates.

## Constraints and non-goals

- Do not redesign the accepted core opaque owner/value model or admission
  relation.
- Do not derive producer IDs or reserve one producer per nominal.
- Do not introduce a registry callback, generic producer trait, schema/layout
  publication, side table, post-build overlay, optional producer, or dynamic
  fallback.
- Do not add a schema-1 compatibility reader, default field, migration map, or
  source-string reconstruction.
- Do not let adapter-context or Rust ABI depend on core or sema.
- Do not redesign nominal record layout, ownership/slots/paths, AWBC tag
  allocations, save schema 3, View, activation, or Stream ordering.
- Do not include a production overlay.

## Expected output

Return one independently usable design-only archive named
`arcweft-lang-01.3.1.2.3.2.1.1.1-external-opaque-producer-declaration-authority-correction-final-contract.zip`.
It must contain `OPEN_QUESTIONS=0`, exact Rust-shaped owner/API/error/codec and
derive decisions, schema-2 wire examples, digest grammar/version decisions,
the complete producer/consumer/deletion inventory, compile-clean
implementation order, and positive/negative test matrices. Keep all sidecars
inside the ZIP.
