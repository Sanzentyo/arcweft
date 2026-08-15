# Lowering and synthetic-fact closure

## Fact ownership

The existing `HirRuntimeSemanticOwnerInventory` remains the sole HIR traversal
authority. Its inherent implementation gains `synthetic_expression_sites()`
and `synthetic_pattern_sites()` iterators that enumerate exactly the closed
variants listed in the CSV tables. No second HIR walker, extension trait, or
runtime-plan helper guesses sites from source syntax.

The compiler projection pushes each expected site with the exact normalized
type available from checked expression/statement/intrinsic/variant/layout
resolution. `RuntimePlanSemanticFacts::try_new` compares the expected and
submitted sets before it constructs the canonical maps. Error order is:

1. wrong HIR generation/snapshot;
2. unresolved/wrong owner family;
3. duplicate fact;
4. missing expected fact;
5. unexpected extra fact;
6. local/variant/layout/intrinsic semantic mismatch;
7. unsupported checked/operational projection.

No builder mutation occurs before all seven classes pass.

## Builder transaction

For one expression or pattern the lowerer produces a private draft containing:

- the raw recursive core enum;
- one `(RuntimeIndexPath, RuntimeNormalizedType)` row for root `[0]` and every
  present child in the enum's canonical pre-order;
- for patterns, the exact binding coordinates produced from the landed runtime
  local table and binding-path grammar.

The plan builder independently visits the raw enum, compares exact path sets,
projects every normalized type with the existing `runtime_plan_type_kind`, calls
`intern_batch`, rewrites declarations to plan-local IDs, validates bindings,
and publishes the typed wrapper. There is no interval in which a wrapper holds
an unchecked ID or an owner can store an untyped node.

## Canonical node order

- root is `[0]`;
- direct children use their enum field order and zero-based vector order;
- optional absent children emit no row but retain their reserved ordinal;
- nested paths append one checked `u32` ordinal;
- maximum depth is 64;
- duplicate paths, path overflow, non-root-first paths, missing rows, and extra
  rows are errors before type interning;
- exact duplicate semantic declarations share the existing type ID;
- conflicting kind for one semantic identity aborts the complete batch without
  mutation.

## Lowerer construction

`FinalFlowLowerer` owns the `RuntimePlanBuilder`. It never constructs a second
builder. Every nested expression/pattern lowerer is short-lived and borrows the
same mutable builder. Closure parameters, match arms, if-let patterns, flow
statement patterns, source handlers, stream patterns, and helper/function
bodies therefore all share one local/type table owner.

Synthetic literals such as Agent `kind`, ChoiceAction `enabled`, assertion
messages, or flow-assignment `Unit` have explicit accepted synthetic fact rows.
Their Rust `RuntimeValue` variant is never treated as type evidence.
