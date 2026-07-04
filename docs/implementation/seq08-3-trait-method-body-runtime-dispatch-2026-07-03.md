# seq08.3 Trait Method Body Runtime Dispatch

## Source

Applied from `D:/sanze/Downloads/arcweft-seq08.3-trait-method-body-runtime-dispatch-2026-07-03.zip`.

The package asked for executable user-authored `IntoIterator` / `Iterator`
witness dispatch without `IntoSeq`, range fallback, stringly conformance
rediscovery, or compatibility layers.

## Applied

- `TypeCheckReport` now carries the frozen `TraitCatalog` used for lowering.
- `TraitMethodImpl` keeps parsed method body statements and tail value.
- `ForIterationEvidenceFamily` distinguishes built-in families, executable
  witness pairs, and unsupported witness reasons.
- Runtime plans can carry lowered `RuntimeTraitMethod` callables.
- Runtime iterator witness evidence now stores typed `TraitCalls` targets.
- The core engine can call `into_iter(self)` and `next(&mut self)` through the
  witness call table and update witness iterator state.
- Types that implement `Iterator` directly now resolve as identity
  `IntoIterator` sources. The source value becomes the iterator state and only
  the `next(&mut self)` witness is required at runtime.
- Trait method return types substitute nested `Self::Assoc` occurrences, such
  as `Option<Self::Item>`, through impl associated type assignments.
- Assignment statements are parsed and checked as `target = expr`, and runtime
  method bodies can assign direct record fields through `RuntimeExpr::AssignField`.
- Strict runtime expression lowering now preserves `let` and direct field
  assignment statements inside block expressions. This closes the source
  method-body shape needed for `if { let value = self.current; self.current =
  ...; Some(value) } else { None }`.
- Expression parsing now treats `<` after a field access as a comparison unless
  a complete method turbofish is immediately followed by a call, so
  `self.current < self.end` no longer falls back to a raw expression.

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
- source-to-runtime-plan lowering and core-engine execution of a bare named
  DSL `struct Hoge` with only `impl Iterator for Hoge`, using
  `RuntimeIteratorWitnessExecutable::IdentityIntoIterator`;
- built-in iterator fast paths remain represented separately.

Structural audit completed and reported the existing workspace hotspot set:
3 error(s), 126 warning(s). The error-level files remain
`crates/arcweft-core/src/value.rs`,
`crates/arcweft-lang-sema/src/checker/expr.rs`, and
`crates/arcweft-runtime-plan/src/flow.rs`. New seq08.3 owner modules are below
the repository warning threshold:
`crates/arcweft-compiler/src/trait_methods.rs` is 145 physical LOC and
`crates/arcweft-runtime-plan/src/trait_methods.rs` is 254 physical LOC.
The current report was written to
`target/structure-audit-seq08-iterator-identity/`.

## Explicit Gaps

AWBC does not yet have the typed trait-method table, `CallTraitMethod` opcode,
codec, verifier, and VM/product-step execution path. Runtime-plan AWBC lowering
continues to reject witness-backed `for` lowering with an error diagnostic
rather than pretending to lower through a stringly intrinsic. Follow-up:
`docs/reviews/requests/2026-07-03-seq-08.3.1-awbc-trait-method-call-table-and-vm-closure.md`.

The source function-body expression closure gap for a multi-line
`next(&mut self)` body containing `if self.current < self.end { ... } else {
... }` is closed for `let`, direct field assignment, and value-producing block
tails. Broader statement families inside runtime expression blocks remain out
of scope unless they lower through typed runtime expressions.

## Design Deviations

- No compatibility layer was introduced for unsupported witnesses.
- No `IntoSeq` or range fallback was added.
- AWBC trait dispatch is intentionally not represented as `trait.method.N`
  intrinsics; unsupported AWBC paths remain explicit diagnostics until the typed
  AWBC table/VM work lands.
