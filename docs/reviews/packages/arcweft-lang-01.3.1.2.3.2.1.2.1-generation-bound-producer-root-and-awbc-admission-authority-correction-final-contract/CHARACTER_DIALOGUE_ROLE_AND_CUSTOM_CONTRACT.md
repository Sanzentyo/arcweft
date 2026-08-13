# CharacterDialogue role, custom-field, catalog, and schema authority

## 1. Layering

The shared role coordinate enum lives in `arcweft-interaction-model`, which is a
permitted lower dependency of both semantic and runtime layers.

- sema owns accepted source/type evidence;
- runtime-plan owns checked projection and generation-contract emission;
- core owns the serialized generation contract and operational aggregate;
- dialogue owns domain validation, tuple18 encoding/decoding, typed wrappers,
  patching, and schema publication;
- runtime-driver owns activation and generation-image orchestration.

`arcweft-dialogue` does not import runtime-plan, sema, compiler, HIR, syntax, or
runtime-driver.

## 2. Current `Named` rows

The current callable-family rows using `TypeKind::Named` for dialogue roles are
deleted in the same semantic gate. They are replaced with typed
`TypeKind::CharacterDialogueRole` coordinates.

The standard-library semantic registration supplies exactly one accepted
declaration for Stage, Portrait, Focus, Cleanup, Hook, and RichText. Those
declarations are ordinary typed semantic facts, not names. Alias normalization
must terminate in closed `TypeKind` values.

If either `Named` or `CharacterDialogueRole` remains after accepted-role
substitution, runtime projection returns a typed error with role, semantic type
ID, and source span. It never guesses opaque evidence.

## 3. Exact role relationship

The role fact owner records six base declarations. Style is not an independent
fact:

```text
Style = Choice([EntityRef, RichText])
```

This preserves the current source-order intention while eliminating the
unresolved `"RichTextStyle"` row. Both branches are traversed for producer
authorization. Runtime values must match exactly one branch.

The admitted role set carries all seven checked types and one generation
identity. Every accessor is read-only.

## 4. Custom semantic projection

The existing sema custom-field registry remains the source owner for:

- field ID;
- accepted semantic type;
- runtime nominal projection where applicable;
- clearability;
- accepted Views;
- declaration/source evidence.

The bridge projects each accepted type to one closed `RuntimeCheckedType` and
constructs
`CharacterDialogueRuntimeCustomFieldDescriptorDeclaration`. It drops runtime-
redundant nominal/layout side scalars because those are represented inside the
closed checked type and canonical nominal catalog.

The bridge verifies that all descriptors belong to the same
`AcceptedNominalWorldStamp` as the standard role facts.

## 5. Runtime custom digest

`CharacterDialogueRuntimeCustomFieldCatalogDeclaration::try_from_checked_projection`
is the only trusted bridge constructor. It computes the runtime digest from the
canonical descriptor body.

Raw Serde contains the claimed digest, because a raw plan/AWBC program is
untrusted. Admission recomputes it. The operational catalog view always returns
the recomputed admitted digest.

No API accepts `(digest, descriptors)` and no schema accepts a raw catalog.

## 6. Limits and ordering

- at most 4,096 custom fields;
- at most 256 accepted Views per field;
- fields strictly ascending by `CharacterDialogueCustomFieldId`;
- accepted Views strictly ascending by `RuntimeViewId`;
- duplicate field or View IDs fail;
- every field checked type is closed and traversed under the generation work
  budget;
- the first error is field order, then checked-type path.

## 7. Character and View catalog correlation

The CharacterDialogue producer declaration stores one
`RuntimeCharacterCatalogDigest` and one `RuntimeViewCatalogDigest`.

Raw `CharacterCatalog` and `ViewRegistry` are admitted for a generation only
after canonical digest recomputation. The resulting non-Serde wrappers carry
the exact generation identity.

A custom descriptor's accepted View IDs are checked against the admitted View
registry during catalog admission and again when a concrete dialogue value
selects its View. A missing View fails before nested custom value validation
for that field, following domain order.

## 8. Schema construction

`CharacterDialogueRuntimeSchema::try_from_generation` is the sole constructor.

It checks, in order:

1. all three wrappers have identical `RuntimeGenerationIdentity`;
2. producer is exactly `std.character_dialogue`;
3. the specialized admission points to the admitted CharacterDialogue payload;
4. role declaration canonical identity and derived Style are valid;
5. custom digest and descriptor map match the admitted declaration;
6. Character catalog digest matches;
7. View catalog digest matches;
8. every nominal key reachable from role/custom types is admitted for the
   producer;
9. only then publish the schema.

The schema stores the borrowed specialized admission and admitted catalog
wrappers. It does not copy the role/custom maps or nominal descriptors.

## 9. Tuple18 relationship

The retained `.1.2` tuple18 representation remains exact:

- stage, portrait, focus, cleanup: outer Option of the corresponding role;
- hooks: values sequence whose element type is Hook;
- style: exact derived Style choice;
- rich text: exact RichText type;
- custom values: sorted tuple2 entries validated by field descriptor;
- inline failure: direct closed variant;
- voice: nested Option/voice variant defined separately.

The top-level opaque producer remains atomic to core.

## 10. Encode/decode

Before encoding:

1. generation/schema correlation is already established;
2. validate CharacterDialogue domain invariants;
3. validate every role/custom value through the producer shape view;
4. check Character/View/custom rules;
5. build the tuple18;
6. derive the exact CharacterDialogue opaque semantic identity;
7. wrap and canonically encode.

Before decoding:

1. verify schema generation;
2. verify opaque producer;
3. verify tuple18 shape;
4. decode character and derive exact semantic identity;
5. verify the opaque identity;
6. validate fields in tuple index order using admitted role/custom facts;
7. perform domain/canonical validation;
8. re-encode when persisted external bytes require canonical equality;
9. publish.

## 11. Typed wrappers

`CharacterDialogueTypedValue` and role/custom wrappers retain `.1.2`'s
non-Deserialize, admitted-only construction.

Each wrapper additionally carries or borrows the generation identity through
its owning schema/value object. Equality and hashing are valid only for values
admitted under the same generation; cross-generation comparison either returns
a typed mismatch in fallible APIs or includes generation identity in the
canonical equality key.

No wrapper accepts a raw `RuntimeValue`, raw checked type, raw custom digest,
or raw catalog.

## 12. Normalize, clear, and patch

The `.1.2` descriptor-aware semantics remain. Every operation uses the schema's
generation-bound producer shape.

- normalize validates first, normalizes through the selected exact checked
  shape, reconstructs nominal nodes through admitted shapes, and validates
  again;
- clear uses checked-type-directed rules and custom `clearable`;
- structured patch preflights every path and replacement, applies to a
  candidate, rebuilds nominal boundaries on unwind, revalidates the whole
  value, then publishes atomically.

A generation mismatch is detected before path resolution or mutation.

## 13. Digest, equality, and hash

CharacterDialogue digest is schema-owned and includes the retained canonical
opaque RuntimeValue bytes. Those bytes already include:

- exact opaque owner;
- tuple18 structure;
- custom runtime digest;
- Character/View digests;
- nested voice representation;
- role/custom values.

The schema's generation identity is included in any external cache key that
could compare values across generation images. No raw domain object computes
runtime bytes without a schema.
