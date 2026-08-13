# Descriptor lookup and transformation contract

## 1. Required inputs

Every CharacterDialogue runtime-typed operation receives all of:

- the active expected `RuntimeCheckedType`;
- the `std.character_dialogue` producer admission capability;
- the current `RuntimeValuePath`; and
- the operation-specific limits/domain policy.

There is no overload that receives only an optional nominal ID, layout hash,
raw value, or field ordinal.

## 2. Recursive validator

For each expected checked type:

- primitive: require the exact corresponding value kind and width/range rule;
- sequence: require a value sequence and validate elements in index order;
- tuple: require exact arity and validate in tuple order;
- choice: use the deterministic source-order selection rule in
  `AUTHORITY_AND_CATALOG.md`;
- result/option: require built-in owner, ordinal, exact name, payload presence,
  and child predicate;
- nominal variant: require exact owner, ordinal, exact case name, payload
  presence, and child predicate;
- nominal record: perform producer-bound descriptor lookup, then
  `validate_against_layout`, then recurse through defining-order fields;
- opaque: require owner admission and treat payload atomically; and
- `Never`: reject.

The first error is returned with the exact path. No later field is inspected
after a defining-order predicate failure.

## 3. Normalization

The final operation is logically:

```rust
fn normalize_runtime_value(
    producer: RuntimeNominalRecordProducerAdmission<'_>,
    expected: &RuntimeCheckedType,
    path: &RuntimeValuePath,
    value: RuntimeValue,
) -> Result<RuntimeValue, RuntimeNominalRecordTreeError>;
```

It is private implementation behavior owned by the original dialogue typed
value/schema module; the signature above is normative shape, not a public free
function.

Rules:

1. validate the input against `expected` before transformation;
2. finite `f32`/`f64` normalize negative zero to positive zero; non-finite
   values fail at their path;
3. tuple/sequence/selected variant payloads recurse in canonical order;
4. opaque values remain atomic and unchanged;
5. a nominal node obtains its handle before any child transform, validates the
   original, transforms children under descriptor field predicates, rebuilds
   through `try_construct`, and calls `validate_against_layout` defensively;
6. field order and one-based IDs are never recomputed from names;
7. anonymous records lacking an active closed checked type reject rather than
   becoming an untyped success; and
8. the returned complete tree is validated again before a live wrapper is
   published.

The old branch that copies `type_id` and `layout` and invokes public `new` is
deleted.

## 4. Empty/clear semantics

Clear is not inferred from the current runtime value alone. It is selected by
the active checked type:

| Expected checked shape | Empty result |
|---|---|
| `Unit` | `Unit` |
| `Option(T)` | canonical `None` |
| `Sequence(T)` | empty values sequence |
| `Tuple(T...)` | recursively empty each item; fail atomically if any item cannot empty |
| `Nominal` | lookup/validate descriptor, recursively empty every field, rebuild through handle |
| `Choice` | select the current accepted branch deterministically, then apply that branch's rule |
| primitive except Unit | reject |
| opaque | reject unless the producer defines a separate explicit domain clear operation (CharacterDialogue defines none here) |
| result or nominal variant | reject unless the containing domain field has an explicit case-level clear rule |
| `Never` | reject |

For optional top-level dialogue roles, `PatchField::Clear` removes the option;
it does not call child empty. For custom fields, removal is permitted only when
the active descriptor says `clearable`; otherwise it rejects.

The old `RuntimeValue::Record(Vec::new())` fallback and descriptorless nominal
rebuild are deleted.

## 5. Structured patch path type

`StructuredPatch<T>` changes from:

```text
BTreeMap<dialogue::RuntimeFieldPath(Vec<u16>), PatchField<RuntimeValue>>
```

to:

```text
BTreeMap<arcweft_core::value::RuntimeValuePath, PatchField<RuntimeValue>>
```

Only path segments appropriate to the active checked/value shape are accepted:

- tuple -> `TupleElement`;
- values sequence -> `SequenceElement`;
- nominal record -> `NominalRecordField`;
- nominal/built-in variant payload -> `VariantPayload`;
- anonymous record/record column only where an independently accepted anonymous
  carrier contract exists -> `RecordField`/`RecordColumn`.

A `NominalRecordField` is resolved through the active descriptor. It is never
converted from a user ordinal or field name.

## 6. Atomic patch algorithm

For one `StructuredPatch`:

1. enforce operation-count and depth limits;
2. sort by existing `RuntimeValuePath` ordering;
3. reject duplicate and prefix-overlapping paths;
4. for every operation, resolve the complete path against the original value
   and active checked type;
5. establish Set/Clear mutation eligibility and validate every replacement;
6. compute every required nominal admission handle and every clear result;
7. if any preflight fails, return without cloning/mutating;
8. clone one candidate value;
9. apply operations in deterministic path order;
10. when unwinding through a nominal node, rebuild the full defining-order
    field vector through the previously resolved handle;
11. validate the complete candidate against its checked type;
12. run dialogue role limits/domain/canonical validation; and
13. replace the live wrapper only after all checks succeed.

Sequence mutation is exact: index `< len` may replace/remove; index `== len`
may append only for `Set`; index `> len` is missing. Tuple and nominal fields
are never physically removed; `Clear` must produce a valid empty child.

No partially rebuilt nominal value is observable, including on a late field or
domain failure.

## 7. Live typed wrappers

`CharacterDialogueTypedValue` stores only the admitted `RuntimeValue`. It has
no public raw constructor and no `Deserialize`. Role/custom wrappers are
created only by schema methods that run normalize, catalog-aware validation,
limits, and canonical encoding.

Replacing a wrapper's inner value by struct update or a private
`replace_runtime_value` helper is deleted. Patch returns a newly admitted role
wrapper from the same schema method that owns validation.
