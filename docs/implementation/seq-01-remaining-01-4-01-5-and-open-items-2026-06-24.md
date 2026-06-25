# Request 01 remaining work: 01.4, 01.5, and incomplete 01.1〜01.3 details (2026-06-24)

Base revision reviewed: `db1aafe0151350312c6cdfc53afbd856120efc90`

This document is intentionally separate from stable design chapters. It records exactly what this Request 01.1〜01.3 overlay did not complete, and what should be designed/implemented next.

2026-06-25 update: the overlay has now been applied to the current checkout and
validated locally. Compiler feedback, formatting, workspace clippy, focused
tests, `just test-workspace`, and the structural audit are no longer open
items for the overlay application itself. The remaining items below are
semantic/product follow-up work.

## Current completion boundary

Implemented in the overlay:

- compiler-side `RuntimePlan` to `AwbcProgram` lowerer API and deterministic inventory scaffolding;
- first compact AWBC VM API and instruction/terminator dispatch skeleton;
- parity observation normalization model;
- first safe Rust compiled-region eligibility and baseline region wrapper over AWBC;
- explicit non-default compact/compiled path shape.

Not implemented in the overlay:

- runtime-driver/runtime-host/native-player migration to compact AWBC execution;
- product AWFB AWBC-only payload migration;
- deletion of structured product execution;
- deletion of `arcweft-core::compact_bytecode`;
- default product selection of compact VM or compiled regions.

## 01.1 implementation items still incomplete

The lowerer is concrete enough to apply as an overlay, but the following are still open before merge:

1. **Precise CFG lowering for every `FlowOp`**
   - Current flow lowering records every family and emits executable table shape, but several complex constructs still use placeholder intrinsics to preserve structure.
   - Required completion:
     - true block-level lowering for `If`, `IfLet`, `Match`, `Loop`, `While`, `For`, `Break`, `Continue`;
     - explicit `AwbcTerminator::Dialogue`, `Choice`, `Await`, `AwaitMany`, `GotoStatic`, `GotoDynamic` emission rather than only safe-point intrinsics;
     - resume point allocation for each suspend boundary;
     - structured source-map ranges for every block and terminator.

2. **Precise expression lowering**
   - Implement branch-producing expression blocks for `RuntimeExpr::If`, `IfLet`, and `Match` rather than intrinsic placeholders.
   - Resolve named calls through the typed intrinsic/host-call registry instead of using generic stable labels.
   - Add exact lowering of method calls and spread arguments.

3. **Source/stream semantics**
   - `SourcePlan` and `StreamPlan` are materialized into AWBC tables, but source/stream handlers still need complete executable block lowering and parity fixtures.

4. **Line task semantics**
   - `LineTaskGroup` and line task nodes are represented, but line binding/out/cancel handler functions need full executable lowering instead of synthetic empty functions.

5. **Fixture corpus**
   - Add one fixture per `FlowOp` family and one fixture per `RuntimeExpr` family.
   - Snapshot canonical AWBC bytes for stable deterministic output.

## 01.2 implementation items still incomplete

1. **Full per-opcode tests**
   - Add unit tests for every non-terminator and terminator.
   - Include register initialization, scope rollback, invalid index traps, and pattern mismatch traps.

2. **Host/task/source replay harness**
   - Add scripted replay inputs for choice, await, await-many, host calls, sources, and streams.
   - Verify the same scripts can drive structured and compact executors.

3. **Structured/compact parity expansion**
   - Normalize structured `RuntimeStepResult` and compact `VmStepOutput` observations.
   - Add fixtures for dialogue, choice, await, await-many, goto, match, loop, source, trap, and budget.

4. **Compact VM resume materialization**
   - Implement host-result APIs for choice selection, await completion, await-many completion, and host-call return values.
   - Call `FiberState::resume_at` only after result shape validation.

5. **Trap source-map reporting**
   - Wire `AwbcSourceMapEntry` lookup into `FiberTrap`.

## 01.3 implementation items still incomplete

1. **Backend policy integration**
   - Connect `AwbcRegionLowerReport` to executor selection policy.
   - Add explicit dev/test selection flags only; do not make this product default.

