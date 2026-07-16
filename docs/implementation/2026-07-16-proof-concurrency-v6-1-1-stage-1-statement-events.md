# Proof-concurrency v6.1.1 Stage 1 statement events

## Scope

This private Stage 1 slice follows Git `b52ad840ed29` and advances the shared
statement grammar required by proof-concurrency v6.1.1. It does not change the
public parser, typed AST, HIR, semantic checker, compiler, or runtime, and it
allocates no production syntax identity.

## Implemented ownership

The private statement parser now emits the complete final statement-kind
inventory from the one document cursor:

- ordinary and derived lets share the same nested pattern, optional type, and
  initializer authority; let-else owns its divergent block;
- assignments and lifetime-set statements retain distinct target and value
  expressions;
- return/out/goto/defer/yield/signal/close/select/break/continue, wait, on,
  assertions, proof calls, and ordinary expression statements retain typed
  children;
- if/else-if/else, loop, while, while-let, for, match, thread, defer-block, and
  unsafe-lifetime statements own source-backed nested blocks;
- if-let/while-let/for heads own their nested patterns, scrutinees, and guards;
- match statements own identity-bearing match arms, patterns, optional guards,
  expression bodies, and braced bodies; and
- malformed current-grammar statements become `ErrorStatement` without
  consuming a following sibling.

Predicate/proof blocks and nested control blocks reuse one braced-block event
authority. Every token is emitted once in source order, and the validated green
text remains byte-for-byte equal to the source. The shared statement inventory
is exercised through a generic block. Predicate/proof blocks admit only their
final let/assertion/proof-call/error surfaces; other terminated statement
families become ordinary `ErrorStatement` recovery. Assertion mode and proof
call resolution remain later semantic context checks.

No removed form, historical kind, spelling-specific diagnostic, source gate,
or compatibility shim was added.

## Direct coverage

Private grammar tests cover:

- simple let, assignment, lifetime set, transfers, wait/on, proof-call, and
  expression statements;
- if/else, while, while-let with guard, for, loop, match arms, thread,
  defer-block, and unsafe-lifetime blocks;
- every derived let kind and let-else block ownership; and
- malformed-statement recovery followed by an intact proof-call sibling.

## Validation

The following commands pass with `CARGO_INCREMENTAL=0`:

```bash
cargo test -p arcweft-lang-syntax --lib
cargo test -p arcweft-lang-syntax parser::predicate_proof_tests --lib
cargo clippy -p arcweft-lang-syntax --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
git diff --check
```

The library suite contains 157 passing tests. The combined landing validation
completed successfully in 94.4 seconds.

## Structure

The canonical report is stored under
`structure-audits/proof-concurrency-v6-1-1-stage-1-statement-events-2026-07-16/`.
It scanned 2,935 files, 1,455 Rust files, 679,733 physical Rust LOC, and 90
manifests with zero errors and 129 pre-existing repository-wide warnings. No
warning names an in-scope file.

- `parser/statement.rs`: 22,836 bytes / 684 physical LOC / 652 code LOC,
  production, hand-maintained, no embedded tests;
- `parser/predicate_proof_tests.rs`: 12,660 bytes / 359 physical LOC / 339 code
  LOC, test, hand-maintained.

The statement responsibility module remains inside the package's explicit
450-800 LOC target.

The predicate/proof context correction and its current metrics are recorded in
`2026-07-16-proof-concurrency-v6-1-1-stage-1-statement-context-correction.md`.

## Remaining boundary

This is not the complete private grammar gate. Detailed expression control
families and recovery, remaining item families, depth-zero multiline
ownership, transaction limits, and all later attachment/public syntax/HIR/
project/runtime stages remain open.
