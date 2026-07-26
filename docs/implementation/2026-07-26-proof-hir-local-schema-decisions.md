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
