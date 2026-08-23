# Final design

## 1. Authority and retained substrate

This design applies to production at
`9a5d30d25620541c3f2975d31e04e04e3bc9514c`. The maintained request, current
source, and the accepted parent ownership matrix outrank both rejected
returned archives.

The following existing authorities remain final:

- `AcceptedNominalCatalog` owns exact semantic nominal declarations;
- `AcceptedRustTypeMetadataCatalog` owns source-backed Rust shape and generic
  substitution;
- `FinalSemanticAnalysis` owns generation-bound projection;
- `RuntimeTypeSchema` and `TypeLayoutHash` own canonical schema/layout identity;
- `RuntimePlanTypeTable`, `RuntimeNominalRecordDomainTable`, and
  `RuntimeVariantDomainTable` own executable plan types;
- `RuntimeNominalRecordLayout`, `RuntimeCheckedType`, and
  `RuntimeVariantIdentity` own live admission;
- `RuntimeValue` owns the live value algebra; and
- `AwbcProgram` plus its current type, string, constant, fiber, and snapshot
  tables own bytecode and restore validation.

There is no `AcceptedRuntimeCarrier`, no `RuntimeValueHandle`, no
`RuntimeTypeCatalog`, and no second persisted catalog. The exact opaque path
is unchanged.

## 2. Semantic catalog join

`AcceptedNominalSemantics` is evolved in place with the data-free semantic
role `RustAdt`. It is not executable carrier evidence and does not say record
or variant. Adapter Rust declarations publish `RustAdt`; adapter-defined
opaque declarations continue to publish `Opaque(AcceptedOpaqueRuntimeCarrier)`.
The obsolete Rust `opaque_producer` publication is deleted.

Registration publishes a `RegisteredTypeCheckEnv` only after a bijective
join between every `RustAdt` nominal row and one Rust metadata row. The join
requires identical `AcceptedNominalId`, `EnvironmentPublicationItemId`, Rust
package owner, `RustExport` origin, arity, visibility/source allocation, and
the same registration transaction. A metadata-only row, a `RustAdt` row
without metadata, an opaque row with metadata, a duplicate, or an owner/item
mismatch aborts the whole environment publication.

`AcceptedRustTypeMetadata` therefore retains its existing publication item.
`RegisteredEnvironmentDigest` already commits the nominal catalog, visibility
index, and Rust metadata digest; it remains the exact joined-world stamp.
Unrelated catalog rows may change that stamp but never change a type's layout
hash.

`AcceptedNominalRecord::try_instantiate` returns
`TypeKind::AcceptedNominal` for both `RustAdt` and exact opaque rows. Which
runtime carrier is legal is always rejoined from the exact catalog; it is
never copied into `AcceptedNominalType`.

## 3. Final-analysis projection and stale authority

`FinalSemanticAnalysis::project_accepted_rust_nominal(world, nominal, limits)`
is the only structural accepted projection entry. It first charges the work
budget, then requires `matches_symbol_lease(world.symbols())`, the exact
world/revision, the joined environment allocation, the exact `RustAdt` row,
and the matching metadata row. It substitutes the supplied generic arguments
through the current metadata catalog and builds every reachable structural
nominal definition transactionally.

The returned `RuntimeAcceptedRustNominalProjection` has private fields and
retains the registered environment digest, nominal-world stamp, Rust metadata
digest, root semantic identity, root runtime nominal identity, root layout,
root kind, and one core-owned accepted schema graph. Getters are read-only.
There is no public raw-parts constructor.

The compiler already receives `RegisteredSemanticWorld` and
`FinalSemanticAnalysis`. Every lowering call revalidates the projection stamp
against those exact owners before it can stage a runtime-plan row. A stale
world, symbol revision, metadata catalog, environment digest, or foreign
analysis rejects before a type seed, layout, canonical value byte, constant,
or snapshot is exposed.

## 4. Recursive and generic graph

Projection is keyed by the existing
`TypeKind::AcceptedNominal(...).semantic_identity_digest()`, which includes
the exact accepted declaration and instantiated arguments. Runtime nominal
IDs are derived from that typed digest as `aw.accepted.<lowercase-hex>`; no
source/display spelling is parsed.

The traversal uses `Unseen`, `Visiting`, and `Complete` states per semantic
identity. Entering `Visiting` emits a typed `RuntimeTypeSchema::NominalRef`
edge. It does not recurse, reject, or create a placeholder definition.
Completing the first visit publishes the one definition. Mutual recursion is
therefore finite; generic instantiations are distinct nodes; repeated exact
instantiations reuse the completed node. Non-nominal type cycles and dangling
references reject.

