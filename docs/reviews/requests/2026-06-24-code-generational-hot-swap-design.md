# Request: True Code-Generational Hot Swap Design

## Request

Please design the implementation path for true code-generational hot swap in
Arcweft. The current incremental hot-swap work has the patch/container/session
foundation, but it intentionally stops short of running old and new executable
generations side by side.

The design should be concrete enough to turn into small Rust implementation
cuts with focused tests.

## Why this needs a decision

`docs/implementation/incremental-hot-swap-bundle-2026-06-23.md` records true
code-generational execution as remaining work:

- old fibers/tasks should continue against old executable tables;
- new entries should start on the new generation;
- old generations should retire only after all fibers, host tasks, and explicit
  pins are released.

The current code has useful pieces but no explicit implementation design for
the full execution model:

- `arcweft-runtime-driver::swap::ProgramGeneration` records generation
  fingerprints and compatibility classes.
- `SwapSession` owns active/retired generations and quiescent commit/retire
  phases.
- `BundleSession` pins generations for the active runtime fiber and
  outstanding host task dispatches.
- `BundleSession::hot_swap_bundle` currently reports
  `SwapCompatibility::CodeGenerational` as restart-required instead of keeping
  multiple executable tables active.
- The native patch endpoint can restart for code-generational or
  restart-required patch artifacts, but it does not yet run mixed executable
  generations in one long-lived player process.

This means the repository states the desired behavior, but does not yet answer
the ownership, routing, and retirement questions needed for implementation.

## Design questions

Please propose concrete answers for:

1. How should `BundleSession` represent multiple live executable generations?
   Should each `ProgramGeneration` own an executor/runtime image, or should
   executors live in a separate generation-runtime table keyed by
   `GenerationId`?
2. How should the active flow fiber remember the executable generation it is
   running on?
3. How should host task dispatch and completion route back to the generation
   that requested the task?
4. When a new entry starts after a code-generational commit, how is it bound to
   the new generation while old fibers/tasks remain on the old one?
5. Which data can be shared across generations, and which must be generation
   local? Include bytecode tables, runtime type tables, adapter requirements,
   content catalogs, display/image catalogs, source labels, and presentation
   state.
6. What is the exact retire condition for old executable generations?
7. What invariants should be enforced before commit, during runtime steps, and
   during host-task completion?
8. What should happen when a code-generational patch changes adapter
   requirements, state layouts, runtime type ABI, entrypoints, or host-call
   signatures?
9. How should this interact with AWFB patch compatibility labels and native
   player patch transport sidecars?

## Constraints

- Keep `arcweft-core` Sans I/O.
- Keep product players free of syntax/HIR/sema/compiler dependencies.
- Do not reintroduce parser/compiler compatibility shims or removed source
  syntax.
- Preserve deterministic runtime behavior.
- Do not use `unsafe` or unstable Rust.
- Prefer typed APIs over stringly generation ids or ad hoc maps.
- Keep generation ownership in runtime-driver/player layers; lower-level
  bundle/container crates should continue to expose data and verification
  models only.

## Expected output

Please provide:

- a recommended architecture;
- affected crates/modules;
- new or changed public/private types;
- the step-by-step implementation order;
- focused tests for each step;
- failure/restart behavior for unsupported generational changes;
- any structural decomposition needed before implementation.

## Useful current evidence

Start with these files:

- `crates/arcweft-runtime-driver/src/swap.rs`
- `crates/arcweft-runtime-driver/src/session.rs`
- `crates/arcweft-runtime-driver/tests/session.rs`
- `crates/arcweft-player-native/src/patch_endpoint.rs`
- `crates/arcweft-bundle/src/patch.rs`
- `docs/implementation/incremental-hot-swap-bundle-2026-06-23.md`
- `docs/05-build-and-security/packaging.md`
