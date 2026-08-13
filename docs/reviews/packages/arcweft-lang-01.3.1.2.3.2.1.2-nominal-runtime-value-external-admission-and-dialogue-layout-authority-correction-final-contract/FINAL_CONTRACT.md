# Final contract

## 1. Precedence, status, and immutable substrate

This contract is Lang-01.3.1.2.3.2.1.2. It narrowly supersedes only the
CharacterDialogue/external-producer decisions that prevented the returned
Lang-01.3.1.2.3.2.1 A4 deletion from being implemented. Parent A1 through A3
remain accepted and are not reopened.

The retained parent and opaque-owner child remain authoritative for:

- `RuntimeNominalRecordLayout`, defining-order fields, one-based
  `RuntimeRecordFieldId`, and nominal/semantic/layout identities;
- authored-order nominal expression evaluation followed by ephemeral field-ID
  scatter into defining layout order;
- `RuntimeNominalRecordValue { type_id, layout, fields }` and canonical bytes;
- `RuntimeNominalRecordError` precedence: nominal, layout, count, defensive
  field ID, first field predicate in layout order;
- crate-private `try_from_accepted_layout` and public
  `validate_against_layout`;
- accepted A3 anonymous/column carriers;
- closed `RuntimeCheckedType`, explicit `RuntimeOpaqueTypeOwner`, exact and
  producer-wide opaque admission; and
- direct replacement of unreleased representations without compatibility
  readers or version allocation.

Every Arcweft-owned version remains exactly `1`.

## 2. D1 — sole external nominal-value construction authority

The sole generic authority is a **borrowed operational admission handle** issued
from the nominal-record catalog of an admitted runtime plan:

```text
RuntimePlan (quarantined Serde data)
  -> RuntimePlan::try_admit
  -> AdmittedRuntimePlan
  -> RuntimeNominalRecordCatalog
  -> RuntimeNominalRecordProducerAdmission<'generation>
  -> RuntimeNominalRecordAdmission<'generation>
  -> RuntimeNominalRecordValue::try_from_accepted_layout (crate-private)
```

The two handle types have private fields, no public constructor, and no
`Serialize`/`Deserialize`. `RuntimeNominalRecordAdmission::try_construct` is
the only public external-producer operation that can reach the crate-private
value constructor. It always uses the exact catalog-owned layout and accepts
only fields in defining layout order.

`RuntimeNominalRecordLayout::try_from_checked_projection` remains a descriptor
constructor, not a value-publication authority. Passing a separately built
layout, nominal ID, semantic ID, or layout hash never yields a handle.

## 3. D2 — how the canonical layout reaches a producer

The compiler/runtime-plan bridge emits one canonical catalog declaration per
runtime generation from the already accepted `RuntimeResolvedNominalRecord`
`Arc` interning map. Each catalog key is
`(RuntimeNominalTypeId, RuntimeSemanticTypeId, TypeLayoutHash)`. Equal keys must
have structurally equal descriptors; a conflict fails plan admission.

External authorization rows contain only a producer ID plus catalog keys. They
do not copy field descriptors. CharacterDialogue rows are the transitive set of
nominal checked types reachable from its accepted role types and custom-field
checked types. The runtime-plan bridge computes that set from accepted facts;
it is not reconstructed from names, schema strings, nominal strings, or layout
hashes at runtime.

`RuntimePlan::try_admit(self)` performs whole-plan verification, catalog
canonicalization/interning, producer-row resolution, reachability checks, and
then returns `AdmittedRuntimePlan`. Runtime execution, restore, replay, View
activation, session activation, and producer-schema construction receive only
that admitted wrapper or borrowed handles from it. Raw `RuntimePlan` remains a
quarantined persistence/input carrier and is not executable.

## 4. D3 — final CharacterDialogue schema ownership

`CharacterDialogueRuntimeSchema<'generation>` owns no nominal ID and no root
`TypeLayoutHash`. Those scalars are deleted because the final top-level value is
not a nominal record.

Its exact inputs are:

1. accepted `CharacterCatalog`;
2. accepted `ViewRegistry`;
3. one `CharacterDialogueRuntimeCustomFieldCatalog` whose digest and closed
   field descriptors are one object;
4. one `CharacterDialogueRuntimeRoleTypes` containing the complete closed
   checked types for stage, portrait, focus, cleanup, hook, style, and rich
   text; and
