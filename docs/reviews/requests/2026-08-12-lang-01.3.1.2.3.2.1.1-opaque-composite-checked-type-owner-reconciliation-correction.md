# Lang-01.3.1.2.3.2.1.1 — opaque composite checked-type owner reconciliation correction

## Sequence position and precedence

This is Lang-01.3.1.2.3.2.1.1. It is a narrow mandatory correction to the
returned Lang-01.3.1.2.3.2.1 nominal-record and record-sequence owner
contract. It must return before that package's atomic A1 gate can be accepted.

The retained-byte parent authority is
`docs/reviews/packages/zips/arcweft-lang-01.3.1.2.3.2.1-nominal-record-and-record-sequence-owner-reconciliation-correction-final-contract.zip`.
Its searchable frozen mirror is the sibling package directory. The ZIP
SHA-256 is
`4b15a5eaea31663a9323f41f75345b2acb6faa0ea3a61784eeeabd482a13966a`.

The following accepted substrate must not be redesigned without a concrete
repository-evidenced defect:

- the ownership lattice and G1.2-A identity/slot/path foundation;
- `RuntimeRecordFieldId`, `RuntimeOwnedSlotId`, and `RuntimeValuePath`;
- `RuntimeNominalRecordLayout` as the final core executable descriptor;
- existing `RuntimeSeqError` as the record-sequence admission error;
- exact `TypeLayoutHash` identity for project nominal checked types; and
- ABI 1, codec 8, activation, View, and Stream decisions outside this gap.

## Split reason

The returned contract requires `RuntimeNormalizedType::checked_type()` to
project recursively while leaving all non-nominal `RuntimeCheckedType`
variants unchanged. Current accepted semantic types also contain
producer-owned opaque leaves for which no canonical `RuntimeTypeSchema` or
`TypeLayoutHash` is published. Examples include accepted nominal producer
types and character-dialogue types. `TypeKind::Named` is likewise a resolved
reference, not a standalone layout owner.

This becomes result-changing when an opaque leaf occurs inside `Result`,
`Option`, `Tuple`, `Choice`, or `Sequence`. Current runtime-plan lowering must
construct one closed `RuntimeCheckedType` for the whole composite, but there is
no truthful checked-type representation for the leaf.

Repository evidence rules out local substitutes:

- standalone `RuntimeTypeSchema::Named(name).try_layout_hash()` is
  `UnresolvedNamed`; hashing only the name is not a canonical layout;
- an accepted nominal catalog row and accepted Rust metadata do not own the
  runtime schema/tag/default/skip policy needed to derive a layout hash;
- `CharacterDialogueRuntimeSchema` consumes an expected layout supplied by a
  caller and is not the top-level layout authority; and
- substituting a semantic digest for `TypeLayoutHash` would recreate the
  forbidden type-only fallback corrected by the parent package.

An experimental selected-case projection also fails. Mapping `Ok<T>` to
`Result<T, Never>` and `Err<E>` to `Result<Never, E>` gives the same semantic
type two different runtime checked types. `RuntimeCheckedType::accepts_value`
then rejects a legitimate value of the unselected case. AWBC interns both
Result branches in one variant type-table row and has no variant covariance,
so the refinement can also disagree across scrutinee/pattern, branch merge,
and function boundaries.

Choosing `Opaque`/`Dynamic`, optional predicates, producer schema publication,
or selected-case refinement locally would change native acceptance, AWBC wire
typing, and canonical save behavior. This request is independently throwable
because the nominal-record layout decisions themselves remain accepted.

## Required decisions

1. Define the one final representation for a producer-owned opaque semantic
   type in `RuntimeCheckedType`, including exact Rust-shaped owner, variant or
   carrier fields, visibility, traits, constructors, and accessors.
2. Define how that representation nests recursively in `Result`, `Option`,
   `Tuple`, `Choice`, and `Sequence`, including empty/uninhabited cases and
   recursive generics.
3. Decide whether variant construction and patterns retain the complete
   composite owner or a selected-case predicate. If selected-case evidence is
   retained, define exact refinement/subtyping and rejoining rules rather than
   using `Never` as an implicit sentinel.
4. Define the exact behavior of `RuntimeCheckedType::accepts_value` for every
   affected native value and make absence of producer evidence fail closed.
