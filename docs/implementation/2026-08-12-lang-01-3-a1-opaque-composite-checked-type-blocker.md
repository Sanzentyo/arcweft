# Lang-01.3 A1 opaque composite checked-type blocker

Date: 2026-08-12

Inspected and pushed package-intake baseline:
`948e92b5f740d97cf87a94bef7263248ee4c4824` on `main`, equal to
`origin/main` before this implementation investigation.

This note supersedes only the production-readiness conclusion in
[`2026-08-12-lang-01-3-1-2-3-2-1-return-intake.md`](2026-08-12-lang-01-3-1-2-3-2-1-return-intake.md).
The returned archive's integrity, validators, nominal-record layout decision,
existing `RuntimeSeqError` decision, and accepted earlier identity/slot/path
foundation remain valid evidence. A1 is not complete.

## Implemented experimentally and inspected

The uncommitted A1 investigation followed the returned package far enough to
exercise the disputed boundary:

- added the package-specified core nominal record descriptor and exact nominal
  checked-type layout identity;
- projected project nominal schemas through final semantic analysis instead of
  reconstructing source strings;
- carried nominal record descriptors through runtime-plan facts; and
- mapped producer-owned semantic types without an exact canonical layout to
  the existing `RuntimeTypeShape::Opaque` fail-closed path.

This production work is deliberately not staged or committed because A1 is an
atomic compile-clean gate and its final public shape depends on the correction
below.

## Validation evidence

Passed during the investigation:

- `cargo test -p arcweft-core --all-features --jobs 4`: 269 core unit tests
  passed;
- `cargo check -p arcweft-lang-sema --all-targets --all-features --jobs 4`;
- `cargo check -p arcweft-runtime-plan --all-targets --all-features --jobs 4`;
- `cargo check -p arcweft-compiler --all-targets --all-features --jobs 4`;
- `cargo fmt --all`; and
- `git diff --check` at the recorded intermediate points.

The compiler entry suite exposed the unresolved result:

- with the required full recursive checked-type projection, 3 tests passed and
  11 failed when opaque leaves inside `Result` could not produce a closed
  `RuntimeCheckedType`;
- an explicitly rejected experiment used `Never` for an unselected Result or
  Option branch; this improved the entry suite to 12 passed and 2 failed but
  still rejected the selected opaque `Reduction<GameState>` payload; and
- the two remaining failures were
  `project::entry_tests::sel_005_checks_selected_entry_identity_and_kind_before_runtime_lowering`
  and
  `project::entry_tests::body_only_change_preserves_binding_and_changes_compile_artifact_identity`,
  both reporting that a runtime checked type had no closed representation for
  the semantic payload.

The `Never` experiment was removed. It is not a partial solution: it assigns
different checked types to `Ok` and `Err` of the same semantic Result, rejects
valid values through `accepts_value`, and disagrees with AWBC's single complete
variant type-table row and invariant compatibility rules.

Workspace-wide tests, final workspace check/Clippy, Tier 2, and structural
audit were not run because the A1 exit condition is already blocked.

## Blocked

The returned package fixes exact layout identity for project nominal records,
but it does not define the checked-type representation for a producer-owned
opaque type nested inside a composite. Existing alternatives change observable
results:

- inventing a layout hash from a name or semantic digest corrupts canonical
  layout identity;
- adding `RuntimeCheckedType::Opaque` or using AWBC Dynamic changes acceptance,
  codec, and verification contracts;
- making branch predicates optional changes existing Result/Option shape; and
- selected-case-only refinement changes type compatibility and branch-merge
  behavior.

The independently throwable blocker request is
[Lang-01.3.1.2.3.2.1.1 opaque composite checked-type owner reconciliation](../reviews/requests/2026-08-12-lang-01.3.1.2.3.2.1.1-opaque-composite-checked-type-owner-reconciliation-correction.md).

Until it returns, the following receive no completion credit:

- the Lang-01.3.1.2.3.2.1 A1 gate and all A2-A6 carrier migration;
- parent G1.2-B through G1.4;
- View ownership/catalog/product migration;
- activation/save publication; and
- Stream handle or partial publication.

## Preserved results and non-goals

No accepted nominal-record owner, record-sequence error, ownership, identity,
slot, path, ABI, activation, View, or Stream decision is reopened by this
blocker. No production overlay, compatibility shim, fabricated schema, or
type-name special case is requested.
