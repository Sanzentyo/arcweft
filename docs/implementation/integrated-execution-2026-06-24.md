# Integrated execution implementation note (2026-06-24)

Source package:
`D:/sanze/Downloads/arcweft-integrated-execution-design-2026-06-24.zip`

This note records implementation cuts from the integrated execution package
that were concrete enough to apply directly. Items whose design is still
insufficient remain tracked in `docs/reviews/requests/` and are excluded from
the current completion boundary until answered.

## Implemented cuts

- Cut 1 extracted `arcweft-layout` as a Sans I/O presentation crate:
  - moved shared design/output viewport fit transform primitives out of
    `arcweft-render-wgpu`;
  - exposed raw/contain/cover/stretch fit policies and inverse mapping;
  - added layout unit expression, safe-area context, text overflow policy,
    text fitting result, and text fitting diagnostic data contracts;
  - updated native Agent observation to use the shared geometry contract.
- Cut 2 added the first `arcweft-core::awbc` executable-table contract:
  - typed AWBC function, block, register, type, constant, host-call, content,
    effect, instruction, terminator, suspend, and trap data models;
  - explicit `AwbcOpcode` v1 reserved opcode names;
  - `AwbcVerifierBudget` with table, function, block, instruction, register,
    and operand limits;
  - `AwbcProgram::verify` for ABI, duplicate function/block ids, runtime type
    references, host-call signatures, entrypoint targets, register/table index
    bounds, branch/suspend block targets, and budget failures.
- Resource codec groundwork added `arcweft-bundle::resource_codec`:
  - compact product resource section codec families and magic bytes;
  - shared section header validation for magic, schema version, decoded byte
    budget, string table count, public-id table count, and record count;
  - sorted/deduplicated string tables;
  - public-id tables that reject duplicates rather than silently deduplicating;
  - section-family mapping to existing AWFB container kinds where available;
  - patch compatibility classification for migrated resource section families.
- Persistent compiler object groundwork added
  `arcweft-project::persistent_object`:
  - compiler-private `.awbo` object kind, stability, build identity, key, and
    envelope contracts;
  - typed payload contracts for parsed syntax, interface summaries, HIR bodies,
    line-task evidence, runtime-plan units, bytecode units, and link plans;
  - canonical key digests using existing `BuildDigest` / `NamedDigest` types;
  - payload digest, payload length, magic, schema, kind, and key validation.
- Baseline runtime code-generation groundwork added `arcweft-runtime-codegen`:
  - executor selection policy for VM/native AOT/Wasm AOT under trust, profile,
    and platform capabilities;
  - target code artifact inventory and host-local runtime code cache keys;
  - callable codegen facts, backend support/fallback reasons, frame layouts,
    safe points, code regions, entrypoints, and structured compiled-step exits.
- Windowed native live-patch groundwork added
  `arcweft-player-native::windowed_patch`:
  - typed patch events for bundles, transport sidecars, and restart requests;
  - event source and restart reason contracts;
  - a FIFO queue that only pops at the `AfterRenderSubmitted` mutation-safe
    frame boundary;
  - retained patch reports for debug overlays/log/tooling without killing the
    running player on invalid patches.
- Agent controller runtime tier groundwork added
  `arcweft-agent-runner::runner::AgentControllerExecutorFactory`:
  - the default controller path still uses the bytecode VM;
  - REPL/dev policies can now supply a tiered controller executor without
    replacing Agent host-call dispatch.

## Explicit boundaries

- The existing `arcweft-core::compact_bytecode` sidecar remains in place for
  current product AWFB validation.
- Product `ProgramBytecode` sections still carry the structured
  `BytecodeProgram` required by the current VM.
- `arcweft-core::awbc` is a Sans I/O data/verifier boundary only in this cut.
  It does not implement binary AWBC product encoding, lowering from
  `RuntimePlan`/`FlowOp`, compact VM execution, patch fingerprints, or deletion
  of the structured product bytecode payload.
- `arcweft-bundle::resource_codec` is a shared contract module only in this
  cut. Product runtime-types, entrypoints, adapter requirements, content
  catalog, display catalog, and source-map sections still use their current
  product encoding until each section receives a concrete binary record codec.
- `arcweft-project::persistent_object` is a data/verifier contract only in this
  cut. `arcweft-project-loader` cache read-through/write-through still stores
  raw artifact bytes behind `.awci` records and has not yet switched parse/HIR
  query artifacts to `.awbo` payloads.
- `arcweft-runtime-codegen` is a policy and IR contract only in this cut. It
  does not implement Cranelift region lowering, executable memory, ObjectModule
  AOT emission, Wasm AOT helpers, OSR, deoptimization, or background Agent REPL
  compilation.
- `arcweft-player-native::windowed_patch` is the event and state-machine
  contract only in this cut. The already running `scene_windowed` event loop is
  not yet wired to a watch channel, file watcher, local socket, or embedding
  live patch stream.
- The Agent controller executor factory does not implement REPL overlay
  modules, transactional cell commits, generation-aware bindings, or background
  JIT warm commands.

## Design requests still excluding work

- `docs/reviews/requests/2026-06-24-awbc-executable-compact-table-design.md`
  still blocks replacing structured bytecode execution, deleting structured
  `BytecodeProgram` from AWBC product payloads, and inventing a local compact
  VM/lowering model.
- `docs/reviews/requests/2026-06-24-code-generational-hot-swap-design.md`
  still blocks true mixed-generation execution in one `BundleSession`.
- `docs/reviews/requests/2026-06-24-windowed-native-live-patch-design.md`
  still blocks wiring a live patch stream into the already running native
  `winit` scene loop and mutating renderer/session/catalog state from that
  stream.
- `docs/reviews/requests/2026-06-24-patch-target-manifest-signature-design.md`
  still blocks product-grade target manifest rewrite, target signature
  regeneration, release-manifest mutation, and automatic external payload
  fetching.

## Validation

Focused commands run for the implemented cuts:

```bash
cargo test -p arcweft-core --lib
cargo test -p arcweft-bundle --lib
cargo test -p arcweft-project --lib
cargo test -p arcweft-runtime-codegen --lib
cargo test -p arcweft-player-native --lib windowed_patch
cargo test -p arcweft-agent-runner --lib
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audit-integrated-execution-2026-06-24
just test-workspace
```

The most recent structural audit for this package reports `0` errors and `97`
warnings.
