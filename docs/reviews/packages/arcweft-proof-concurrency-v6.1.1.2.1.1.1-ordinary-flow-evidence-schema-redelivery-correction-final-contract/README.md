# Arcweft Proof-concurrency v6.1.1.2.1.1.1

## Ordinary Flow evidence and schema corrected redelivery

Status: `READY_FOR_IMPLEMENTATION`

This archive is the complete standalone replacement requested by
`2026-08-02-seq-proof-01.1.1.2.1.1.1-ordinary-flow-evidence-schema-redelivery-correction.md`.
It does not depend on the rejected v6.1.1.2.1.1 archive.

The contract closes the ordinary source-level `Flow` item from attached syntax
through final HIR, the revision-bound source index, project publication, and the
statement-only `HirThreadBody` shared with `ThreadExpression`.

The inspected Git commit is:

```text
aa983fda6b0de36d2f6867085ecdc95e630c5d99
```

The GitHub repository was inspected as a clean immutable commit tree. No
local mutable worktree was created. `EVIDENCE_SCOPE.md` records the exact
Git-only state and the one-commit head reconciliation.

## Closed result-changing decisions

1. `FlowItemNode` is the sole attached declaration owner. It retains the four
   identity states, typed generics, zero or one parameter group, optional
   return annotation, typed `where`, one heterogeneous contract sequence,
   statement-only body state, close recovery, and trailing recovery.
2. The internal semantic identity of every recognized Flow is its qualified
   `ItemId`. An authored public ID is optional. An ordinary name is an optional
   presentation and local-lookup spelling. Name-only public publication is
   derived exactly once by the accepted project transaction; no attached or HIR
   ID is fabricated.
3. Omitted return is `HirFlowReturn::OmittedUnit`. It creates no `TypeId`, no
   synthetic type, and no source node.
4. A Flow always owns one callable scope, one requires-phase scope, one
   ensures-phase scope, and one body scope. Parameter locals live in the
   callable scope. The result local exists only when at least one condition-form
   `ensures` clause exists.
5. All nine maintained Flow contract families are typed:
   `requires`, `ensures`, `invariant`, `assume`, `reads`, `effects`,
   `no_effect`, `modifies`, and `decreases`.
6. `Flow` and `ThreadExpression` use context-specific attached body owners but
   one final `HirThreadBody`. Neither body has an ordinary block tail.
7. `HirThreadFlowItem` has exactly the sixteen variants fixed in
   `FINAL_HIR_RUST_SCHEMA.md`. Dialogue application is the only `ExprId` item;
   every other row owns a `StmtId`.
8. Source order is represented by heterogeneous contract and body-item
   ordinals. Arena allocation order is never used as semantic order.
9. `HirModule::source_site(expected_source, HirSourceQuery)` remains the sole
   public source query. This package adds roles to the original role enums and
   query enum; it creates no Flow-specific reader.
10. Syntax and HIR construction are transactional. Limit failure, cancellation,
    panic, stale/foreign input, or invariant failure publishes no partial
    identity, arena slot, scope, local, source row, diagnostic, project
    candidate, checked result, cache fact, or invalidation fact.
11. The public switch is deletion-driven. The current detached Flow AST,
    clone-HIR, value-tail assumptions, legacy dialogue carriers, raw contract
    expressions, and source-string reconstruction are deleted rather than
    adapted.

## Archive map

- `ATTACHED_SYNTAX_RUST_SCHEMA.md` — exact syntax-owned records and enum
  extensions.
- `FINAL_HIR_RUST_SCHEMA.md` — exact final HIR records and source-query
  extensions.
- `IDENTITY_SIGNATURE_MATRIX.tsv` — identity, generic, parameter, return,
  `where`, and body-header decisions.
- `CONTRACT_CLAUSE_MATRIX.tsv` — all nine clauses with scope, payload,
  recovery, diagnostic, and accounting behavior.
- `FLOW_THREAD_ITEM_MATRIX.tsv` — exhaustive Flow/Thread item projection.
- `SCOPE_LOCAL_MATRIX.tsv` — parents, local visibility, source origins, and
  deterministic allocation.
- `SOURCE_RECOVERY_DIAGNOSTIC_MATRIX.tsv` — source roles, query behavior,
  recovery, and diagnostic ownership.
- `ACCOUNTING_LIMIT_TRANSACTION_MATRIX.tsv` — exact inclusive limits,
  preflight charges, and atomic outcomes.
- `POISON_PRECEDENCE.md` — one deterministic primary-issue rule.
- `CONSUMER_DELETION_INVENTORY.tsv` — current producers/readers and direct
  final owners.
- `IMPLEMENTATION_ORDER.md` — one compile-clean deletion-driven series.
- `TEST_MATRIX.tsv` — positive, malformed, negative, recovery, stale/foreign,
  rollback, limits, source-query, project, and consumer migration evidence.
- `REQUIREMENTS_TRACEABILITY.tsv` — every requested area closed.
- `REPOSITORY_EVIDENCE_LEDGER.tsv` and `PREDECESSOR_LEDGER.tsv` — inspected
  revision, repository blobs, and selected predecessor authority.
- `VALIDATION_REPORT.md` — package-level mechanical and semantic checks.
- `MANIFEST.sha256` — filename-sorted SHA-256 and exact byte length for every
  non-manifest member.

## Implementation status boundary

This is a design-only contract. No production Rust, test, fixture, manifest,
stable design chapter, branch, patch, PR, implementation overlay,
compatibility layer, source gate, or dual reader is included. The Rust
snippets are normative schemas, not a source patch.

`OPEN_QUESTIONS.md` is exactly the four bytes `none`.
