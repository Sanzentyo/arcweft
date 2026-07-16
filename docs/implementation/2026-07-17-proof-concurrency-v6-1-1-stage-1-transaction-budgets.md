# Proof-concurrency v6.1.1 Stage 1 transaction budgets

## Source contract and ordering constraint

The source package is
`arcweft-proof-concurrency-v6.1.1-typed-ast-proof-block-hir-runtime-identity-final-contract.zip`
with SHA-256
`1b7de5f2c10a5b29d67c72011e4272df9a76af8907fd21fe162de54809fc69ef`.
Its 20-member manifest and `READY_FOR_IMPLEMENTATION` status were inspected
again before this cut. The package fixes Stage 1 through Stage 8 as an ordered
migration and explicitly forbids exposing the final `ProofBlock` beside the
current detached syntax model.

The editing parent was main `beb5e9cfbb6b`, after the independent signed
verification-trust authority cut. Lang-01.6 release/bundle files were not
modified here.

Main has not completed the package's full Stage 1 gate, private attachment and
reconciliation (Stage 2), or the atomic public syntax switch (Stage 3).
Consequently this cut advances the required private Stage 1 substrate; it does
not publish a second typed AST, let shadow syntax enter HIR, or claim Stage 4
completion.

## Implemented safe state

- One transaction-local `GrammarBudget` is shared by all shadow parsers for a
  document. The sixty-fifth predicate/proof parameter or contract clause, the
  257th generic/where predicate, and the sixty-fifth assertion condition fail
  before their start event enters the event vector.
- General statement, expression, type, pattern, top-level item,
  identity-bearing node, and exact-diagnostic budgets are charged by the same
  event sink. Direct event-builder tests are revalidated by that owner before a
  Rowan tree is constructed.
- Per-declaration counters live on the declaration frame, so two declarations
  may each use their exact inclusive allowance without accidentally sharing a
  document-global generic, where, parameter, or clause counter.
- Every generic parameter now has its accepted identity-bearing
  `GenericParameter` wrapper and its `LifetimeParameter` or `TypeParameter`
  child. No token receives identity.
- `assert.prove(a, b, ...)` produces one structural expression list containing
  independently typed condition expressions in source order. It is no longer
  collapsed into one expression fragment.
- Predicate/proof missing name, parameter group, parameter close, malformed
  header, clause order, missing clause expression, and missing body diagnostics
  use the final package codes. The private spelling-specific contract-mode
  diagnostic was removed; those tokens now use ordinary expression recovery.

All changes remain inside `arcweft-lang-syntax`. No public syntax API, HIR,
sema, verifier, runtime-plan, core, persisted codec, release, or bundle
contract changed.

## Direct evidence

The direct predicate/proof matrix now proves:

- exact maximum and one-over failure for predicate and proof parameters;
- per-declaration exact maximum and one-over failure for generics, where
  predicates, and combined requires/ensures clauses;
- 64 independently typed assertion conditions and rejection of the 65th;
- exact canonical recovery diagnostics and zero-width missing-clause range;
- ordinary `ErrorExpression` recovery instead of a removed-form diagnostic;
  and
- byte-for-byte Rowan losslessness at every accepted exact maximum.

Validation completed on this cut:

```bash
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-syntax --lib --all-features
CARGO_INCREMENTAL=0 cargo check -p arcweft-lang-syntax --all-targets --all-features
CARGO_INCREMENTAL=0 cargo clippy -p arcweft-lang-syntax --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
git diff --check
```

The focused library result is 206 passing tests. The canonical structural
audit reports are stored in:

- `structure-audits/proof-concurrency-v6-1-1-stage-1-budgets-baseline-2026-07-17/`;
- `structure-audits/proof-concurrency-v6-1-1-stage-1-budgets-2026-07-17/`.

The fresh post-change audit scanned 3,140 files, including 1,574 Rust files,
720,634 physical Rust LOC, and 92 package manifests. It reported zero errors
and 128 existing repository-wide warnings. The new production budget owner is
below the preferred 800-LOC responsibility-module ceiling and contains no
embedded test module.

An additional workspace-wide check was attempted after the focused proof
validation:

```bash
CARGO_INCREMENTAL=0 cargo check --workspace --all-targets --all-features
```

It reached the downstream workspace crates, then stopped in the independent
Lang-01.5 profile-metadata slice: the `arcweft-lsp` test target at
`profiles/state.rs:602` still passes `ProfileId::new("test")` as a `ProfileId`,
although that constructor now returns `Result<ProfileId, IdentifierError>`.
This proof cut does not modify that unrelated owner; the focused syntax test,
check, and warning-denying clippy evidence above all pass.

## Remaining package boundary

This is not proof-concurrency v6.1.1 completion. Before the final ordinary-name
typed AST and exact `ProofBlock` can become public, the package still requires:

1. completion of every remaining current/reduced top-level family in the
   private full-document Stage 1 grammar;
2. private grammar-node reconciliation, snapshot-owned attachment, bound
   handles, and fatal rollback tests;
3. the one workspace-compiling public syntax switch that deletes detached and
   line-identity authority;
4. only then, the final predicate/proof typed wrappers and semantic context;
5. private arena HIR followed by its atomic project/sema/verifier switch; and
6. session-only runtime assertion identity plus the persisted guard/fingerprint
   boundary.

The final proof trust surface is ordinary `proof` plus
`#[verify.trusted(reason = ...)]`; this cut introduces no dedicated
`trusted axiom` syntax or HIR variant.
