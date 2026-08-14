# Lang-01.3.1.2.3.2.1.2.1.1.1 — checked-value path and resolvable root-site correction

## Sequence position and precedence

This is Lang-01.3.1.2.3.2.1.2.1.1.1. It is a narrow mandatory correction to
the returned Lang-01.3.1.2.3.2.1.2.1.1 catalog-digest, role-root, and
construction-authority contract.

The returned retry archive is retained as
`docs/reviews/packages/zips/arcweft-lang-01.3.1.2.3.2.1.2.1.1-catalog-digest-role-root-and-construction-authority-correction-final-contract_1.zip`,
SHA-256
`e0aa31dfefa5bc0d9fab213d19fef6fd74a142cef6dd7d4e6922d05c077bc998`.
Its searchable extracted mirror is the sibling package directory with the
same `_1` suffix. The `_1` suffix is repository intake naming only: the source
download used `(1)`, which is not an accepted repository file-name suffix.

The implementation audit used Git commit
`36f83f8509417d1110a34f1b32aee6f4a113dcf3` on `main`, equal to
`origin/main`, with an initially clean tree before ZIP intake.

This is a design-only request. It must not return production code, a patch, an
overlay, or a compatibility path. Every Arcweft-owned schema, ABI, codec,
digest-domain, protocol, and persistence version remains exactly `1`.

## Accepted retry decisions that remain fixed

Do not redesign these retry decisions without a concrete current-source
defect:

- the exact CharacterCatalog and ViewRegistry digest owners, domains,
  transcripts, ordering, limits, and generation-bound wrappers;
- the six authored `std.character_dialogue` opaque role declarations, derived
  ordered Style Choice, typed role coordinate, and removal of relevant
  `Named` success paths;
- lossless 32-byte projection from `RuntimeSemanticTypeId` to the distinct
  project/producer root newtypes;
- distinct project- and producer-root structured error owners;
- one parent `AdmittedRuntimeGeneration`, project-site-scoped borrowed
  construction domains, and no generation-erasing capability escape;
- the version-1 AWBC nominal-record domain table and mandatory project versus
  producer domain operand on `MakeRecord` and record constants;
- exact checked Variant owner/ordinal/name/payload behavior, unique Choice,
  shared work budget 65,536, nesting limit 64, ordered branch evidence, and
  structured error propagation; and
- no raw operational plan/AWBC publication, unchecked nominal construction,
  optional authority, fallback, compatibility alias, old reader, defaulted
  authority field, or version increment.

The lower `CharacterDialogueRuntimeRole` inherent constants and tags are
mechanically closed and may be implemented while this request is outstanding.
The Character/View digest cuts are also independent of the residual root/path
questions once their returned transcript is verified against current source.

## Residual blockers

### 1. Conflicting `RuntimeValuePath` authority

Decision 13 declares a new
`arcweft_core::pattern::RuntimeValuePath` and `RuntimeValuePathSegment` with
checked-value-specific variants. Current production already has the public,
canonical, Serde-capable owner in
`crates/arcweft-core/src/value/ownership/path.rs`. It is used by affine
ownership evidence and contains, among others, `TupleElement`,
`SequenceElement(u64)`, `NominalRecordField`, `FunctionCapture`,
`VariantPayload`, and iterator segments. The retry's own inventory row
`GBA-INV-032` says to reuse this existing owner, while Decision 13 redeclares
the same Rust names under another module with incompatible segment names,
integer widths, payloads, derives, and wire grammar.

Choosing to replace the existing enum, extend it, or introduce a distinctly
named checked-validation path changes public Rust APIs, Serde bytes, canonical
ordering, error evidence, and the affine path contract. It cannot be inferred
from the retry.

### 2. Root-site rows are not independently resolvable

Decisions 9 and 10 and their CSVs list conceptual typed boundaries, but many
current raw owners do not retain the checked semantic coordinate that admission
is required to recompute. A self-declared `RuntimePlanTypedRootUse` or
`AwbcTypedRootUse` therefore cannot be its own verification authority.

Concrete examples include current `FlowOp` expression/constant sites and
current AWBC `AwbcTaskPlan`, `AwbcAudioCommand`, `AwbcEffectPlan`,
`AwbcChoice`, and `AwbcContentUnit`. The returned `AwbcTypedSite` uses an
unshaped `slot: u32` for several of these tables, but neither the Rust enums nor
the CSV define canonical slot ordinals, the exact current field selected by
each ordinal, how it resolves to one `AwbcTypeId`, or what happens when the
current table stores only an indirect signature/function/value reference.

Admission must be able to derive the expected semantic ID and checked type
from a separately verified table-owned coordinate or typed lowering fact. It
must reject a raw artifact that merely changes both its root-use claim and its
claimed type declaration consistently.

### 3. Compile-clean phase order is impossible as written

Decision 15 phase 2 requires
`RuntimeCheckedValueValidator<'generation>` to contain
`&AdmittedRuntimeGeneration`, but the current repository has no such type and
the same order does not construct generation admission until phase 7. A
placeholder admitted generation, public validator constructor, temporary
resolver, or boolean fallback would violate the returned authority model.

### 4. Checked-value shape and nominal evidence are incomplete

Decision 13's `RuntimeValueShape` omits current legal `RuntimeValue` families:
`Range`, `MatrixF32`, `MatrixF64`, `TensorF32`, and `TensorF64`. Conversely it
lists `Bytes`, although current bytes are physically a `RuntimeValue::Seq`.
The selected outer-shape error therefore cannot describe every raw value.

