# Request: Integrated Execution Design Bundles and Sequencing

## Request

Please use this request as the umbrella design plan for the remaining
integrated execution work. Several existing request files describe individual
gaps, but some of those gaps must be designed together because they share
runtime contracts, fingerprints, ABI rules, or player state transitions.

Do not treat the existing request files as independent implementation tickets
when this document says they belong in the same design bundle.

## Existing request files covered

- `docs/reviews/requests/2026-06-24-awbc-executable-compact-table-design.md`
- `docs/reviews/requests/2026-06-24-product-resource-section-codecs-design.md`
- `docs/reviews/requests/2026-06-24-patch-target-manifest-signature-design.md`
- `docs/reviews/requests/2026-06-24-code-generational-hot-swap-design.md`
- `docs/reviews/requests/2026-06-24-windowed-native-live-patch-design.md`
- `docs/reviews/requests/2026-06-24-persistent-compiler-query-cache-design.md`
- `docs/reviews/requests/2026-06-24-layout-fit-mode-coordinate-contract.md`
- `docs/reviews/requests/2026-06-24-layout-units-text-fitting-and-shared-capture.md`

This request also includes the remaining design gaps recorded in
`docs/implementation/integrated-execution-2026-06-24.md` for:

- full-script AOT/JIT lowering beyond the `arcweft-runtime-codegen` contracts;
- Agent REPL overlay modules, transactional cell commits, generation-aware
  bindings, and background JIT warm commands.

## Design Sequence

### 1. Design Bundle A: Executable Runtime Core

Design these together:

- AWBC executable compact table;
- runtime executable IR / executor-neutral fiber state;
- baseline full-script codegen ABI and safe-point contract.

Primary existing request:

- `2026-06-24-awbc-executable-compact-table-design.md`

Additional implemented context:

- `arcweft-core::awbc` already owns first-pass executable table data/verifier
  contracts.
- `arcweft-runtime-codegen` already owns executor policy, runtime-code cache
  keys, safe-region contracts, frame layouts, and structured compiled-step
  exits.

Why these must be designed together:

- Compact bytecode, compiled regions, and the VM must agree on frame layout,
  resume points, host-call/effect ABI, suspension state, traps, and source-map
  identity.
- Codegen cannot be designed independently from the VM fallback boundary.
- Persistent bytecode-unit cache keys and code-generational fingerprints will
  be derived from this executable representation.

Required decisions:

- complete AWBC v1 opcode/table schema;
- canonical binary codec and decode budgets;
- lowering from `RuntimePlan` / `FlowOp` / runtime expressions;
- compact VM execution model;
- executor-neutral `FiberState`, frame layout, resume point, and safe-point
  ABI;
- verifier rules beyond index bounds;
- baseline compiled-region ABI, `CompiledStepExit` mapping, VM fallback rules,
  and host-local code cache keys;
- criteria for removing structured `BytecodeProgram` from product AWBC
  payloads.

Expected output:

- one coherent executable-runtime design document;
- ordered implementation cuts;
- focused parity and differential tests against the existing structured VM;
- explicit non-goals for Tier 2 optimization, browser Wasm AOT, and
  speculative deoptimization.

### 2. Design Bundle B: Product Resource Sections, Patch Fingerprints, and Signing

Design these together:

- compact product resource section codecs;
- patch compatibility fingerprints for each section family;
- patch target materialization, manifest rewrite, signatures, and release
  manifest interaction.

Primary existing requests:

- `2026-06-24-product-resource-section-codecs-design.md`
- `2026-06-24-patch-target-manifest-signature-design.md`

Why these must be designed together:

- Section schemas determine descriptor digests, content roots, and patch
  compatibility classification.
- Manifest rewrite and signing policy must know which logical resources changed
  and whether the result is content-only, code-compatible, code-generational, or
  restart-required.
- External payload descriptors and release manifests must use the same typed
  resource identity and digest rules as the section codecs.

