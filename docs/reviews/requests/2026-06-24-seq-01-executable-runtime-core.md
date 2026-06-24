# Request 01: Executable Runtime Core

## Sequence Position

This is the first design request in the integrated execution sequence.

Submit this request before the product resource, patch materialization,
generation hot-swap, persistent cache, and Agent REPL runtime-tier requests.

## Request

Please design Arcweft's executable runtime core as one coherent model. This
request intentionally combines:

- AWBC executable compact table design;
- runtime executable IR and executor-neutral fiber state;
- baseline full-script AOT/JIT ABI and safe-point contract.

The design must be concrete enough to turn into small Rust implementation cuts
with focused tests.

## Existing Request Files To Incorporate

Use these existing requests as source material, but answer them together rather
than independently:

- `docs/reviews/requests/2026-06-24-awbc-executable-compact-table-design.md`

Also incorporate the implemented contracts recorded in:

- `docs/implementation/integrated-execution-2026-06-24.md`
- `crates/arcweft-core/src/awbc.rs`
- `crates/arcweft-runtime-codegen/src/lib.rs`

## Why These Must Be Designed Together

Compact bytecode, the VM, and compiled regions must agree on the same execution
facts:

- frame layout;
- resume points;
- safe points;
- host-call and effect ABI;
- suspension state;
- traps and failure reporting;
- source-map identity;
- VM fallback rules.

If AWBC, runtime IR, fiber state, and codegen ABI are designed separately, later
work will either duplicate executable models or introduce compatibility shims
inside unfinished compiler/runtime code.

## Current Implementation Evidence

The repository currently has:

- structured `BytecodeProgram` execution as the product runtime source of truth;
- `arcweft-core::awbc` as a first-pass Sans I/O executable-table data/verifier
  contract;
- `arcweft-core::compact_bytecode` as an older compact validation sidecar;
- product `ProgramBytecode` sections that still carry structured
  `BytecodeProgram`;
- `arcweft-runtime-codegen` policy and IR contracts for executor selection,
  code artifact inventory, frame layouts, safe regions, cache keys, and
  structured compiled-step exits;
- `arcweft-lang-jit-cranelift` for pure helper JIT/AOT only, not full-script
  runtime regions.

## Required Design Decisions

Please provide concrete answers for:

1. What is the canonical AWBC v1 executable payload schema?
2. What is the complete compact opcode set for control flow, expressions,
   dialogue, choice, await, await-many, host tasks, effects, loops, match,
   scoped bindings, dynamic targets, return, trap, and budget yield?
3. What are the exact table schemas for constants, functions, blocks,
   registers/locals, runtime types, content units, line task groups, stream
   plans, source plans, pure helpers, host calls, display/source maps, and
   resource references?
4. What is the canonical binary codec inside AWBC, and what decode budgets must
   be enforced?
5. How does current `RuntimePlan` / `FlowOp` / `RuntimeExpr` lower into compact
   executable tables?
6. What is the executor-neutral `FiberState` model?
7. How are frame layouts, resume points, safe points, and suspension state
   shared between the compact VM and compiled regions?
8. What verifier checks are required beyond index bounds, including
   register/stack discipline, control-flow validity, type layout compatibility,
   host-call ABI compatibility, entrypoint signatures, and effect/capability
   constraints?
9. How does the compact VM execute AWBC while preserving current structured VM
   behavior?
10. What is the baseline full-script compiled-region ABI?
11. How does `CompiledStepExit` map back into VM/fiber state?
12. When should compiled execution fall back to VM?
13. What codegen cache keys are required for JIT, native AOT, and future Wasm
   AOT artifacts?
14. What are the migration criteria for deleting structured `BytecodeProgram`
   from product AWBC payloads?

## Required Implementation Order In The Design

Please propose small compiling cuts in this order or justify a better order:

1. Freeze AWBC schema and binary codec.
2. Add compact verifier coverage beyond current table checks.
3. Lower structured runtime plan into compact AWBC while still executing the
   structured VM for parity.
4. Add compact VM executor behind explicit test/dev selection.
5. Add differential tests against structured VM fixtures.
6. Migrate runtime-driver and runtime-host executor construction to the compact
   execution boundary.
7. Add full-script baseline region lowering ABI without optimizing codegen.
8. Remove structured `BytecodeProgram` from product AWBC payload only after
   parity and migration gates pass.

## Tests To Specify

The design should include focused tests for:

- deterministic AWBC encode/decode;
- invalid opcode/table/register/branch/host-call/type diagnostics;
- decode budget failures;
- VM parity for flow/dialogue/choice/await/await-many/goto/match/loop/source
  fixtures;
- safe-point suspend/resume;
- compiled-region fallback to VM;
- source-map and display-map consistency;
- product AWFB decode without structured bytecode after migration.

## Constraints

- Keep `arcweft-core` Sans I/O.
- Keep product players free of syntax, HIR, sema, compiler, verifier, CLI, and
  LSP dependencies.
- Do not preserve two unfinished executable bytecode models as silent
  compatibility layers.
- Do not invent one-off opcodes from current tests alone.
- Do not use `unsafe` or unstable Rust.
- Prefer typed APIs over stringly opcode/table/ABI records.

## Expected Output

Please produce one design document with:

- recommended architecture;
- affected crates/modules;
- new or changed public/private types;
- AWBC schema and runtime IR schema;
- VM execution plan;
- full-script codegen ABI and fallback plan;
- verifier rules and budgets;
- migration plan;
- implementation cuts;
- test plan;
- explicit non-goals.

