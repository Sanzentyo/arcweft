# Request 01.1〜01.3 implementation note (2026-06-24)

Base revision: `db1aafe0151350312c6cdfc53afbd856120efc90`

## Scope

This overlay implements the Request 01 branch up to Request 01.3 only. It was
applied to the current checkout on 2026-06-25 and repaired to satisfy the local
workspace lint/test policy.

- 01.1 `RuntimePlan` / `FlowOp` / `RuntimeExpr` to canonical `AwbcProgram` lowering.
- 01.2 compact AWBC VM and structured/compact parity observation schema.
- 01.3 baseline full-script compiled-region lowering over verified AWBC.

Requests 01.4 and 01.5 are not implemented in this package. Their required design and implementation plan is recorded in `seq-01-remaining-01-4-01-5-and-open-items-2026-06-24.md`.

No additional `01.1.x`, `01.2.x`, or `01.3.x` request file was added during the
2026-06-25 application pass. The remaining details inside 01.1-01.3 are covered
by the existing request files under `docs/reviews/requests/` and are summarized
as implementation follow-up boundaries in
`seq-01-remaining-01-4-01-5-and-open-items-2026-06-24.md`.

## Changed files

### `arcweft-runtime-plan`

- `src/lib.rs`
  - adds `pub mod awbc_lower;`.
- `src/awbc_lower.rs`
  - public compiler-side lowerer API:
    - `AwbcLowerInput`
    - `AwbcLowerOptions`
    - `AwbcLowerReport`
    - `AwbcLowerError`
    - `lower_runtime_plan_to_awbc`
    - `lower_runtime_plan_to_awbc_with_input`
- `src/awbc_lower/inventory.rs`
  - deterministic table interning for strings, runtime types, constants, signatures, frame layouts, line task groups, effect plans, task plans, choices, content units, source plans, stream plans, functions, entries.
- `src/awbc_lower/frame.rs`
  - function-local frame/register/scope allocator.
- `src/awbc_lower/pattern.rs`
  - `RuntimePattern` to `AwbcPattern` lowering.
- `src/awbc_lower/expr.rs`
  - `RuntimeExpr` to expression-register instruction lowering.
- `src/awbc_lower/flow.rs`
  - `FlowOp` family lowering.
- `src/awbc_lower/line.rs`
  - dialogue content and line-task content-unit bridge.
- `src/awbc_lower/source.rs`
  - `SourcePlan` and `StreamPlan` lowering.
- `src/awbc_lower/tests.rs`
  - focused smoke fixture for deterministic table production.

### `arcweft-core`

- `src/awbc.rs`
  - adds `pub mod vm;` and `pub mod parity;` to the AWBC boundary.
- `src/awbc/vm.rs`
  - compact VM step API and opcode dispatch.
  - uses `FiberState` as the executor-neutral runtime state.
  - exposes host/suspension operations as typed VM exits.
  - no structured VM fallback.
  - repaired nested return handling so delegated VM execution only reports a
    root return after the root fiber is terminal.
- `src/awbc/parity.rs`
  - normalized parity trace schema for current structured VM and compact VM observations.

### `arcweft-runtime-codegen`

- `src/lib.rs`
  - adds `pub mod awbc_region;`.
- `src/awbc_region.rs`
  - verified AWBC region eligibility scan.
  - safe Rust baseline region object implementing `CompiledRegion`.
  - maps compact VM exits into `CompiledStepExit`.
  - preserves zero-budget fallback semantics through the existing compiled-region ABI.
- `src/region.rs`
  - treats an already-terminal root fiber return from a baseline AWBC region as
    an applied return instead of applying it twice.
- `src/tests.rs`
  - adds AWBC region acceptance, host-boundary rejection, and baseline VM
    execution coverage.

## Design decisions implemented

### 01.1 lowering

The lowerer is compiler-side and lives in `arcweft-runtime-plan`. `arcweft-core` remains Sans I/O and compiler-free. The lowerer outputs an `AwbcProgram`, stats, and diagnostics, and optionally invokes `AwbcProgram::verify`.

Deterministic interning is centralized in `AwbcInventory`; call sites no longer hand-build IDs. The inventory owns string, constant, signature, frame, effect, task, choice, content, function, source, and stream IDs.

Frame allocation uses one slot vector per function. Locals, temps, return values, and runtime state registers are represented by `FrameSlotKey`. Scopes are explicit and converted into AWBC `EnterScope` / `ExitScope` markers.

### 01.2 compact VM

`arcweft-core::awbc::vm` provides a Sans I/O VM step boundary:

- `VmStepOptions`
- `VmStepOutput`
- `VmExit`
- `VmObservation`
- `VmHost`
- `RejectingVmHost`
- `step`
- `step_with_host`

The VM uses current `RuntimeValue` at execution time. Constants are materialized from AWBC constant records. Registers are stored in `FiberFrame::registers`, and safe points are represented by `FiberState` suspension/terminal state.

The VM implements expression-style non-terminators, local control-flow terminators, returns, traps, budget yield, and typed suspensions for dialogue, choice, await, await-many, and host call boundaries. Host I/O is not performed.

The parity harness normalizes visible structured VM and compact VM observations. It deliberately avoids comparing internal instruction counts by default, because current structured VM and compact VM have different dispatch granularity.

### 01.3 compiled-region lowering

`arcweft-runtime-codegen::awbc_region` implements the first full-script baseline lowering layer:

- `AwbcRegionLowerOptions`
- `AwbcRegionLowerReport`
- `RejectedRegion`
- `BaselineAwbcRegion`
- `lower_awbc_regions`
- `compiled_identity`

The first backend is a safe Rust baseline compiled region. It does not allocate executable memory and does not perform host I/O. It uses verified AWBC, the compact VM stepper, and the existing `CompiledRegion` ABI so the first region path does not duplicate semantics permanently.

Unsupported opcodes and host boundaries produce rejected regions unless `allow_host_boundaries` is explicitly enabled. Fallback is represented with `CompiledStepExit::FallbackToVm` and consumes zero budget through the existing transactional boundary.

## Validation status

Run successfully in the current checkout on 2026-06-25:

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
cargo +nightly -Zscript tools/structure-audit.rs --root .
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/seq-01-1-through-01-3-2026-06-24
```

The structural audit reported 0 errors and 99 warnings. Report files were
written under
`docs/implementation/structure-audits/seq-01-1-through-01-3-2026-06-24/`.

## Known implementation limitations inside 01.1〜01.3

The implementation is a concrete, locally validated overlay. The remaining
limits are design/semantic completeness boundaries, not compile blockers:

1. Some lowerer paths intentionally produce canonical effect/intrinsic placeholders for complex structured constructs such as map, match-value, loop iteration, and dynamic goto. Those placeholders are typed AWBC records, not product-default execution claims.
2. The compact VM covers the core value/register/control/suspension surface, but parity fixtures must still be expanded before product selection.
3. `BaselineAwbcRegion` is the first safe Rust backend and delegates executable semantics to the compact VM. Optimizing AOT/JIT code generation remains out of scope for 01.3.
4. Product runtime selection remains explicitly outside this cut; compact VM and
   baseline compiled regions are not product defaults.

## Next required work

Proceed to the remaining markdown plan:

- `docs/implementation/seq-01-remaining-01-4-01-5-and-open-items-2026-06-24.md`