Required decisions:

- prioritized product section migration order;
- canonical schemas for runtime types, entrypoints, adapter requirements,
  content catalogs, display catalogs, source maps, resource catalogs, contracts,
  debug symbols, and graph/entity indexes;
- string/public-id/cross-section reference encoding;
- section-specific decode budgets and validation rules;
- section-to-patch compatibility classification;
- target manifest rewrite rules;
- signature disposition rules for dev, CI, release, and offline inspection;
- external payload and release manifest fetch/reference rules;
- crate ownership boundaries for Sans I/O bundle logic vs CLI/project-loader
  adapter logic.

Expected output:

- one product-artifact and patch-materialization design document;
- section-by-section implementation cuts;
- deterministic byte/golden fixtures;
- release-signing and local-dev validation scenarios;
- explicit rollback behavior for failed materialization.

### 3. Design Bundle C: Generation Runtime and Windowed Live Patch

Design these together:

- true code-generational hot swap;
- windowed native live patch.

Primary existing requests:

- `2026-06-24-code-generational-hot-swap-design.md`
- `2026-06-24-windowed-native-live-patch-design.md`

Why these must be designed together:

- Windowed live patch must know whether code-generational patches are applied
  as mixed generations or restart in the same window process.
- Runtime generation ownership, host-task routing, and retire conditions decide
  which window/session/catalog/presentation state can survive a patch.
- The native patch event queue already exists, but event-loop commit/restart
  behavior depends on the generation runtime policy.

Required decisions:

- `BundleSession` representation for multiple executable generations;
- generation-runtime table ownership and runtime image lifetime;
- fiber/task/host completion generation tickets;
- exact retire conditions and explicit pin behavior;
- shared vs generation-local data for bytecode/runtime types/content/display
  catalogs/source labels/presentation state;
- handling of adapter requirement, runtime type ABI, state layout, host-call
  signature, and entrypoint changes;
- windowed runtime owner type and patch event injection API;
- safe event-loop boundaries for preparation, commit, restart, renderer cache
  invalidation, and catalog refresh;
- invalid patch reporting without killing the player;
- one-shot transport sidecar vs live stream behavior.

Expected output:

- one shared generation/windowed patch architecture document;
- separate but compatible implementation cuts for runtime-driver and native
  player;
- Sans-GPU unit tests for queue ordering, safe boundaries, restart decisions,
  and invalid-patch preservation;
- later smoke validation plan for actual windowed `arcw run --watch`.

### 4. Design Bundle D: Persistent Compiler Query Cache

Design after Bundle A, and after the section/fingerprint decisions from Bundle
B are stable enough to define output artifact identities.

Primary existing request:

- `2026-06-24-persistent-compiler-query-cache-design.md`

Why this should not be designed first:

- Bytecode-unit and link-plan reuse depend on the AWBC/runtime IR design.
- Bundle-section reuse depends on product section schemas and content roots.
- Fine-grained semantic reuse must not be claimed until module-aware sema
  actually exists.

Required decisions:

- `.awbo` payload codecs for parse/interface/HIR body/line-task evidence/
  runtime-plan/bytecode/link-plan artifacts;
- which payloads are stable across compiler versions vs exact compiler-identity
  only;
- read-through/write-through policy from in-memory watch cache to `.awci`
  record to `.awbo` object;
- validation and soft-miss behavior;
- `BuildSnapshot` query reuse evidence;
- `arcw cache explain` output;
- safe stages to skip before module-aware sema.

Expected output:

- one persistent query-cache design document;
- implementation cuts that start with parse/HIR-body exact-compiler artifacts
  and avoid premature typecheck/runtime-plan reuse;
- clean/repeated build equivalence tests.

### 5. Design Bundle E: Agent REPL Runtime Tiers

Design after Bundles A and C. This can reference Bundle D for cache behavior,
but should not depend on persistent cache being implemented.

No standalone request file currently covers this whole bundle; use this section
as the request seed.

