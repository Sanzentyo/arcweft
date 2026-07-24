# AW-AH-009.3 static capacity associated-callee blocker

Date: 2026-07-24

## Status

`ACCEPTED_IMPLEMENTATION_READY` for the static associated capacity-call
migration. The former design blocker was closed by the verified
AW-AH-009.3.3.4 return.

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
- AW-AH-009.3.3.4 typed associated-capacity correction:
  `arcweft-aw-ah-009.3.3.4-typed-associated-capacity-callee-authority-reconciliation-final-contract.zip`,
  SHA-256
  `DD8096DEDEF9FE2446291B3849DCEABD8BB5192B88533AA12FEE2DFC3CCEC484`.

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

## Accepted implementation boundary

The returned correction selects one typed route:

```text
ParenthesizedCalleeSyntax::PathMember
  -> existing Expr::Call(CallExpr)
  -> SourceBackedTypeRef / nominal resolution
  -> CallCallee::AssociatedType
  -> single resolve_call_target
  -> CapacityMethodId
  -> checker-owned facts
  -> native semantic signature projection
```

Dot-member syntax is value-first and may try nominal type resolution only when
typed value lookup is absent. Explicit generic `::member` syntax is
type-associated. Typed environment methods precede capacity, capacity precedes
associated traits, and data-last/untyped fallback is ineligible.

The implementation must switch parser/source-map, existing HIR clone, nominal
receiver, resolver, checker, and signature authority in one compiling
production cut. The same cut deletes the old import, early checker success
branch, `well_known_static_capacity_method_type`, generic text slicing, bare
`Vec` `_` placeholder, and every static-capacity label reader.

Do not add a `BuiltinCallableId`, 24th callable family, string parser, sentinel
receiver ID, compatibility wrapper, second resolver, parallel call HIR, or
stricter fake rejection schema. `String::with_capacity`,
`Bytes::with_capacity`, and bare `Vec::with_capacity` are not new aliases.

## Validation

Package intake and implementation-boundary update only. No Rust behavior or
test expectation is changed by this note.
