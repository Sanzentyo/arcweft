# Proof-concurrency v6.1.1 Stage 0 and event-builder substrate

## Source contract

The implementation source is
`arcweft-proof-concurrency-v6.1.1-typed-ast-proof-block-hir-runtime-identity-final-contract.zip`
with SHA-256
`1b7de5f2c10a5b29d67c72011e4272df9a76af8907fd21fe162de54809fc69ef`.
All 20 archive members, lexical manifest order, exact ZIP membership, extracted
membership, the zero-valued manifest self-entry, and every non-self SHA-256
matched. `OPEN_QUESTIONS.md` contains `none`, and the package status is
`READY_FOR_IMPLEMENTATION`.

This slice starts from Git `1a77efcf4bed1def1e030f269ccd3534ab96196c` /
Jujutsu change `uqyzvpuvwtxstwwxskksonrptzukwusp`, after the native physical
geometry substrate landed. The package design basis was older Git
`76d39983ad8770a87d6e81745785b6b362a381b4`; no package patch or reference
checkout was applied.

## Implemented safe state

This is a private preparatory slice, not completion of proof-concurrency cut
01.1.1.

- The complete accepted grammar node/token inventory and exact semantic child
  role inventory now have a crate-private owner under `grammar`. The current
  public line CST remains the only source-backed syntax authority.
- `SyntaxKind` owns token/structural/identity-bearing classification. No local
  extension trait or string classification was introduced.
- The accepted syntax budgets for predicate/proof parameters, combined
  contract clauses, generics, where predicates, and identity-bearing nodes are
  added to the existing `SyntaxLimit` owner.
- The existing `SyntheticRole` and `HirLimit` owners contain the accepted final
  role and allocation families instead of requiring later parallel enums.
- `AssertionMode::is_runtime_capable` and the three
  `CallableDeclarationOwner` policy methods live on their repository-owned
  enums.
- A crate-private grammar event stream and validator construct a lossless Rowan
  `GreenNode`, record identity-bearing nodes by element-index event path,
  retain zero-width missing-token metadata at its owner path, and stage typed
  diagnostics.
- Event validation rejects token/node kind misuse, multiple/nested/invalid
  roots, unbalanced nodes, non-contiguous or invalid UTF-8 ranges, misplaced
  EOF/missing tokens, invalid diagnostic ranges, and incomplete byte coverage.
  Failed construction has no database or cache side effects.

The shadow grammar is intentionally dead-code-allowed with a reason while it
is test-only. It allocates no production `SyntaxNodeId`, enters no cache, and
is not exported. That preserves the package's required safe intermediate state:
there is no public detached grammar tree beside the existing CST and no HIR
value consumes shadow identity.

## Direct evidence

The grammar tests cover:

- exact UTF-8, whitespace, comment, and CRLF round-trip;
- distinct stable event paths for same-line identity-bearing descendants;
- missing-token and diagnostic zero-byte behavior; and
- gap, balance, and kind-misuse failures.

Commands run successfully on the working change:

```bash
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-syntax --lib
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-hir --lib
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-syntax --lib grammar::
cargo fmt --all -- --check
CARGO_INCREMENTAL=0 cargo clippy -p arcweft-lang-syntax -p arcweft-lang-hir --all-targets --all-features -- -D warnings
CARGO_INCREMENTAL=0 cargo check --workspace --all-targets --all-features
CARGO_INCREMENTAL=0 cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The editing baseline structural audit scanned 2,874 files, 1,416 Rust files,
and 665,623 physical Rust LOC across 90 manifests, with zero errors and 128
repository-wide warnings. A cut-specific post-change report is stored under
`structure-audits/proof-concurrency-v6-1-1-stage-0-2026-07-16/`.
That report scanned 2,879 files, 1,420 Rust files, and 666,734 physical Rust
LOC across 90 manifests, with zero errors and the same 128 warnings.

## Remaining acceptance work

The active proof-concurrency package remains open. In the package's mandatory
order, the remaining work is:

1. complete the one-pass full-document grammar parser for every existing
   family and satisfy the complete private Stage 1 gate;
2. implement private grammar-node reconciliation and typed attachment;
3. perform the atomic public syntax switch and migrate every syntax consumer;
4. replace the provisional predicate/proof/trusted surface with the final
   ordinary-name grammar and exact `ProofBlock`;
5. implement the private immutable HIR database, arenas, scopes, locals,
   captures, liveness, and lowering transaction;
6. perform the atomic HIR/project switch, one-table symbol migration, and
   delete linked/clone HIR;
7. implement assertion guard/fingerprint codecs plus the session-only runtime
   identity inventory and presentation boundary; and
8. complete all caller deletion, direct/compile-fail tests, workspace
   validation, documentation, and final structural evidence.

No Stage 1, syntax switch, HIR switch, runtime boundary, or package-completion
claim is made by this note.