5. one `RuntimeNominalRecordProducerAdmission` already bound to the canonical
   producer `std.character_dialogue` and the active plan generation.

Construction rejects a producer other than `std.character_dialogue` and
preflights every nominal node reachable from all role/custom checked types. As a
result, missing, stale, conflicting, or wrong-producer descriptor evidence is
rejected before any encode/decode publication.

For an exact value, the schema derives the exact CharacterDialogue semantic
identity from the decoded/encoded `CharacterId` and the retained
`CharacterDialogueType::Exact` projection. A caller never supplies the owner.

## 5. D4 — final physical representation

### 5.1 CharacterDialogue root

The sole runtime representation is:

```text
RuntimeValue::Opaque(RuntimeOpaqueValue {
  producer = "std.character_dialogue",
  semantic_identity = exact CharacterDialogue<Character> identity,
  payload = RuntimeValue::Tuple(exactly 18 producer-owned fields),
})
```

The complete 18-field order is normative in
`CHARACTER_DIALOGUE_REPRESENTATION.md`. The former
`std.character_dialogue` `RuntimeNominalRecordValue` payload, its
`expected_layout`, its layout scalar on `CharacterDialogue`, and all root
nominal construction/shape validation paths are deleted.

Opaque remains atomic to core. The dialogue owner validates its payload before
wrapping and after deserialization/restore. No `RuntimeTypeSchema` is fabricated
for the tuple.

### 5.2 Custom entries

Each custom entry is exactly:

```text
RuntimeValue::Tuple([
  RuntimeValue::String(canonical_field_id),
  admitted_runtime_value,
])
```

Entries are stored in a `RuntimeSeq::values` sequence in strict ascending
`CharacterDialogueCustomFieldId` order. The active custom catalog resolves the
ID to one closed `RuntimeCheckedType`; the producer capability validates the
entire value tree. The old nominal entry ID, nominal/layout declaration fields,
layout hash, and four-field wrapper are deleted.

### 5.3 Inline failure

Inline failure is stored directly as the existing closed nominal variant owned
by `arcweft.dialogue.InlineFailurePolicy`, including its existing nested
`InlineFallback` and `FallbackStylePolicy` variants. The obsolete
`std.inline_failure_policy` one-field nominal-record wrapper is deleted.
Owner, ordinal, exact case name, payload presence, and payload checked type are
all validated.

## 6. D5 — resolution of `Dynamic`

`RuntimeTypeSchema::Named("Dynamic")` is removed from the custom-entry path.
No `RuntimeCheckedType::Dynamic`, producerless opaque type, optional validator,
or arbitrary-value predicate is added.

Custom values are producer-owned opaque substructure at the CharacterDialogue
payload layer, but each value is separately admitted against the exact closed
checked type selected by its field ID. This is not an unchecked opaque leaf:
the top-level opaque payload is validated by its producer and every nested
nominal record is validated through the plan-scoped descriptor capability.

## 7. D6 — externally supplied nested nominal values

A nested nominal value is never admitted from its stored `(type_id, layout)`
pair alone. The caller supplies the active expected `RuntimeCheckedType`; a
CharacterDialogue producer capability resolves its nominal triple against the
active catalog before inspecting the value.

Deterministic outcomes are:

- conflicting catalog declarations: plan admission fails with
  `RuntimeNominalRecordCatalogError::ConflictingLayout`;
- unknown producer: `RuntimeNominalRecordLookupError::ProducerNotAdmitted`;
- nominal absent from the active catalog: `Missing`;
- semantic projection differs from the active catalog: `StaleSemanticIdentity`;
- layout differs from the active catalog: `StaleLayout`;
- exact descriptor exists but is not authorized for this producer:
  `WrongProducer`;
- descriptor found, value nominal mismatch: `RuntimeNominalRecordError::Type`;
- then layout, count, field-ID, and first defining-order predicate failure.

The error retains `RuntimeValuePath` and typed source error through every
higher boundary.

## 8. D7 — normalize, empty, and structured patch

Nominal values are **transformable only with an active descriptor and producer
capability**. They are not globally atomic, not descriptorlessly transformable,
and not unconditionally rejected.

Normalization receives the expected checked type and path. It first validates
the input tree, recursively canonicalizes only the selected checked shape, and
for every nominal node rebuilds through `RuntimeNominalRecordAdmission`, then
calls `validate_against_layout` defensively before returning. It preserves
field IDs and defining layout order.

