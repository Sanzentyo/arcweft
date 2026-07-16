# Proof-concurrency v6 production integration audit

Date: 2026-07-15

## Package and repository basis

- Package: `arcweft-proof-concurrency-v6-final.zip`
- Package SHA-256:
  `d8573d91e21a1ebef052adb2eaeb0ce1f6c14328b7e55c50676c638c74759529`
- Package repository basis:
  `ec20509c910f45e8299f70c2f87cae5b568fb375`
- Package-declared status: `audited_design_and_reference_implementation`
- First production integration head after the safe cuts:
  `511cfe814608d93772f9693e04f7ff79a79b0ece`

The archive is not a production overlay. It deliberately contains independent
reference crates, schemas, examples, and eleven design requests; it reports
`rust_compile: not_run_no_toolchain` and `production_integration: not_applied`.
Reference source must therefore be reconciled with current crate ownership and
cannot be copied into the workspace as a second runtime or safety authority.

## Production-safe slices completed

Two defects had one unambiguous target behavior, existing production owners,
and direct behavioral tests:

1. `f73a33d9cc4a5b5dc364251e35f39f33d715df90` keeps scheduler progress
   nonterminal and broadcasts it to current joined waiters without removing
   joinable in-flight work.
2. `511cfe814608d93772f9693e04f7ff79a79b0ece` makes structured live-line delay
   triggers use accumulated logical elapsed time from activation, matching the
   existing AWBC executor.

The focused tests, workspace check and Clippy, fast suite, formatting, diff
check, and structural audit for these slices are recorded in their individual
implementation notes.

## Why the remaining cuts are not isolated compiler-error work

The rest changes authorities and serialized execution state. A compiling
choice can still be semantically wrong, and ordinary tests cannot select among
the missing public contracts:

- cuts 1-3 choose syntax, stable HIR identity, move paths, and the shared CFG;
- cuts 4-7 choose proof evidence, resource authority, borrow functionalization,
  and effect algebra used for parallel admission;
- cut 8 chooses task-scope ownership and capture/delegation transfer;
- cut 9 replaces task-equals-awaiter with work plus subscriptions, adds an
  inbox, attempt generations, and explicit AwaitMany lanes;
- cut 10 replaces line traversal and one-fiber checkpoints with persistent
  line graph and global execution state;
- cut 11 changes AWBC slot semantics, artifacts, tooling, and the final
  migration/deletion boundary.

In particular, changing current cancellation, `AwaitMany` result slots, line
sequence/parallel traversal, or AWBC `Move` alone would memorialize a temporary
hybrid that the target contract explicitly removes.

## Required production order

Use the package's sequence as a dependency order, not as permission to apply
all topics concurrently:

1. surface grammar and stable HIR identity;
2. Copy/Move/mutability and move-path dataflow;
3. shared semantic CFG and proof facts;
4. generic proof kernel and backend evidence;
5. affine resource kernel and partition laws;
6. borrow/prophecy functionalization and closures;
7. effects and parallel admission;
8. structured fibers and task scopes;
9. work/subscription scheduler, inbox, and AwaitMany;
10. persistent line graph, global checkpoint, replay, and hot swap;
11. AWBC/runtime/tooling/artifact migration and deletion of provisional paths.

Design work may overlap only when it consumes the same accepted earlier API.
Production changes must remain sequential at shared HIR, sema, runtime-plan,
core, scheduler, compiler, and LSP owners. Character registration and
exported-part/source-identity work also touch HIR/sema identity, so proof cut 1
must consume whichever of those contracts has landed rather than recreating a
parallel source-name or project-symbol authority.

## Next independent handoff

The package's request 01 was a broad topic list, not an exact production API.
Its 01.1 reconciliation produced a safe implementation slice that was rebased
onto Character 009.1.1, fully validated, and pushed as Git `5a36cd0af830` / JJ
`nowqxzku`. That cut establishes source/syntax/HIR session identity vocabulary,
reference syntax and borrow kind, incremental reconciliation, and typed
assertion substrate without restoring the removed ownership block.

Cut 1 is not complete. Use the standalone
[`seq-proof-01.1.1 typed-AST identity and proof-block reconciliation request`](../reviews/requests/2026-07-16-seq-proof-01.1.1-typed-ast-syntax-identity-proof-block-reconciliation.md)
next. It owns the remaining lossless-node/typed-node attachment, proof-block,
HIR arena/transaction, and assertion-fault identity decisions. Do not throw
production cuts 2-11 until that contract is accepted and the remaining cut-1
implementation passes its completion matrix. Later follow-up request numbers
must preserve the sequence (`02.1` through `11.1`) and must not require the ZIP
to be understandable.

## Remaining verification boundary

No claim is made that the reference crates compile or that schemas are accepted
production formats. Each later cut must add direct behavioral/codec/compile
evidence, run the applicable focused suites, and pass workspace check, workspace
Clippy, formatting, diff check, and structural audit before its own push.
