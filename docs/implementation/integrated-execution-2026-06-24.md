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

## Design requests still excluding work

- `docs/reviews/requests/2026-06-24-awbc-executable-compact-table-design.md`
  still blocks replacing structured bytecode execution, deleting structured
  `BytecodeProgram` from AWBC product payloads, and inventing a local compact
  VM/lowering model.

## Validation

Focused commands run for the implemented cuts:

```bash
cargo test -p arcweft-core --lib
cargo test -p arcweft-bundle --lib
cargo test -p arcweft-project --lib
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/arcweft-structure-audit.rs --root . --write docs/implementation/structure-audit-integrated-execution-2026-06-24
just test-workspace
```

The most recent structural audit for this package reports `0` errors and `97`
warnings.
