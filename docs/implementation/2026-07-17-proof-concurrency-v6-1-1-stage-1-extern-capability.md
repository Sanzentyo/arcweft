# Proof-concurrency v6.1.1 Stage 1 external capability grammar

## Contract and safe-state boundary

This cut continues the ordered implementation from
`arcweft-proof-concurrency-v6.1.1-typed-ast-proof-block-hir-runtime-identity-final-contract.zip`
(SHA-256
`1b7de5f2c10a5b29d67c72011e4272df9a76af8907fd21fe162de54809fc69ef`).
Its editing parent is proof Stage 1 main `83d11a659ae7`.

Only the crate-private, one-pass shadow grammar changes here. The public
`ParsedSource`, current `ExternCapabilityItem`, HIR, sema, project indexes,
runtime plans, and serialized formats retain their existing sole authority.
No shadow identity enters HIR, a cache, or executable output. This is another
Stage 1 compiling cut, not the atomic syntax switch or package completion.

The previous test/bench post audit at
`structure-audits/proof-concurrency-v6-1-1-stage-1-test-bench-2026-07-17/`
is the structural baseline. A duplicate baseline report is intentionally not
committed; this cut stores only its current-checkout post audit.

## Implemented concrete family

- `Visibility? extern capability Name { ... }` now joins grouped full-document
  parsing instead of remaining one raw logical-line wrapper.
- Documentation, outer attributes, visibility, the ordinary capability name,
  the body, and real or missing brace nodes remain in one lossless token/event
  stream.
- Opaque capability `type` members own typed declaration/name children.
  Generic parameters and an authored alias target are retained structurally
  when present; their semantic acceptance remains with later layers.
- Capability `fn` members own ordinary names, generic parameters, every
  curried fixed-parameter group, typed parameter patterns and types, and an
  optional typed return type.
- `effects { ... }` owns real or missing brace nodes and independently typed
  expression children using the shared expression grammar. No raw effect text
  is reparsed.
- Member documentation, attributes, optional visibility, semicolons, and
  multiline signature/effect continuations remain lossless.

## Recovery

- Missing capability names and bodies own zero-width `MissingName` or
  `MissingBody` nodes and structured diagnostics.
- Unexpected header suffixes, member suffixes, and unsupported member forms
  own ordinary current-grammar `ErrorNode` or `ErrorItem` recovery.
- An unbraced effect clause receives missing delimiter nodes without consuming
  the next member.
- An unclosed effect clause synchronizes before the following `type` or `fn`.
  A declaration-indentation fallback keeps the final outer `}` owned by the
  capability body rather than incorrectly donating it to the nested effect.
- An unclosed capability body synchronizes before the following unindented
  top-level declaration, which remains a sibling item.

Every direct recovery fixture reconstructs the exact source bytes. No removed
syntax spelling, compatibility branch, source gate, or historical kind was
introduced.

## Explicit design non-goal

The canonical language chapter lists `CapabilityPolicyDecl` without defining
its production, typed payload, ownership, semantics, diagnostics, or runtime
presence. This cut does not guess a spelling or preserve a raw string.

The independently throwable follow-up request is
[proof-concurrency v6.1.1.1 capability policy declaration final contract](../reviews/requests/2026-07-17-proof-concurrency-v6.1.1.1-capability-policy-declaration-final-contract.md).
It requires an evidence-based keep/delete/derive/manifest decision plus exact
AST/HIR/sema/project/runtime participation and tests. The already verified
type/function/effect substrate must not be redesigned without a concrete flaw.

## Ownership exclusions

This cut does not modify Lang-01.2 state/reducer/Agent/entry binding,
Lang-01.3 live-source authoring, Lang-01.5 build/profile metadata, View/style
ownership, the public capability AST, HIR, sema, host adapters, or runtime
plans.

## Validation

The baseline syntax suite on `83d11a659ae7` passed 210 tests. Its previously
stored structural baseline reported 3,147 scanned files, 1,577 Rust files,
721,945 physical Rust LOC across 92 manifests, zero errors, and 128 existing
warnings.

Post-change focused evidence:

```bash
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-syntax --lib parser::extern_capability_grammar_tests --all-features -- --nocapture
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-syntax --lib --all-features
CARGO_INCREMENTAL=0 cargo check -p arcweft-lang-syntax --all-targets --all-features
CARGO_INCREMENTAL=0 cargo clippy -p arcweft-lang-syntax --all-targets --all-features -- -D warnings
CARGO_INCREMENTAL=0 cargo check --workspace --all-targets --all-features
CARGO_INCREMENTAL=0 cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
git diff --check
```

The five direct tests and all 215 syntax library tests pass. Focused check and
warning-denying Clippy pass. Workspace-wide check passes. Workspace-wide
Clippy reaches downstream crates and is blocked by one independent existing
warning outside this cut:

- `arcweft-runtime-driver/src/session/hot_swap.rs:111` assigns a cloned source
  label where the active lint requests `clone_into`
  (`clippy::assigning_clones`).

This proof worker does not modify that runtime-driver owner. The syntax-owned
Clippy command above completes with `-D warnings`.

## Structure

The canonical current-checkout report is stored under
`structure-audits/proof-concurrency-v6-1-1-stage-1-extern-capability-2026-07-17/`.
It scanned 3,152 files, including 1,579 Rust files and 722,647 physical Rust
LOC across 92 manifests, and reported zero errors and 128 existing warnings.

- `parser/extern_capability_grammar.rs`: 17,542 bytes / 534 physical LOC,
  production capability header/member/effect/recovery grammar;
- `parser/extern_capability_grammar_tests.rs`: 5,721 bytes / 157 physical LOC,
  direct unit tests;
- `parser/document.rs`: 23,780 bytes / 760 physical LOC, private document/event
  orchestration; and
- `parser.rs`: 24,317 bytes / 708 physical LOC, production parser facade.

All changed production files remain below repository warning thresholds. This
cut changes no Cargo dependency, feature, public API, serialization contract,
or crate boundary, so dependency fan-in and fan-out are unchanged.

## Remaining ordered boundary

Stage 1 remains open. Other sufficiently designed retained declaration
families and their malformed/recovery cross-products still require direct
typed descendants. The split capability-policy contract remains a design
non-goal until returned. Only after Stage 1 closes may the package proceed to
private attachment/reconciliation, the atomic public syntax switch, and final
predicate/proof typed wrappers and `ProofBlock`. No partial HIR identity
migration is permitted before those gates.
