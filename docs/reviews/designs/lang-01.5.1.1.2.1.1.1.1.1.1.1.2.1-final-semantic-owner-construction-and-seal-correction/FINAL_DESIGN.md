# Final design

## 1. One final publication owner

`FinalSemanticAnalysis` remains the only published semantic fact product. It
gains the sealed `CheckedEntryCatalog` and the sealed runtime-nominal
projection catalog. `CompiledProject` does not retain a second Entry catalog;
its existing `checked_entries()` accessor delegates to
`final_analysis().checked_entries()` during the deletion cut.

Construction has two private phases inside `analyze_final_project`:

1. **Prepare.** The existing analyzer produces types, bindings, calls, items,
   statements, patterns, checked callables, exact non-Entry C2 owner rows, and
   one `PreparedExpressionFact` per expression. Ordinary expressions are
   `Complete(CheckedExpression)`; Entry-reference expressions are
   `Entry(PreparedEntryExpression)`. Both variants expose the same checked
   type, type-selection, and effect accessors to recursive checking and
   candidate evaluation. A private query view supplies the Entry checker only
   generation validation, checked callables, types, items, calls, and the one
   nominal projection context.
2. **Seal.** The Entry checker constructs one `CheckedEntryCatalog`. The seal
   joins every prepared Entry reference to exactly one catalog binding,
   verifies the source item, copies the binding digest and semantic value-type
   digest into the final row, consumes the prepared rows, completes callable
   joins/child edges, runs the existing complete-inventory validations, and
   returns one `FinalSemanticAnalysis` containing the same catalog.

Neither the draft nor its query view is public. There is no `Arc`, callback,
tooling lease, compiler field, or query API that can observe it. The draft has
no `Clone`; its seal consumes `self`. Dropping it publishes nothing.

## 2. Prepared Entry facts are staging, not parallel authority

`check_entry_reference` inserts a private
`PreparedExpressionFact::Entry(PreparedEntryExpression)` at the expression's
generation-bound `ExprId`. It does not rescan syntax during seal. The same
prepared expression map is owned first by `SemanticFactState` and then moved
into `FinalSemanticAnalysisDraft`; there is no pending side map beside a
complete expression map.

The prepared Entry expression contains only:

- the already validated `PublicId`;
- the unique HIR Entry `ItemId` selected by the current executable view;
- `TypeKind::entity_ref(EntityKind::Entry)` and its semantic digest; and
- the expression type-selection/effect facts already computed by the analyzer.

`PreparedExpressionFact` supplies `ty`, `type_selection`, and `effects`
accessors. `SemanticFactState` candidate checkpoints journal replacement of
the enum row itself using the existing insert/replace undo record. Commit keeps
the selected prepared row; rollback restores or removes it exactly as it does
for a complete expression. A probe cannot promote an Entry row to a final
checked row.

The seal consumes prepared expressions in key order. For an Entry row it:

1. validates the canonical checked Entry ID/public ID equality;
2. validates `binding.source_item() == reference.lookup_owner`;
3. validates the prepared `TypeKind::entity_ref(EntityKind::Entry)` and its
   `SemanticTypeDigest` against the final checked expression type; and
4. copies `CheckedEntryBindingDigest` from that exact binding.

Only then does it construct `CheckedEntryReference` and the final
`CheckedExpression`. Missing, duplicate, wrong-ID, wrong-owner, or wrong-type
joins fail before any final expression or catalog is published. Complete rows
move directly. The seal creates the final `BTreeMap<ExprId,
CheckedExpression>` once; the prepared map is consumed and does not survive.

There is no public pending resolution variant, `Structural` placeholder,
optional digest, or second analysis pass.

## 3. Entry checker dependency contraction

The current Entry checker uses only generation validation, checked callables,
`ty`, `item`, `calls`, and project nominal schema projection. Its internal
context is changed from `&FinalSemanticAnalysis` to the concrete private
`PreparedEntrySemanticAuthority<'_>` exposing exactly those operations. No
public trait or extension trait is added. The published convenience function
`check_project_entries` is deleted; tests obtain entries from a successfully
sealed final analysis.

