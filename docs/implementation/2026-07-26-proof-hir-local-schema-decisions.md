# Proof final-HIR locally closed schema decisions

Date: 2026-07-26

Status: `IMPLEMENTATION_AUTHORIZED_LOCALLY`

This note closes proof-concurrency follow-ups `01.1.1.2.1` and `01.1.1.3`
without an external design return. Current attached syntax, the accepted
qualified arena/source-map architecture, and existing language semantics leave
one natural owner for each row. The two request files remain only as gap-audit
history and must not be dispatched.

Proof `01.1.1.4`, which defines final semantic leaves and expression payloads,
remains the external HIR design blocker.

## 01.1.1.2.1 item and declaration-member ownership

The final source-item inventory is the attached `TypedItemNode` inventory:

```text
Module, Use, Flow, Function, Predicate, Proof, Trait, Impl, Enum, Struct,
TypeAlias, Resource, Character, View, Action, Activity, Signal, Metric, Layer,
Entry, ExternCapability, Test, Bench, Source, Style, Error
```

`EntityDeclaration` is replaced by its seven concrete declaration kinds.
Source-less base variants such as generic `Callable`, `State`, `ExternModule`,
`DialogueDefaults`, `MemoFunction`, `Parser`, and `TopLevelFlow` are deleted;
any surviving runtime concept uses its non-source owner. `Source` remains only
until the Lang-01.3 atomic Source-to-Stream switch. `Error` owns recovery and
never fabricates a valid declaration.

Declaration members live in a dedicated member arena in the same qualified HIR
database as items, expressions, statements, patterns, types, scopes, and
locals. An item stores ordered `DeclarationMemberId` values. The closed member
variants are View export, Activity input/output port, Metric
unit/label/bucket, optional Character display name, and typed Layer
reference/policy/expression. Semantic payloads contain typed IDs and owned
enums; revision-bound ranges remain in the source-component table. There is no
generic member map, dotted-string split, syntax clone, or inline expression
payload.

Member lowering is transactional. The parent item publishes only after all
required member and source-component slots validate. Exact limits commit;
one-over, stale/foreign IDs, and required-child failure roll back all related
slots.

## 01.1.1.3 statement if-let and unsafe-audit insertion

Statement-form `if let` is
`HirStmtKind::IfLet(HirIfLetStmt)`. `HirIfLetStmt` owns the typed pattern,
scrutinee expression, optional guard, then scope/body, and optional typed else
branch. Pattern bindings are visible to the guard and then body, never to the
scrutinee or sibling/outer scopes. Nested `else if` and `else if let` retain
their typed statement identities; no fabricated `ExprId` or
statement-to-expression shim is allowed. All child allocations and source
components share one rollback unit.

The unsafe-audit edit location is a typed source component named
`UnsafeAuditInsertion`, keyed by the qualified audit `StmtId` and ordinal zero
in the revision-bound HIR source table. It is derived directly from the
attached opening-brace component. A public query requires the accepted HIR
project generation and source snapshot and projects a `SourceEdit`; stale,
foreign, rolled-back, missing, or unclosed-brace components produce no edit.
No raw range is copied into semantic HIR, runtime, AWBC, bundle, cache,
save/checkpoint, or replay data.

## Implementation boundary

Implement both decisions deletion-first: remove obsolete item variants,
generic member carriers, statement compression, and detached audit-range
owners, then use compile fallout as the consumer inventory. Do not add aliases,
wrappers, dual readers, source-string fallbacks, source gates, or
removed-syntax diagnostics.

The Layer reference member may depend on the final `HirIdRef` selected by
Proof `01.1.1.4`; that dependency does not reopen the member inventory or arena
ownership fixed here. Public migration still requires the normal focused,
workspace, Tier 2 where applicable, rollback, compile-fail, and structural
validation gates.

## Recovered acceptance traceability

- Recovered into the Proof public-switch worktree: 2026-08-07
- Inspected Git revision:
  `f587e75750d9c5d9b6d8c84e0f098a4cfa80f68b`
- Working tree: dirty Proof public-switch integration
- Current validation authority:
  [`2026-08-06-proof-concurrency-v6-1-1-full-matrix-closure.md`](2026-08-06-proof-concurrency-v6-1-1-full-matrix-closure.md)

This section restores locally closed schema decisions that were present only
in the protected integration checkout. It does not inherit that checkout's
progress labels, test results, structural measurements, or revision claims.
The earlier statement that Proof `01.1.1.4` was still an external blocker is
historical; the accepted correction chain and the current public-switch matrix
now govern completion.

### Final item and member payload closure

Common documentation, structured outer attributes, and visibility live once
in `HirItemPrefix`. Attributes retain a root-preserving `HirPath` and ordinary
typed call arguments. They never retain an argument string.

The source-item families map to closed typed payloads:

- Module and Use own root-preserving module/import paths and source-ordered
  typed bindings;
- Flow owns its retained identity, signature/contracts, callable scope, and
  ordered `HirThreadBody`;
- Function is block/error-bodied only, while Predicate and Proof retain their
  accepted expression/block/error alternatives;
- Trait, Impl, Enum, Struct, TypeAlias, ExternCapability, Entry, Test, and
  Bench own their family-specific typed members instead of a generic member
  map;
