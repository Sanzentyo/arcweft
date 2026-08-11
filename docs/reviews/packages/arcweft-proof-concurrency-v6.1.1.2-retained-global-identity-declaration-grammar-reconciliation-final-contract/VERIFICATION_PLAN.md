# Verification plan

This is a prescribed implementation verification plan. This design archive does not claim these commands were run in the connector-only design session.

## 1. Package and matrix

```bash
unzip -l arcweft-proof-concurrency-v6.1.1.2-retained-global-identity-declaration-grammar-reconciliation-final-contract.zip
sha256sum -c arcweft-proof-concurrency-v6.1.1.2-retained-global-identity-declaration-grammar-reconciliation-final-contract.zip.sha256
# Verify all 18 manifest entries and the 64-zero MANIFEST.txt self rule.
# Verify TEST_MATRIX.md has exactly 184 unique IDs.
```

## 2. Private grammar and ID owners

```bash
cargo test -p arcweft-id --lib --tests
cargo test -p arcweft-lang-syntax --lib parser::retained_header_tests
cargo test -p arcweft-lang-syntax --lib parser::character_grammar_tests
cargo test -p arcweft-lang-syntax --lib parser::view_grammar_tests
cargo test -p arcweft-lang-syntax --lib parser::action_grammar_tests
cargo test -p arcweft-lang-syntax --lib parser::activity_grammar_tests
cargo test -p arcweft-lang-syntax --lib parser::signal_grammar_tests
cargo test -p arcweft-lang-syntax --lib parser::metric_grammar_tests
cargo test -p arcweft-lang-syntax --lib parser::layer_grammar_tests
cargo test -p arcweft-lang-syntax --lib parser::retained_grammar_tests
cargo test -p arcweft-lang-syntax --lib --tests
```

## 3. Public syntax and attachment switch

```bash
cargo test -p arcweft-lang-syntax --lib --tests
cargo test -p arcweft-lang-syntax --test retained_declarations
cargo test -p arcweft-lang-syntax --test attachment_identity
cargo test -p arcweft-lang-syntax --test parser_declarations_recovery_comments
cargo test -p arcweft-lang-syntax --test public_api_deletion
```

The deletion test must be compile-fail or direct API evidence for absent generic/source-less APIs; it must not inspect checked-in source text.

## 4. HIR, sema, and project

```bash
cargo test -p arcweft-lang-hir --lib --tests
cargo test -p arcweft-lang-sema --lib --tests
cargo test -p arcweft-project --lib --tests
cargo test -p arcweft-project-loader --lib --tests
```

Required focused suites cover source-slot identity, parameter/member IDs, HIR rollback/liveness, project collisions/accessibility/re-exports, View/Action facets, Character alias, Activity manifest binding, signal observable shapes, metric schema checks, Layer graph/content checks, and asset catalog generations.

## 5. Compiler/runtime/product/tooling consumers

```bash
cargo test -p arcweft-compiler --lib --tests
cargo test -p arcweft-runtime-plan --lib --tests
cargo test -p arcweft-verify --lib --tests
cargo test -p arcweft-bundle --lib --tests
cargo test -p arcweft-presentation --lib --tests
cargo test -p arcweft-view --lib --tests
cargo test -p arcweft-agent-repl --lib --tests
cargo test -p arcweft-cli --lib --tests
cargo test -p arcweft-lsp --lib --tests
```

Use the exact affected package list from `cargo metadata` if current names change; do not omit a consumer that still matches or parses generic entity syntax.

## 6. Stable workspace gates

Use one stable feature combination for the cut:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
just test-fast
just test-workspace
just test-doc
git diff --check
```

Do not alternate feature sets merely to obtain a passing cache. Record any path that requires an additional feature combination and why.

## 7. Tier 2

The public migration spans multiple crates and materially affects runtime, render/presentation, Agent, and bundle/capture-adjacent paths. Therefore run:

```bash
just test-tier2
```

Reconcile stale expectations to the final typed contract. Do not add aliases, dual readers, obsolete source forms, or duplicate runtime paths to satisfy stale tests.

## 8. Dependency and structure

```bash
cargo metadata --format-version 1 --all-features > target/retained-identity-cargo-metadata.json
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

The metadata review must prove the direction `syntax -> HIR -> sema -> runtime-plan/verify -> tooling`, keep `arcweft-core` Sans I/O, and show no syntax/HIR dependency in bundle/runtime core data layers that would invert ownership.

The structural audit is checked in under a sequence-named implementation audit directory and records exact current bytes, physical LOC, embedded test LOC, responsibilities, and dependency fan-in/fan-out.

## 9. Validation reporting

For every command record:

- exact Git commit and Jujutsu change;
- command and stable feature selection;
- pass/fail/blocked status;
- exact test counts when emitted;
- any corrected stale fixture and the production-contract reason;
- remaining failures without reclassifying them as passing.

No fabricated log, inferred pass, or historical result is presented as a current execution.
