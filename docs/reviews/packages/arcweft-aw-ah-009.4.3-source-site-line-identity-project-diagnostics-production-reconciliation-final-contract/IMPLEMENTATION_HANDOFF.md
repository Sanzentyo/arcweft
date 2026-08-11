# Implementation handoff

## Preconditions

AW-AH-009.4.2 must be implemented and validated first, producing the exact
source-backed application ExprId, immediate coordinates, typed HirIdRef,
component spans, and executable/poison state. This contract is not authority to
recreate those facts through source parsing.

## Compiling frontiers

### Frontier 1 — AW-AH-009.4.2 implementation

Implement the prior contract and pass its focused syntax/HIR matrix. No line
identity work begins on a provisional source node.

### Frontier 2 — lower durable/private substrate

Independently mergeable when green:

- add `arcweft-id::dialogue::{DialogueLineId, DialogueTextKey,
  MAX_DIALOGUE_ID_BYTES}`;
- source Cut 1's frozen field value from that constant without changing the
  public field or behavior;
- add private HIR owner, scope, candidate, diagnostic, and transaction types;
- add direct invariant/compile-fail tests; and
- add no public successful line path yet.

### Frontier 3 — package-aware lowering

In one compiling cut:

- introduce final `HirModuleKey`/`LoweringRequest` use;
- migrate every `lower_to_hir`/document/project-loader/compiler/LSP caller;
- bind package/module/source before HIR allocation;
- retain exact HIR snapshot and source identity; and
- delete the old package-late lowering entry point rather than wrapping it.

### Frontier 4 — module-local candidates

Produce bounded candidates from the AW-AH-009.4.2 application arena. They are
private, unaccepted, and do not change successful public runtime/line lookup.
Delete speaker-derived generation inside module lowering as soon as no
successful consumer remains.

### Frontier 5 — project transaction

Add `HirProjectBuilder::finish`, canonical package-qualified modules, collision
rejection, immutable accepted inventory, and direct tests. This may be prepared
privately but must not coexist as a second public successful project builder.

### Frontier 6 — atomic public replacement series

Keep this series unmerged until all steps compile together:

1. switch `HirProject` construction to the final builder;
2. switch accepted project publication to the returned exact project;
3. switch sema/source index/reference/rename facts;
4. switch runtime-plan/verifier/compiler inputs;
5. switch LSP/tooling/Agent/MCP/CLI queries and diagnostics;
6. migrate fixtures/examples/docs; and
7. delete old identity, helpers, errors, linked/parallel inventories.

No reviewable push point may leave the workspace uncompilable or two successful
models available.

### Frontier 7 — deletion proof

Use exhaustive compilation and compile-fail API tests to prove removed types and
constructors are unavailable. Ordinary parser/method-resolution tests prove
`.say` does not execute. Do not use repository source scans.

### Frontier 8 — validation

Run, at minimum, with stable feature combinations:

```bash
cargo fmt --all -- --check
cargo check -p arcweft-id -p arcweft-lang-hir -p arcweft-lang-sema \
  -p arcweft-project-loader -p arcweft-compiler -p arcweft-runtime-plan \
  -p arcweft-verify -p arcweft-lsp -p arcweft-tooling \
  --all-targets --all-features
cargo test -p arcweft-id -p arcweft-lang-hir -p arcweft-lang-sema \
  -p arcweft-project-loader -p arcweft-compiler -p arcweft-runtime-plan \
  -p arcweft-verify -p arcweft-lsp -p arcweft-tooling --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
just test-workspace
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

Because Frontier 6 materially changes public HIR/project identity and reaches
runtime-plan, Agent/MCP, and LSP, also run:

```bash
just test-tier2
```

Migrate stale Tier 2 expectations to the final IDs and one accepted generation;
do not add compatibility paths to satisfy fixtures.

## Push/merge policy

Private Frontiers 2, 4, and private portions of 5 may land as coherent green
substrate. Frontier 3 is a whole-caller migration. Frontier 6 plus deletion is
one direct replacement and is pushed only when the workspace has one compiling
model and required validation for that cut is complete.

## Definition of done

Implementation is complete only when all 100 test rows pass, broad gates are
reported honestly, structural audit has no new error-level ownership problem,
all old line identity constructors are deleted, and one accepted `Arc<HirProject>`
owns all line/text/source facts.
