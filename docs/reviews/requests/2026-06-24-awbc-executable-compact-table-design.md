# Request: AWBC Executable Compact Table Design

## Request

Please design the implementation path for replacing the current structured
`BytecodeProgram` payload in product `AWBC` sections with a fully executable
compact bytecode table.

The design should be concrete enough to turn into small Rust implementation
cuts with focused tests.

## Why this needs a decision

The incremental hot-swap bundle work now has:

- `arcweft-core::compact_bytecode` as a Sans I/O compact validation model.
- Product AWFB `ProgramBytecode` sections encoded as an `AWBC` binary envelope.
- `AWBC` payloads that carry a decoded-and-verified compact validation table
  alongside the structured `BytecodeProgram`.
- VM/runtime execution that still depends on the structured
  `BytecodeProgram`.

The current attached bundle/reference workspace specifies only the compact
verifier shape: `Return`, `Constant`, `Call`, `Jump`, `EnsureContent`, and
bounded function/constant/content/runtime-type indices. It does not define a
complete executable instruction set, constant table schema, expression/value
encoding, host-call/effect ABI, source/display/resource side tables, or VM
migration plan.

This means the repository has a useful compact artifact validation sidecar, but
does not yet have an authoritative executable compact bytecode format that can
replace structured bytecode.

## Design questions

Please propose concrete answers for:

1. What is the complete v1 executable compact opcode set, including control
   flow, expressions, host/effect calls, await/choice/dialogue, tasks, loops,
   match, scoped bindings, and dynamic entity targets?
2. What are the exact table schemas for constants, functions, runtime types,
   content units, line task groups, stream/source plans, pure helpers, host
   calls, display/source maps, and resource references?
3. How should structured `FlowOp` and runtime expressions lower into compact
   instructions without preserving a parallel structured payload?
4. What binary codec is canonical inside `AWBC`, and what length/depth/count
   budgets are required for decode safety?
5. What verifier checks are required beyond index bounds: stack/register
   discipline, control-flow validity, effect/capability constraints,
   entrypoint signatures, host-call ABI compatibility, type layout
   compatibility, and source-map consistency?
6. How should the VM/runtime execute the compact artifact, and where is the
   compatibility boundary with existing `BundleSession`, `ProgramGeneration`,
   and `BytecodeProgram::verify`?
7. How are patch compatibility fingerprints derived from compact tables, and
   which compact changes are content-only, code-compatible, code-generational,
   or restart-required?
8. What migration path should remove the structured `BytecodeProgram` from
   `AWBC` product payloads without adding a compatibility shim for unfinished
   compiler/runtime internals?
9. Which tests/golden fixtures prove parity between structured bytecode
   execution and compact bytecode execution?

## Constraints

- Keep `arcweft-core` Sans I/O.
- Keep product players free of syntax/HIR/sema/compiler dependencies.
- Do not invent a one-off opcode set that only satisfies the current tests.
- Do not preserve two unfinished executable bytecode models as silently
  equivalent compatibility layers.
- Preserve deterministic runtime behavior.
- Do not use `unsafe` or unstable Rust.
- Prefer typed APIs over stringly opcode, table, or ABI records.
- Keep filesystem/network/cache/signing work outside core bytecode and bundle
  verification models.

## Expected output

Please provide:

- the canonical `AWBC` executable payload schema;
- affected crates/modules;
- new or changed public/private types;
- the lowering plan from structured bytecode/runtime-plan data to compact
  executable tables;
- the VM execution plan for compact bytecode;
- verifier rules and decode budgets;
- patch compatibility fingerprint rules;
- step-by-step implementation order;
- focused tests for each step;
- migration criteria for deleting the structured `BytecodeProgram` payload from
  product `AWBC` sections.

## Current goal boundary

Until this design is answered, the current incremental hot-swap goal should not
implement:

- replacing structured `BytecodeProgram` execution with compact table
  execution;
- deleting the structured `BytecodeProgram` payload from `AWBC`;
- inventing an executable compact opcode/table model from local test pressure;
- compatibility shims that silently accept two unfinished executable bytecode
  models as equivalent.

The current goal may keep the implemented compact validation table sidecar in
`AWBC` and should continue validating it on decode.

## Useful current evidence

Start with these files:

- `crates/arcweft-core/src/compact_bytecode.rs`
- `crates/arcweft-core/src/bytecode.rs`
- `crates/arcweft-bundle/src/product.rs`
- `crates/arcweft-runtime-driver/src/session.rs`
- `crates/arcweft-runtime-driver/src/swap.rs`
- `crates/arcweft-runtime-host/src/bundle_runner.rs`
- `docs/implementation/incremental-hot-swap-bundle-2026-06-23.md`
- `docs/05-build-and-security/packaging.md`
