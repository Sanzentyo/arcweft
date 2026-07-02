# Seq08.1 DSL trait / impl / associated type substrate

Date: 2026-07-02

## Summary

Seq08.1 makes Arcweft DSL `trait`, `impl`, associated type requirements, associated type assignments, bounds, projections, coherence, and witness evidence real in semantic analysis. This is the substrate required before standard library `Iterator` / `IntoIterator` integration.

The implementation keeps the existing layer boundaries:

```text
arcweft-lang-syntax -> arcweft-lang-hir -> arcweft-lang-sema -> arcweft-compiler/runtime-plan
```

`arcweft-core` remains unaware of DSL trait conformance.

## Implemented behavior

- `trait` declarations are cataloged in sema.
- `impl Trait for Type` declarations are cataloged in sema.
- Required associated types are validated.
- Associated type assignments are validated.
- Required methods are validated.
- Supertrait requirements are inherited during impl validation.
- `Self::Assoc` and `T::Assoc` are structured projections.
- `where T: Trait<Assoc = Type>` is represented as a typed predicate.
- Duplicate and overlapping impls are rejected conservatively.
- Impl orphan rules are enforced within the current package/module boundary.
- Trait method calls resolve through direct impl witnesses or active generic predicates.
- Ambiguous trait method and projection cases produce structured diagnostics.
- `TraitWitnessId` records typed conformance evidence in sema.

## Deliberately deferred

- Associated type constructors (`type Mapped<B>`) are parsed but rejected.
- Associated type defaults are parsed but rejected.
- Default method bodies are parsed/preserved but rejected.
- Fully qualified syntax is deferred.
- Dynamic trait objects are not implemented.
- `Iterator` / `IntoIterator` standard traits and `for` lowering are seq08.2 work.
- Runtime-plan witness ids are not added until lowering consumes them.

## Diagnostics

Trait diagnostics are represented as `TypeCheckErrorKind::Trait { diagnostic }` and produce `sema.trait.*` codes. This keeps them compatible with existing compiler, CLI, LSP, and Agent diagnostic surfaces.

## Validation status

Validated on 2026-07-03 from the seq08.1 application checkout:

```bash
cargo fmt --all -- --check
cargo check -p arcweft-lang-syntax -p arcweft-lang-hir -p arcweft-lang-sema -p arcweft-compiler -p arcweft-runtime-plan --all-targets
cargo test -p arcweft-lang-syntax --test traits --all-targets
cargo test -p arcweft-lang-sema --lib
cargo test -p arcweft-lang-sema --test traits
cargo test -p arcweft-compiler --test traits
cargo clippy -p arcweft-lang-syntax -p arcweft-lang-hir -p arcweft-lang-sema -p arcweft-compiler -p arcweft-runtime-plan --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

All compile, fmt, test, clippy, and diff-check commands pass. The structural audit command completes and reports repository-wide existing findings: 2151 scanned files, 1056 Rust files, 497405 Rust physical LOC, 1 error, and 126 warnings. The error is the existing `crates/arcweft-lang-sema/src/checker/expr.rs` size finding at 2560 physical LOC. The new seq08.1 trait catalog files are below review thresholds: `crates/arcweft-lang-sema/src/traits.rs` is 40459 bytes / 1197 physical LOC / 1076 code LOC, and `crates/arcweft-lang-sema/src/traits/format.rs` is 1477 bytes / 40 physical LOC / 37 code LOC.