Current `RuntimeNominalRecordValue` retains nominal ID, layout, and fields, but
does not retain `RuntimeSemanticTypeId`. Decision 13 nevertheless requires a
`NominalSemanticIdentity { expected, actual }` failure before admitted lookup,
without defining a legitimate source for `actual`. It must not be reconstructed
from nominal name or layout bytes.

### 5. Catalog wrapper ownership and relationship checks remain unresolved

Decision 03 makes dialogue construction consume the runtime-driver-owned
`AdmittedGenerationCatalogs`, while the accepted dependency constraint forbids
dialogue from depending on runtime-driver. It supplies no lower-layer bridge
type. Its separate `target_generation` scalar is also caller-provided assertion
rather than generation provenance. Finally, `MissingCharacterView` requires a
CharacterCatalog-to-View relationship that current `CharacterCatalog` does not
store and the decision does not derive from a named typed owner.

## Required exact decisions

1. Select one final checked-value path model. State whether the legitimate
   owner remains `value::ownership::path` or a distinctly named validation
   path is required. Give exact Rust names, module/re-exports, derives,
   visibility, constructors, accessors, maximum length, ordering, and every
   segment.
2. If the existing `RuntimeValuePath` is changed, provide the exact deletion
   and consumer migration for all affine ownership, capture, iterator,
   nominal-record, dialogue, restore, replay, View, save, and diagnostic uses.
   Pin the final human and non-human Serde grammar and canonical tags. No alias,
   dual enum, or translation fallback is accepted.
3. Give exact checked-type versus value-path push rules for Sequence, Tuple,
   Choice, Result, Option, opaque payload, Variant payload, and nominal fields,
   including index widths, overflow behavior, and deterministic first error.
4. For every `RuntimePlanTypedSite` and slot, identify the exact current owner
   field/path that independently yields its accepted semantic ID and checked
   type. Where current raw tables lack that evidence, define the mandatory
   typed field/declaration added to that owner, its private/public construction
   boundary, Serde shape, lowering source, and admission check. Update the CSV
   with one mechanically resolvable row per variant and deliberate exclusion.
5. Do the same for every `AwbcTypedSite`. Replace all generic `slot: u32`
   coordinates with exact typed slot enums or provide a complete normative
   ordinal table. Define indirect resolution through signatures, functions,
   registers, constants, patterns, and value references, including bounds,
   duplicate, aliasing, and cycle behavior.
6. Define raw-plan and raw-AWBC tamper checks proving that root-use rows and
   runtime-type declarations cannot mutually self-authorize. State the exact
   independently verified source for project and producer facts and the
   plan-to-AWBC equality transcript; do not add another digest or root map.
7. Correct the compile-clean implementation order. Either introduce the final
   `AdmittedRuntimeGeneration` owner before the validator, or give the
   validator a final lower-layer context whose admitted-generation method
   later constructs it without widening constructors. Provide exact APIs and
   delete-order; no placeholder or temporary compatibility substrate.
8. Update the producer/consumer/deletion inventory and tests for the selected
   path owner, every newly retained typed coordinate, every removed conceptual
   row, and all raw-artifact tamper cases.
9. Complete the checked outer-shape table for every current `RuntimeValue`
   variant and select whether physical byte sequences report `Sequence` or a
   distinct semantic expected shape. Define the legitimate source and timing
   for nominal semantic-identity comparison when the raw value does not carry
   it; no name/layout-derived reconstruction.
10. Make `RuntimeIndexPath` deserialization pass the same nonempty and bounded
    validation as `try_new`, or select a private wire DTO. Likewise ensure
    every mandatory root/site/domain newtype cannot bypass its final checked
    constructor through derived Serde.
11. Define a layer-correct catalog admission bridge shared by dialogue and
    runtime-driver, with exact owner, lifetime/provenance, constructor
    visibility, and generation comparison. Identify the typed source of every
    Character-to-View relationship or remove that check and its error. A free
    caller-supplied generation scalar is not provenance.
12. Reconcile the role-declaration issuance path with current
    `AcceptedNominalWorld`/`TypeCheckEnv`: registration versus post-publication
    construction must have one exact atomic owner and no nonexistent
    `AcceptedNominalEnvironment` API.

## Required precedence and tests

Retain the retry's checked-value, catalog, and `MakeRecord` precedence. Add
tests proving:

- the final path model represents every affine and checked-validation edge
  without lossy conversion or a second canonical wire authority;
- path length/index overflow and nested Choice/nominal evidence report the
  exact selected type and value paths;
- every plan and AWBC site round-trips from a real current owner field to the
  same semantic root;
- changing only a raw root-use row, only its declared checked type, or both
  together fails independent admission;
- each indirect signature/register/constant/pattern reference resolves through
  its typed owner and malformed/cyclic/out-of-bounds references fail before
  root correlation;
- no generic slot ordinal remains without an executable mapping test; and
- each numbered implementation phase compiles without a placeholder admitted
  generation, public validator construction, or boolean authority fallback.
- every current raw `RuntimeValue` family has deterministic outer-shape
  evidence and nominal semantic identity is never guessed; and
- dialogue consumes catalog admission through a permitted lower-layer owner
  tied to the same admitted generation.

## Expected output

Return one independently usable design-only archive named
`arcweft-lang-01.3.1.2.3.2.1.2.1.1.1-checked-value-path-and-resolvable-root-site-correction-final-contract.zip`.
It must contain `OPEN_QUESTIONS=0`, the exact current request copy/hash,
current-main Git and source evidence, one final path authority, mechanically
resolvable plan/AWBC site tables, tamper-proof root correlation, and a
compile-clean owner order. Keep every sidecar inside the ZIP.
