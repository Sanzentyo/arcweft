# Seq08.2 Standard Iterator / IntoIterator

Date: 2026-07-03

## Scope

This cut applies the seq08.2 standard iteration package on top of the seq08.1
trait substrate. The package patch was not directly applied because its patch
file was malformed in this checkout, so the implementation was integrated
manually against current `main`.

Implemented:

- Standard `Iterator` and `IntoIterator` declarations are installed into the
  semantic `TraitCatalog`.
- Built-in impls exist for integer ranges, `Seq<T>`, `Vec<T>`, arrays, and
  slices.
- Standard iterator state types are represented structurally as
  `TypeKind::IteratorState`, not as `TypeKind::Named("RangeIter<T>")` style
  strings.
- `for` source typing resolves through `IntoIterator` conformance instead of
  the old concrete `iter_item_type` path.
- Runtime-plan `FlowOp::For` and `FlowOp::ForNext` carry
  `RuntimeIteratorEvidence`.
- Compiler entry points pass the type-check report's flow iteration evidence
  into runtime-plan lowering.
- Runtime `for` execution uses explicit evidence to create a
  non-materialized `RuntimeIterator`.
- Product AWBC `for` lowering uses direct iterator-state intrinsics:
  `core.iter.into_iter`, `core.iter.next`, `core.option.is_some`, and
  `core.option.unwrap`.
- `docs/01-language/traits-seq-ranges.md` now describes `Seq<T>` as a concrete
  lazy sequence type implementing `Iterator` / `IntoIterator`; `IntoSeq` is not
  retained as a stable protocol.

## Deliberate Shape

The first integration attempt exposed that representing built-in iterator state
as named generic strings would require widening stringly generic substitution.
That was rejected. Built-in iterator states are now sema-owned structural types
with a standard family and item type. User-authored nominal types remain
ordinary `TypeKind::Named` values and are not broadened to make standard
iteration work.

The `TypeCheckReport::for_iteration_evidence` collection is scoped to runtime
flow checking. Function-local `for` expressions still type-check through
`IntoIterator`, but they are not inserted into the runtime-flow evidence stream.
This prevents unrelated checked functions from shifting flow lowering evidence
by traversal order.

## Remaining Work

User-authored `IntoIterator` witnesses can be accepted by sema, but executable
runtime dispatch for witness-backed iterator method bodies is still blocked.
AWBC lowering rejects witness-backed iterator evidence with an explicit
diagnostic instead of falling back to range or sequence probing.

Follow-up request:

- `docs/reviews/requests/2026-07-03-seq-08.3-trait-method-body-runtime-dispatch.md`

## Validation

Passed:

```bash
cargo fmt --all
cargo fmt --all -- --check
cargo check -p arcweft-lang-sema --all-targets
cargo check -p arcweft-cli --all-targets
cargo check -p arcweft-core -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-compiler -p arcweft-runtime-plan --all-targets
cargo test -p arcweft-lang-sema for_iteration_evidence_is_trait_resolved_for_runtime_flows -- --nocapture
cargo test -p arcweft-compiler compiles_for_loop_with_trait_resolved_iterator_evidence -- --nocapture
cargo test -p arcweft-runtime-plan awbc_product_parity_for -- --nocapture
cargo test -p arcweft-core for_loop_expands_one_iteration_at_a_time -- --nocapture
cargo clippy -p arcweft-core -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-compiler -p arcweft-runtime-plan --all-targets --all-features -- -D warnings
git diff --check
```

Partial / unrelated:

```bash
cargo test -p arcweft-runtime-plan awbc_product_parity -- --nocapture
```

This ran all 47 `awbc_product_parity` tests. The five `for` parity tests passed.
One unrelated existing parity assertion failed in
`awbc_product_parity_entry_root_bindings_named_equivalent` because the AWBC and
structured facade environments contain equivalent named root bindings in
different order.

Structural audit:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

The command completed and reported the current workspace totals:
2,159 files scanned, 1,059 Rust files, 499,056 Rust LOC, 91 package manifests,
2 existing error-level structure findings, and 126 warnings. The seq08.2 changes
touch existing large files including:

```text
crates/arcweft-core/src/value.rs                  83,943 bytes / 2,483 LOC
crates/arcweft-core/src/engine/eval.rs            55,895 bytes / 1,490 LOC
crates/arcweft-lang-sema/src/traits.rs            51,974 bytes / 1,547 LOC
crates/arcweft-runtime-plan/src/flow.rs           92,231 bytes / 2,545 LOC
crates/arcweft-runtime-plan/src/awbc_lower/flow.rs 57,678 bytes / 1,507 LOC
```

`crates/arcweft-runtime-plan/src/flow.rs` remains above the production-file
error threshold. This cut limits the change there to evidence plumbing; a broad
module split is intentionally left out of seq08.2 because it would mix the
iterator contract change with a separate runtime-plan ownership refactor.
