# Lang-01.3.1.2.3.2.1.2 — nominal runtime-value external admission and dialogue layout-authority correction

## Sequence position and precedence

This is Lang-01.3.1.2.3.2.1.2. It is a narrow mandatory correction to the
returned Lang-01.3.1.2.3.2.1 nominal-record and record-sequence owner
contract. It must return before that package's A4 unchecked nominal-value
constructor deletion can be accepted. A1 through A3 remain accepted and must
not be reopened without a concrete repository-evidenced defect.

The retained-byte parent authority is
`docs/reviews/packages/zips/arcweft-lang-01.3.1.2.3.2.1-nominal-record-and-record-sequence-owner-reconciliation-correction-final-contract.zip`.
Its SHA-256 is
`4b15a5eaea31663a9323f41f75345b2acb6faa0ea3a61784eeeabd482a13966a`;
its searchable frozen mirror is the sibling package directory.

The checked-type vocabulary relevant to this correction is also constrained
by the retained
`arcweft-lang-01.3.1.2.3.2.1.1-opaque-composite-checked-type-owner-reconciliation-correction-final-contract.zip`
package, SHA-256
`93af482a2914ca4a9e6b985aa7a09c040f569bd71141611dcaa4d579ac01640c`.

The implementation audit used Git commit
`98ccafa5f0113a50f8a0f5e985df5f695c401588` on `main`, equal to
`origin/main`, with a clean working tree. This is a design-only request. It
must not return a production patch, compatibility layer, or implementation
overlay.

All Arcweft-owned schema, ABI, codec, digest-domain, and protocol version
numbers remain exactly `1`. This request allocates no version and must not use
a version bump as a migration mechanism.

## Accepted substrate that remains fixed

- `RuntimeNominalRecordLayout`, its defining-order checked fields, one-based
  `RuntimeRecordFieldId` projection, and exact nominal/semantic/layout
  identities;
- `RuntimeNominalRecordExpr` admission and authored-order evaluation followed
  by ephemeral field-ID scatter into defining layout order;
- `RuntimeNominalRecordValue` storage as nominal ID, layout hash, and values in
  defining layout order;
- `RuntimeNominalRecordError` and the type, layout, count, field-ID, then field
  predicate validation precedence;
- crate-private core `RuntimeNominalRecordValue::try_from_accepted_layout` as
  the checked value publication primitive;
- public `validate_against_layout` as the restored/existing-value validator;
- A3 private anonymous/column carriers and their admitted public pair-input
  boundaries;
- the closed `RuntimeCheckedType`/opaque-owner model returned by the selected
  child packages; and
- direct replacement of unreleased contracts, with no dual reader, fallback,
  source reconstruction, copied side table, or compatibility constructor.

## Split reason

The parent requires deletion of public unchecked
`RuntimeNominalRecordValue::new` and `validate_shape` in one compile-clean A4
cut, after migrating every producer and restored-value consumer to checked
layout authority. Current production cannot make that migration from the
returned decisions alone.

`arcweft-dialogue` is a legitimate external producer of nominal runtime
values. `CharacterDialogueRuntimeSchema` receives only an expected
`TypeLayoutHash`; it does not receive or own the complete
`RuntimeNominalRecordLayout` required by checked value admission. Its fixed
custom-entry and inline-failure payloads likewise retain nominal IDs and
layout hashes but no closed runtime field descriptor. The custom-entry schema
contains a `Named("Dynamic")` field that has no exact projection into the
current closed `RuntimeCheckedType` vocabulary.

Dialogue typed values also accept nested nominal records supplied by external
producers. `normalize_runtime_value`, `empty_runtime_value`, and structured
patch traversal currently rebuild such records from the old nominal ID,
layout hash, and transformed fields by calling the unchecked constructor.
They have no active producer-owned field descriptor against which the changed
fields can be checked.

Making `try_from_accepted_layout` public is not an acceptable local repair. A
public caller can construct a `RuntimeNominalRecordLayout` with independently
chosen nominal, semantic, layout-hash, and field predicates; core cannot prove
that those scalars came from the canonical compiler/runtime-plan projection.
Publishing the value constructor would therefore turn an implementation
descriptor into an external identity/layout minting authority.

Failing every descriptorless nominal normalize, clear, or patch operation is
safe but changes which currently accepted CharacterDialogue operations
succeed. Retaining an unchecked dialogue-only constructor, preserving fields
without rechecking them, or treating a layout hash as a field schema would
violate the parent authority. The final owner and behavior therefore require
an independently throwable correction rather than an implementation guess.

## Required exact decisions

1. Define the sole layer-correct authority by which `arcweft-dialogue` and any
   other non-core producer may construct a checked nominal runtime value while
   `RuntimeNominalRecordValue::try_from_accepted_layout` remains crate-private.
   Give exact owner modules, Rust declarations, visibility, constructors,
   accessors, derives, and error types. The authority must not permit an
   external caller to mint arbitrary nominal/layout identity.
2. Define how an active, canonically projected `RuntimeNominalRecordLayout`
   reaches a legitimate external producer. Close whether it is carried by an
   admitted catalog entry, a non-forgeable admission capability, a producer
   callback owned above core, or a different single typed boundary. Do not
   duplicate the descriptor or reconstruct it from names, schema strings, or
   layout hashes.
3. Give the final `CharacterDialogueRuntimeSchema` input and ownership model.
   State how its nominal identity, expected layout, complete defining-order
   fields, and CharacterDialogue opaque producer evidence are correlated
   before encode/decode publication.
