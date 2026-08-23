# Lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1 — accepted structural nominal runtime carrier correction

Status: `RESOLVED_BY_ACCEPTED_DESIGN`

Accepted resolution:
[repository-local final design](../designs/lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1-accepted-structural-nominal-runtime-carrier/README.md),
validated against production commit
`9a5d30d25620541c3f2975d31e04e04e3bc9514c`.

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

## Mandatory redispatch inputs and repository preflight

This maintained request was hardened in place after the first returned archive
used the wrong wrapper, had no repository access, and proposed owners that do
not exist in Arcweft. The sequence, objective, and required output name are
unchanged; the dispatch bytes are intentionally not the rejected request
bytes.

The responder must receive and use all of these inputs:

- a checkout of current `main`, with its full Git commit SHA and clean/dirty
  state recorded before design work;
- the applicable repository and scoped `AGENTS.md` files;
- this current maintained request, not the request copy frozen in the rejected
  package;
- the accepted parent and its ownership matrix linked above; and
- the
  [rejected-return intake](../../implementation/2026-08-23-lang-01-5-1-1-2-1-1-1-1-1-1-1-1-accepted-structural-nominal-carrier-return-intake.md)
  as failure evidence, not as design authority.

Before selecting any schema, the responder must record a repository-derived
preflight containing:

1. `git rev-parse HEAD`, `git rev-parse origin/main`,
   `git status --short --branch`, and Cargo workspace metadata; the frozen
   production baseline must be clean `main == origin/main` before output files
   are created, while later design-output dirt is reported separately;
2. the actual crate, path, symbol, visibility, dependency direction, and Git
   blob identity for every current sema, core, compiler, runtime-plan, AWBC,
   value-codec, snapshot, restore, and adapter owner used by the design;
3. a complete producer/consumer inventory for decisions 1–8; and
4. an explicit proof that each cross-crate constructor and catalog join follows
   the current layer direction.

If the checkout, required source, or predecessor evidence is unavailable, stop
and report that exact blocker. Do not infer owners from this request, invent a
crate, repeat a template, or emit the required final-contract ZIP. Every cited
type or function must either exist at the inspected SHA or be explicitly
introduced in a compile-clean cut with its one owner, dependency direction,
constructor, consumers, tests, and superseded deletion target.

For this request specifically, the preflight must reconcile the existing
sema-owned accepted catalogs with core-owned runtime value/type/layout
authorities without making core depend upward on sema. It must inventory the
actual `RuntimeValue`, `RuntimeNominalTypeId`, `TypeLayoutHash`,
`RuntimeCheckedType`, runtime-plan type table, nominal record/variant carriers,
AWBC value tags/codecs, snapshot, and restore owners before proposing a new
field, case, identity, or wire tag. A parallel accepted-nominal algebra,
metadata-only success, copied catalog, per-value version field, future-tag
placeholder, or nonexistent umbrella runtime crate is forbidden.

Before `READY_FOR_IMPLEMENTATION`, the design must name the current wire/tag
allocation owner, select the exact noncolliding tag through that owner, and
provide canonical version-1 varint golden bytes for every admitted record and
variant payload family. A per-value fixed-width `u16` version, “future max+1”
tag, unchecked constructor, legacy/migration reader, or opaque forwarding rule
is a hard failure. Repository-aware negative tests must prove one schema and
layout digest across sema, compiler, runtime-plan, AWBC, live `RuntimeValue`,
snapshot, and restore for all eight decisions.

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
compile-clean/deletion order, test matrix, machine-readable final contract,
repository-aware validator and negative self-tests, manifest, source inventory,
`FINAL_STATUS`, and
`OPEN_QUESTIONS`. It may claim `READY_FOR_IMPLEMENTATION` only when all eight
decisions above are closed and `OPEN_QUESTIONS` is exactly `none`.

The archive has one top-level wrapper whose name exactly equals the ZIP
basename without `.zip`. It contains a byte-identical copy of this hardened
request and records that copy's SHA-256 in its manifest. The manifest covers
every returned file. Decision traceability maps decisions 1–8 to concrete
source evidence, schemas, canonical fixtures, tests, and deletion cuts; a
generated excerpt or section-name audit is not traceability. Every numbered
decision has one unique traceability row naming its owner, Rust-shaped schema,
consumers, positive test, negative mutation, and deletion cut.

Before return, the responder must run the repository-aware validator against
the cited Git commit, run negative self-tests that mutate every mandatory gate,
verify the archive wrapper/member hashes/request mirror, and include the exact
commands and results. The produced ZIP must then be reopened and the same
wrapper, member, manifest, request, status, and validator checks rerun against
its actual bytes. A missing required member, unknown source SHA, unresolved
owner, illegal dependency, open result-changing question, validator failure,
negative-test failure, manifest mismatch, or wrapper mismatch forbids emission
of the named final-contract ZIP. Report the blocker instead; do not package a
self-declared failed delivery as a final contract.
