# Lang-01.3.1.2.3.2.1.2.1.1.1.1 final contract

Status: **READY_FOR_IMPLEMENTATION**  
Open questions: **0**  
Evidence commit: `80348beed0efa72db07f712122217b4e679e0a97` (`main`)  
Request SHA-256: `2498106d805515f2fba326ef55685a8699aec2ab1abb986e22bc2f0a1f984cc6`

This archive is the design-only correction requested by
`SOURCE_REQUEST.md`. It contains no production source, patch, overlay,
compatibility path, or version increment. Every Arcweft-owned schema, ABI,
codec, digest-domain, protocol, save, snapshot, and bundle version remains
exactly `1`.

## Final authority split

1. `arcweft-core` owns immutable raw DTOs, public checked builders, private
   fields, custom version-1 decoding, structural admission, generation storage,
   checked-value validation, typed-site resolution, and admitted wrappers.
2. `arcweft-runtime-plan` is the legitimate external lowerer. It calls the
   public checked core builders; no friend-crate visibility or caller gate is
   claimed.
3. `arcweft-compiler::project::runtime_generation` is the standard accepted-
   world assembler. It reads non-Serde semantic/sema/catalog owners, constructs
   one lower-layer projection, and asks core to issue the generation before
   lowering plan or AWBC artifacts.
4. Raw plan and AWBC declarations are claims checked against that already
   issued generation. Neither artifact has a self-admission method.
5. `arcweft-runtime-driver::generation_runtime::RuntimeDriverGeneration` owns
   one same-parent admitted plan/AWBC product plus admitted Character/View/
   custom catalogs. Restore and replay issue the independent generation before
   decoding executable or value payloads.

## Normative lookup

- `REQUEST_DECISION_MATRIX.csv` maps request decisions 1–12 one-to-one.
- `RAW_CONSTRUCTION_API.md`, `GENERATION_ISSUANCE_API.md`,
  `ADMISSION_PAIR_DRIVER_API.md`, `CHECKED_VALIDATION_API.md`,
  `AUDIO_SITE_API.md`, and `RUNTIME_EXPR_NODE_API.md` are the exact Rust-shaped
  APIs.
- `AWBC_SITE_RESOLUTION.csv`, `AWBC_AUDIO_TYPED_SLOTS.csv`,
  `RUNTIME_EXPR_NODE_RESOLUTION.csv`, and `RUNTIME_PLAN_TYPE_KIND.csv` are the
  corrected mapping tables.
- `TEST_MATRIX.csv` is the executable acceptance matrix; contradictory parent
  rows are replaced, not annotated as optional.
- `PRODUCER_CONSUMER_DELETION_INVENTORY.csv` is the complete retained-parent
  inventory with the correction rows applied and new cross-crate owners added.
- `IMPLEMENTATION_ORDER.csv` is the compile-clean, deletion-driven order.
- `MANIFEST.sha256` covers every other archive member.

The retained parent substrate is enumerated in
`RETAINED_PARENT_SUBSTRATE.md`; this correction supersedes only the boundaries
explicitly named there.
