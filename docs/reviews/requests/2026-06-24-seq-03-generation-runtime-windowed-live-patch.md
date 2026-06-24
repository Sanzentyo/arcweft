# Request 03: Generation Runtime and Windowed Live Patch

## Sequence Position

This is the third design request in the integrated execution sequence.

Submit this after `2026-06-24-seq-01-executable-runtime-core.md` has defined
executor-neutral fiber state and after Request 02 has defined patch
compatibility labels/fingerprints well enough to classify patch artifacts.

## Request

Please design true code-generational hot swap and windowed native live patch as
one coherent runtime/player model. This request intentionally combines:

- generation runtime ownership and retirement;
- mixed-generation execution in one `BundleSession`;
- host-task generation routing;
- windowed native live patch event-loop integration;
- window/session/renderer/catalog preservation rules.

The design must be concrete enough to turn into small Rust implementation cuts
with focused tests.

## Existing Request Files To Incorporate

Use these existing requests as source material, but answer them together rather
than independently:

- `docs/reviews/requests/2026-06-24-code-generational-hot-swap-design.md`
- `docs/reviews/requests/2026-06-24-windowed-native-live-patch-design.md`

Also incorporate the implemented contracts recorded in:

- `docs/implementation/integrated-execution-2026-06-24.md`
- `crates/arcweft-runtime-driver/src/swap.rs`
- `crates/arcweft-runtime-driver/src/session.rs`
- `crates/arcweft-player-native/src/patch_endpoint.rs`
- `crates/arcweft-player-native/src/windowed_patch.rs`
- `crates/arcweft-player-native/src/scene_windowed.rs`

## Why These Must Be Designed Together

Windowed live patch behavior depends on generation runtime policy:

- code-generational patches may either run mixed generations or restart inside
  the existing window process;
- host-task completion must route to the generation that emitted the task;
- old generations can retire only after fibers, tasks, and explicit pins are
  released;
- windowed state preservation depends on whether the patch mutates content,
  executable tables, runtime type ABI, adapter requirements, or renderer
  resource catalogs.

Designing windowed patching without generation rules would bake in restart
behavior that may conflict with true mixed-generation execution.

## Current Implementation Evidence

The repository currently has:

- `ProgramGeneration` fingerprints and compatibility classes;
- `SwapSession` with active/retired generation concepts;
- `BundleSession` generation pins for active runtime and host task dispatches;
- restart-required behavior for `CodeGenerational` in `BundleSession`;
- `NativePatchEndpoint` for headless/in-process live-compatible patch apply or
  session restart;
- windowed native scene loop owning its own `BundleSession`, image catalog,
  renderer state, input state, and `winit` event loop;
- `arcweft-player-native::windowed_patch` typed patch event queue and safe
  frame-boundary report model.

## Required Design Decisions

Please provide concrete answers for:

1. How should `BundleSession` represent multiple live executable generations?
2. Should `ProgramGeneration` own executor/runtime images, or should executors
   live in a separate `GenerationRuntimeTable` keyed by `GenerationId`?
3. How does each active fiber remember its executable generation?
4. How do host task dispatch and completion route back to the requesting
   generation?
5. How are new entries bound to the new generation after commit?
6. Which data is shared across generations, and which is generation-local?
7. What exact retire condition must be satisfied before old generations can be
   dropped?
8. What invariants must hold before commit, during runtime steps, and during
   host-task completion?
9. What happens when patches change adapter requirements, state/frame layouts,
   runtime type ABI, entrypoints, or host-call signatures?
10. How do generation runtime decisions interact with AWFB patch compatibility
    labels and native patch transport sidecars?
11. Which component owns active `BundleSession` in windowed mode?
12. How do patch events enter the `winit` loop?
13. At which event-loop/runtime/render boundaries may patches be prepared,
    committed, restarted, or rejected?
14. How do content-only and code-compatible patches refresh catalogs, renderer
    caches, active frames, and pending host tasks?
15. Which window, surface, renderer, input, visual clock, active flow, and
    presentation state survives each patch class?
16. How are invalid patches reported without killing the running player?

## Required Implementation Order In The Design

Please propose small compiling cuts in this order or justify a better order:

1. Add explicit generation runtime table and generation-local runtime image
   ownership.
2. Bind active fibers and host tasks to generation ids.
3. Implement new-entry binding to the active generation after commit.
4. Implement retire accounting with fiber/task/pin release tests.
5. Change `BundleSession` code-generational patches from restart-required to
   true mixed generation where supported.
6. Add unsupported-change restart behavior for adapter/runtime ABI/state layout
   changes.
7. Introduce shared windowed runtime owner for `scene_windowed`.
8. Wire typed patch events into the event loop.
9. Commit or restart only at safe frame boundaries.
10. Add renderer/catalog refresh and invalid patch report behavior.

## Tests To Specify

The design should include focused tests for:

- old fiber continues on old generation after code-generational commit;
- new entry starts on new generation;
- host task completion routes to old generation;
- old generation retires only after fibers/tasks/pins release;
- adapter or ABI changes trigger restart-required;
- windowed patch queue ordering;
- no session/catalog/renderer mutation before safe frame boundary;
- invalid patch leaves old session intact;
- content-only catalog refresh;
- restart-required patch preserves window/renderer where policy allows.

## Constraints

- Keep product players free of syntax, HIR, sema, compiler, verifier, CLI, and
  LSP dependencies.
- Keep filesystem/socket/watch transport outside `arcweft-runtime-driver`.
- Do not mutate session, catalog, or renderer resources mid-step or mid-frame.
- Preserve deterministic runtime behavior.
- Do not use `unsafe` or unstable Rust.
- Prefer typed generation and patch APIs over ad hoc maps or string ids.

## Expected Output

Please produce one design document with:

- recommended architecture;
- affected crates/modules;
- new or changed public/private types;
- generation runtime table/state machine;
- windowed runtime owner and patch event API;
- restart/live-apply behavior by compatibility class;
- renderer/catalog refresh rules;
- implementation cuts;
- test plan;
- explicit non-goals.

