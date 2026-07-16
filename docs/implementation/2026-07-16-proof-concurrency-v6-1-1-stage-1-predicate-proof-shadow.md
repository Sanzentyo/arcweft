# Proof-concurrency v6.1.1 Stage 1 predicate/proof shadow grammar

## Scope and safe state

This private Stage 1 slice follows the document lexer landed at Git
`b266271774f3289c28660389758741d482fd667c` and was rebased onto the View
exported-part production reconciliation at Git
`55502e49996643b2449950cea4848aba87610835`. Its source contract is
`arcweft-proof-concurrency-v6.1.1-typed-ast-proof-block-hir-runtime-identity-final-contract.zip`
(SHA-256
`1b7de5f2c10a5b29d67c72011e4272df9a76af8907fd21fe162de54809fc69ef`).

The new tree remains crate-private shadow output. Public `ParsedSource`, the
line CST, detached typed AST, HIR, caches, compiler, and tooling still use only
the existing production path. No shadow event path allocates a production
`SyntaxNodeId` or enters HIR. This note therefore does not claim the complete
Stage 1 gate, the Stage 3 public syntax switch, or the final predicate/proof
surface acceptance.

## Implemented grammar descendants

The shared source cursor now emits identity-bearing descendants for the final
predicate/proof declaration shape without parsing a source string twice:

- visibility, ordinary name or zero-width `MissingName`;
- generic group, lifetime/type parameters, distinct delimiter nodes;
- exactly one fixed parameter group, parameters, pattern/type descendants,
  and zero-width missing delimiters;
- proof return type and predicate authored-return typed recovery;
- ordered `where`, `requires`, and `ensures` nodes;
- expression bodies with typed expression-family nodes;
- distinct predicate/proof body, block, open/close delimiter, statement-list,
  statement, authored tail, and zero-width omitted-tail nodes;
- common let/assertion descendants and proof-call statement classification;
  and
- ordinary `ErrorItem` recovery for an entity-reference name after `proof` or
  `predicate`, with no removed-spelling node or diagnostic.

A second fixed parameter group is retained under ordinary current-header
`ErrorNode` recovery and reports the current malformed-header family. It never
creates a second `FixedParameterGroup`. A `requires` after `ensures` remains a
typed clause with `syntax.contract.invalid_clause_order`. Missing names,
parameter delimiters, bodies, and block closers use zero-width missing events
at the exact cursor anchor.

The declaration parser owns orchestration only. The implementation was split
immediately into `lexer`, `expression`, `pattern`, `type_ref`, `statement`, and
`shadow_recovery` responsibilities; the pre-existing public
`parser/recovery.rs` remains byte-for-byte unchanged.

## Direct evidence

Private direct tests cover:

- byte-for-byte losslessness for a complete generic proof header with tuple
  pattern/type, return, where, requires, ensures, and binary expression body;
- distinct proof block, brace, let, assertion, condition, authored tail, and
  omitted-tail families;
- ordinary `ErrorItem` recovery followed by a valid current proof;
- missing-name and missing-parameter nodes/tokens;
- second-group and clause-order diagnostics; and
- retained predicate return recovery.

The affected crate passes:

```bash
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-syntax --lib
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-syntax parser::predicate_proof_tests --lib
CARGO_INCREMENTAL=0 cargo check -p arcweft-lang-syntax --all-targets --all-features
CARGO_INCREMENTAL=0 cargo clippy -p arcweft-lang-syntax --all-targets --all-features -- -D warnings
CARGO_INCREMENTAL=0 cargo check --workspace --all-targets --all-features
CARGO_INCREMENTAL=0 cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
git diff --check
```

## Structural evidence

The canonical report is stored under
`structure-audits/proof-concurrency-v6-1-1-stage-1-predicate-proof-2026-07-16/`.
It scanned 2,919 files, 1,448 Rust files, 673,016 physical Rust LOC, and 90
package manifests with zero errors and 129 repository-wide warnings.

All new production responsibilities are below the package's 800-LOC ordinary
module ceiling: document 347 LOC, lexer 448, predicate/proof 407, statement
171, expression 133, shadow recovery 197, pattern 45, and type reference 58.
The two sibling test modules are classified as tests (141 and 158 LOC), so no
new production module contains an embedded test region. The existing parser
facade remains 678 LOC because the atomic facade split belongs to the public
syntax switch; this cut adds only private module declarations to it.

## Remaining work

The private parser still needs complete descendant grammar for every existing
item, statement, expression, pattern, and type family, plus documentation and
attribute attachment, multiline declaration ownership, exact inclusive
transaction limits, and full recovery/synchronization matrices. Expression,
pattern, and type modules currently classify the outer family and do not yet
emit every nested child required by the final attachment gate.

Only after that complete private Stage 1 gate may Stage 2 reconciliation and
typed attachment start. Public syntax/HIR switches, final predicate/proof sema,
the immutable HIR database/arenas/project migration, and runtime assertion
identity remain open in the package's mandatory order.
