# Lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1 — accepted structural nominal runtime carrier correction

Status: `OPEN_DESIGN_REQUEST`

## Parent, split reason, and precedence

This request is a design-gated child of the accepted
[`runtime launch receipt, keyed ordinal, and current-owner correction`](2026-08-22-lang-01.5.1.1.2.1.1.1.1.1.1.1.1-runtime-launch-receipt-keyed-ordinal-and-current-owner-correction.md)
and its
[`OWNERSHIP_MATRIX.md`](../designs/lang-01.5.1.1.2.1.1.1.1.1.1.1.1-runtime-launch-receipt-keyed-ordinal-and-current-owner/OWNERSHIP_MATRIX.md).

The parent correctly requires exact catalog evidence for structural
`AcceptedNominal` admission, but its Rust-shaped sketch does not close the
current repository's live carrier, layout, codec, and restore decisions.
Production currently has exact accepted opaque producer/value-class/
persistence evidence and source-backed Rust ADT metadata, but no single owner
that proves a Rust accepted record or variant has an executable runtime
layout. Cut 2 therefore deletes speculative structural carrier markers and
fails closed instead of inventing a parallel schema.

Current maintained source and accepted validated substrate take precedence.
Do not redesign `ProjectNominal` runtime schema/layout projection, the exact
opaque accepted carrier, current `RuntimeValue`, `RuntimeCheckedType`, DenseSeq,
or AWBC snapshot owners without a concrete repository-evidenced flaw.

## Decisions required

Return one coherent design that closes all of these result-changing choices:

1. the source-order field carrier for accepted Rust records and enum record
   payloads, including stable field identities and duplicate handling;
2. the live runtime representation of unit, tuple, and record enum payloads,
   including the exact one-field tuple normalization rule;
3. recursive and mutually recursive accepted ADTs, nested opaque values, and
   generic instantiation without a copied side table or source reconstruction;
4. one canonical runtime schema and layout-digest algorithm shared by sema,
   compiler lowering, runtime-plan/AWBC, canonical value identity, snapshot,
   and restore;
5. exact `RuntimeCheckedType` predicates for structural accepted record and
   variant payloads, including Result/Option-shaped Rust payloads;
6. the catalog join that distinguishes semantic Rust metadata from admitted
   executable runtime layout evidence;
7. constructor/getter visibility and stale-world/generation rejection at every
   cross-crate consumer; and
8. the deletion and compile-clean order that replaces the current fail-closed
   path without adding a compatibility reader or a second type algebra.

Every Arcweft-owned version marker remains `1`.

## Consumers to inventory

- `arcweft-lang-sema` accepted nominal and Rust metadata catalogs, ownership
  classifier, semantic identity, substitution, and final analysis;
- adapter/Rust registration inputs and their source-order metadata producers;
- compiler runtime semantic projection and nominal lowering;
- `arcweft-runtime-plan`, core runtime type schemas/layouts, checked types,
  canonical runtime value identity, and nominal/variant value carriers;
- AWBC lowering, value snapshots/codecs, save/restore validation, and runtime
  task/Need producer argument validation; and
- maintained runtime documentation, schema fixtures, structural gates, and
  external adapter compile fixtures.

## Non-goals

- no production patch or implementation overlay in the returned package;
- no string/display-name lookup, source-spelling reconstruction, metadata-only
  admission, wildcard structural fallback, or producer-wide weakening;
- no `V2` type, version bump, legacy reader, migration map, or optional old
  field; and
- no reopening of exact opaque accepted admission or ProjectNominal layout
  authority absent a demonstrated flaw.

## Required implementation order

The design must provide a compile-clean dependency order beginning with the
lowest live/schema owners, then accepted catalog construction, sema ownership,
compiler/runtime-plan lowering, AWBC/value codecs, restore, and deletion of the
temporary fail-closed branch. No downstream type may be referenced before its
owner is published in the same or an earlier cut.

## Required tests

- source-order record fields and enum cases, including reorder-sensitive
  digests and duplicate rejection;
- unit/tuple/record enum payloads, one-field tuple behavior, Result/Option
  payloads, generics, recursion, mutually recursive types, and nested opaque
  values;
- exact live carrier acceptance plus wrong owner, field, ordinal, name,
  payload, layout, producer, semantic identity, class, and persistence
  negatives;
- canonical digest and AWBC snapshot round trips for every admitted shape;
- stale catalog/world/generation, missing layout/case/field, and metadata-only
  admission rejection before bytes or mutation are exposed;
- compiler/runtime-plan/AWBC/snapshot differential tests proving one schema
  and layout digest; and
- repository-aware negative fixtures that fail if a parallel model, fallback,
  version bump, copied side table, or source reconstruction is introduced.

## Required returned archive

Return exactly:

`arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1-accepted-structural-nominal-runtime-carrier-correction-final-contract.zip`

The archive must contain the complete final contract, Rust-shaped schemas,
owner/consumer and dependency matrices, canonical transcript fixtures,
compile-clean/deletion order, test matrix, repository-aware validator and
negative self-tests, manifest, source inventory, `FINAL_STATUS`, and
`OPEN_QUESTIONS`. It may claim `READY_FOR_IMPLEMENTATION` only when all eight
decisions above are closed and `OPEN_QUESTIONS` is exactly `none`.
