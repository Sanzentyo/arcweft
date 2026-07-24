# AW-AH-009.3 static capacity associated-callee blocker

Date: 2026-07-24

## Status

`DESIGN_BLOCKED` for the static associated capacity-call migration only. Other
implementable AW-AH-009.3 acceptance rows continue independently.

This note records a contract gap found while removing the old checker success
dispatch from the AW-AH-009.3.3 shared-resolver path. It does not approve the
current fallback as a final boundary and does not add production behavior.

## Accepted inputs inspected

- AW-AH-009.3.1 call-surface contract:
  `arcweft-aw-ah-009.3.1-call-surface-syntax-production-reconciliation-final-contract.zip`,
  SHA-256
  `6EDE771A895AF981A583FDFD50A080F2ECA57BF7A2925216CF725F7DBB418588`;
- AW-AH-009.3.3 shared-resolver contract:
  `arcweft-aw-ah-009.3.3-callable-catalog-shared-resolver-production-reconciliation-final-contract.zip`,
  SHA-256
  `9D1F989F5E0E698AEFF1098DD7ECEE7E01A66616A00A0571EE333A3B1B7DDC78`;
- stable language examples in
  `docs/01-language/standard-types-and-prelude.md` and
  `docs/01-language/traits-seq-ranges.md`;
- current `main` parent `f6e2a3a3` and the active AW-AH-009.3 working change.

## Current production evidence

The stable language surface requires `String.with_capacity`,
`Bytes.with_capacity`, and `Vec<T>.with_capacity`. The callable model already
owns `CapacityMethodId` and `CallableFamily::CapacityMethod`, and the accepted
shared-resolver contract fixes the capacity precedence and intentionally
unchecked argument schema.

The remaining checker path nevertheless calls
`well_known_static_capacity_method_type(&str)` before the shared resolver. That
helper recognizes string spellings, maps bare `Vec` to a named `_` placeholder,
and reconstructs `Vec<T>` from text. Consequently a registered static capacity
call can succeed with `old_dispatch_calls == 1` and
`shared_resolver_invocations == 0`, without checker-owned target facts for
native signature help.

The accepted `CallCallee::Selected` request owns a value
`receiver_expression: TypeExpressionId`. Neither accepted package defines a
typed type-receiver/associated-callee carrier, generic identity resolution, or
the collision rules needed to feed `String` or `Vec<T>` to the resolver. The
AW-AH-009.3.1 rule that static generic callees use the Pratt/path grammar removes
source scanning but does not select that semantic owner.

Deleting the fallback now would remove a documented language path. Retaining
it and marking the migration complete would violate the shared-resolver and
work-accounting contract. Inventing a type-receiver enum locally would make
alias, generic, qualification, and value/type collision behavior guesswork.

## Required correction and implementation boundary

The independently throwable
[`AW-AH-009.3.3.4 correction request`](../reviews/requests/2026-07-24-aw-ah-009.3.3.4-typed-associated-capacity-callee-authority-reconciliation.md)
asks for the exact typed syntax/HIR/sema authority, resolution precedence,
recovery, registered/non-registered convergence, tests, and deletion order.

Until a returned package is inspected and accepted:

- do not add a `BuiltinCallableId`, new callable family, string parser, sentinel
  receiver ID, compatibility wrapper, or second resolver;
- do not remove the currently documented static capacity behavior;
- do not report the static capacity old-dispatch row as migrated;
- continue bounded-cache, accepted-HIR lease, request lifecycle, ordinary
  traversal, other callable-family evidence, and limit work;
- classify this exact acceptance row as `DESIGN_BLOCKED`, not implemented or
  non-applicable.

After a ready correction returns, the implementation must switch authority in
one compiling production cut and delete the old import, early checker branch,
and stringly helper rather than retaining dual dispatch.

## Validation

Documentation-only boundary record. No Rust behavior or test expectation is
changed in this cut.
