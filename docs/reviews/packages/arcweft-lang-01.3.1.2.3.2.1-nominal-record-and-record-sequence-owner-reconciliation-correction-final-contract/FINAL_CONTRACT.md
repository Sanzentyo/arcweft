# Final contract

## 1. Precedence and scope

This contract is Lang-01.3.1.2.3.2.1. It is normative only for nominal-record
layout ownership, nominal record admission/lowering, anonymous/column record
carrier admission needed by G1.2-A, and record-sequence error ownership. It
narrowly supersedes conflicting rows in Lang-01.3.1.2.3.2. Every other accepted
parent decision remains in force, including the foundation implemented at
`08bc30c0c8eac77152a42e92a5ca2f83280b94bc`.

No production source is included. Implementation SHALL begin from exact commit
`2585f527b02808305b3a8cab0442eb522e8d0352` or a descendant that has been mechanically reconciled against
this symbol closure.

## 2. Final owner selection

The absent parent name `RuntimeNominalRecordSchema` SHALL NOT be declared,
aliased, or emulated. The final core owner is:

```text
arcweft_core::value::RuntimeNominalRecordLayout
```

Its source owner is the existing nominal-record module under
`arcweft_core::value`. It is an executable layout descriptor, not a persistence
schema. Its exact Rust declaration and API are in `RUST_OWNERS_AND_APIS.md`.

The descriptor has exactly four semantic components:

1. canonical runtime nominal identity: `RuntimeNominalTypeId`;
2. checked semantic projection identity: `RuntimeSemanticTypeId`;
3. exact transitive runtime layout identity: `TypeLayoutHash`; and
4. defining-layout-order fields, each carrying one diagnostic name and one
   closed `RuntimeCheckedType` predicate.

There is no field-ID vector. A field ID is derived from its zero-based layout
ordinal through the accepted one-based `RuntimeRecordFieldId` constructor.
There is no embedded `RuntimeTypeSchema`, no entry-role copy, no HIR/sema object,
and no source span.

## 3. Identity, equality, and sharing

Canonical runtime value identity remains the existing pair
`(RuntimeNominalTypeId, TypeLayoutHash)`. `RuntimeSemanticTypeId` records the
checked generic/type projection that produced a descriptor but is not added to
`RuntimeNominalRecordValue` or canonical runtime-value bytes.

`RuntimeNominalRecordLayout` equality is structural across all four components.
`Arc::ptr_eq` SHALL NOT be used for equality, ordering, hashing, cache keys,
replay identity, or validation. Runtime-plan allocates and interns one
`Arc<RuntimeNominalRecordLayout>` for each accepted key
`(nominal, semantic_identity, layout)` in one executable generation. If that
key is observed with different structural fields, fact publication fails with
`ConflictingNominalRecordLayout`.

The `Arc` is retained by nominal-record expressions and nominal record patterns.
`RuntimeNominalRecordValue` retains only its existing `type_id`, `layout`, and
layout-order `Vec<RuntimeValue>`. Values therefore do not duplicate a schema or
extend canonical byte identity.

## 4. Layer reconciliation

### 4.1 Sema

`CheckedProjectNominal` remains the semantic authority for declaration,
instantiated generic arguments, and semantic digest. Sema continues to reject
source-level duplicate, missing, extra, and field-type errors. Sema does not
move into core and does not construct runtime values.

### 4.2 Compiler/runtime-plan bridge

The bridge projects the sema-accepted nominal record into two products in one
operation: a defining-order closed `RuntimeCheckedType` field list and an
*ephemeral* `RuntimeTypeSchema::Record`. The latter is never retained by the
executable descriptor. Its existing inherent
`RuntimeTypeSchema::try_layout_hash` method is the sole production algorithm
that creates the `TypeLayoutHash`; no semantic digest bytes or local BLAKE3
call may be substituted. The bridge then calls
`RuntimeNominalRecordLayout::try_from_checked_projection` with that hash.

For an entry-role projection, the existing sema `NominalSchemaDigest` is an
independent checked witness over the same canonical `TypeShape` bytes. The
compiler SHALL first project `checked.schema()` through the existing
`RuntimeSchemaProjection::schema`, call `try_layout_hash`, and require bytewise
equality with `checked.schema_digest()` before constructing
`RuntimeNominalRole`. The exact baseline currently writes the digest bytes
directly into `TypeLayoutHash`; that shortcut is replaced in the A1 cut so
schema canonicalization has one runtime owner and drift is a typed error.