2. **Eligibility refinement**
   - Current eligibility rejects unsupported opcodes and optionally host boundaries.
   - Required completion:
     - infer effect sets and type support per region;
     - reject dynamic targets unless a typed target table exists;
     - reject non-deterministic host boundaries for compiled execution;
     - record rejection evidence in CLI/debug output.

3. **`CompiledStepExit` validation tests**
   - Add tests for stale generation, invalid safe point, consumed-budget contract, zero-budget fallback, trap mapping, and return mapping.

4. **Artifact/cache identity**
   - Connect region lowering to canonical AWBC bytes and `RuntimeCodeCacheKey` construction.
   - Record backend revision and optimization tier in artifact metadata.

## Request 01.4 design: runtime-driver/runtime-host executor migration

### Goal

Migrate runtime-driver, runtime-host, native player, and related CLI/dev executor construction from structured `BytecodeProgram` construction to a narrow AWBC execution facade. Product payload format does not change in this cut.

### Current structured executor inventory

Known construction points to audit and rewire:

- `arcweft-core::executor::BytecodeVmExecutor`;
- CLI runtime executor templates and instances;
- `arcweft-runtime-driver::session::BundleSession`;
- `arcweft-runtime-host::bundle_runner`;
- native player paths that build or select bundle sessions;
- Agent runner/controller paths that build bytecode VM executors.

### Target facade

Introduce an explicit facade in a product-safe crate, preferably `arcweft-core::awbc::executor` or `arcweft-runtime-driver::executor` depending on final dependency review:

```rust
pub enum ArcweftExecutionTier {
    StructuredLegacy,
    CompactAwbcVm,
    BaselineCompiledAwbc,
}

pub struct ArcweftExecutionConfig {
    pub tier: ArcweftExecutionTier,
    pub allow_structured_legacy: bool,
    pub allow_compiled_fallback: bool,
}

pub trait ArcweftExecutableSession {
    fn step(&mut self, input: RuntimeStepInput, options: RuntimeStepOptions) -> RuntimeStepResult;
    fn checkpoint(&self) -> ArcweftExecutionCheckpoint;
    fn restore(&mut self, checkpoint: ArcweftExecutionCheckpoint) -> Result<(), RestoreError>;
    fn telemetry(&self) -> ArcweftExecutorTelemetry;
}
```

The facade owns selection between structured legacy, compact VM, and baseline compiled regions during migration. Product players must not call `BytecodeVmExecutor` directly once this cut lands.

### Migration flags

Temporary flags must be explicit and deletion-gated:

- `--executor structured-legacy` only for dev/test parity;
- `--executor compact-awbc` for compact VM smoke;
- `--executor baseline-compiled-awbc` for eligible compiled region smoke;
- no silent structured fallback in product code.

### Host/runtime boundary behavior

The compact facade exposes:

- dialogue and choice through normalized flow observations;
- await/await-many and host calls through typed suspension exits;
- line effects as typed effect batches;
- source/stream events through existing `RuntimeStepInput`/`RuntimeStepOutput` shapes;
- traps with source-map anchors;
- checkpoints through `FiberState` snapshots.

### Implementation cuts

1. Inventory current executor construction and dependency edges.
2. Add facade types and telemetry without rewiring callers.
3. Add structured legacy adapter behind `ArcweftExecutionTier::StructuredLegacy`.
4. Add compact AWBC adapter using `AwbcProgram` + `FiberState` + `awbc::vm`.
5. Add baseline compiled adapter using `awbc_region` and transactional fallback.
6. Rewire runtime-driver and runtime-host to the facade.
7. Rewire native player smoke paths.
8. Add parity smoke tests at runtime-driver/runtime-host boundary.
9. Remove direct product-player `BytecodeVmExecutor` construction after parity gates pass.

### Tests

- executor selection policy;
- compact dialogue/choice observation parity;
- await/await-many host exit and resume;
- source/stream event replay;
- trap/source map reporting;
- checkpoint restore;
- native/CLI compact smoke;
- dependency audit preventing syntax/HIR/sema/compiler/CLI/LSP from entering product players.

