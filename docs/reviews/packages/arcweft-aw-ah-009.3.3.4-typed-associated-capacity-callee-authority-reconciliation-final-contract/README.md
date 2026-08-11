# AW-AH-009.3.3.4 final correction

## Status

`READY_FOR_IMPLEMENTATION`

This design-only archive closes the typed authority gap for static associated capacity calls. It contains no Rust patch, test patch, manifest edit, fixture edit, stable-language edit, schema file edit, or repository overlay.

## Baseline and sequence

- Sequence: `AW-AH-009.3.3.4`.
- Git baseline inspected: `5f33ea20fcde7317332c95324701ed4ea7ab813a`.
- Jujutsu change supplied by dispatch: `yxvlsqorouqlolxvwtltxltmtqutsxku`.
- Output language: English.

The Git tree was inspected at the exact commit through the private Arcweft GitHub connector. The connector exposes Git objects, not Jujutsu change metadata, so the supplied Jujutsu change is recorded but was not independently resolved. No decision relies on Jujutsu-only content.

## Consumed accepted packages

| Package | Verified SHA-256 |
|---|---|
| `arcweft-aw-ah-009.3.1-call-surface-syntax-production-reconciliation-final-contract.zip` | `6ede771a895af981a583fdfd50a080f2eca57bf7a2925216cf725f7dbb418588` |
| `arcweft-aw-ah-009.3.3-callable-catalog-shared-resolver-production-reconciliation-final-contract.zip` | `9d1f989f5e0e698aeff1098dd7ecee7e01a66616a00a0571ee333a3b1b7ddc78` |

Both outer digests and every non-self internal-manifest entry were verified before this correction was authored.

## Selected authority

The final authority is one typed pipeline:

```text
Pratt/path/type tokens
  -> ParenthesizedCalleeSyntax::PathMember(PathMemberCalleeSyntax)
  -> the existing immutable Expr::Call(CallExpr) HIR clone
  -> existing SourceBackedTypeRef / TypeResolutionInput
  -> ResolvedAssociatedTypeReceiver
  -> CallCallee::AssociatedType
  -> the existing resolve_call_target entry
  -> CapacityMethodId::resolve_associated
  -> the existing check_resolved_call transaction
  -> checker-owned CallTargetFacts
  -> native semantic signature projection
```

Syntax owns authored structure and exact ranges but does not decide whether a path is a value or a type. HIR preserves the same call value and source identity without a parallel call AST. Sema performs value/type classification and nominal resolution. The callable resolver owns candidate precedence. `CapacityMethodId` remains the sole capacity identity and schema owner.

## Accepted source families

The correction preserves all relevant existing source families:

```arcw
String.with_capacity(64)
Bytes.with_capacity(4096)
Vec<I32>.with_capacity(8)
Vec<T>.with_capacity(8)
Vec<I32>::with_capacity(8)
Vec<T>::with_capacity(8)
Vec::<I32>.with_capacity(8)
Vec::<I32>::with_capacity(8)
```

The first four are the canonical dot-member forms. The explicit-generic `::with_capacity` forms preserve current valid static-generic source, including the repository fixture `Vec<i32>::with_capacity(4usize)`. Turbofish receiver spelling is retained losslessly. This correction does not introduce `String::with_capacity`, `Bytes::with_capacity`, or bare `Vec::with_capacity` aliases.

## Closed decisions

1. **Type-receiver owner:** `AuthoredTypeRef` plus an extended `TypeRefSourceMap` own the receiver tree, every path/generic lexeme, and local ranges. `ParenthesizedCallSyntax` owns the member separator and terminal member. Sema owns `ResolvedAssociatedTypeReceiver` and the value/type distinction.
2. **Call-surface preservation:** `Expr::Call(CallExpr)`, `CallExpr` fields, `CallSurfaceSyntax`, argument-list ownership, recovery, cursor rules, and argument semantics remain unchanged. Only `ParenthesizedCallSyntax::callee` becomes a typed enum that retains the existing range API.
3. **Collision precedence:** dot-member syntax first performs typed value lookup. Any present, ambiguous, inaccessible, or poisoned value outcome is terminal and never retries as a type. Only typed value absence permits nominal type resolution. Explicit-generic `::member` syntax is a type-associated form and does not perform runtime value lookup. After a type is accepted, typed environment `Method` records win over capacity; capacity wins over associated traits; data-last and untyped method fallback are structurally ineligible.
4. **Capacity identity:** `CapacityMethodId::resolve_associated` accepts only `with_capacity` on resolved `String`, `Bytes`, and `Vec<T>` receivers. The ID records the exact authored argument-entry count. The result and `CallableInstantiation::TypeReceiver` contain the exact resolved receiver.
5. **Argument behavior:** the existing parent contract's `variadic_unchecked` schema is normative. Every authored positional, named, spread, or recovered argument expression is checked exactly once without an expected type. The baseline `homogeneous(..., Named("_"), ...)` implementation is drift and must be replaced in the existing `CapacityMethodId` impl; `_` is not retained.
6. **Bare `Vec`:** normal builtin generic-arity failure. It never creates `Vec<_>`, never constructs `TypeKind::Named("_")`, and never produces a capacity candidate.
7. **Registered/non-registered convergence:** one typed `CallResolverAuthority` enum feeds one `resolve_call_target`; both modes create the same associated receiver seed and call the same capacity identity/schema methods.
8. **Deletion:** parser/HIR/type-resolution/resolver/checker/signature wiring and deletion of the old import, early string success branch, helper, bare-Vec placeholder, and every static-capacity string reader occur in one compiling production switch.
9. **Tests:** all positive, negative, spelling, collision, identity, parity, counter, limit, deletion, and Tier 2 rows are executable and closed in `TEST_MATRIX.md`.

## Normative precedence over parent packages

This correction supersedes only these parent assumptions:

- AW-AH-009.3.1's `ParenthesizedCallSyntax::callee: TextRange` is refined to a typed `ParenthesizedCalleeSyntax` that retains the same `callee_range()` result.
- AW-AH-009.3.3's `CallCallee::Selected` no longer has to represent a type receiver through a fake `receiver_expression`; `CallCallee::AssociatedType` is the explicit alternative.
- The accepted AW-AH-009.3.3 capacity schema remains `variadic_unchecked`; the baseline placeholder/exact homogeneous implementation is not normative.

The correction does not change the 23-family inventory, `CallableCandidateId::CapacityMethod`, `CallableFamily::CapacityMethod`, ordinary selected-call ordering, transactional probing, checker target facts, signature projection, limits, cache identity, or AW-AH-009.4.2 Dialogue ownership.

## Archive map

- `FINAL_CORRECTION.md` — normative APIs, invariants, recovery, and authority-switch order.
- `TYPE_RECEIVER_MODEL.md` — receiver spellings, syntax/HIR/source/type identity, and failure model.
- `RESOLVER_INTEGRATION.md` — request shape, precedence, candidate/schema construction, parity, and work accounting.
- `TEST_MATRIX.md` — exact executable closure matrix.
- `REQUIREMENTS_TRACEABILITY.md` — request and parent-row completion mapping.
- `REPOSITORY_EVIDENCE.md` — inspected paths and verification boundary.
- `OPEN_QUESTIONS.md` — exactly `none`.
- `FINAL_STATUS.md` — readiness decision.
- `MANIFEST.txt` — filename-sorted SHA-256 and byte length for every other member.
