# seq08.3 Trait Method Body Runtime Dispatch

## Source

Applied from `D:/sanze/Downloads/arcweft-seq08.3-trait-method-body-runtime-dispatch-2026-07-03.zip`.

The package asked for executable user-authored `IntoIterator` / `Iterator`
witness dispatch without `IntoSeq`, range fallback, stringly conformance
rediscovery, or compatibility shims.

## Applied

- `TypeCheckReport` now carries the frozen `TraitCatalog` used for lowering.
- `TraitMethodImpl` keeps parsed method body statements and tail value.
- `ForIterationEvidenceFamily` distinguishes built-in families, executable
  witness pairs, and unsupported witness reasons.
- Runtime plans can carry lowered `RuntimeTraitMethod` callables.
- Runtime iterator witness evidence now stores typed `TraitCalls` targets.
- The core engine can call `into_iter(self)` and `next(&mut self)` through the
  witness call table and update witness iterator state.
- Trait method return types substitute nested `Self::Assoc` occurrences, such
  as `Option<Self::Item>`, through impl associated type assignments.
- Assignment statements are parsed and checked as `target = expr`, and runtime
  method bodies can assign direct record fields through `RuntimeExpr::AssignField`.

## Verification

Executed locally:

```bash
cargo check --workspace --all-targets
cargo test -p arcweft-core engine_executes_for_loop_through_trait_method_witness_calls -- --nocapture
cargo test -p arcweft-compiler lowers_user_defined_into_iterator_to_executable_trait_calls -- --nocapture
cargo clippy -p arcweft-core -p arcweft-lang-sema -p arcweft-compiler -p arcweft-runtime-plan --all-targets --all-features -- -D warnings
git diff --check
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

The focused tests cover:

- engine execution of a `for` loop through runtime trait-method witness calls;
- source-to-runtime-plan lowering of a user-defined `IntoIterator` /
  `Iterator` pair into `RuntimeIteratorWitnessExecutable::TraitCalls`;
- built-in iterator fast paths remain represented separately.

Structural audit completed and reported the existing workspace hotspot set:
3 error(s), 125 warning(s). The error-level files remain
`crates/arcweft-core/src/value.rs`,
`crates/arcweft-lang-sema/src/checker/expr.rs`, and
`crates/arcweft-runtime-plan/src/flow.rs`. New seq08.3 owner modules are below
the repository warning threshold:
`crates/arcweft-compiler/src/trait_methods.rs` is 128 physical LOC and
`crates/arcweft-runtime-plan/src/trait_methods.rs` is 265 physical LOC.

## Explicit Gaps

AWBC does not yet have the typed trait-method table, `CallTraitMethod` opcode,
codec, verifier, and VM/product-step execution path. Runtime-plan AWBC lowering
continues to reject witness-backed `for` lowering with an error diagnostic
rather than pretending to lower through a stringly intrinsic. Follow-up:
`docs/reviews/requests/2026-07-03-seq-08.3.1-awbc-trait-method-call-table-and-vm-closure.md`.

The full package fixture with a multi-line `next(&mut self)` body containing
`if self.current < self.end { ... } else { ... }` still exposes a source
function-body expression closure gap. The runtime expression model can execute
the needed condition, field read, and direct field assignment, but the full DSL
fixture should be closed in a dedicated source-to-runtime regression package.
Follow-up:
`docs/reviews/requests/2026-07-03-seq-08.3.2-iterator-method-body-source-closure.md`.

## Design Deviations

- No compatibility shim was introduced for unsupported witnesses.
- No `IntoSeq` or range fallback was added.
- AWBC trait dispatch is intentionally not represented as `trait.method.N`
  intrinsics; unsupported AWBC paths remain explicit diagnostics until the typed
  AWBC table/VM work lands.