The graph contains only definitions reachable from the requested root. Its
canonical encoder sorts definitions by semantic identity while preserving
field, tuple item, and case order inside each row. A layout hash is BLAKE3 of
the version-1 domain, the selected root identity, and its reachable sorted
graph. Derived layout hashes are not input atoms, so recursive graphs have no
cyclic digest dependency. An unrelated catalog or graph row cannot perturb a
layout.

Nested exact opaque types are leaf schemas containing their exact producer,
semantic identity, admission, value class, persistence, and recursively
checked generic arguments. Producer-wide weakening is never inferred.

## 5. Record carriers and stable fields

All accepted Rust structs and newtypes use
`RuntimeValue::NominalRecord(RuntimeNominalRecordValue)`. Their layout retains
`RuntimeNominalRecordShape::{Unit, Tuple, Record, Newtype}`. Every field has
an explicit one-based `RuntimeRecordFieldId` derived from its zero-based
source ordinal, an optional name, and its checked type.

The invariants are closed:

- `Unit`: zero fields;
- `Tuple`: every field name is `None`, including zero- and one-field tuples;
- `Record`: every field name is nonempty and unique in source order; and
- `Newtype`: exactly one unnamed field.

`RuntimeNominalRecordValue` gains the semantic identity already held by its
layout. Its public unchecked `new` is deleted. The only constructor validates
type, semantic identity, layout, field count, each stable field ID, and each
field predicate against an accepted layout. Snapshot restore calls the same
constructor through the current AWBC descriptor.

`RuntimeValue::Record` remains the structural carrier for enum record
payloads. `RuntimeFieldValue` fields remain private. Its accepted constructor
requires exact contiguous field IDs and source-order names. Canonical value
identity stops sorting by name and instead encodes accepted field-ID order;
duplicate, missing, reordered, or mismatched IDs reject.

## 6. Enum payload normalization

All accepted Rust enums use `RuntimeValue::Variant`. The outer owner is
`RuntimeVariantIdentity::Nominal { nominal, semantic_identity, layout }` and
the case is its source-order `u32` ordinal plus exact case name.

Payload normalization is fixed:

| Rust metadata payload | Live payload |
|---|---|
| `Unit` | `None` |
| `Tuple([])` | `Some(RuntimeValue::Tuple([]))` |
| `Tuple([T])` | `Some(RuntimeValue::Tuple([value]))` |
| `Tuple([T0, ...])` | `Some(RuntimeValue::Tuple(values))` |
| `Record([])` | `Some(RuntimeValue::Record([]))` |
| `Record(fields)` | `Some(RuntimeValue::Record(fields))` |

A one-field tuple is never flattened. Empty tuple and empty record payloads
remain distinct from a unit case. Variant and record names are semantic
metadata and participate in the layout; source spans and Rust display paths
do not.

`EnumVariantPayload::Record` changes from `BTreeMap` to an ordered boxed
slice. Projection uses a `BTreeSet` only for duplicate detection and never
collects into a map. Struct record fields receive the same duplicate and
empty-name gate. Reordering fields or cases changes the layout digest.

## 7. Checked predicates

`RuntimeCheckedType` is extended in place with exact structural record fields,
and nominal variants gain `layout`. `RuntimeCheckedType::Record` requires the
same field count, contiguous IDs, names, order, and recursively accepted
values. Tuple, Option, and Result retain their existing conventions:
`Some=0`, `None=1`, `Ok=0`, and `Err=1`.

Recursive nominal validation is resolved against the existing plan/AWBC type
graph rather than materializing an infinitely recursive `RuntimeCheckedType`
tree. `RuntimePlan::accepts_value(type_id, value, limits)` walks the existing
type table plus nominal record/variant domains. `AwbcProgram` owns the exact
inverse walk over AWBC type rows. Both descend the finite value tree, charge
depth/node work before descent, and call the same core record, variant, Option,
Result, opaque, tuple, and record predicates. A recursive type back-edge is
legal; a cyclic live `RuntimeValue` is not constructible.

`RuntimePlan::checked_type` continues to return finite predicates. If a
recursive nominal back-edge is encountered while materializing a diagnostic
tree, it returns the existing typed cycle diagnostic; executable acceptance
uses `accepts_value`, never a weakened shallow predicate.

## 8. Compiler and runtime-plan projection