`RuntimeResolvedNominal` gains the accepted `TypeLayoutHash`. Record expression
and pattern facts use `RuntimeResolvedNominalRecord`, which pairs that nominal
fact with exactly one shared layout descriptor. Publication validates item
family, nominal identity, semantic identity, layout identity, field count,
field names, and structural conflict before a fact becomes executable.

The existing `nominals` and `pattern_nominals` record-expression maps and their
push/accessor APIs are deleted in the same compile-clean cut that introduces
`nominal_records` and `pattern_nominal_records`; no dual fact reader remains.

### 4.3 Entry roles and persistent schemas

`RuntimeNominalRole` remains the entry-registration owner of identity, layout,
and `RuntimeTypeSchema`. `RuntimeTypeSchema` remains the persistent wire/schema
validation owner. Neither is renamed or copied into the new core layout.

For an explicitly role-backed nominal, generation requires exact equality of
`RuntimeNominalRole.identity` and `.layout` with the projected nominal/layout,
and the role's `schema.try_layout_hash()` must equal `.layout`. Role schema
validation remains in the entry owner. A project nominal with no explicit role
binding still uses the same transient schema canonicalization solely to obtain
its layout hash; it is not registered as an entry role and the schema is not
retained. Core never depends on entry-role construction APIs, HIR, sema,
runtime-plan, or compiler.

## 5. Nominal initializer admission and evaluation

`RuntimeExpr` gains one nominal-record expression variant whose checked carrier
contains a shared layout and authored-order initializer entries. Every entry
stores the accepted one-based field ID beside its diagnostic name and child
expression.

Admission validates names and maps them to IDs before plan publication.
Initializer expressions remain in authored order. At execution:

1. evaluate each initializer in authored order;
2. place its result into an ephemeral `Vec<Option<RuntimeValue>>` indexed by the
   accepted field ID;
3. after all initializer evaluations succeed, consume the buffer in defining
   layout order; and
4. call `RuntimeNominalRecordValue::try_from_accepted_layout`.

The temporary buffer is construction state, not a persisted side table. It is
never attached to the value and never visited as a second record model.

Current lowering of a nominal HIR record into anonymous `RuntimeExpr::Record`
is deleted. A nominal expression may never lose nominal/layout identity during
lowering.

## 6. `try_from_accepted_layout` decision table

The method receives a trusted descriptor and values already arranged in layout
order. It performs exactly:

1. exact field-count equality;
2. defensive derivation of every layout ordinal into `RuntimeRecordFieldId`;
3. first field-value predicate mismatch in defining layout order, using
   `RuntimeCheckedType::accepts_value`; and
4. construction with the descriptor's nominal ID and layout hash.

It does not receive or check initializer names. Duplicate, missing, extra, and
unknown names are rejected by `RuntimeNominalRecordExpr` admission before
execution. It does not reorder values. It does not accept an independently
supplied nominal ID or layout hash, so no constructor-time identity mismatch is
possible.

`validate_against_layout`, used for deserialized/restored/existing values,
performs exact nominal ID, layout hash, field count, ordinal identity, and field
predicate checks in that order.

Nested nominal field predicates compare both nominal ID and `TypeLayoutHash`.
The repository's current type-only check is deleted. `RuntimeCheckedType` owns
this behavior through an inherent `accepts_value` method; the free helper is
deleted rather than wrapped by a trait.

## 7. Unchecked nominal constructor cut

The public `RuntimeNominalRecordValue::new` and current `validate_shape` are
deleted in the same compile-clean cut that migrates all evaluators, constants,
AWBC paths, root/replay validators, snapshots, bundles, and save/restore
consumers to the admitted constructor or `validate_against_layout`. No deprecated
wrapper, alias, compatibility constructor, feature flag, or fallback reader is
allowed.

Direct deserialization may temporarily remain only because enclosing live plan
and value types still require Serde under the parent schedule. Every ingress
must validate against the active layout before publication or traversal.

## 8. Anonymous and column carriers

The accepted parent carrier shapes remain:

- `RuntimeFieldValue { field, name, value }` with private fields;
- `RecordSeqField { field, name, values }` with private fields; and
- nominal values with one layout-order values vector and derived IDs.

Anonymous admission assigns IDs in duplicate-free authored order. Record-column
admission assigns IDs in accepted stored column order. Nominal IDs derive from
defining layout order. All three produce contiguous `1..=field_count` identities
without a side ID vector.

Raw field construction is deleted from external and internal call sites in the
same cut that private fields land. Sequence materialization and row
reconstruction preserve the already accepted IDs; they do not regenerate IDs
from names.

## 9. Record-sequence error owner

The absent name `RecordSeqError` SHALL NOT be introduced. The existing public
`RuntimeSeqError` remains the sole tuple/record column admission error and gains
the exact record-specific variants in `RUST_OWNERS_AND_APIS.md`.

Record-column failure precedence is:

1. field count outside the one-based `u32` identity space;
2. for each stored field in ascending ordinal: field-ID conversion, column
   length, then duplicate diagnostic name;
3. the first failing ordinal wins.

Thus current ordinary behavior—column length before duplicate at the same
field—is preserved. The count preflight dominates representational overflow;
the `InvalidRecordFieldIdentity` variant remains a defensive mapping for the
accepted ID constructor and is exercised by synthetic arithmetic tests without
allocating `u32::MAX + 1` fields.

`RecordSeq::new` is deleted. `RuntimeSeq::record_columns` changes in the same
cut to accept `(String, RuntimeSeq)` pairs and delegates to
`try_from_accepted_fields`. No split error enum or compatibility overload
remains.

## 10. Pattern correction

`RuntimePattern::Record` replaces its `owner: Option<RuntimeCheckedType>` with
`nominal_layout: Option<Arc<RuntimeNominalRecordLayout>>`. `None` means an
anonymous record. `Some(layout)` means a nominal record.

Nominal matching first validates exact value identity/layout/shape against the
descriptor. Pattern field names are resolved through that sole descriptor to
accepted field IDs; values are indexed by those IDs. The current positional
`zip` behavior is deleted. Pattern lookup by an authoritative layout name is
not a visitor fallback and does not create a second schema.

## 11. Shared value-path visitor

The single path-aware visitor consumes inherent carrier accessors only:

- anonymous record: stored `RuntimeFieldValue::field()` -> `RecordField`;
- record column: stored `RecordSeqField::field()` -> `RecordColumn`;
- nominal record: `RuntimeNominalRecordValue::field_id(ordinal)` ->
  `NominalRecordField`.

Names are never used to recover a visitor path. Ownership classification,
nesting/node accounting, snapshot owner graph extraction, and other record
walkers migrate to this visitor or an owner method that delegates to it. No
parallel traversal or side identity table remains.

## 12. Canonical bytes and public wire behavior

Existing canonical runtime-value distinctions remain unchanged:

- anonymous records continue to use their existing anonymous record encoding;
- nominal records continue to encode nominal ID, layout hash, count, and values
  in layout order; and
- anonymous and nominal records remain canonically distinct.

Field IDs are traversal identities. They are not added to canonical anonymous
or nominal runtime-value semantic bytes. The already accepted independent
Serde/fixed-LE codecs for `RuntimeRecordFieldId`, `RuntimeOwnedSlotId`, and
`RuntimeValuePath` are unchanged.

The plan's interim Serde shape changes because the nominal expression now
retains its descriptor and field identities. This is an unreleased internal
replacement, not a new AWBC ABI or codec allocation. AWBC remains ABI 1/codec 8
under the parent contract. No dual reader is added.

## 13. Trait schedule

While enclosing `RuntimeValue`, `RuntimeSeq`, `RuntimeExpr`, and
`RuntimePattern` require live `Clone` and Serde, the new/private carriers retain
matching derives. At the already accepted parent snapshot-projection and live
carrier deletion stages, those derives are removed together with the enclosing
requirements. This correction creates no new trait-retention stage and does
not remove traits early.

## 14. Readiness

All result-changing decisions requested by Lang-01.3.1.2.3.2.1 are closed.
`OPEN_QUESTIONS.md` is exactly `none`. Implementation order, deletion cuts,
consumer closure, errors, precedence, and tests are normative sidecars in this
archive.
