# Seq05.0 Agent REPL Runtime Tiers Dispatch

This note records application of
`arcweft-seq05.0-agent-repl-runtime-tiers-dispatch-2026-06-28.zip`.

## Applied Scope

Seq05.0 is documentation-only. It contains no production Rust implementation.

Applied the revised request files:

- `docs/reviews/requests/2026-06-28-seq-05.1-repl-overlay-cell-transaction-package.md`
- `docs/reviews/requests/2026-06-28-seq-05.2-repl-commands-agent-runner-package.md`
- `docs/reviews/requests/2026-06-28-seq-05.3-repl-executor-tiering-warm-codegen-package.md`

## Accepted Split

Keep seq05 as three independently applicable packages:

| Package | Owner boundary |
| --- | --- |
| seq05.1 | REPL session/overlay/transaction substrate, rollback invariants, binding/generation evidence, and immediate committed-cell bytecode VM execution. |
| seq05.2 | Typed user command parsing, command dispatch, read-only trace policy, Agent runner/session command integration, and deterministic command evidence. |
| seq05.3 | REPL/dev executor tier policy, `:warm` / `:codegen` handlers, non-blocking status, invalidation, and VM fallback. |

The main boundary correction is that committed-cell VM execution belongs to
seq05.1. A cell transaction is not complete until the accepted cell can execute
through the VM-first path and publish deterministic evidence.

## API Direction

Seq05.1 should prefer a new tooling-layer crate:

```text
crates/arcweft-agent-repl/
```

Seq05.2 must call seq05.1 public APIs for `:cells`, `:undo`, `:reset`,
capability/generation inspection, and cell submission. It must not inspect
transaction internals.

Seq05.3 should consume seq05.1 executable snapshots and invalidation tokens,
plus seq05.2 typed command hooks. It must not redesign the REPL state machine or
add source-level tiering syntax.

## Submission Order

Implementation order should be sequential:

1. seq05.1: substrate crate/API, transaction phases, rollback, immediate VM execution.
2. seq05.2: typed command parser/dispatch and Agent runner/session integration.
3. seq05.3: tier policy, `:warm`, `:codegen`, background status, invalidation, VM fallback.

Design for seq05.2 and seq05.3 may run in parallel if they treat seq05.1 APIs
as input boundaries.

## Validation

Executed for this docs-only application:

```bash
git diff --check
```

No Rust build/test was required because this package does not add production
source.

## Non-Goals

- No production Rust implementation in seq05.0.
- No JIT/AOT/source syntax changes.
- No product-player dependency changes.
- No persistent cache requirement.