`analyze_final_project` returns
`Result<FinalSemanticAnalysis, FinalSemanticProjectError>`. The error keeps
ordinary semantic failure and ordered Entry diagnostics distinct so the
compiler retains their existing diagnostic codes/stages.

## 4. Deterministic error precedence

The new top-level first-error order is fixed:

1. cancellation observed at the current phase boundary;
2. HIR/symbol/registered-world generation mismatch;
3. existing analyzer recovery, lookup, type, callable, and work errors in its
   current deterministic traversal order;
4. duplicate fact collection and prepared-inventory validation in the current
   `FinalSemanticAnalysis::try_new` order;
5. runtime nominal projection generation, owner, arity, cycle, work, schema,
   and layout errors, in accepted semantic-type digest order;
6. Entry diagnostics in HIR Entry item order and each checker's current field
   order, followed by duplicate checked Entry ID;
7. prepared Entry-reference joins in declaration-rooted C1 semantic-path order;
8. final expression/pattern/item/call/edge completeness validation in the
   current validation order; and
9. final cancellation check immediately before returning the sealed value.

Checked arithmetic occurs before allocation/descent. Each projection root gets
a fresh `ProjectionBudget` charged against
`NominalResolutionLimits::PRODUCTION` for nodes, recursive depth, generic
arguments, and per-root work. The context separately charges every root/cache
request to `NominalAggregationLimits::PRODUCTION.work_per_project`; a cache hit
still charges one aggregate request but does not charge recursive root work.
Both counters use checked `u64`. Tests inject smaller valid per-root and
aggregate limits through crate-private constructors. Limit failure precedes
the allocation or recursive step it would authorize.

The compiler then runs proof verification. Consequently Entry binding failure
now precedes proof-verification diagnostics when both exist. This is the one
intentional phase-order correction required to avoid exposing a partial public
analysis across the `arcweft-verify` crate boundary. Entry selection validation
remains after verification, as today.

The existing `#[cfg(test)]` manual final-analysis constructor supplies an empty
Entry catalog only after proving the fixture contains zero Entry declarations
and zero Entry-reference expressions. Tests containing either must use the
full sealing path; a manual fixture cannot bypass Entry completeness.

## 5. One nominal projection

`RuntimeNominalProjectionContext` moves to
`final_analysis/nominal_schema.rs` and takes only:

- the exact `ProjectSymbolTable` generation;
- the accepted `BTreeMap<TypeId, TypeKind>`;
- `NominalResolutionLimits` and `NominalAggregationLimits`; and
- checked cancellation.

`NominalSchemaExpander` consumes this context instead of a published analysis.
A single exhaustive typed visitor walks every prepared type, local, capture,
expression (both prepared variants), pattern, statement, item, call, callable,
Entry role, and C2 owner row. It recursively visits every `TypeKind` child and
emits a typed `RuntimeNominalProjectionRequest` for every
`CheckedProjectNominal`; raw names and source scans are forbidden. Requests are
deduplicated and processed in `SemanticTypeDigest` order.

Each root resets its per-root `ProjectionBudget`; project aggregate work is
never reset. The context memoizes by `SemanticTypeDigest`, detects a visiting
key cycle before descent, and produces one `RuntimeNominalProjectionCatalog`.
Every `RuntimeProjectNominalProjection` retains the canonical `TypeShape` as
well as runtime identity/schema/layout/kind. Entry checking and C2 field/case
construction borrow the same context/cache. The final seal reruns the typed
request inventory over the final facts and rejects the first absent cache row
with `MissingCachedProjection`; post-seal methods are borrowed catalog lookups
and never expand a schema.

Recursive project declarations continue to use the existing named-schema
projection where legal. A cyclic generic substitution remains an error. Cache
entries are inserted only after a complete projection succeeds; failures do
not leave negative or partial cache rows.