Why this comes later:

- REPL cell commits need runtime generation semantics.
- Background JIT warm commands need the full-script codegen ABI.
- Project-bound bindings need generation and program-hash invalidation rules.

Required decisions:

- overlay module model for REPL cells;
- transactional parse/HIR/sema/effect/verifier/commit pipeline;
- generation-aware binding invalidation;
- immediate VM execution with optional background codegen;
- `:warm`, `:codegen`, `:generations`, `:cells`, `:undo`, and `:reset`
  command semantics;
- host capability/effect policy before commit;
- trace/read-only mode that never executes cells;
- use of `AgentControllerExecutorFactory` for dev/REPL tier selection.

Expected output:

- one Agent REPL runtime-tier design document;
- implementation cuts that keep product policy bytecode-VM-first;
- tests for rollback, binding invalidation, generation changes, and nonblocking
  JIT readiness.

### 6. Independent Design Bundle F: Layout, Capture, and Visual Goldens

This can be designed independently from Bundles A-E, but the two existing
layout requests should be handled together.

Design these together:

- layout fit-mode coordinate contract;
- typed layout units / text fitting / shared capture / visual golden policy.

Primary existing requests:

- `2026-06-24-layout-fit-mode-coordinate-contract.md`
- `2026-06-24-layout-units-text-fitting-and-shared-capture.md`

Why these should be bundled:

- Coordinate systems, layout units, text fitting, capture metadata, hit testing,
  and visual golden tolerances all describe the same presentation-space
  contract.
- Designing them separately risks mismatched design/output coordinates or
  renderer-specific capture behavior.

Required decisions:

- design-space vs output-space coordinate ownership;
- layout unit resolution across HIR/sema/runtime-plan/UI/renderer/Agent observe;
- text fitting and overflow semantics;
- shared capture metadata and selected-object/layer capture behavior;
- platform-font and GPU/CI visual golden policy.

Expected output:

- one presentation-layout/capture design document;
- implementation cuts that keep `arcweft-layout` Sans I/O;
- deterministic metadata tests plus explicit visual smoke/golden policy.

## Global Sequencing Summary

Recommended order:

1. Bundle A: executable runtime core.
2. Bundle B: product resource sections, patch fingerprints, materialization,
   and signing.
3. Bundle C: generation runtime and windowed live patch.
4. Bundle D: persistent compiler query cache.
5. Bundle E: Agent REPL runtime tiers.
6. Bundle F: layout/capture/visual goldens can run in parallel with 1-5.

Bundle B may start while Bundle A is being finalized, but it must not finalize
bytecode/runtime-type section fingerprints until Bundle A is stable. Bundle C
must not finalize mixed-generation behavior until Bundle A defines
executor-neutral fiber state. Bundle D must not claim bytecode-unit or
link-plan reuse until Bundle A is stable.

## Required Response Format For Each Bundle

For each design bundle, provide:

- recommended architecture;
- affected crates/modules;
- new or changed public/private types;
- crate ownership and dependency-boundary rules;
- state machines or data schemas where relevant;
- exact implementation order in small compiling cuts;
- focused tests for each cut;
- smoke/CLI validation commands;
- explicit non-goals and remaining design risks.

## Current Implementation Boundary

Until the relevant bundle design is answered, implementation should not:

- replace structured bytecode execution with compact AWBC execution;
- delete structured `BytecodeProgram` from product AWBC payloads;
- invent local compact opcodes, resource codecs, fingerprints, or state
  migration rules from current test pressure;
- wire a live patch stream into an already running windowed `winit` loop;
- claim true mixed-generation hot swap;
- claim cross-invocation compiler query reuse for parse/HIR/typecheck/runtime
  stages;
- implement REPL overlay modules or background full-script JIT warm commands.

Implementation may continue to keep the already-added contracts, event queues,
local/dev patch materialization, and bytecode-VM fallback behavior.
