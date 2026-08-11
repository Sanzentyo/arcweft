# Mandatory implementation and validation order

## Phase 1 — owner and immutable inventories

1. Add `BuiltinTypeConstructor::Ref`.
2. Add `ALL`, `ENTITY_FAMILY_PROJECTIONS`, `from_type_path`,
   `argument_expectation`, and `project_entity_family` as inherent behavior.
3. Add `EntityKind::AUTHORED_FAMILIES` and `authored_type_name`; make
   `from_type_name` consume the same inventory.
4. Add `TypeArgumentKind` and migrate wrong-kind payload/ordering.
5. Add `Ref` to the existing HIR and accepted/open reservation gates.
6. Add direct enum/inventory/catalog/publication tests before resolver changes.

## Phase 2 — the single recursive resolver

1. Delete the free resolver `builtin(path)` selector and use the owner enum.
2. Replace `entity_family_argument: bool` with typed argument expectation.
3. Use the same expectation logic in unary-chain and general generic traversal.
4. Add `NodeValue::argument_kind`.
5. Extend existing `apply_entity_family_builtin` to `Ref` and call the enum’s
   inherent projection.
6. Preserve current outer/child node facts, source maps, limits, diagnostics,
   poison deduplication, and work accounting.
7. Add direct accepted/detached resolver tests, including const-int wrong kind.

## Phase 3 — consumers, with no dual path

1. Migrate the normal checker’s type-bearing surfaces to the checked result.
2. Migrate callable and entry role signatures.
3. Migrate project-index consumers and assert edge ownership.
4. Add typed LSP hover/completion/definition/rename behavior.
5. Delete the last context-free helper branch that recognizes `Ref` by spelling.
6. Do not leave a compatibility reader or fallback success while migrating.

## Phase 4 — runtime/schema boundary checks

1. Confirm runtime-plan/verify use `TypeKind::Ref` directly.
2. Add persistent authored-signature digest tests.
3. Assert no bytecode schema change.
4. Assert persisted entry data containing `Ref` remains a deterministic typed
   unsupported shape; do not add a `Named` encoding.

## Phase 5 — normal validation

Run from a clean checkout at the implementation commit:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p arcweft-lang-sema --lib nominal
cargo test -p arcweft-lang-sema --lib callable
cargo test -p arcweft-lang-sema --lib entry
cargo test -p arcweft-lang-sema --lib project_index
cargo test -p arcweft-lang-hir --lib symbol
cargo test -p arcweft-lsp --lib nominal
cargo test -p arcweft-compiler --lib persistent
cargo test --workspace --all-targets --all-features
```

Use exact package/test filters supported by the checkout if Cargo rejects a
mnemonic filter; record the concrete commands and counts.

## Phase 6 — Tier 2 validation

Run the repository’s normal Tier 2 slices affected by reference identity,
project indexing, tooling, bytecode/interface digest, and save/replay schema
policy. The required behavioral categories are enumerated in
`TEST_MATRIX.csv`; Tier 2 tests must call typed public or crate-owned APIs.

Also perform the standard structure audit required by `AGENTS.md`, recording:

- files and Rust LOC;
- largest changed files/functions;
- dependency changes (expected: none for this correction);
- forbidden architectural patterns found through typed/code review rather than
  permanent source-text gate tests;
- exact warnings and disposition.

## Phase 7 — final acceptance

The implementation is acceptable only if:

- every `TEST_MATRIX.csv` row passes;
- no production fallback reports success after failed resolution;
- no `From<&TypeRef> for TypeKind` or equivalent context-free path remains;
- accepted/open/project/external collision tests pass;
- LSP cannot rename language-owned constructor/family atoms;
- no bytecode/save schema was silently changed;
- `git diff` contains only intended production/tests/docs changes for the
  implementation, not this contract generation workspace.