4. Decide the final physical/runtime representation of the fixed
   CharacterDialogue record, custom-entry record, and inline-failure record.
   If any remains `RuntimeNominalRecordValue`, provide its exact closed layout
   descriptor and checked field predicates. If a record becomes an opaque or
   anonymous payload, define the exact single owner and delete the old nominal
   construction path rather than retaining parallel representations.
5. Resolve the custom-entry `Dynamic` field without adding an open checked
   predicate that silently accepts arbitrary values. State whether the field
   is producer-owned opaque data, a closed choice, a separately admitted
   typed payload, or is removed by a representation change.
6. Define descriptor lookup and validation for externally supplied nested
   nominal values in `CharacterDialogueTypedValue`. Identity plus layout hash
   is not sufficient field-shape evidence. Missing, conflicting, stale, and
   wrong-producer descriptor outcomes must be typed and deterministic.
7. Define the final semantics of nominal branches in
   `normalize_runtime_value`, `empty_runtime_value`, and structured patch
   traversal. For each operation, state whether nominal values are atomic,
   transformable only with an active descriptor, or rejected. If transformed,
   require post-transform `validate_against_layout` before publication and
   preserve field-ID/layout order.
8. Define how deserialized CharacterDialogue typed values, runtime values,
   session/save values, root/replay values, and bundle/plan values obtain the
   active descriptor and validate before ownership traversal, activation, or
   domain decoding. Separate A4 work from parent A6 codec/golden closure
   without leaving an unchecked publication interval.
9. Define exact error mapping and source/path evidence across core nominal
   errors, dialogue value errors, runtime-driver activation, restore/replay,
   and bundle ingress. Do not flatten identity/layout/type failures to one
   unstructured string where a typed boundary can preserve them.
10. Give the exact deletion inventory and compile-clean implementation order
    for public `RuntimeNominalRecordValue::new`, `validate_shape`, every
    descriptorless reconstruction branch, stale test helpers, and any
    identity/layout-only validator that would remain a second authority.

## Required precedence

The returned design must preserve these orders or identify a concrete typed
owner that necessarily refines them:

1. descriptor lookup and producer/domain admission;
2. nominal identity;
3. layout hash;
4. field count;
5. defensive field-ID derivation;
6. first field predicate failure in defining layout order;
7. CharacterDialogue domain/canonical-form validation; and
8. publication, ownership traversal, restore, replay, or activation.

For structured patching, path resolution and mutation eligibility must be
defined before mutation. A failed operation publishes no partially rebuilt
nominal value.

## Required producer and consumer inventory

Inspect and close at least:

- `arcweft-core::value::nominal_record`, `pattern`, `pure`, structured engine,
  AWBC VM/fiber/verifier, root/replay validation, ownership, and nesting;
- `arcweft-dialogue::character_dialogue::{schema, typed_value, patch}` and all
  CharacterDialogue nominal test helpers;
- runtime-plan nominal layout catalogs and compiler/runtime-plan bridges;
- runtime-driver session activation, View runtime values, root replay, and
  persistence;
- bundle and save/load entry points that deserialize `RuntimeValue` or a plan
  containing nominal values; and
- agent/CLI/runtime-accelerator projections that inspect or reproduce nominal
  values.

## Required tests

- an admitted external producer can construct one nominal value only through
  the selected canonical descriptor authority;
- public callers cannot mint a nominal value from arbitrary nominal/layout
  scalars or an independently fabricated descriptor;
- CharacterDialogue encode/decode checks exact identity, layout, count,
  field-ID derivation, field types, and domain canonical form in the required
  order;
- custom-entry and inline-failure values use one closed final representation
  and reject malformed payloads without a fallback;
- externally supplied nested nominal values reject missing, stale,
  conflicting, and wrong-producer descriptors;
- normalize, clear, and patch tests cover the selected nominal atomicity or
  descriptor-aware transformation semantics, including atomic failure;
- restored/session/root/replay values validate before ownership traversal or
  activation;
- `RuntimeNominalRecordValue::new` and `validate_shape` fail to compile from
  both external and internal obsolete call sites after the final deletion;
- anonymous and nominal canonical bytes remain distinct, field order remains
  defining layout order, and all existing A1-A3 identity tests remain green;
  and
- focused core, dialogue, runtime-plan, runtime-driver, bundle/save, workspace
  check, workspace Clippy, structural audit, and applicable Tier 2 commands
  are specified for the final cut.

## Constraints and non-goals

- Do not make `try_from_accepted_layout` public and do not add a public raw
  nominal/layout/fields constructor.
- Do not treat `TypeLayoutHash`, nominal name, schema name, or semantic digest
  as a recoverable field descriptor.
- Do not add `RuntimeCheckedType::Dynamic`, optional validation, a producerless
  opaque owner, or a descriptorless success fallback.
- Do not introduce a dialogue-only unchecked constructor, friend feature,
  extension trait, copied descriptor table, source-string resolver, or
  post-build overlay.
- Do not redesign accepted A1-A3 record identities, layouts, expressions,
  anonymous carriers, or field-ID ordering without a concrete defect.
- Do not redesign affine ownership/slots, activation-domain identity, final
  HIR View products, or Stream publication.
- Do not allocate or increment any Arcweft-owned version number; every such
  version remains `1` and the unreleased representation is replaced directly.
- Do not include production code or a patch in the returned design archive.

## Expected output

Return one independently usable design-only archive named
`arcweft-lang-01.3.1.2.3.2.1.2-nominal-runtime-value-external-admission-and-dialogue-layout-authority-correction-final-contract.zip`.
It must contain `OPEN_QUESTIONS=0`, exact Rust-shaped owner/API/error decisions,
the complete producer/consumer/deletion inventory, validation and mutation
precedence, a compile-clean implementation order, and positive/negative test
matrices. Keep every sidecar inside the ZIP.
