# Proof-concurrency v6.1.1 Stage 1 declaration diagnostics

## Scope

This private-grammar cut follows Git `cebdfa4acace` and reconciles staged
predicate/proof declaration recovery with the package's canonical shared
`syntax.decl.*` diagnostic family. It changes no public parser API, production
AST, HIR, runtime, cache schema, or serialized format.

The private grammar now reports:

- `syntax.decl.missing_name` for either declaration family;
- `syntax.decl.invalid_header` for a missing or extra parameter group;
- `syntax.decl.unclosed_parameters` for a missing parameter close;
- `syntax.decl.clause_order` for `requires` after `ensures`;
- `syntax.decl.contract_mode_not_allowed` for `prove`, `check`, or `debug`
  immediately following a declaration contract keyword;
- `syntax.decl.missing_body` for either declaration family.

Contract-mode recovery retains the clause expression and anchors the diagnostic
to the exact authored mode token. The change does not reintroduce a dedicated
recognizer or diagnostic for any removed language spelling.

## Direct evidence

`declaration_contract_modes_are_retained_with_the_canonical_diagnostic` parses
both a predicate `requires check` clause and a proof `ensures prove` clause,
asserts two canonical diagnostics, checks their exact source slices, and
requires a byte-for-byte green-tree round-trip. Existing missing-name,
parameter, clause-order, body, block-synchronization, and next-item recovery
tests now assert the shared declaration codes.

## Validation

The following commands pass with `CARGO_INCREMENTAL=0`:

```bash
cargo test -p arcweft-lang-syntax parser::predicate_proof_tests --lib -- --nocapture
cargo test -p arcweft-lang-syntax --lib
cargo clippy -p arcweft-lang-syntax --all-targets -- -D warnings
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

The focused declaration grammar tests pass 28/28 and the syntax library passes
176/176. Workspace check and Clippy complete with all targets and all features
and no warning. The structural audit scans 2,982 files, 1,466 Rust files,
684,135 physical Rust LOC, and 90 manifests with zero errors and 128
repository-wide warnings.

The canonical report is stored under
`structure-audits/proof-concurrency-v6-1-1-stage-1-declaration-diagnostics-2026-07-16/`.

- `parser/predicate_proof.rs`: 17,840 bytes / 511 physical LOC, production
  declaration grammar;
- `parser/predicate_proof_tests.rs`: 31,402 bytes / 942 physical LOC, direct
  unit-test module.

Both files remain below their applicable structural warning thresholds. No
manifest, Cargo feature, dependency edge, or crate boundary changed.

## Remaining work

Stage 1 remains open. Exact hard-limit transaction behavior, complete remaining
item-family descendants, and the remaining malformed/recovery cross-products
are not claimed by this cut. Stages 2 through 8 remain open.
