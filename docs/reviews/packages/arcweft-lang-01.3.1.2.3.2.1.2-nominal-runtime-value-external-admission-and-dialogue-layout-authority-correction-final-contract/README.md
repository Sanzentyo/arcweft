# Lang-01.3.1.2.3.2.1.2 final design contract

Status: **READY_FOR_IMPLEMENTATION**  
Assignment kind: **design-only**  
Production overlay: **none**  
Pinned implementation-audit commit: `98ccafa5f0113a50f8a0f5e985df5f695c401588`  
`OPEN_QUESTIONS=0`

This archive is the independently usable correction contract requested by
Lang-01.3.1.2.3.2.1.2. It closes the external nominal-value construction
boundary needed before the parent A4 deletion of public
`RuntimeNominalRecordValue::new` and `validate_shape` can be accepted.

## Selected result

The final design has three coupled decisions:

1. **Whole-plan nominal catalog admission.** The existing core nominal layout
   remains the sole executable descriptor. Whole-plan admission produces a
   non-Serde, private-field producer capability. Only a value-admission handle
   borrowed from that capability may invoke the crate-private checked nominal
   constructor. A caller cannot turn arbitrary nominal/layout scalars or an
   independently fabricated layout directly into a runtime value.
2. **CharacterDialogue is one exact opaque producer payload.** The obsolete
   CharacterDialogue nominal record, custom-entry nominal record, and
   inline-failure nominal wrapper are deleted. The exact opaque payload is a
   producer-owned 18-element tuple. Custom entries are canonical two-element
   tuples `(field_id, value)`. Inline failure is its existing closed direct
   variant. There is no `Dynamic` checked predicate and no fake
   `RuntimeTypeSchema` for opaque payload bytes.
3. **Nested nominal values are descriptor-aware.** Role/custom checked types
   and the active `std.character_dialogue` producer capability are required for
   admission, normalization, clear, patch, decode, restore, replay, and
   activation. Every transformed nominal boundary is rebuilt through the
   handle and revalidated before publication. Structured patching preflights
   all paths and mutation eligibility before cloning/mutating and publishes
   atomically.

All Arcweft-owned schema, ABI, codec, digest-domain, and protocol version
numbers remain exactly `1`. No dual reader, compatibility constructor,
source-name reconstruction, copied descriptor table, fallback, extension
trait, or implementation overlay is authorized.

## Normative files

- `FINAL_CONTRACT.md` — final decisions and precedence.
- `RUST_OWNERS_AND_APIS.md` — exact Rust-shaped owners, visibility, derives,
  constructors, accessors, and errors.
- `AUTHORITY_AND_CATALOG.md` — canonical descriptor flow and non-forgeable
  operational admission.
- `CHARACTER_DIALOGUE_REPRESENTATION.md` — exact opaque payload layout and
  deletion of the three nominal wrappers.
- `DESCRIPTOR_LOOKUP_AND_TRANSFORMATION.md` — nested lookup, normalize, empty,
  and atomic patch semantics.
- `VALIDATION_ERROR_AND_PATH_PRECEDENCE.md` — deterministic validation order and
  typed cross-layer error mapping.
- `PERSISTENCE_ACTIVATION_AND_CODEC.md` — plan/value quarantine, restore,
  replay, bundle, and A4/A6 split.
- `PRODUCER_CONSUMER_DELETION_INVENTORY.md` and `.csv` — complete migration and
  deletion closure.
- `IMPLEMENTATION_ORDER.md` — compile-clean, deletion-driven implementation
  schedule.
- `TEST_MATRIX.md` and `.csv` — positive, negative, precedence, compile-fail,
  restore, and workspace gates.
- `REQUIREMENTS_TRACEABILITY.md` — every request requirement mapped to a
  decision and test.
- `NON_GOALS.md` — prohibited alternatives and unchanged parent decisions.
- `VALIDATION_EVIDENCE.md` — evidence actually inspected and checks actually
  run for this archive.
- `IMPLEMENTATION_STATUS.md`, `PACKAGE_STATUS.txt`, `OPEN_QUESTIONS.txt`,
  `contract.json`, `PARENT_ARTIFACTS.sha256`, and `MANIFEST.sha256` — package
  metadata and integrity.
- `SOURCE_REQUEST.md` — byte-identical copy of the supplied request.

## Evidence boundary

The exact pinned GitHub blobs/tree, root and scoped `AGENTS.md`, the supplied
request, retained searchable parent/child package mirrors, and the key core,
runtime-plan, dialogue, plan, path, session, save, and View sources were
statically inspected. This environment did not contain a local repository
checkout and could not run Cargo, Clippy, rustfmt, Tier 2, or structural gates.
Those commands are therefore specified as implementation acceptance gates, not
reported as completed. Archive construction, extraction, hashes, JSON/CSV
parsing, request-copy identity, file-policy checks, and deterministic ZIP
reproduction are executed locally and recorded in `VALIDATION_EVIDENCE.md`.