- Resource and retained declaration families own typed IDs, `TypeId`s,
  `ExprId`s, closed enums, and declaration-member IDs;
- Source remains only until the Lang-01.3 atomic Source-to-Stream switch;
- Style owns native typed selector/environment/token payloads, never CSS or
  Takumi data; and
- Error is limited to unclassified syntax or transactional child failure.
  A recognized family remains that family with typed poison.

Supporting records remain closed: `HirVisibility` is
`Public | Crate | Super`; generic parameters own lifetime/type names and typed
bounds; parameters own one pattern, required type, optional default, and
source-preorder locals; where predicates own one subject type and ordered
bounds. Declaration-member identity is the parent `ItemId` plus a zero-based
source ordinal, and publication verifies owner, contiguity, family admission,
and same-module children.

Test and Bench keep their authored plan ID as `HirIdRefValue`; header ID syntax
is not an expression. Test additionally owns the closed built-in/custom/recovered
test-kind value. Their plan body is a source-backed statement-only block scope
with ordered `StmtId`s and no value tail. A missing body keeps an empty poisoned
block scope, never the parent item scope. `goto` remains an ordinary typed
statement with a normally lowered target expression.

### Unsafe-audit source and semantic closure

The sole source query gains the statement key
`Stmt { owner: StmtId, role: HirStmtSourceRole }`; its closed statement roles
are `Whole` and `UnsafeAuditInsertion`. This is the same immutable source index,
not a second statement map or edit reader.

`HirUnsafeAudit.id` uses `HirIdRefValue`. Its body is exactly a retained block
scope plus ordered statements, or Missing. A missing body receives no synthetic
scope. The `SAFETY` flag comes only from the parser-owned attached `DocBlock`
classification; HIR does not scan comments. The primary recovery order is
invalid ID, recovered reason, missing body, first poisoned body statement, then
unclosed delimiter.

A clean, fully delimited unsafe statement publishes one required present
insertion component at the checked opening-brace boundary. A poisoned statement
keeps its typed family, publishes an optional absent requirement, and cannot
fabricate a source edit. The former transaction-fatal incomplete-body path is
not an alternative authority.

### Module publication and invalidation

One `HirModule` owns the exact syntax snapshot, source snapshot, immutable
source identity/document lease, eight typed arena snapshots, declaration-member
index, source index, diagnostics, status, source-ordered top-level items, and
invalidation epoch. The slot snapshot is the sole source/synthetic allocation
ledger. Declaration members remain a secondary item/ordinal arena rather than
a ninth raw HIR ID family. Consumers share one accepted `Arc<HirModule>`; the
module value itself contains no ownership alias for another HIR.

The source-ordered item row is derived from the current `ParsedSource` item and
entry projections, excluding source-file attributes, synthetic items, and
declaration members. Publication proves exact coverage, uniqueness, module
ownership, and strictly increasing attached-source order without treating arena
slot order as source order.

Publication stages all slots, arenas, members, sources, diagnostics, and status;
freezes complete coverage and graphs; derives invalidation from the exact
previous/current module pair; publishes the shared slot ledger; and only then
performs the infallible database insertion. No fallible check may remain after
the ledger becomes visible. A failed stage publishes no IDs, diagnostics,
source rows, invalidations, or partial module.

`changed_items` compares the final item plus its optional member arena;
`retired_items` contains prior live items absent from the candidate;
`executable_status_changed` is exact status inequality; and
`symbol_revision_changed` follows any changed/retired item or status change.
The database derives these facts; callers cannot submit them. Recoverable
diagnostics remain typed, while identity, limit, transaction, and publication
failures never enter the recoverable list.

### Attached type, Activity, and Layer closure

An attached type lowers directly into the final type arena in one transaction.
`HirTypeKind::Recovery` pairs with the same typed invalid-type issue in the
poison state. Source-backed parent IDs precede child IDs; type paths preserve
their authored root; an elided region is one synthetic key owned by the type,
not a dummy child or side ledger. Invalid named-region syntax remains the known
Reference family with no fabricated valid region. Source bytes are charged
once per lowering request, while effect names and numeric/count fields use
their exact typed limits.

Activity owns one callable scope with synthetic requires/ensures child scopes.
Port locals live in the callable scope, use the existing Parameter local kind,
and are absent for missing/invalid names. Declaration-member ordinals use the
global interleaved port order; direction-filtered input/output rows reference
those same IDs. Accepted Activity limits remain 256 combined ports, 64 combined
contract clauses, and 1,024 declaration entries. Recovery selects policies and
the primary issue deterministically without placeholder names or locals.

Layer keeps one closed kind and source-ordered declaration-member row.
Reference, policy, and expression members retain typed
`Present | Recovered | Missing` state, assignment state, and duplicate state.
Recovered kinds receive no valid default; missing values receive no fabricated
ID or expression; unknown syntax remains attached recovery evidence rather
than a generic HIR member. Publication re-derives kind, members, value state,
scope, poison, and source ownership from the same attached snapshot.

All of these boundaries remain deletion-driven. Generic entity/member readers,
string policy or family helpers, copied ranges, detached trees, and source-text
fallbacks are removed when the final owner is connected; none is repaired or
retained as a compatibility path.