The generic core type row is renamed in place from
`RuntimePlanTypeProjection::ProjectNominal` to `Nominal`; there is no legacy
alias. `RuntimeResolvedNominal` retains typed source provenance as
`Project { declaration, owner }` or `AcceptedRust`, while its executable
identity, semantic identity, and layout are the same core fields for both.
`RuntimeVariantOwner::Project` becomes the general `Nominal` owner and the old
case is deleted.

The compiler projects the complete accepted schema graph as one atomic batch
of the existing `RuntimePlanTypeSeed`, nominal record-domain, and
variant-domain seeds. Record domains gain shape, explicit field IDs, and
optional names; variant domains gain the exact layout. The aggregate plan
builder verifies graph isomorphism and every layout against the core schema
proof before committing any table. It consumes and discards the inert schema
graph; `RuntimePlan` retains only its existing type/domain authorities.

All reachable accepted nominal definitions are admitted even when only the
root appears syntactically, so mutual-recursion restore never depends on
incidental expression reachability. Dependency direction remains
`core <- sema`, `core <- runtime-plan`, and
`sema + runtime-plan -> compiler`; core and runtime-plan never import sema.

## 9. AWBC lowering and tag allocation

No new AWBC type or constant tag is allocated. Existing tags are the allocator
authority and are reused:

- runtime type: Tuple `10`, Record `12`, Variant `13`, Nominal `22`, Opaque
  `23`, NominalRecord `24`;
- constant: Unit `0`, Tuple `10`, Record `12`, Variant `13`, Opaque `18`.

`AwbcRuntimeType::NominalRecord` evolves in place with record shape and
explicit field IDs/names. `AwbcRecordField` gains its explicit field ID.
Nominal `AwbcVariantIdentity` gains layout. All additions are mandatory in
the version-1 row; there is no old reader, optional compatibility branch, or
per-value version.

Ordinary lengths, IDs, field IDs, and ordinals use the current shortest
base-128 `u32` varint helper. Schema and canonical value encoders call that
same core helper. Fixed-width floats, digests, and exact-width scalar payloads
remain fixed-width. `WIRE_AND_RESTORE.md` contains exact golden fragments and
noncanonical negatives.

## 10. Snapshot and restore

`AwbcRuntimeValueSnapshot` remains the sole snapshot enum. The raw public
`into_runtime_value()` path and unchecked nominal reconstruction are deleted.
Fiber/product/task snapshot restoration first decodes DTOs, then invokes the
current `AwbcProgram` with the expected `AwbcTypeId`. The program resolves its
existing type/string/constant rows, constructs a private candidate graph, and
uses the same accepted nominal layouts and case domains as execution.

Nominal record snapshots retain type ID, semantic identity, layout, and
source-order fields. Nominal variant snapshots retain owner layout through
`RuntimeVariantIdentity`. Restore rejects a wrong type, semantic identity,
layout, shape, field ID/name/order, case ordinal/name/payload, opaque owner,
or dangling AWBC reference before a candidate session is swapped into the
executor. The outer session save remains the existing strict typed JSON
envelope at version `1`; it does not acquire a nested version.

`FiberState::validate_for_program` remains the final candidate consistency
gate. The runtime driver continues to restore into a private candidate and
swap only after all fibers, tasks, and product state validate. The later
accepted `RuntimeSnapshotAuthority` composes this exact `AwbcProgram`
validation; it does not add another nominal catalog.

## 11. Deterministic failure precedence

Projection and lowering use this first-error order:

1. configured work/scalar limits;
2. analysis, symbol, world, and environment generation;
3. exact nominal lookup, visibility, owner, origin, and arity;
4. nominal/metadata item and catalog join;
5. generic substitution and unresolved types;
6. field/case count, empty name, duplicate name, and source ordinal;
7. recursively retainable child and exact opaque evidence;
8. graph conflict, dangling reference, and illegal non-nominal cycle;
9. schema canonicalization and layout derivation; and
10. compiler/runtime-plan/AWBC isomorphism.

Restore first performs outer byte/JSON limits and strict decode, then AWBC
ID/range/varint validation, then exact type/nominal/layout/domain joins, then
recursive values, then whole-candidate validation. Failure exposes no digest,
constant, live value, snapshot result, or state mutation.

## 12. Readiness

All eight maintained-request decisions are closed. Implementation follows
the compile-clean cuts in `CUTS_TESTS_AND_DELETION.md`. No production patch is
part of this design. `OPEN_QUESTIONS.md` is exactly `none`.
