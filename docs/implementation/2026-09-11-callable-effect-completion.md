# Callable effect completion investigation — 2026-09-11

Status: **IN_PROGRESS; no production effect fix or completed callable model**.

The inspected `main` and `origin/main` are
`c00992a791d55fea70de6a3e736c0527ea913482`. The index is empty. The preserved
30-file callable/scope working copy is described in
[scope-owner evidence](2026-09-10-correlated-call-scope-owner.md). This
investigation adds two acceptance tests in the existing higher-order-effect
test module, bringing the unfinished inventory to 31 files before this note.
The [full convergence goal](2026-09-08-convergence-goal-plan.md) remains active.

## Observed failure and owner gap

The existing `inferred_callback_row_preserves_the_exposed_nonempty_contract`
test uses `apply(handler: i64 -> i64, value: i64) { handler(value) }` and passes
a closure that calls a function with `fs.read` effects. An exact test run with
a temporary backtrace at final call sealing identifies the failing operation:

1. `SealedSelectedCall::callee_update` calls `selected_callable_type`.
2. That function builds the checked base's callable type, then calls
   `FrozenCallTypeSolution::instantiate_result`.
3. The rejected type is the declaration body's dynamic `handler(value)`:
   `Function { binder: empty, params: [I64], return_type: I64, effects: Unknown }`.

This is not evidence that the checked base reconstructs nested rows from the
raw schema. Its existing `callable_type_with_terminal_effects` already uses
the checked higher-order effect projection for parameter and result children.
The unresolved row here is the invocation row supplied for the parameter's
dynamic call. Replacing that row with an empty set would discard the required
`fs.read` behavior, rather than close its ownership.

The current `CallableEffectGraph` indexes selected project-call targets and
closes `EffectSet` rows. `PreparedExecutionEffectRow` also owns a concrete
`EffectSet`; neither represents invocation dependencies on declaration-owned
callback effect parameters. The existing `GenericEffectReference` has the
Free/Bound/Inference vocabulary, but semantic `TypeKind::Function` still uses
the single-tail `EffectRow` representation. `ResidualGenericBinder` records
type and constant origins and constructs an effect arity of zero. These
boundaries must be reconciled together; changing only final catalog rows or
one projection cannot establish a scoped effect scheme.

The existing [coupled callable request](../reviews/requests/2026-09-08-aw-ah-009.4.2.1.1.1-function-scheme-specialization-and-callable-value-execution.md)
already requires adjudicating declaration-owned effect parameters, body and
closure dependencies, candidate applicability, and residual effect constraints
together. This investigation supplies narrower current-source evidence for
that requirement; it does not split it into a representation-only subcut.

## Added acceptance evidence

Two positive cases were added to
`crates/arcweft-lang-sema/src/final_analysis/tests/higher_order_effects.rs`:

- `invoked_callback_keeps_its_own_row_when_returned_with_another_callback_result`
  invokes a reader and a writer, then returns the reader alongside the computed
  result. The enclosing call must expose `{ fs.read, fs.write }`, while the
  returned callback retains only `{ fs.read }`. Unifying both input rows into
  their union would incorrectly widen the escaping callback.
- `shared_prefix_opens_later_callback_effects_independently` retains
  `apply_later(21i64)` before supplying its callback. Two uses supply a pure
  closure and a reader respectively. Prefix creation and the pure use have
  empty invocation rows; the reader use has `{ fs.read }`. Reusing one mutable
  effect binding or omitting residual effect quantifiers cannot satisfy this.

Both fixtures reach the same final-seal `Instantiation(Effect(UnknownRow))`
failure. Their syntax/HIR ingress succeeds. Their post-seal assertions remain
unreached and therefore are acceptance requirements, not successful evidence.

## Validation actually run

Local logs are in
`.arcweft-local/validation/2026-09-11-callable-effect-completion/`.

- `closure-row-trace.log`: the exact existing callback test failed; the
  temporary backtrace identified `selected_callable_type` as the failing caller.
- `closure-type-trace.log`: the same exact test failed and printed the complete
  unresolved `i64 -> i64` invocation type above.
- Both temporary diagnostic statements were removed. The entire modified
  `call_seal.rs` matched the pre-trace working-copy bytes by SHA-256 afterward.
- `higher-order-required-cases.log`:
  `cargo test -p arcweft-lang-sema --lib --all-features final_analysis::tests::higher_order_effects::`
  compiled the complete library test target and ran **12 tests: 6 passed,
  6 failed**. The failures are the four pre-existing inferred-row cases plus
  the two new acceptance cases. All commands ran sequentially with normal
  Cargo concurrency and no explicit job count. Formatting passed.
- No new workspace test, Clippy, doctest, runtime, codec or structure result
  is claimed for this investigation. The accepted record-storage cut's
  validation remains its own historical evidence.

The documentation cut records this investigation, publishes the previously
local scope-owner evidence, links it from the proposed model, and updates the
existing coupled request. Its Markdown links resolve and diff checks pass.
All 71 retained review archives were inventoried and SHA-256 hashed; their
tracked bytes are unchanged and no new archive awaits intake. The two Rust
acceptance tests remain with the unfinished implementation rather than being
published as a completed Rust cut.

## Required continuation

Complete the coupled source/application and callable-value model, including
scoped effect references, variable unions, constraint retention and unique
completion, source declaration ownership, residual opening, and every
source/ABI/runtime consumer. In particular, distinguish a declaration's
parameters from local existential inference, preserve separate callback rows
when values escape, and retain recursive effect dependencies in their equation
owner. A scalar substitution cycle and a valid monotone recursive body equation
are different obligations.

The authenticated child-source protocol, whole-component candidate comparison
and cross-application residual scope also remain required. Preserve the
accepted borrowed-driver accounting boundary and the accepted record-storage
owner. This is unfinished implementation/design work, not an external blocker
or a completed subset of the convergence goal.