### Explicit non-goals

- product AWFB format migration;
- deleting structured product payloads;
- true mixed-generation hot-swap;
- optimized native codegen.

## Request 01.5 design: product AWFB bytecode migration

### Goal

Migrate product AWFB `ProgramBytecode` from structured `BytecodeProgram` plus old compact sidecar to AWBC-only executable payloads after 01.4 runtime selection is stable.

### Current product shape

Current product bundles still carry structured bytecode as product runtime source of truth. AWBC/compact tables are not the sole executable payload yet. Old compact sidecar exists for validation and must not be treated as product execution source.

### Target product shape

`ProgramBytecode` section becomes canonical AWBC-only:

```text
AWBC envelope
  magic: AWBC\r\n\x1a\n
  envelope_version: 2
  encoding: awbc_canonical_v1
  awbc_len: u32
  awbc_bytes: canonical AwbcProgram bytes
  awbc_digest: content identity
```

Structured `BytecodeProgram` is not embedded in new product payloads. Runtime types, entrypoints, display maps, source maps, resources, and patch fingerprints reference AWBC table identities.

### Migration compatibility policy

- New builders produce AWBC-only by default only after 01.4 compact runtime passes parity gates.
- Old structured products are accepted only through an explicitly named inspection/migration path, for example `arcw bundle inspect --allow-legacy-structured-bytecode`.
- Product players reject structured-only products unless a dev/test compatibility flag is explicitly set.
- No product path silently chooses structured execution for new AWBC-capable products.

### Diagnostics

Add structured diagnostics for:

- structured-only product rejected;
- malformed AWBC payload;
- unsupported AWBC ABI/codec version;
- missing source/display maps;
- runtime type or resource digest mismatch;
- patch target AWBC identity mismatch;
- legacy compact sidecar encountered after deletion gate.

### Deletion gates

Delete old paths only when all are true:

1. 01.4 facade is default for product runtime selection.
2. Compact VM parity fixtures pass for the required corpus.
3. AWBC-only AWFB fixture corpus decodes and executes.
4. Patch/generation identity tests use AWBC digest.
5. No product path calls `BytecodeVmExecutor` directly.
6. No product encoder writes structured `BytecodeProgram` into AWFB.
7. No product decoder requires `arcweft-core::compact_bytecode`.

### Implementation cuts

1. Inventory AWFB bytecode section encode/decode entrypoints.
2. Add explicit AWBC-only section encode/decode tests while legacy decode remains explicit.
3. Add builder option and diagnostics for AWBC primary executable section.
4. Switch default builder to AWBC-only after 01.4 parity gate.
5. Switch product players to require AWBC for new products.
6. Add old-product fixture coverage and inspection-only legacy path.
7. Delete structured product payload conversion.
8. Delete old compact sidecar.

### Tests

- AWBC-only AWFB decode;
- structured-only product rejection;
- malformed AWBC diagnostics;
- source/display/resource consistency;
- product signing/hash identity stability;
- patch/generation compatibility;
- deletion-gate grep/checks proving no product path depends on structured payloads or old compact sidecars.

### Explicit non-goals

- runtime-driver rewiring, which belongs to 01.4;
- release signing implementation beyond identity rules;
- browser/native optimized AOT generation;
- external one-shot migration tools unless separately requested.

## Validation completed after applying this overlay

Completed on 2026-06-25:

```bash
cargo fmt --all -- --check
cargo check -p arcweft-core --all-targets
cargo check -p arcweft-runtime-plan --all-targets
cargo check -p arcweft-runtime-codegen --all-targets
cargo test -p arcweft-core awbc
cargo test -p arcweft-runtime-plan awbc_lower
cargo test -p arcweft-runtime-codegen awbc_region
cargo clippy --workspace --all-targets --all-features -- -D warnings
just test-workspace
cargo +nightly -Zscript tools/arcweft-structure-audit.rs --root .
cargo +nightly -Zscript tools/arcweft-structure-audit.rs --root . --write docs/implementation/structure-audits/seq-01-1-through-01-3-2026-06-24
```

The structural audit completed with 0 errors and 99 warnings.
