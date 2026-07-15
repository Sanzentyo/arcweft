# Proof concurrency v6.1 surface/HIR identity implementation

## Accepted source contract

The implementation source of truth is
`arcweft-proof-concurrency-v6.1-surface-hir-identity-production-reconciliation-final-contract.zip`
with SHA-256
`ca518abcb7da28fe438ec3fcad52f03de6fd980402ef1d8b56c33df0d83f8525`.
All fourteen archive members and their manifest hashes were verified. The
contract reports `READY_FOR_IMPLEMENTATION` for its explicitly specified
implementation slice. The separate remaining-acceptance section below records
the repository-level decisions that still require request 01.1.1 before the
broader v6.1 sequence can be completed.

The implementation originally started from Git
`8140470a0dda25adebc2985a9ea077e853c17666` in the isolated
`codex-proof` Jujutsu workspace. Early isolated increments used
`D:\git\arcweft-targets\proof`; landing validation used the proof
worktree-local target. Incremental compilation remained disabled so parallel
package validation could not share incompatible crate metadata.
The current increment is rebased onto main `3342e9215bdc` after AW-AH-009.1.1
Character nominal registration production reconciliation. Its parent cut
permanently deleted the obsolete borrow-block AST/HIR and every caller after a
temporary direct rejection/lowering test passed and was removed.

## Character production-contract reconciliation

- Incremental lineage and exact content provenance remain separate. A
  `ParsedSource` now retains both its session-only `SourceSnapshotId` and the
  accepted `SourceDocument`; edit validation compares the revision-bound
  `SourceSpan` against that exact document identity.
- `SyntaxDatabase::parse_initial` consumes a `SourceDocument` rather than
  rebuilding provenance from a display name and raw text. Successful reparses
  create the next exact document revision with the same `SourceDocumentId` and
  a separately advanced `SourceGeneration`.
- An edit whose range is out of bounds or not a UTF-8 boundary cannot be
  constructed as a `SourceSpan`; those failures remain tested at the
  `arcweft-source` boundary. The syntax transaction directly tests ordering,
  overlap, foreign-revision rejection, stale snapshots, and atomic rollback.
- Character's semantic facades remain intact. Assertion diagnostics live in
  `diagnostics/error.rs`, and reference normalization lives in `env/base.rs`;
  no implementation was moved back into the facade files.
- `TypeKind::first_mismatch` treats shared versus mutable `BorrowKind` as the
  first owned structural mismatch before lifetime or referent traversal.
- Runtime-plan errors retain Character's exact revision-bound `SourceSpan`
  while Proof supplies the typed unresolved-proof error kind and stable
  `verify.proof.unresolved` code.

This is a direct replacement of the pre-Character in-memory integration. No
old `SourceSpan` constructor, source-name compatibility path, dual reader, or
compatibility alias was added.

## Implemented substrate

- `BorrowKind` now has its final syntax responsibility owner,
  `arcweft_lang_syntax::reference`. Callers import that module directly; no
  compatibility re-export remains in `types`.
- Shared/mutable reference identity is already preserved through semantic
  types, trait matching, compiler fingerprints, and runtime-plan labels.
- Source, syntax, and HIR session identity vocabulary is present with private
  raw fields, checked generation behavior, module-qualified HIR slots, typed
  stale-ID errors, synthetic roles, and inclusive hard-limit enums.
- Typed surface/HIR reference and assertion payloads have exact child and
  source-range accessors. Semantic assertion context, runtime policy, and fact
  class have dedicated owners.
- `SyntaxDatabase` now commits source generations and never-reused syntax IDs
  transactionally. No-op edits return the exact current snapshot, while
  byte-changing reparses use deterministic semantic-shape reconciliation that
  preserves trivia-insensitive and same-parent identities. Reconciliation
  derives block-parent nesting from the otherwise flat lossless line CST, so a
  node moved across lexical parents receives a fresh ID while a unique sibling
  reordered inside one parent retains its ID.
- The expression parser now has separate lexer, Pratt, and prefix
  responsibility modules. Prefix borrow/deref precedence, exact operator
  ranges, typed missing-operand failures, and the inclusive depth-64 limit are
  covered by direct parser tests.
- Typed assertion statements reject expression-position use, preserve
  document-absolute condition ranges, require pure Boolean conditions through
  the semantic effect graph, and create one ordered verifier obligation per
  `Prove` condition. Runtime-plan lowering emits authored-order `Check` and
  debug-profile guards, removes `Debug` condition evaluation from release
  plans, and rejects unresolved `Prove` before code generation with
  `verify.proof.unresolved`.
