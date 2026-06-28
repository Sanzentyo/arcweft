# Request 05: Agent REPL Runtime Tiers

## Sequence Position

This is the fifth design request in the integrated execution sequence.

Submit this after:

- `2026-06-24-seq-01-executable-runtime-core.md` has defined AWBC/runtime IR
  and full-script codegen ABI;
- `2026-06-24-seq-03-generation-runtime-windowed-live-patch.md` has defined
  generation runtime semantics.

This request may reference persistent cache behavior from Request 04, but it
must not depend on persistent cache implementation to work.

## Request

Please design the Agent REPL runtime-tier model. This request covers overlay
modules, transactional cell commits, generation-aware bindings, immediate VM
execution, optional background JIT/AOT warming, and REPL command semantics.

The design must be concrete enough to turn into small Rust implementation cuts
with focused tests.

## Existing Context To Incorporate

No standalone request currently covers this whole design. Use this file as the
design request and incorporate:

- `docs/implementation/integrated-execution-2026-06-24.md`
- `crates/arcweft-agent-runner/src/runner.rs`
- `crates/arcweft-runtime-codegen/src/lib.rs`
- the executable runtime core design produced by Request 01;
- the generation runtime design produced by Request 03.

## Why This Comes After Requests 01 and 03

Agent REPL runtime tiers depend on lower runtime contracts:

- transactional cell commits need executable artifact and verifier boundaries;
- generation-aware bindings need generation runtime semantics;
- background JIT warm commands need the full-script codegen ABI;
- host tasks must carry generation tickets once mixed-generation execution is
  supported;
- project-bound bindings must be invalidated by program hash/generation rules.

## Current Implementation Evidence

The repository currently has:

- Agent runtime interaction surfaces for observe/capture/query/action/resource
  handling;
- Agent controller bytecode execution through the bytecode VM;
- `AgentControllerExecutorFactory` as a default-VM executor factory hook for
  future dev/REPL tier selection;
- `arcweft-runtime-codegen` policy contracts but no full-script lowering;
- no full Arcweft language REPL overlay module or transactional cell commit
  model.

## Required Design Decisions

Please provide concrete answers for:

1. What is the base project snapshot plus REPL overlay module model?
2. How are cells represented as items/statements/expressions?
3. What is the transactional parse/HIR/sema/effect/verifier/commit pipeline?
4. What changes are rolled back on failed parse, typecheck, effect policy, or
   verifier failure?
5. How are project-bound bindings tracked and invalidated when program hash or
   generation changes?
6. How does each committed cell produce an AWBC/runtime IR region or entry?
7. How is immediate execution guaranteed through the VM before background JIT?
8. How do background JIT/AOT warm commands schedule codegen without blocking
   cell response?
9. How do host capability and effect policies apply before commit?
10. What are the semantics of `:observe`, `:step`, `:tasks`, `:cancel`,
    `:load`, `:reload`, `:warm`, `:cells`, `:undo`, `:reset`,
    `:capabilities`, `:codegen`, and `:generations`?
11. How does read-only trace mode avoid executing cells?
12. How does `AgentControllerExecutorFactory` select bytecode VM vs tiered
    executor for dev/REPL policy?

## Required Implementation Order In The Design

Please propose small compiling cuts in this order or justify a better order:

1. Add REPL session overlay data model without executing cells.
2. Add transactional parse/HIR lowering for cells.
3. Add effect/capability/verifier gate before commit.
4. Add committed-cell VM execution through the default bytecode executor.
5. Add cell listing, undo, and reset.
6. Add generation-aware project-bound binding invalidation.
7. Add `:warm` and `:codegen` commands as background requests, not source
   syntax.
8. Add tiered executor selection through `AgentControllerExecutorFactory`.
9. Add trace/read-only mode.

## Tests To Specify

The design should include focused tests for:

- failed cell rollback;
- successful cell commit and immediate VM execution;
- effect policy rejection before commit;
- generation change invalidates project-bound bindings;
- `:undo` removes only the latest committed cell effects;
- `:reset` returns to base project snapshot;
- `:warm` does not block execution;
- read-only trace mode never executes cells;
- tiered executor factory can be selected without changing host-call dispatch.

## Constraints

- Do not reintroduce source-level `jit`, `aot`, `lazy use`, or `eager use`.
- `:warm` is a REPL command, not source syntax.
- Product policy remains bytecode-VM-first.
- Keep product players free of compiler/REPL dependencies.
- Preserve deterministic execution and observation.

## Expected Output

Please produce one design document with:

- REPL overlay architecture;
- cell transaction state machine;
- generation/binding invalidation rules;
- command semantics;
- executor tiering policy;
- implementation cuts;
- test plan;
- explicit non-goals.

## Follow-Up Package Split

The original seq05 request has been split into independently throwable package
requests:

- `docs/reviews/requests/2026-06-28-seq-05.0-agent-repl-runtime-tiers-dispatch-package.md`
- `docs/reviews/requests/2026-06-28-seq-05.1-repl-overlay-cell-transaction-package.md`
- `docs/reviews/requests/2026-06-28-seq-05.2-repl-commands-agent-runner-package.md`
- `docs/reviews/requests/2026-06-28-seq-05.3-repl-executor-tiering-warm-codegen-package.md`

Use seq05.0 first when asking another designer to confirm or improve the split.
Use seq05.1 as the first implementation package after the split is accepted.
