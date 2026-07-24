# AW-AH-009.3 static capacity associated-callee blocker

Date: 2026-07-24

## Status

`IMPLEMENTED_FOCUSED_VALIDATED`. The former design blocker was closed by the
verified AW-AH-009.3.3.4 return, and the static associated capacity-call
authority migration is implemented. Final workspace, Tier 2, and structural
gates are recorded with the AW-AH-009.3 closure follow-up.

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

## Superseded production evidence

The stable language surface requires `String.with_capacity`,
`Bytes.with_capacity`, and `Vec<T>.with_capacity`. The callable model already
owns `CapacityMethodId` and `CallableFamily::CapacityMethod`, and the accepted
shared-resolver contract fixes the capacity precedence and intentionally
unchecked argument schema.

The pre-migration checker path called
`well_known_static_capacity_method_type(&str)` before the shared resolver. That
helper recognizes string spellings, maps bare `Vec` to a named `_` placeholder,
and reconstructs `Vec<T>` from text. Consequently a registered static capacity
call can succeed with `old_dispatch_calls == 1` and
`shared_resolver_invocations == 0`, without checker-owned target facts for
native signature help.

At intake time, the accepted `CallCallee::Selected` request owned a value
`receiver_expression: TypeExpressionId`. Neither accepted package defines a
typed type-receiver/associated-callee carrier, generic identity resolution, or
the collision rules needed to feed `String` or `Vec<T>` to the resolver. The
AW-AH-009.3.1 rule that static generic callees use the Pratt/path grammar removes
source scanning but does not select that semantic owner.

That evidence justified direct replacement rather than repairing or wrapping
the fallback. The obsolete helper and early success branch are now deleted.

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

## Implemented production authority

Production now follows the accepted route above:

- `String.with_capacity`, `Bytes.with_capacity`, `Vec<I32>.with_capacity`, and
  `Vec<T>.with_capacity` retain a source-backed typed receiver and reach
  `CapacityMethodId` through the shared resolver;
- direct `.member<Type>(...)` and turbofish `.member::<Type>(...)` call type
  applications keep typed authored arguments and exact ranges instead of
  encoding `<...>` into a member string;
- lexical, project, imported, and environment values are resolved before a
  nominal receiver. Only typed absence permits nominal fallback;
- aliases, qualification, and generic parameter identity are resolved by the
  existing HIR/nominal owners;
- bare `Vec.with_capacity(8)` fails generic arity before candidate resolution,
  as required by AW-AH-009.3.3.4 T08/C17. The contradictory package-local
  CAP-005 row is superseded by that explicit precedence;
- the old helper, import, early success branch, generic text slicing, `_`
  placeholder, and static-capacity label readers have zero production users
  and were removed rather than renamed.

This switch adds no callable family, second resolver, display-label parser,
sentinel receiver, compatibility carrier, or dual authority.

## Validation

Focused validation passes for the syntax, HIR, sema, compiler,
project-loader, and LSP owners. The associated-capacity matrix covers success,
typed rejection, recovery, value/type collision, exact/one-over bounds,
registered/detached parity, checker/signature parity, and exactly-once work.
The final broad gate results are recorded in
[AW-AH-009.3 focused boundary follow-up](2026-07-24-aw-ah-009-3-focused-boundary-follow-up.md).