- The assertion-condition budget is now an inclusive syntax-transaction limit:
  exactly 64 conditions commit, while the 65th returns the typed
  `SyntaxLimit::AssertionConditions` failure without advancing the source
  generation or consuming a syntax-node identity.
- A dereference of a mutable semantic reference is an assignment place, while
  shared-reference writes and borrow-expression targets are rejected through
  the ordinary typed assignment-target diagnostic. The package explicitly
  excludes the runtime reference-storage and borrow-checking model, so this cut
  does not invent executable pointer storage beneath that typed boundary.
- The obsolete ownership block has no CST category, AST type, HIR type, lowerer,
  semantic/runtime/compiler/tooling visitor, or executable path. A temporary
  spelling-specific recognizer was used to verify that deletion, then removed;
  obsolete input now fails only through the current grammar's ordinary parser
  recovery and no historical diagnostic contract remains. The last semantic
  test fixtures using `borrow expr as name: Type { ... }` were replaced by
  current typed `let` references or removed when they duplicated the surviving
  reference/await test.
- Stable language and runtime chapters now describe lexical typed references
  and explicit `drop`; no runnable example or stable design chapter authors the
  removed ownership block.
- Direct compile-fail suites now prove that the removed `BorrowBlock`,
  `FlowItem::BorrowBlock`, `HirBorrow`, `HirFlowItem::Borrow`, and
  `LinePlanItem::Assert` APIs cannot be imported or matched. The same suites
  prove that syntax/HIR/source session identities expose no raw tuple
  constructor and implement neither Serde serialization nor deserialization.
  These tests compile against public APIs and do not inspect checked-in source.

The prior focused BorrowKind note remains the evidence for that initial slice:
`2026-07-15-proof-concurrency-borrow-kind-substrate.md`.

## Cross-package reconciliation

AW-AH-009.1 replaces the baseline `CallableSymbolTable` with one generalized
`ProjectSymbolTable`. The final integrated model will add predicate/proof
callable targets to that single authority. It will not recreate a second proof
or callable table.

AW-AH-009.1.1 `SourceDocumentIdentity` and proof v6.1 `SourceSnapshotId` are
distinct: the former binds exact content provenance, while the latter
identifies an in-memory incremental lineage. Final HIR modules must retain
both.

Exported-part HIR records remain per-module typed records. Proof v6.1 still
deletes linked/flattened HIR and `append_module_body`; project-wide exported
part aggregation will iterate `HirProjectView` without remapping IDs.

## Remaining acceptance boundary

The accepted package leaves the typed-AST identity attachment path and the
concrete `ProofBlock` representation underspecified relative to the current
parser. The standalone correction request
[`2026-07-16-seq-proof-01.1.1-typed-ast-syntax-identity-proof-block-reconciliation.md`](../reviews/requests/2026-07-16-seq-proof-01.1.1-typed-ast-syntax-identity-proof-block-reconciliation.md)
owns those two decisions. It blocks replacement of the provisional proof and
predicate declaration path, but does not block reference syntax, obsolete
borrow-block deletion, incremental identity substrate, or caller cleanup.
That request also owns the exact runtime assertion-fault identity projection:
the current core request only carries materialized condition/message/profile,
while the requested fault additionally requires final typed statement identity,
condition index, and source range.

This cut is not complete until all phases in the contract finish together:

1. implement immutable HIR arenas, live intervals, scopes, locals, captures,
   stale resolution, and the remaining syntax-limit/recovery matrix;
2. finish expression/type parser decomposition and add final reference-type,
   assertion, predicate, and proof grammar;
3. lower typed AST directly without string reparsing;
4. delete provisional proof forms, the line-plan Rust assertion shape, linked
   HIR, and every remaining caller/fixture compatibility path;
5. migrate sema, verifier, runtime-plan, compiler, CLI, LSP, tooling,
   formatter, stable documentation, and examples;
6. satisfy every test-matrix ID, compile-fail API check, focused/workspace
   command, formatting, denied-warning Clippy, `just test-workspace`, and the
   structural audit.

Until that boundary is met, this workspace is an implementation increment and
must not be described or pushed as the completed v6.1 production cut.

## Validation so far

- `cargo test -p arcweft-source` — 2 library tests passed; doc tests passed
  with no cases.
- `cargo test -p arcweft-lang-syntax --lib` — 133 passed after the new owner,
  incremental identity, reference, and assertion modules, including exact
  current-grammar rejection ranges for unsupported trailing tokens.
- `cargo test -p arcweft-lang-hir --lib` — 28 passed, including the typed
  identity cases.
