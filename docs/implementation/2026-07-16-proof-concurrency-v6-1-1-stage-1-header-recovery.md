# Proof-concurrency v6.1.1 Stage 1 header and block recovery

## Scope

This private grammar cut is Jujutsu change
`mltyvuwmqvwmsxsvtqrsuxqxxxywyqvy`, based on Git `a1c9aaba2b14`.
It closes two lossless logical-line and synchronization gaps from the final
predicate/proof package.

First, an unclosed parameter group or declaration block now synchronizes at an
unindented following declaration instead of absorbing it. The broken owner
receives its existing zero-width missing delimiter and current
`syntax.*.missing_*_close` diagnostic exactly at the synchronization byte. A
following declaration with documentation and outer attributes keeps those
prefixes under its own item.

Second, `<...>` nesting in a declaration header now participates in logical
line formation. Newlines inside generic parameters and generic return types do
not terminate a logical line. The rule stops before `requires`, `ensures`, an
expression body, or a block body, so comparison operators in contract/body
expressions are not misclassified as angle delimiters. A fixed parameter group
may begin on the next logical line after a generic group.

All behavior remains inside the crate-private one-pass shadow parser. Recovery
uses current declaration kinds and current diagnostics only; it does not add a
removed-form recognizer, source gate, compatibility shim, CSS route, or Takumi
route.

## Direct evidence

Focused tests prove:

- missing `)` before `proof next()` leaves two independent `ProofItem` nodes
  and anchors the missing delimiter at the next declaration;
- missing `}` before `proof next()` has the same independent ownership;
- a documentation/attribute prefix before the next declaration is not consumed
  by the broken predicate;
- multiline generic parameters, a split fixed parameter group, and a generic
  return type form one proof with exactly four normative logical lines; and
- all fixtures round trip byte-for-byte through the green tree.

## Validation

The following commands pass with `CARGO_INCREMENTAL=0`:

```bash
cargo test -p arcweft-lang-syntax parser::predicate_proof_tests --lib
cargo test -p arcweft-lang-syntax --lib
cargo clippy -p arcweft-lang-syntax --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
git diff --check
```

The focused suite has 19 passing tests and the syntax library suite has 164
passing tests. Workspace check and Clippy complete without errors or warnings.

## Structure

The canonical report is stored under
`structure-audits/proof-concurrency-v6-1-1-stage-1-header-recovery-2026-07-16/`.
It scanned 2,943 files, 1,455 Rust files, 680,512 physical Rust LOC, and 90
manifests with zero errors and 129 pre-existing repository-wide warnings. No
warning names an in-scope file.

- `parser/document.rs`: 22,340 bytes / 698 physical LOC / 641 code LOC,
  production, hand-maintained, no embedded tests;
- `parser/predicate_proof_tests.rs`: 22,510 bytes / 682 physical LOC / 646 code
  LOC, test, hand-maintained.

Both files remain within the preferred 300-800 LOC responsibility range. The
remaining Stage 1 gap is complete nested event coverage for the expression and
other existing grammar families, followed by the package's mandatory typed
attachment and later HIR/runtime stages.
