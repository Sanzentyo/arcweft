# Final contract

## 1. Authority, scope, and precedence

This document closes only the opaque-composite checked-type gap described by
`SOURCE_REQUEST.md`. It is based on exact static inspection of commit
`a38c736ba577172b1f4c3fe1a0c3e85443e97e6f`. The parent nominal-record contract remains authoritative except for
the explicit narrow replacements in `SUPERSESSION_DELTA.md`.

The accepted ownership lattice, `RuntimeRecordFieldId`, `RuntimeOwnedSlotId`,
`RuntimeValuePath`, `RuntimeNominalRecordLayout`, `RuntimeSeqError`, project
nominal `TypeLayoutHash`, activation, View, Stream, and ABI 1 are not redesigned.
No source/display name, accepted Rust metadata row, semantic digest, or nominal
ID is reinterpreted as `TypeLayoutHash`.

## 2. D-01 — one opaque checked-type representation

`arcweft_core::pattern::RuntimeCheckedType` gains exactly one variant:

```rust
Opaque { owner: RuntimeOpaqueTypeOwner }
```

The owner is a closed value composed of a validated producer ID, the already
projected `RuntimeSemanticTypeId`, and one closed admission mode. It is not a
schema, a layout, a name hash, a dynamic type, a callback, or a side-table key.
The matching runtime value representation is `RuntimeValue::Opaque` with the
same producer and exact semantic identity plus the producer-validated payload.
All fields are private and construction is through inherent APIs specified in
`RUST_OWNERS_AND_APIS.md`.

## 3. D-02 — recursive composites

`RuntimeNormalizedType::checked_type()` recursively retains this leaf without
special casing its position. `Result`, `Option`, `Tuple`, `Choice`, and
`Sequence` continue to own complete closed checked-type trees. Empty tuple is
inhabited by the empty runtime tuple; empty choice and `Never` are uninhabited.
A producer-owned generic opaque nominal is an atomic recursion cut whose exact
semantic identity already includes normalized arguments. A structurally
recursive non-opaque type remains rejected by the existing recursion/depth
boundary; no placeholder or late patching is introduced.

## 4. D-03 — complete variant owner, never selected-case type

Every `Ok`, `Err`, `Some`, `None`, nominal-enum constructor, and every matching
pattern retains the complete composite owner. A selected case is represented
only by its source-ordered ordinal and checked case descriptor. `Never` is not
used as a missing-branch sentinel. There is no variant covariance and no
selected-case subtyping. Branch rejoining is decided by the semantic checker;
AWBC consumes the emitted complete type and does not invent a join.

## 5. D-04 — native acceptance and fail-closed behavior

`RuntimeCheckedType::accepts_value` is the sole Arcweft-owned native predicate.
The old private free matcher is deleted. An opaque checked type accepts only a
`RuntimeValue::Opaque`; raw payloads, nominal records, variants, `Dynamic`, and
name-only values fail. Exact admission requires producer and semantic identity
equality. Producer-wide admission accepts an exact value from the same producer
and is used only for a producer-defined semantic top such as
`CharacterDialogue::Any`. The value payload is never guessed or structurally
interpreted by core. Producer validation happens before wrapping and again at a
producer-owned decode/restore boundary. Missing producer evidence is a typed
projection or decode error and never becomes `Dynamic`.

## 6. D-05 — one matching AWBC representation

ABI remains 1. The inspected codec 10 becomes codec 11. The runtime type table
gains tag 23 for `Opaque`; the constant table gains tag 18 for an exact opaque
constant. Admission tags are 0 (`ExactIdentity`) and 1 (`ProducerWide`).
`MakeVariant`, pattern verification, branch merge, calls, returns, and VM value
validation all use the same `RuntimeOpaqueTypeOwner::accepts_owner` or
`accepts_value` relation. A producer-wide row may accept an exact row from the
same producer; it never makes nominal `Variant` rows covariant.

## 7. D-06 — canonical producers

Accepted nominal catalog rows that currently lower through producerless
`AcceptedNominalSemantics::Opaque` or runtime-facing `TypeKind::Named` become
producer-bearing opaque accepted rows. `Reduction<T>`, `AgentError`, `ArcError`,
`ReducerError`, and the other standard domain atoms listed in
`PRODUCER_PROJECTION_CONTRACT.md` use this route and publish no fabricated
schema/layout. `CharacterDialogueType` owns exact/any projection through the
canonical producer ID `std.character_dialogue`; its existing
runtime schema validates the payload and no longer masquerades as top-level
layout authority. Project nominal declarations with an actual closed schema
continue to use the parent's nominal layout route.

## 8. D-07 — TypeKind and RuntimeTypeShape reconciliation

`TypeKind::Named(String)` remains a resolved compile-time/host reference and is
not runtime-projectable. Any attempt to project it directly returns
`MissingOpaqueProducerEvidence`. Runtime-facing standard atoms are published as
`TypeKind::AcceptedNominal`, which carries mandatory producer evidence.
`RuntimeTypeShape::Named` is deleted. Bare `RuntimeTypeShape::Opaque` is
replaced by `Opaque { producer, admission }`; the enclosing
`RuntimeNormalizedType.identity` supplies the exact semantic identity.
`RuntimeTypeSchema` gains no opaque variant.

## 9. D-08 — RuntimeResolvedVariant closure

`RuntimeResolvedVariant::checked_selection()` becomes the only public success
projection for lowering. It returns one validated selection containing the
complete `RuntimeCheckedType`, ordinal, and owned `RuntimeCheckedVariantCase`.
The owner-local boolean `accepts_variant_case`, direct lowerer calls to
`RuntimeVariantOwner::checked_type`, selected-case helpers, and any `Never`
fallback are deleted at the same compile-clean cut. Exact APIs and errors are in
`RUNTIME_RESOLVED_VARIANT_API.md`.

## 10. D-09 — persistence and Serde

Public Rust Serde shapes do change: `RuntimeCheckedType::Opaque`,
`RuntimeValue::Opaque`, and their carriers are serialized. Canonical runtime
value tag 16 is allocated. AWBC codec 11 and session-save schema 3 are single
hard cutovers. Existing outer bundle schema and `awbc_v1` product key remain
unchanged because ABI stays 1. Codec 10 and save schema 2 readers are not kept;
no migration registry, dual reader, compatibility carrier, or optional field is
added. `RuntimeTypeSchema` canonical bytes and project nominal layout hashes are
unchanged.

## 11. D-10 — corrected A1 continuation

The parent's monolithic A1 is replaced by four named, compile-clean subgates:
A1.1 core carrier/acceptance, A1.2 producers/projection/variant API, A1.3 AWBC
codec/verifier/VM parity, and A1.4 persistence/deletion/full closure. Each gate
runs format, focused tests, workspace check, and workspace Clippy with warnings
denied. Parent A2 and later work resumes only after A1.4. No compatibility state
survives between gates.

## 12. Global prohibitions

The implementation shall not add a schema hash derived from a name, copy a
producer schema, interpret semantic identity as layout, add `Dynamic` fallback,
retain a second checked-type projection, install a predicate registry/side
table, hard-code one error spelling in VM/core, use an extension trait around
Arcweft-owned enums, retain selected-case type refinement, or add any old-format
reader.

## 13. Completion

All ten required decision groups are closed. The package provides exact owner,
API, error, wire, producer, consumer, deletion, ordering, and positive/negative
native/AWBC parity decisions. `OPEN_QUESTIONS=0`.
