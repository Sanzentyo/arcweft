# Final status

```text
STATUS=READY_FOR_IMPLEMENTATION
SEQUENCE=AW-AH-009.3.3.4
GIT=5f33ea20fcde7317332c95324701ed4ea7ab813a
JUJUTSU_CHANGE=yxvlsqorouqlolxvwtltxltmtqutsxku
OPEN_QUESTIONS=none
```

## Readiness decision

This correction is `READY_FOR_IMPLEMENTATION`.

It selects one typed source/HIR owner chain, one exhaustive parenthesized callee surface, one explicit value/type receiver distinction, one associated collision precedence, one registered/detached resolver entry, one `CapacityMethodId` owner implementation, one compiling direct-switch order, and a complete executable test matrix.

## Closed readiness gates

- canonical dot, current explicit-generic `::with_capacity`, and turbofish spellings: **closed**;
- exact path/generic/member lexeme and source-range ownership: **closed**;
- `CallExpr`/argument surface preservation and focused parent override: **closed**;
- HIR preservation without a parallel call AST: **closed**;
- nominal/alias/module/generic identity and bare-Vec behavior: **closed**;
- value/type classification without sentinel, display label, or source reparse: **closed**;
- lexical/project/environment value collision precedence: **closed**;
- typed environment > capacity > trait; data-last/untyped fallback ineligible: **closed**;
- accepted capacity ID/family/origin/result/arity/`TypeReceiver`: **closed**;
- accepted `variadic_unchecked` schema and removal of `_` implementation drift: **closed**;
- registered/non-registered convergence through one resolver entry: **closed**;
- malformed/unknown/ambiguous/value-call recovery and exactly-once argument work: **closed**;
- same-switch old-reader deletion: **closed**;
- checker/native/LSP parity and exact counters: **closed**;
- exact positive, negative, collision, identity, limit, cancellation, deletion, Tier 2, and structural rows: **closed**;
- parent-contract precedence and all non-goals: **closed**.

## Required implementation outcome

Implementation may be declared complete only when:

1. parser/type-source map, existing HIR clone/source binding, nominal resolution, associated resolver, checker facts, and native signature projection are connected as specified;
2. current `Vec<i32>::with_capacity(4usize)` and every required canonical/turbofish form pass through the typed route;
3. the old import, early string success branch, `well_known_static_capacity_method_type`, generic text slicing, bare-Vec placeholder, and all static-capacity label readers are absent in the same compiling switch;
4. the existing `CapacityMethodId` impl owns associated recognition and accepted `variadic_unchecked` schema construction with no `_`;
5. every row in `TEST_MATRIX.md`, workspace validation, Tier 2, and structural audit succeeds and is recorded;
6. no compatibility alias, deprecated carrier, dual reader, fallback, source gate, signature-only resolver, or parallel HIR call owner exists.

## Evidence limitation

The exact Git tree and supplied inputs were inspected and the output archive is mechanically verified. Production compilation was intentionally not performed because the dispatch prohibits implementation changes. The Jujutsu ID is recorded from dispatch but was not independently queried through the GitHub connector. Neither limitation leaves a result-changing design decision open.
