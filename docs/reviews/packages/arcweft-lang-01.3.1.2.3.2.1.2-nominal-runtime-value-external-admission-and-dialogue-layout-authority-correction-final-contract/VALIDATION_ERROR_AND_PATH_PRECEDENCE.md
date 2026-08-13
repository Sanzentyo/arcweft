# Validation, error, and path precedence

## 1. Universal nominal precedence

At every nominal record boundary the exact order is:

1. active catalog lookup and domain/producer admission;
2. stored nominal identity;
3. stored layout hash;
4. field count;
5. defensive one-based field-ID derivation;
6. first field predicate failure in defining layout order;
7. enclosing CharacterDialogue/domain/canonical validation; and
8. publication, ownership traversal, restore, replay, or activation.

Catalog conflicts are rejected during plan admission and therefore precede all
runtime value validation.

## 2. CharacterDialogue root precedence

Because the root is final opaque rather than nominal:

1. require producer `std.character_dialogue`;
2. require a tuple payload and exact count 18;
3. parse the minimum character identity from tuple index 0;
4. derive and compare exact opaque semantic identity;
5. preflight all role/custom nominal descriptor authorizations;
6. decode fixed fields in index order, applying universal nominal precedence at
   every nested nominal node;
7. validate custom order/IDs/types and inline variant cases;
8. validate character/View/look catalogs, contract digests, limits, and other
   domain rules;
9. require canonical re-encode equality for external persisted input; and
10. publish.

A malformed later field never replaces an earlier producer/identity/descriptor
failure.

## 3. Structured patch precedence

1. limits;
2. duplicate/prefix-overlap rejection;
3. path resolution against original value and expected checked type;
4. mutation eligibility and replacement validation for every operation;
5. candidate clone;
6. deterministic mutation;
7. nominal rebuild/validation on unwind;
8. complete checked-tree validation;
9. dialogue domain/canonical validation; and
10. atomic publication.

No mutation begins before step 4 succeeds for all paths.

## 4. Typed core error ownership

- `RuntimeNominalRecordCatalogError` — malformed/conflicting generation
  catalog and producer declarations; source of `RuntimePlanError`.
- `RuntimeNominalRecordLookupError` — missing, stale, unknown producer, and
  wrong-producer lookup.
- retained `RuntimeNominalRecordError` — value identity/layout/count/ID/field
  predicate.
- `RuntimeNominalRecordTreeError` — exact `RuntimeValuePath` plus one of the
  above or a non-nominal checked-type mismatch.
- existing `RuntimeValuePathError` — missing/wrong aggregate/invalid identity
  path resolution.

No core variant stores a formatted source error string.

## 5. Dialogue error mapping

`CharacterDialogueValueError` preserves source types:

| Boundary | Target variant | Preserved evidence |
|---|---|---|
| schema construction producer mismatch | `OpaqueProducer` | expected/actual producer IDs |
| root exact type mismatch | `OpaqueSemanticIdentity` | expected/actual semantic IDs |
| fixed producer payload shape | `PayloadShape` | exact `RuntimeValuePath` + typed expected shape |
| role value | `RoleValue` | role enum + `RuntimeNominalRecordTreeError` |
| custom value | `CustomValue` | custom field ID + tree error |
| patch path | `PatchPath` | operation ordinal + path + `RuntimeValuePathError` |
| patch replacement/rebuild | `PatchValue` | operation ordinal + path + tree error |
| opaque wrapping | retained `OpaqueValue` | `RuntimeOpaqueValueError` |
| canonical value encoding | retained `RuntimeSchema` | `RuntimeSchemaError` |

The old catch-all `Field { field, reason: String }` is not used for
identity/layout/type failures. It may be deleted entirely once all domain shape
sites move to `PayloadShape`, identity variants, limits, or catalog errors.

## 6. Runtime-plan and runtime-driver mapping

The existing `RuntimePlanError` gains a transparent/source-preserving
`NominalRecordCatalog` variant. `RuntimePlan::try_admit` returns it before an
`AdmittedRuntimePlan` exists.

The existing runtime-driver `BundleSessionError::DecodeBytecode(RuntimePlanError)`
continues to preserve plan admission errors. Runtime-value ingress adds typed
variants instead of formatting them into `DecodeBundle { message }` or
`InvalidRuntimeValue { path: String, message: String }`:

```rust
NominalRuntimeValue {
    path: RuntimeValuePath,
    #[source]
    source: RuntimeNominalRecordTreeError,
}
CharacterDialogue {
    path: RuntimeValuePath,
    #[source]
    source: CharacterDialogueValueError,
}
```

Equivalent source-preserving variants are added to the existing root/replay
and session-save error owners. In `BundleSessionSaveError`, nominal and
CharacterDialogue failures do not use the existing string-only
`InvalidRuntimeValue` path. Non-nominal legacy validation may keep that variant
until its own owner correction, but it cannot receive errors covered here.

## 7. Bundle and save ingress

Bundle/container codecs own byte/envelope decode failures only. They return raw
quarantined plan/value carriers. The runtime driver, which has the active plan,
slot types, dialogue catalogs, and generation identity, performs semantic
admission and returns the typed errors above. Bundle code must not catch those
errors and replace them with an unstructured string.

Save/restore maps the persisted owner/slot identity to an active checked type
first. A missing owner/slot is its existing typed error. Once mapped, nominal
validation errors retain path and core source. CharacterDialogue values retain
its domain source.

## 8. Ownership traversal ordering

`RuntimeValue` ownership/nesting walkers are not validators and never recover a
descriptor. Any ingress API that currently walks before active validation is
reordered. The same validated value may then use the accepted shared path
visitor. A traversal error cannot mask a descriptor, nominal, layout, count,
field-ID, or field-type error.
