# Scrutinee type sources

This file closes every contextual type source. None is a name lookup, default,
fallback, or stored copy on a checked statement.

## Exact role table

| Private role | HIR owner/child | Sole expected-type owner and accessor | Completion proof |
| --- | --- | --- | --- |
| `TriggerInput` | `HirTrigger::Input(PatternId)` / `TriggerPattern` | `RegisteredTypeCheckEnv::statement_ingress().input()`; exactly `TypeKind::entity_ref(EntityKind::Input)` | completed `CheckedPattern::ty()` equals the borrowed registered type |
| `TriggerEvent` | `HirTrigger::Event(PatternId)` / `TriggerPattern` | unique same-typed stateful Entry event among all accepted executable roots that can reach the statement; each `HirEntryTypeBinding::ty()` is read with `PreparedEntrySemanticAuthority::ty(TypeId)` | consumed `PreparedEventScrutineeProof`; final seal compares its `SemanticTypeDigest` to every corresponding `CheckedStatefulEntry::event().semantic_type()` |
| `TriggerSignal` | `HirTrigger::Signal { target, value }` / typed target and optional value roles | completed target must be `TypeKind::entity_ref_with_value(EntityKind::Signal, T)`; optional pattern receives exactly borrowed `T` | checked target still has Signal-with-value `T`; checked value pattern has exactly `T` |
| `TriggerSelect` | `HirTrigger::Select(PatternId)` / `TriggerPattern` | `HirProjectEvaluationTopology::enclosing_choice_lifecycle(statement)` proves one accepted Choice lifecycle; expected type is exactly `TypeKind::entity_ref(EntityKind::ChoiceOption)` | completed pattern type equality plus same owner proof |
| `TriggerTask` | `HirTrigger::Task(PatternId)` / `TriggerPattern` | `RegisteredTypeCheckEnv::statement_ingress().task()`; exactly `TypeKind::StatementIngress(StandardStatementIngressTypeId::TaskEvent)` | completed pattern type equality |
| `TriggerScope` | `HirTrigger::Scope(PatternId)` / `TriggerPattern` | `RegisteredTypeCheckEnv::statement_ingress().scope()`; exactly `TypeKind::StatementIngress(StandardStatementIngressTypeId::ScopeExit)` | completed pattern type equality |
| `SelectFrame` | `HirSelectBranchHead::Frame { pattern, .. }` / `SelectPattern` | `RegisteredTypeCheckEnv::statement_ingress().frame()`; exactly `TypeKind::StatementIngress(StandardStatementIngressTypeId::FrameBoundary)` | checked pattern and local types equal the registered type; branch ordinal matches HIR child/body roles |
| `SelectEvent` | `HirSelectBranchHead::Event { pattern, .. }` / `SelectPattern` | same Entry-reachability query and Entry type authority as `TriggerEvent` | same consumed Event proof and branch-ordinal proof |
| Trigger Timeout | `HirTrigger::Timeout(ExprId)` / `TriggerExpression` | exact `TypeKind::Duration` | completed `CheckedExpression::ty()` equals Duration |
| Trigger Expression | `HirTrigger::Expression(ExprId)` / `TriggerExpression` | exact `TypeKind::Bool` | completed expression type equals Bool |
| Select Bind | `HirSelectBranchHead::Bind { binding, source }` / binding and source roles | completed checked source result after ordinary prefix-Try analysis | checked binding local type equals the exact checked source result; no propagation bit exists |

Mark has no scrutinee. A recovered trigger or branch never asks this table for a
success type.

## Standard publication roles

The three concrete standard type IDs and their typed registration mapping are:

| Role ID | Type ID | Exact semantic type |
| --- | --- | --- |
| `StatementIngressTypeRoleId::Task` | `StandardStatementIngressTypeId::TaskEvent` | `TypeKind::StatementIngress(TaskEvent)` |
| `StatementIngressTypeRoleId::Scope` | `StandardStatementIngressTypeId::ScopeExit` | `TypeKind::StatementIngress(ScopeExit)` |
| `StatementIngressTypeRoleId::Frame` | `StandardStatementIngressTypeId::FrameBoundary` | `TypeKind::StatementIngress(FrameBoundary)` |

`TypeCheckEnv::new()` constructs exactly one
`StatementIngressTypePublicationInput` for each row. The accepted base
environment is the sole producer. `ProjectRegistrar` consumes these in the
ordinary registered-environment transaction; `RegisteredTypeCheckEnv` owns the
sealed record. Source-backed adapters do not publish or override these fixed
language roles.

These atoms deliberately live in sema's type algebra rather than importing
`arcweft_core::TaskEvent` or `arcweft_core::ScopeExit`. Sema already depends in
the permitted direction on core for shared value vocabulary, but a Rust
runtime struct is not a source semantic type and cannot become one by type
name. Runtime conversion, if later admitted, must be an explicit compiler or
runtime-plan projection.

## Accepted Entry-root reachability

The authoritative preparation is private. It interleaves Entry-root context
with selected-call discovery so contextual locals can participate in call
checking without a circular global phase:

```rust
pub(crate) struct PreparedExecutableIngressWorklist {
    pending: BTreeSet<CallableDeclarationKey>,
    facts: PreparedExecutableIngressFacts,
}

pub(crate) struct PreparedExecutableIngressFacts {
    declarations: BTreeMap<
        CallableDeclarationKey,
        PreparedDeclarationIngressProof,
    >,
}

impl Analyzer<'_, '_, '_> {
    pub(crate) fn complete_contextual_declarations(
        &mut self,
        roots: Box<[PreparedEntryRootSeed]>,
        includes: Box<[PreparedIncludeFlowProof]>,
        limits: StatementPreparationLimits,
    ) -> Result<Box<[PreparedEventScrutineeProof]>, StatementPreparationError>;
}
```

`StatementPreparationLimits` is a new private final-analysis testable limit
carrier, not a public contract or semantic payload. Its exact fields are
`max_declarations`, `max_edges`, `max_entry_contributors`,
`max_contextual_statements`, and `max_work`. The private production constructor
derives the first four from the already accepted bounded callable-declaration,
selected-call plus Include, stateful-Entry, and HIR-statement counts;
`max_work` is their checked contributor-delta traversal sum/product. All
additions, products, queue growth, and preallocation use checked `u64`;
overflow, preallocation beyond a bound, or N+1 work fails transactionally.

The worklist performs these typed operations:

1. A shared private `PreparedEntryRootSeed` resolves each stateful Entry's
   unique accepted `HirEntryMember::Goto` to the exact Flow declaration. The
   final Entry checker consumes the same seed; it does not resolve again.
2. Each root seeds its exact resolved event `TypeKind`, semantic digest, and
   Entry item identity into the target declaration. A recovered/open/poison
   type rejects.
3. The first digest reaching a declaration selects the expected Event type for
   all of that declaration's Event-bearing statements, located with
   `topology.semantic_path(statement)`. In a short lexical scope, an immutable
   `PreparedEntrySemanticAuthority` and `StatementScrutineeTypeAuthority`
   borrow the draft's disjoint type/item/current-call stores and the worklist's
   one `PreparedExecutableIngressFacts`. After seeding, both views are dropped
   before either the call store or worklist facts are mutated.
4. Pattern, expression, and call checking for that declaration then completes.
   Selected `CallTargetFacts` add directed edges only when their accepted
   target is an executable project declaration. Calls to externals or
   non-executable owners do not fabricate project edges.
5. Prepared Include resolution adds an edge from its owning executable root to
   the exact accepted Flow `CallableDeclarationDigest`. The same move-only
   proof is later consumed to build `CheckedIncludeFlowTarget`.
6. Every selected call/Include edge propagates the source declaration's digest
   and complete sorted Entry contributor set. A later equal digest merges new
   contributors and propagates only the delta; an unequal digest rejects.
   Deterministic declaration-key ordering and explicit visited/SCC state make
   recursion finite and traversal-order independent.
7. For every contributing stateful Entry, its unique accepted
   `HirEntryMember::EventType(HirEntryTypeBinding)` supplies `TypeId`; the
   already prepared type map supplies `&TypeKind` through `ty(TypeId)`.
8. After all selected edges are known, a fresh deterministic traversal from
   the same root seeds must reproduce every declaration digest/contributor set.
   Any missing edge, stale call, zero-reached Event declaration, recovery,
   missing type, or unequal digest rejects. Equal digests yield the exact
   statement-scoped proofs and consumes the worklist into a move-only
   `PreparedExecutableIngressSeal`. Only that seal can construct the Entry
   authority used by final Entry checking.

The worklist does not retain a reachability catalog in final analysis.
Declaration state and traversal scratch are private and dropped; only
statement-scoped move-only proofs survive until the Entry seal, where they are
consumed.

## Analyzer phase order

The necessary in-place phase order is:

```text
HIR topology, types, callable schemas, non-contextual seeds
  -> Entry-root + Include preparation
  -> deterministic Entry-seeded declaration worklist
       -> borrow contextual selector and seed one declaration
       -> drop selector borrow
       -> complete its patterns/expressions/selected calls/statements
       -> propagate equal event digest/contributors over selected edges
  -> independent completed-graph reachability recomputation
  -> checked Entry catalog seal and Event-proof consumption
  -> final analysis publication
```

This is one move-only draft transaction. `StatementScrutineeTypeAuthority`
borrows the registered record, HIR project/topology, and prepared Entry
authority only during the middle two steps. It cannot be cloned into the final
report.

## Choice lifecycle query

HIR owns the query because it owns typed parentage:

```rust
pub struct HirChoiceLifecycleContext {
    choice: ExprId,
    owner: HirSemanticPath,
}

impl HirProjectEvaluationTopology {
    pub fn enclosing_choice_lifecycle(
        &self,
        statement: StmtId,
    ) -> Result<HirChoiceLifecycleContext, HirChoiceLifecycleContextError>;
}
```

The constructor for `HirChoiceLifecycleContext` is private. The query follows
typed expression-owned/body edges and accepts only the Choice plan roles that
can legally own Select triggers. Zero owners, multiple owners, a recovered
edge, a different generation, or an unrelated nested body rejects. The query
does not expose a raw parent map or infer from source location.

## Non-authorities

The following are explicitly non-authoritative for every row above:

- `TypeKind::Named`, `EntityKind::Other`, `Any`, or a default type;
- `GameEvent`, `TaskEvent`, `ScopeExit`, `Frame`, or any other terminal/source
  string lookup;
- runtime-core Rust type names or runtime payload inspection;
- first/nearest Entry selection without complete accepted reachability;
- a cached contextual-type map on final analysis;
- whole-analysis or whole-Entry-catalog digest equality;
- source spans, ranges, display labels, and raw HIR IDs.
