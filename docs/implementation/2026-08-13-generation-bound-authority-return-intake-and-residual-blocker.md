# Generation-bound authority return intake and residual blocker

Date: 2026-08-13

Continues:
`docs/implementation/2026-08-13-nominal-runtime-value-authority-package-intake-and-blocker.md`

Inspected Git baseline:
`175a74da637ca5f455abdefda49c6b62897b00e2` on `main`, equal to
`origin/main`, with a clean working tree before this ZIP intake.

## Returned archive intake

Retained archive:
`docs/reviews/packages/zips/arcweft-lang-01.3.1.2.3.2.1.2.1-generation-bound-producer-root-and-awbc-admission-authority-correction-final-contract.zip`

SHA-256:
`aa43429b6ffe5aac6489c94c7ff7a117ca1bbd43c764fed6ff4a1f3b5d540e06`

The 98,367-byte ZIP contains 28 files below one redundant wrapper. It has no
unsafe path, rooted/drive path, traversal component, symlink/reparse entry, or
case-insensitive collision. The wrapper was stripped in the searchable frozen
mirror. All 28 extracted files match their ZIP-member SHA-256 values and all
27 internal `MANIFEST.sha256` rows pass. `SOURCE_REQUEST.md` matches the
repository request, and the retained parent `.1.2` archive hash matches
`PARENT_ARTIFACTS.sha256`.

The package declares `READY_FOR_IMPLEMENTATION`, `OPEN_QUESTIONS=0`, no
production overlay, and every Arcweft-owned version fixed at `1`.

## Readiness adjudication

The return fixes the parent package's circular producer authorization, shared
plan/AWBC generation aggregate, raw execution bypass, custom-field digest
ownership, nested voice, and generation-correlation model. Sol max was used to
audit the result-changing boundaries against current production.

The package is nevertheless `NOT_READY` as a complete A4 authority contract.
Five exact decisions remain open:

- canonical CharacterCatalog and ViewRegistry digest transcripts and owner
  APIs;
- the accepted declaration/registration owner and exact closed types for the
  six base CharacterDialogue roles;
- exact plan/AWBC root-coordinate creation and exhaustive typed-table mapping;
- project nominal construction authority and AWBC `MakeRecord` domain
  selection; and
- the typed checked-value/unique-Choice validation API, error shapes, shared
  work budget, and nominal-tree error mapping.

The returned root error also carries only `RuntimeProducerRootId`, so it cannot
represent the separately required project-root order, duplicate, unresolved,
or lookup errors involving `RuntimeProjectRootId`.

The narrow follow-up request is:

`docs/reviews/requests/2026-08-13-lang-01.3.1.2.3.2.1.2.1.1-catalog-digest-role-root-and-construction-authority-correction.md`

Its required returned archive is:

`arcweft-lang-01.3.1.2.3.2.1.2.1.1-catalog-digest-role-root-and-construction-authority-correction-final-contract.zip`

## Safe implementation completed while blocked

The audit identified two independent prerequisite subcuts whose final owner
and behavior are already exact:

- `CharacterDialogueCustomFieldId` moved from dialogue to the lower
  `arcweft-interaction-model::dialogue` owner while preserving the
  `character_dialogue_field.*` family, `PublicId` validation, 128-byte limit,
  and transparent string Serde. Dialogue re-exports the same type and maps its
  lower typed error into the existing domain error. Sema imports the lower
  owner directly.
- `CharacterDialogueRuntimeRole` was added beside that ID with fixed enum
  order/tags and snake-case Serde names.
- core gained only the raw typed generation/root/View/catalog digest scalar
  substrate. These byte wrappers are serialized evidence, not operational
  admission or construction capability. The custom-field digest intentionally
  exposes no public scalar constructor until its canonical catalog owner is
  implemented.

No project-root collection, semantic role projection, Character/View digest,
generation admission, AWBC admission/execution cut, dialogue schema cut, or
unchecked nominal deletion is claimed.

## Validation performed

- `cargo test -p arcweft-interaction-model --all-features --jobs 1`: 8 passed;
- `cargo test -p arcweft-dialogue --all-features --jobs 1`: 39 passed,
  including 4 compile-fail doctests;
- `cargo test -p arcweft-core --all-features generation_contract::tests
  --jobs 1`: 3 passed;
- `cargo test -p arcweft-lang-sema --all-features --jobs 1`: 202 passed,
  including 10 compile-fail API tests;
- `cargo check -p arcweft-interaction-model -p arcweft-dialogue
  -p arcweft-lang-sema -p arcweft-core --all-targets --all-features --jobs 1`:
  passed;
- `cargo clippy -p arcweft-interaction-model -p arcweft-dialogue
  -p arcweft-lang-sema -p arcweft-core --all-targets --all-features --jobs 1
  -- -D warnings`: passed after correcting three documentation-markdown
  findings;
- `just structure-audit`: passed with 185 existing review triggers and zero
  blocking violations;
- `just structure-audit-gate`: passed with zero blocking violations;
- `cargo fmt --all`: passed; and
- `git diff --check`: passed before the final documentation update.

Workspace-wide, structural, and Tier 2 validation remain to be run before the
final A4 authority cut. The earlier core focused run emitted one dead-code
warning for a provisional crate-private custom-digest constructor; that
constructor was deleted before the successful focused test and the final
four-crate Clippy run is warning-free.