One projection request uses this exact order: symbol/type identity validation;
cancellation; checked aggregate-work increment; aggregate limit; cache lookup;
fresh root-budget creation on a miss; cancellation; checked root charge; root
limit; visiting-cycle check; expansion; canonical shape/schema/layout identity
cross-check; cache insertion. Final completeness uses: cancellation; checked
request-inventory accounting; request/catalog identity comparison; first
digest-ordered missing projection. A borrowed post-seal lookup checks requested
versus row identity before reporting `MissingCachedProjection`.

## 6. Ordered environment record authority

The environment-record option is selected without a second record catalog.
`TypeCheckEnv::nominal_records` and its nested maps are deleted.
`AcceptedNominalSemantics` in `env/nominal.rs` gains a declaration-ordered
`Record { ty, semantic_type, fields }` variant. The existing
`AcceptedNominalRecord`, `AcceptedNominalCatalog`, catalog digest, and
registered-world stamp remain the one accepted authority.

The standard-record constructor creates the accepted ID/path, exact `TypeKind`,
semantic type digest, and complete field vector atomically. Each field gets a
checked `u32` ordinal and private semantic ID from owner semantic type,
ordinal, and field type digest. Duplicate names and overflow reject the whole
record before catalog insertion. `AcceptedNominalCatalog::exact(path)` selects
the row; a borrowed linear `field(name)` lookup over its bounded ordered vector
is the sole name-to-row boundary. There is no stored name index, raw public
record constructor, or public field-ID mint.

`AcceptedNominalRecord::try_instantiate` returns the Record row's `ty` for an
empty argument list. The catalog digest hashes the ordered typed fields through
the existing `AcceptedNominalSemantics` hash, so the registered semantic-world
stamp changes with owner, order, or field type. Field selection and
record-pattern checking borrow that same exact catalog row.

Environment record patterns are admitted for `TypeKind::Named` only when the
resolved/omitted pattern head selects that exact accepted record. Project and
environment records share `CheckedFieldSemanticId`, authored-source row order,
duplicate/unknown/rest validation, and stable field coordinates.

This C2 correction does not add an environment-record shape to the runtime-plan
type algebra. A compiler path that requires executable lowering of an
environment Field or record pattern fails closed with a typed
`UnrepresentableEnvironmentRecordField` error carrying owner identity and
ordinal, never the diagnostic name. Project nominal fields continue to lower
through their existing runtime nominal/layout authority. Environment rows are
complete semantic transcript/coverage facts; they do not fabricate runtime
representation.

## 7. View modifier fails closed

There is no accepted View modifier catalog at the inspected revision. The
`CheckedViewCall::Modifier` variant and `AcceptedViewModifierSemanticId` sketch
are deleted. A View-context unresolved-dot call whose receiver checks as
`ViewValue` returns `FinalSemanticAnalysisError::CallResolutionFailed` at the
call owner. Compiler View lowering deletes its unreachable Modifier arm.

This does not reserve a modifier digest domain and does not create a successor
request: there is no accepted executable modifier behavior to implement in
this Match cut.

## 8. C2/C3 boundary

C2 constructs exact typed owner rows and owner-defined leaf digests. It does
not construct recursive expression, statement, body, RichText, Postfix child,
coverage, or Match digests.

`PostfixBracketResolution` keeps the selected `ExprId` privately because
compiler lowering and reachability use it. C3 hashes only the closed
Index/Dialogue tag and recursively obtained selected-child digest.

The existing `CheckedRichTextReport` remains the C2 typed fact owner. Raw tag
IDs and source sites stay lookup/diagnostic-only. C3 enumerates accepted tokens
in order, maps accepted opens to checked `u32` token ordinals, maps closes to
those ordinals, rejects missing/duplicate/foreign pairing, and hashes the
stable ordinal—not `HirRichTextTagId`.

## 9. Removed Select variants

Neither `TupleElement` nor `RecordElement` has a producer. Both variants and
all validation/transcript/compiler readers are deleted in C2. Their existing
tags `0x0405` and `0x0406` remain named reserved constants in the structured
C2 tag registry and can never be reassigned.
