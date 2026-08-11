# Proof-concurrency v6.1.1.4.1.1.1.1.2.1
## Call source/resolver authority correction — final contract

Status: `READY_FOR_IMPLEMENTATION`

Repository: `Sanzentyo/arcweft`  
Audited main: `004ff3d69f241954eb808985878c348b165a815c`  
Rejected predecessor return: `BC8DE35E8C4D69008344EC44B9CFF1C5C59EE17ECB2CA54006B0ECF6EE923B50`  
Required archive: `arcweft-proof-concurrency-v6.1.1.4.1.1.1.1.2.1-call-source-resolver-authority-correction-final-contract.zip`

This archive is the complete standalone replacement for the rejected E12/C01-C03
Call package. It is not a delta and does not depend on the rejected archive.

## Closed authority decisions

1. `HirSourceIndex` keyed by `HirSourceQuery`, queried only through
   `HirModule::source_site(expected_source, query)`, remains the sole final-HIR
   component-source authority. Call `Whole` is arena-slot metadata. Optional
   absence, owner poison, and role inapplicability remain lookup results.
2. Current syntax is exactly `Expr`, `Ident '=' Expr`, and postfix `Expr '...'`.
3. Cursor focus retains the production boundary:
   opening token outside; `open.end()` is slot zero; comma start is the following
   slot; trailing-comma start is one-past; without a trailing comma the final
   argument remains active at the close start.
4. Known calls remain `HirExprKind::Call`. Missing callee, unresolved dot,
   associated receiver/member/separator failures, bare-generic arity failure,
   explicit call type applications, and argument recovery remain distinct typed
   structural states.
5. Present-invalid expression/type syntax retains its real qualified poisoned
   `ExprId`/`TypeId`; only genuinely missing syntax lacks the authored ID.
6. `HirCallArgumentOrdinal`, central `HirLimit`, semantic `CallableLimits`,
   the 256-candidate production ceiling, the ordinary 128-argument boundary, and
   the RichText 32-argument boundary are reused.
7. Existing `resolve_call_target`, `CallTargetFacts`, checked slot facts,
   resolver work, candidate probing, selected replay, and signature projection
   are extended in place. No reduced or parallel resolver/fact model is created.
8. The Proof two-witness bound is a verifier projection over complete semantic
   facts, not a replacement candidate ceiling. Semantic facts retain up to 256
   candidates; Proof retains the first two canonical witnesses plus an exact
   omitted count.
9. Associated receiver/type-arity terminal failure invokes the shared resolver
   zero times but logically checks and retains every argument exactly once.
10. The central attached `ExpressionProjection::Call` owns current-grammar shape
    and one typed component manifest until final-HIR/source publication. Detached
    syntax readers are deleted in the same compiling authority switch.
11. All new identities are fully defined. No opaque placeholder identity is
    introduced.
12. Call-produced `RecoveryOperand` ordinals are reachable only at `0..=128`.
    The general `1023/1024` role-admission boundary remains in the predecessor
    generator suite.
13. Consumer migration is deletion-driven and leaves zero aliases, wrappers,
    extension traits, compatibility shims, source-string reparsers, source
    gates, old cursor scanners, or parallel readers.

## Package map

- `RUST_FACING_SCHEMAS.md`: exact final HIR, recovered component, issue,
  source-role, and checker-fact schemas.
- `ATTACHED_CALL_PROJECTION.md`: central attached projection, component owner,
  and parser/attachment/lowering order.
- `SOURCE_AUTHORITY_AND_CURSOR.md`: sole source query and exact cursor rules.
- `RESOLVER_AND_ACCOUNTING.md`: integration into existing resolver/facts and
  physical/logical accounting.
- `LIMIT_AND_TRANSACTION_CONTRACT.md`: exact limits, reachable boundaries,
  rollback, retry, and Proof witness reconciliation.
- TSV matrices: payload, source, synthetic owner, cursor, resolver accounting,
  migration, tests, traceability, and contradiction audit.
- Complete dispatch copies, repository/predecessor audit, validation report, and
  `MANIFEST.json`.

No production code, patch, branch, PR, overlay, adjacent status, adjacent hash,
or adjacent manifest is part of this return.