- `cargo check -p arcweft-lang-sema --all-targets --all-features` — pass.
- denied-warning Clippy for source, syntax, HIR, and sema at the recorded
  compiling increments — pass.
- `cargo fmt --all -- --check` at the recorded compiling increments — pass.
- focused expression tests (8) and incremental identity tests (17) — pass.
- a temporary removed-borrow rejection check passed and was then deleted with
  its spelling-specific recognizer and diagnostic, leaving ordinary recovery;
- syntax library tests after the current-grammar parser rebase, semantic
  parent reconciliation, normative linear-storage reconciliation, foreign
  snapshot checks, assertion-position checks, invalid-edit, and
  diagnostic-limit additions — 133 passed. The prefix matrix now directly
  covers shared versus mutable borrow trivia, logical `&&`, missing operands,
  multiline closure/postfix ranges, UTF-8 byte ranges, and generic
  `syntax.expr.unexpected_token` rejection of unsupported trailing syntax;
- reference type and function-signature integration tests — 13 passed;
- semantic library tests after removing the final borrow-block fixtures and
  adding assertion purity plus mutable-dereference assignment checks — 516
  passed;
- denied-warning Clippy for syntax and sema after the current-grammar parser
  rebase — pass;
- syntax-focused denied-warning Clippy after semantic-parent reconciliation —
  pass;
- `cargo test -p arcweft-lang-syntax --lib --all-features` after adding the
  assertion-condition transaction limit — 134 passed, including exact-64 and
  one-over atomic rollback;
- `cargo test -p arcweft-lang-syntax --test public_api --all-features` — 1
  trybuild driver passed with four compile-fail cases;
- `cargo test -p arcweft-lang-hir --test public_api --all-features` — 1
  trybuild driver passed with three compile-fail cases covering removed HIR and
  non-forgeable/non-serializable session identities;
- denied-warning Clippy for syntax and HIR with all targets and all features
  after the compile-fail and assertion-limit additions — pass;
- `jj diff --git | git apply --check --reverse --whitespace=error-all -` —
  pass; this is the whitespace/applicability equivalent used because the
  isolated Jujutsu workspace has no standalone `.git` working-tree metadata;
- `cargo test -p arcweft-runtime-plan --test runtime_plan` — 65 passed,
  including assertion order/profile/release omission and unresolved proof;
- `cargo test -p arcweft-verify --lib` — 38 passed, including ordered proof
  obligations;
- `cargo test -p arcweft-compiler --test assertions` — 1 passed;
- denied-warning Clippy for syntax, sema, runtime-plan, compiler, and verifier
  with all targets and all features — pass;
- `cargo check --workspace --all-targets --all-features` after the obsolete
  borrow-block API and all downstream visitor branches were deleted — pass.
- Rebased `nowqxzku` onto Character main `3342e9215bdc`; all seven textual
  conflicts were resolved without restoring a removed-syntax recognizer or
  compatibility API.
- Focused `cargo check` for source, syntax, HIR, sema, runtime-plan, verifier,
  compiler, CLI, LSP, Agent REPL, and tooling with all targets/features —
  pass.
- Focused all-target/all-feature tests for source, syntax, HIR, sema,
  runtime-plan, verifier, and compiler — pass, including public trybuild
  contracts and the Character mismatch matrix.
- Denied-warning Clippy for those crates plus CLI, LSP, Agent REPL, and tooling
  with all targets/features — pass.
- Landing `CARGO_INCREMENTAL=0 cargo check --workspace --all-targets
  --all-features` — pass.
- Landing `CARGO_INCREMENTAL=0 cargo clippy --workspace --all-targets
  --all-features -- -D warnings` — pass.
- Landing `CARGO_INCREMENTAL=0 just verify` — pass. The first run reached the
  workspace-test build and stopped only because drive D: had no free space
  (`os error 112`). The completed Character worktree target was verified to be
  inside that worktree and cleaned with `cargo clean`, releasing 70.1 GiB; the
  unchanged command then completed successfully.
- Final formatting, diff whitespace/conflict-marker, truncation-artifact, and
  production removed-syntax scans — pass. Removed API names remain only in
  public-API compile-fail evidence, not in production recognizers, diagnostics,
  CST, AST, HIR, or runtime paths.
- Canonical structural audit after the Character rebase and landing validation
  — 2,853 files, 1,408 Rust files, 661,972 physical Rust LOC, 90 manifests, 0
  errors and 128
  warnings; reports were regenerated under
  `structure-audits/proof-concurrency-surface-identity-2026-07-15/`.
