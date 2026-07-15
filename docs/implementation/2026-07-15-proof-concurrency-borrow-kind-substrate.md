# Proof-concurrency typed borrow-kind substrate

## Scope

This implementation slice takes only the decision-complete reference-type
portion of `arcweft-proof-concurrency-v6-final.zip`. It is based on main
revision `8140470a0dda25adebc2985a9ea077e853c17666`.

- `arcweft-lang-syntax::types::BorrowKind` is the single owned shared/mutable
  borrow discriminator.
- `TypeRef::Ref` carries `BorrowKind` together with its optional typed lifetime
  and referent type.
- `&T`, `&mut T`, `& mut T`, and `&'asset mut T` are parsed without string
  recovery in later layers. An identifier such as `mutable` remains a shared
  referent rather than being split at the `mut` prefix.
- `arcweft-lang-sema::types::TypeKind::BorrowRef` preserves the same
  `BorrowKind`. Equality, hashing, trait matching, substitution, alias erasure,
  normalization, stable project labels, compiler fingerprints, and runtime-plan
  labels distinguish shared and mutable references.
- Semantic source labels now preserve the lifetime apostrophe and render
  mutable references as `&'asset mut T`.

## Explicit non-goals

This slice does not add prefix borrow/dereference expressions, stable
`LocalId`/`ScopeId`/expression identities, assertion syntax, predicate/proof
declarations, borrow dataflow, proof discharge, or runtime loan behavior. Those
surfaces require the decision-complete contract requested by
[`2026-07-15-seq-proof-01.1-surface-hir-identity-production-reconciliation.md`](../reviews/requests/2026-07-15-seq-proof-01.1-surface-hir-identity-production-reconciliation.md).

## Verification

- `cargo check -p arcweft-lang-sema --all-targets --all-features`
- `cargo check -p arcweft-compiler -p arcweft-runtime-plan --all-targets --all-features`
- `cargo clippy -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-compiler -p arcweft-runtime-plan --all-targets --all-features -- -D warnings`
- `cargo test -p arcweft-lang-syntax --test parser_function_signatures_and_types reference_types_preserve_shared_and_mutable_borrow_kinds -- --exact --nocapture`
- `cargo test -p arcweft-lang-sema tests::typecheck::semantic_reference_types_preserve_borrow_kind -- --exact --nocapture`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .` (`0` errors,
  `127` pre-existing/current warnings, dry-run)

The initial focused checks used `D:\git\arcweft\target`. Concurrent builds
later demonstrated that separate workspaces can overwrite same-named crate
metadata in one shared target. Final validation therefore uses the dedicated
`D:\git\arcweft-targets\proof` directory with `CARGO_INCREMENTAL=0`; that
temporary target is removed after integration.
