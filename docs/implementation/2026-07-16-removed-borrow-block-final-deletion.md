# Removed borrow-block final deletion

## Scope

This cut removes the unpublished `borrow expr as name: Type { ... }` ownership
block from main. It follows the repository policy that a removed spelling may
have a dedicated recognizer or test only long enough to prove it cannot lower
or execute; the final parser must retain no historical contract.

The deletion removes:

- CST structured-block classification and parser dispatch;
- `BorrowBlock`, `FlowItem::BorrowBlock`, `HirBorrow`, and
  `HirFlowItem::Borrow`;
- HIR lowering, module assignment, cache projection, type-check and semantic
  analysis, verifier, runtime-plan inventory, compiler fingerprint, CLI, LSP,
  and tooling traversal branches;
- the borrow-block-only `typed_pattern_binding` helper;
- spelling-specific fixtures and stable language/runtime examples.

Surviving borrow-state tests now use ordinary typed `let` bindings and explicit
`drop`. Stable documentation describes lexical typed references rather than a
special ownership block.

## Removal proof

A temporary direct test parsed the removed block, asserted ordinary parser
failure, and asserted that its recovered tree could not lower to HIR. The test
passed and was then deleted. The final tree contains no dedicated removed-form
diagnostic, recognizer, AST/CST/HIR node, or spelling-specific test.

## Validation

- temporary parser/HIR rejection test: 1 passed, then deleted;
- `cargo check --workspace --all-targets --all-features`: passed after one
  omitted cache-fact match arm was found by compilation and removed;
- `cargo test -p arcweft-lang-syntax --lib`: 79 passed;
- `cargo test -p arcweft-lang-hir --lib`: 26 passed;
- `cargo test -p arcweft-lang-sema --lib`: 512 passed after two surviving old
  fixtures were migrated to typed lexical references;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed;
- structural audit: 0 errors, 126 pre-existing warnings; exact evidence is in
  [`structure-audits/removed-borrow-block-2026-07-16`](structure-audits/removed-borrow-block-2026-07-16/README.md).

## Remaining boundary

This cut completes only removal of the obsolete ownership block. Prefix
reference/assertion/incremental-HIR implementation remains in the isolated
proof-concurrency workspace and will be rebased after the independent Character
registration implementation lands. Replacement of the provisional proof
declaration model and final typed-AST identity attachment remains owned by
[`2026-07-16-seq-proof-01.1.1-typed-ast-syntax-identity-proof-block-reconciliation.md`](../reviews/requests/2026-07-16-seq-proof-01.1.1-typed-ast-syntax-identity-proof-block-reconciliation.md).

