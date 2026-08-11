# Source correction request

## Sequence and precedence

This is the cross-cut correction `Lang-01.3.1.2.3.1`. It follows and corrects the exact parent archives recorded in `INPUT_IDENTITIES.md`. It is required before production implements either the affine public switch or the final-HIR View execution public switch.

The direct user decision is authoritative: the AWBC ABI number remains **1** because the wire is unreleased, has no external consumer, and was not formally frozen. Do not preserve ABI 2 as an alias, compatibility reader, migration state, or documentation label.

## Required corrections

1. Reconcile the sole generic affine `RuntimeValue` owner with final-HIR View value execution. Close Copy/Move/borrow behavior for every View role, retained slot, default, repeat, nested call, handler, export, and save boundary.
2. Keep ownership-complete AWBC semantics in ABI 1 with codec 8 and `CopyValue = 0x2a`.
3. Make snapshot activation exclusivity span every driver in one runtime execution domain.
4. Serialize and restore the exact affine owner allocator cursor.
5. Bind prepared drop to the exact source slot/value and remove the independent commit value parameter.
6. Resolve the `RuntimeValueSnapshotV2` `Eq` contradiction.
7. Make authored `#[static]` requirements independently visible and enforceable in View product wire.
8. Define deterministic certified ancestor/descendant fragment validation and runtime selection.
9. Provide exact Rust-shaped APIs, wire/save deltas, diagnostics, deletion inventory, compile-clean order, and positive/negative/tamper/parity/full matrices.

## Constraints

- design-only; no production patch, branch, PR, compatibility layer, source gate, or dual reader;
- preserve all parent decisions not explicitly superseded;
- keep lower/data owners Sans I/O;
- do not add a Stream-only value model, View-only ownership sidecar, second evaluator, or global static activation registry;
- do not pull later mount/Action/RichText/parser surface work into this correction.

## Expected output

One archive named:

`arcweft-lang-01.3.1.2.3.1-affine-view-abi1-reconciliation-correction-final-contract.zip`

with `OPEN_QUESTIONS.md` exactly `none\n` and explicit verification scope.
