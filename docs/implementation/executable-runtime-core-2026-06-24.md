# Executable runtime core implementation record — 2026-06-24

## Basis

- Repository: `Sanzentyo/arcweft`
- Revision: `23fc9206f08340df438aece556065d5235bb27eb`
- Final connector recheck: `main` was identical to that revision.
- Request: `request/2026-06-24-seq-01-executable-runtime-core.md`
- Design: `docs/02-runtime/executable-runtime-core.md`

## Implemented in the overlay

### `arcweft-core::awbc`

The previous first-pass single-file AWBC contract is replaced directly; no
extension trait or second compatibility enum is introduced.

- `schema.rs`
  - canonical typed IDs and complete table model;
  - stable `AwbcOpcode::{encoded, from_encoded, is_terminator}` implementation
    on the owning enum;
  - complete non-terminator and terminator operands;
  - runtime types/constants, frames/functions/blocks/resume points;
  - patterns, calls/tasks/effects, content/line tasks, stream/source plans,
    helpers, maps/resources, and entries.
- `codec.rs` + `codec/*`
  - fixed AWBC v1 envelope and table order;
  - manual canonical wire format, minimal ULEB128, strict tags/UTF-8/trailing
    bytes;
  - allocation, aggregate, tensor, and nesting budgets.
- `verify.rs` + `verify/*`
  - header/canonical table verification;
  - range ownership and function/frame/signature validation;
  - pattern graph validation;
  - CFG reachability/backedge checks;
  - definite register initialization and scope-stack joins;
  - opcode, call/result, effect/capability, entry, source/display/resource
    checks and semantic budgets.
- `fiber.rs`
  - executor-neutral cursor/frame/register/scope state;
  - typed suspension and await-many state;
  - safe points, call-frame transitions with typed return destinations, budget,
    terminal values;
  - complete transactional checkpoint/restore.
- `tests.rs`
  - canonical codec round trip/determinism;
  - decode byte budget;
  - uninitialized register and escaped branch diagnostics;
  - budget safe-point suspend/resume;
  - nested return resume and destination write-back.

### `arcweft-runtime-codegen`

The placeholder region/frame IDs are replaced with AWBC-owned IDs rather than
kept as duplicate wrappers.

- typed executor policy and artifact inventory;
- 256-bit typed opcode capability set;
- complete persistent JIT/native/Wasm cache-key inputs and canonical BLAKE3
  digest;
- generation-separated dispatch key;
- safe Rust `CompiledRegion` ABI and metadata validation;
- structured exits and mapping into `FiberState`;
- dispatcher-owned budget accounting;
- checkpoint rollback for failure/fallback;
- zero-budget invariant for VM fallback;
- tests for continue, nested return write-back, transactional fallback,
  fallback budget rejection, and cache-key sensitivity.

## Deliberately not implemented in this overlay

The following are designed in detail but remain later compiling cuts, rather
than being represented by stubs or a hidden fallback model:

- `RuntimePlan`/`FlowOp`/`RuntimeExpr` compiler lowering into AWBC;
- compact VM dispatch and pattern/expression execution;
- differential structured/compact fixture harness;
- runtime-driver/runtime-host executor migration;
- product AWFB `ProgramBytecode` migration;
- deletion of structured `BytecodeProgram` and old `compact_bytecode` sidecar.

## Verification performed

The overlay has been applied to the real repository checkout at basis revision
`23fc9206f08340df438aece556065d5235bb27eb` and adjusted only for local lint
policy and documentation traceability.

```text
cargo test -p arcweft-core awbc --lib                     PASS (6 passed)
cargo test -p arcweft-runtime-codegen --lib                PASS (5 passed)
cargo fmt --all -- --check                                 PASS
cargo check --workspace --all-targets                      PASS
cargo clippy --workspace --all-targets --all-features -- -D warnings  PASS
cargo +nightly -Zscript tools/structure-audit.rs --root .      PASS (0 errors, 99 warnings)
just test-workspace                                        PASS
```

The structure audit report for this cut is retained under
`docs/implementation/structure-audits/executable-runtime-core-2026-06-24/`.
The only new warning-threshold files from this cut are
`crates/arcweft-core/src/awbc/schema.rs` and
`crates/arcweft-core/src/awbc/verify/code.rs`. Both are below the error
threshold and remain cohesive for this cut: `schema.rs` is the complete AWBC v1
typed contract, while `verify/code.rs` is the shared instruction/terminator
dataflow verifier. Future work should split them by stable table family or
opcode family before adding materially more behavior.

## Verification not claimed

- No compiler lowerer, compact VM, bundle switch, or player integration exists
  in this overlay, so parity/product migration tests are specified rather than
  reported as passing.
- Performance, native object loading, executable-memory behavior, and Wasm
  execution were not tested; they are outside this sequence cut.

## Follow-up design requests

Request 01 covered more than this overlay implements. The parts that remain
insufficiently detailed for direct implementation have been split into
sequential request files:

- `docs/reviews/requests/2026-06-24-seq-01.1-runtime-plan-awbc-lowering.md`;
- `docs/reviews/requests/2026-06-24-seq-01.2-compact-vm-and-parity-harness.md`;
- `docs/reviews/requests/2026-06-24-seq-01.3-full-script-compiled-region-lowering.md`;
- `docs/reviews/requests/2026-06-24-seq-01.4-runtime-driver-host-executor-migration.md`;
- `docs/reviews/requests/2026-06-24-seq-01.5-product-awfb-bytecode-migration.md`.

## Integration risk and expected conflicts

- `crates/arcweft-core/src/awbc.rs` and
  `crates/arcweft-runtime-codegen/src/lib.rs` are intentional public-contract
  replacements. Apply only to the basis revision or review conflicts manually.
- Repository searches at the basis revision found no external use of the old
  AWBC table types or runtime-codegen placeholder region types. If the private
  branch advanced after the recorded revision, rerun that structural search.
- No Cargo dependency change is expected: `arcweft-core` already owns
  `serde`/`thiserror`; `arcweft-runtime-codegen` already owns `arcweft-core` and
  `blake3`.