Clear/empty is checked-type-directed. `Option` becomes `None`; a sequence
becomes empty; `Unit` remains `Unit`; tuple and nominal fields are recursively
emptied only when every child supports empty. Choice follows the uniquely
selected/source-order accepted branch. Primitive non-unit values, opaque
values, and variants/results without an explicit domain clear rule reject.
The old anonymous `Record(Vec::new())` fallback is deleted.

Structured patch uses the existing `RuntimeValuePath`, not dialogue-local
ordinal vectors. It validates limits and overlap, resolves every path against
both the original value and expected checked type, checks replacement/clear
eligibility for all operations, and only then clones and mutates. Nominal
boundaries are rebuilt on unwind through their handles. The complete candidate
is revalidated and domain/canonical validation runs before publication. Any
failure publishes no partial value.

## 9. D8 — deserialization, restore, replay, and activation

All deserialized objects are quarantined:

1. deserialize raw `RuntimePlan`/wire DTO/`RuntimeValue`;
2. admit the complete plan and obtain the active catalog;
3. obtain the expected checked type and producer/domain capability from the
   active slot, role, View input, root/replay contract, or dialogue schema;
4. validate every nominal tree and CharacterDialogue opaque payload;
5. only then perform ownership/nesting traversal, domain decoding, restore,
   replay, publication, or activation.

Live `CharacterDialogueTypedValue` and its role wrappers no longer implement
`Deserialize`. Bundle/save codecs decode a raw `RuntimeValue` and call the
schema's role/custom admission method.

A4 owns the representation cut, admitted-plan boundary, typed validation hooks,
and deletion of unchecked constructors. Parent A6 later closes exact codec and
golden fixture coverage. A4 leaves no unchecked publication interval, and A6
does not add an old reader. Every version remains `1`.

## 10. D9 — exact error preservation

Core catalog, lookup, nominal-value, checked-tree, and path errors remain typed
sources. Dialogue wraps them in role/custom/patch variants carrying exact
`RuntimeValuePath`. Runtime-driver activation, root/replay, session restore,
and bundle ingress retain those source types rather than converting them to an
identity/layout/type message string. Domain-only malformed tuple/variant errors
use a typed `CharacterDialoguePayloadShape` plus path.

The mapping is normative in `VALIDATION_ERROR_AND_PATH_PRECEDENCE.md`.

## 11. D10 — deletion closure

The final A4 cut deletes, in one compile-clean workspace state:

- public `RuntimeNominalRecordValue::new`;
- `RuntimeNominalRecordValue::validate_shape`;
- every descriptorless nominal rebuild in core, dialogue, View, driver,
  root/replay, bundle/save, agent, CLI, and accelerator consumers;
- `CharacterDialogue.layout`, schema `expected_layout`, root nominal ID/layout
  accessors, and `CharacterDialogueValue.record`;
- custom-entry nominal ID/layout/schema and its declared nominal/layout fields;
- inline-failure nominal ID/layout/schema and wrapper;
- `RuntimeTypeSchema::Named("Dynamic")` in the custom entry;
- direct `Deserialize` and raw `try_new(nominal, layout, value)` on
  `CharacterDialogueTypedValue`;
- dialogue `RuntimeFieldPath` and ordinal-only patch traversal;
- identity/layout-only validators and stale test helpers; and
- caller-supplied CharacterDialogue opaque-owner wrapping.

The exact inventory and compile-clean order are in the named sidecars. No
parallel representation or compatibility path remains.

## 12. Required precedence

For every nominal boundary, including nested CharacterDialogue values:

1. descriptor lookup and producer/domain admission;
2. nominal identity;
3. layout hash;
4. field count;
5. defensive field-ID derivation;
6. first field predicate failure in defining layout order;
7. CharacterDialogue domain and canonical-form validation; and
8. publication, ownership traversal, restore, replay, or activation.

For CharacterDialogue root decode, exact opaque producer admission and minimal
18-tuple/character extraction precede exact semantic identity comparison; all
nested nominal boundaries then follow the eight-step order above. Structured
patch path resolution and mutation eligibility precede every mutation.

## 13. Readiness

Every result-changing decision requested by Lang-01.3.1.2.3.2.1.2 is closed.
`OPEN_QUESTIONS=0`. This archive contains no production source, patch, branch,
overlay, dual reader, or compatibility implementation.