5. Define the matching AWBC representation: type-table row fields, stable
   tags/codec behavior, `MakeVariant` verification, pattern compatibility,
   branch merge, call/return boundary behavior, and native/AWBC parity.
6. Name the canonical producers and projection APIs for at least accepted
   nominal types, character-dialogue types, `AgentError`, `ArcError`, and
   `ReducerError`. State whether each publishes a closed schema/layout or uses
   the new opaque representation.
7. Reconcile `TypeKind::{Named, AcceptedNominal, CharacterDialogue}` with
   `RuntimeTypeShape`, `RuntimeNormalizedType::checked_type`, and
   `RuntimeTypeSchema`. A name hash, semantic-digest substitution, copied
   schema, and nominal-ID-only fallback are not accepted.
8. Give the exact `RuntimeResolvedVariant` APIs and identify the old methods,
   match helpers, or fallback paths deleted at the compile-clean cut.
9. State whether persisted snapshot/bundle bytes or public Serde shapes change.
   If they do, provide the exact version/tag/migration decision; if they do
   not, provide executable evidence that the new representation is not
   serialized there.
10. Return the corrected A1 continuation order and identify whether A1 remains
    one atomic compile-clean gate or must be split into named compile-clean
    subgates.

## Required producer and consumer inventory

Inspect and close at least:

- `arcweft_core::pattern::RuntimeCheckedType`, `accepts_value`, pattern
  matching, value validation, and every Serde/codec implementation;
- core AWBC schema, type-table codec, structural verifier, fiber/VM, and
  canonical bundle or snapshot consumers;
- `arcweft_runtime_plan::semantic_facts::{RuntimeTypeShape,
  RuntimeNormalizedType, RuntimeVariantOwner, RuntimeResolvedVariant}`;
- runtime-plan final expression/pattern lowering and AWBC pattern/type
  lowering;
- compiler runtime semantic projection for `TypeKind::{Named,
  AcceptedNominal, CharacterDialogue}`;
- accepted nominal, accepted Rust metadata, character-dialogue, and built-in
  error-type producers; and
- entry-role lowering for `Result<Reduction<GameState>, ReducerError>` and
  `Result<Unit, AgentError>`.

## Required tests

- both `Ok` and `Err` constructors of one semantic `Result<T, E>` retain one
  compatible complete type when either or both payload types are opaque;
- selected opaque `Reduction<GameState>` and unselected opaque `AgentError`
  lower without fabricated layout evidence;
- corresponding `Option`, tuple, choice, sequence, nested-generic, and
  recursive cases either succeed through the named authority or fail with the
  exact typed error;
- constructor/pattern, scrutinee/pattern, branch merge, and function
  argument/return boundaries agree;
- native `accepts_value`, AWBC verifier, and AWBC VM behavior are equivalent;
- missing producer evidence fails closed and deterministic error precedence is
  fixed;
- codec/type-table golden bytes and canonical save/restore behavior are pinned
  if affected; and
- the current compiler entry suite, core/runtime-plan suites, workspace check,
  and workspace Clippy pass at every stated compile-clean gate.

## Constraints and non-goals

- Do not invent a layout from `RuntimeTypeSchema::Named`, a nominal name, a
  semantic identity digest, or accepted Rust metadata.
- Do not add a hard-coded exception for one spelling such as `AgentError`,
  `ReducerError`, `GameState`, or `CharacterDialogue`.
- Do not introduce a second checked-type projection, dual verifier, fallback
  reader, side table, or compatibility-only carrier.
- Do not redesign accepted record layouts, runtime IDs, ownership, slots,
  paths, View, activation, ABI 1/codec 8, or Stream order.
- Do not include a production overlay.

## Expected output

Return one independently usable design-only archive named
`arcweft-lang-01.3.1.2.3.2.1.1-opaque-composite-checked-type-owner-reconciliation-correction-final-contract.zip`.
It must contain `OPEN_QUESTIONS=0`, exact Rust-shaped owner/API/error and wire
decisions, a narrow supersession delta against Lang-01.3.1.2.3.2.1, complete
symbol closure, producer/consumer/deletion inventory, corrected A1 order, and
positive/negative/native-AWBC parity test matrices. Keep all sidecars inside
the ZIP.
