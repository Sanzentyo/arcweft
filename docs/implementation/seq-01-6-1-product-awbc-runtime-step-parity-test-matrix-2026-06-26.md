# Seq-01.6.1 Differential Test Matrix

## Harness design

The differential harness belongs in `arcweft-runtime-plan` tests because that
layer may own both a `RuntimePlan` and its canonical AWBC lowering while
`arcweft-core` must not depend upward on the lowerer.

Each fixture creates one `RuntimePlan`, then constructs:

- a structured executor from the plan;
- an `AwbcProgram` from `AwbcLowerer` and an AWBC product executor;
- identical root bindings;
- identical scripted `RuntimeStepInput` values;
- deterministic pure and host backends.

For every step the harness captures:

```rust
struct ParitySnapshot {
    raw: RuntimeStepResult,
    normalized: NormalizedRuntimeStepResult,
    environment: RuntimeEnv,
    observations: RuntimeObservationState,
    sources: BTreeMap<SourceId, SourceRuntimeState>,
    streams: BTreeMap<StreamRuntimeId, StreamRuntimeState>,
}
```

Normalization is allowed only for tier-private identifiers whose public meaning
is already compared separately. Ordering, public IDs, values, diagnostics,
source ranges, status, stop reason, and statistics must remain raw-equal unless
a fixture documents an intentional tier-specific counter.

## Required fixtures

| Family | Fixture | Scripted inputs | Required assertions |
|---|---|---|---|
| Entry: empty | zero-parameter entry returning a constant | empty step | same terminal value, `Done`, no diagnostics |
| Entry: positional | two typed parameters used by an expression | positional root bindings | equal environment and result |
| Entry: named-equivalent | same plan | reversed named bindings | equal parameter mapping |
| Entry: retry | same plan | invalid type, then corrected bindings | first step fails without register mutation; second succeeds |
| Pure helper backend | helper with scalar compact body | recording backend selects accelerated result | stable helper ID/name/arity, equal result, backend stat delta |
| Pure helper fallback | same helper | backend returns `None` | equal result, compact VM/fallback counters increment once |
| Intrinsic | deterministic intrinsic call | empty input | same call target, args, result/failure |
| Dialogue | one dialogue line with start/complete/cancel line tasks | present, idle, explicit advance | line emitted once, no auto-resume, matching task order and cleanup |
| Dialogue stale advance | two sequential lines | advance payload for prior line | stale event ignored/diagnosed; active line remains suspended |
| Choice | guarded three-option choice | present, explicit second-option selection | full filtered set, public IDs/labels/effects, exactly-once resume |
| Choice invalid | same choice | unknown option, then valid option | invalid selection does not mutate destination or resume |
| Await ready | one task await with binding | present, progress, ready | stable task/need IDs, progress order, bound result |
| Await error | same await | error event | same diagnostic category/message/status |
| Await cancel | same await | cancellation event | same cancellation effects and terminal state |
| Await-many order | four items, limit two | out-of-order progress/ready events | request order, in-flight limit, result source order |
| Await-many partial error | same plan | one error after partial completion | partial slots retained; same completion/failure rule |
| Host immediate | immediate deterministic call | matching result in same input | request/result correlation and same value |
| Host suspending | suspending call | no result, stale result, matching result | `Blocked`, stale rejection, exactly-once resume |
| Effects | one fixture row per `AwbcEffectKind` | empty input | mapped output equality or typed unsupported-capability diagnostic |
| Ensure content | repeated ensure for same unit | two drain steps | request once, complete resource metadata |
| Stream | yield two values then close | drain steps across budget boundary | sequence 0/1/2, order, close once, no replay |
| Source | source event, handler, close | duplicate/out-of-order host events | normalized ordering, policy behavior, close idempotence |
| One-op | linear arithmetic plan | `RuntimeStepMode::OneOp` | one attempted op and `OneOp` stop |
| Output | line/effect plan | drain/game step | stop at first visible output without duplicate next step |
| Budget | loop with small VM quantum and host max | repeated steps | `BudgetExhausted`, canonical resume, no repeated output |
| Division trap | verifier-safe division by zero | empty step | source label/range/category, retained prior output, failed fiber |
| Type trap | dynamic type mismatch | empty step | same category/message/map/status |
| Pattern trap | failing bind/test pattern | empty step | same pattern diagnostic and environment rollback |
| Host trap | backend deterministic failure | empty step | host category, mapped source, same partial output |
| Statistics | mixed pure/task/source/stream/line plan | scripted inputs | all `RuntimeStepStats` fields and final facade state |

## Table-driven effect coverage

Do not use a catch-all branch. The test table must enumerate every
`AwbcEffectKind` variant. Adding a new variant must fail compilation until both
its owned mapping and expected parity row are added.

## Product smoke gates

After core/runtime-plan differential tests pass, add focused decoded-AWFB smoke
at these boundaries:

- `arcweft-runtime-driver`: decoded product selects `AwbcProduct` and advances;
- `arcweft-runtime-host`: host request/result loop preserves suspension and
  output ordering;
- `arcweft-player-native`: product bundle reaches the shared player runtime
  without reading structured bytecode;
- source gates: no Game-product path directly constructs
  `BytecodeVmExecutor` or executes `bundle.bytecode.program`.

## Suggested command names

```bash
cargo test -p arcweft-runtime-plan awbc_product_parity_entry -- --nocapture
cargo test -p arcweft-runtime-plan awbc_product_parity_dialogue -- --nocapture
cargo test -p arcweft-runtime-plan awbc_product_parity_choice -- --nocapture
cargo test -p arcweft-runtime-plan awbc_product_parity_await -- --nocapture
cargo test -p arcweft-runtime-plan awbc_product_parity_host -- --nocapture
cargo test -p arcweft-runtime-plan awbc_product_parity_effect -- --nocapture
cargo test -p arcweft-runtime-plan awbc_product_parity_source_stream -- --nocapture
cargo test -p arcweft-runtime-plan awbc_product_parity_budget_trap_stats -- --nocapture
```
