# AWBC record shape projection — 2026-09-12

Inspected `main == origin/main` at
`827aaedb955b81c21ede5c8deed7cb6d5440a826`, with the preserved callable,
source-graph, and aggregate nominal-admission migrations. This is an in-flight
implementation record, not acceptance of C4 or the complete nominal carrier.
The source changes described here are uncommitted and depend on that migration.

## Implemented boundary

AWBC record fields now retain their explicit `RuntimeRecordFieldId`, optional
name, and child type. Nominal records retain the shared
`RuntimeNominalRecordShape`, including distinct Unit, empty Tuple, empty
Record, one-field Tuple/Record, and Newtype forms. Codecs, string remapping,
header verification, checked projection, constant materialization, and
`MakeRecord` consume that row. Runtime-plan lowering forwards the admitted
shape and coordinates. Anonymous record construction requires an actual
record descriptor; the previous Dynamic-record success paths are deleted.

Record constants and `MakeRecord` no longer carry duplicate field names.
Variant constants obtain their selected case name from the type table.
Serde rejects the removed fields. All wire versions remain 1, with the
existing type/constant/opcode tags. The existing outer semantic identity and
ordered generic arguments remain intact. The package's abbreviated row
sketch does not justify duplicating identity into a shape or removing
arguments. Its proposed separate shape enum is served by the existing core
`RuntimeNominalRecordShape` owner.

Structural type edges are checked by one iterative DFS. The owning shape
enumerates the edges; nominal body back-edges stop at identity, while generic
arguments remain structural. Tuple/Function/argument cycles fail, shared
subgraphs are visited once, and nominal recursion remains representable.
The VM shares one record construction method between constants and code.

The untyped opaque-payload producer now supplies a real anonymous record row
with its existing Dynamic child predicate. Typed constants continue to use
their admitted type row. This does not issue nominal source proof or recover
erased source identities from a runtime value.

## Source layout authority correction

An intermediate verifier reconstructed a nominal schema graph from AWBC rows
and compared its derived layouts. A regression demonstrated that this was
invalid: source Bytes schemas have Binary, Base64, Hex, and Array formats,
but all project to the same logical Bytes row. The core source-graph/plan
admission accepts the original graph-derived layouts for all four formats.
Reconstructing Binary from the executable row rejected valid plans for the
other three formats.

That reconstruction module, its validation call, and its dedicated errors
were deleted. The final verifier preserves the source-proved layout and
checks the actual executable structure. Canonical source layout proof
belongs to aggregate admission where the original schema exists; runtime
values and restore must correlate with the active program type and layout.
Hashing the erased projection cannot prove the source layout. The maintained
[runtime chapter](../02-runtime/executable-runtime-core.md) records this
boundary. Frozen package mirrors were not edited.

The final regression constructs the original graph, admits the core plan,
forwards its row to AWBC, round-trips the codec, verifies and executes it,
and checks the value against both graph and plan for every Bytes format.
This directly tests the reason for the correction rather than substituting
a scalar comparison between independently claimed hashes.

## Validation actually run

All commands used the dirty checkout, sequentially and without Cargo job
overrides. Final logs are under the ignored
`.arcweft-local/validation/2026-09-11-effect-row-formulas/` directory.

- Passed: `cargo test -p arcweft-core --all-features`, **525 tests**
  (484 unit and 41 integration; zero doctests),
  `awbc-record-final-core-tests.log`, exit 0.
- Passed with warnings: `cargo clippy -p arcweft-core --all-targets
  --all-features`, `awbc-record-final-clippy.log`, exit 0. This is not a
  warning-free result.
- Passed: seven record-boundary tests and four new wire/serde tests, included
  in the final full core run. Coverage includes all record shapes, supplied
  field IDs, invalid names/coordinates, codec truncation and noncanonical
  IDs, deleted fields, type-table name authority, nominal recursion,
  structural cycles, and a 10,000-row shared DAG.
- Passed: changed-crate formatting and `git diff --check`.
- Passed: `just structure-audit-gate`, 95 packages, 2,327 Rust files,
  1,283,042 Rust physical LOC, 312 review triggers, zero blocking violations.
- Failed during dependency compilation: `cargo check -p
  arcweft-runtime-plan --all-targets --all-features`, exit 101, at the missing
  nominal Variant layout in Dialogue `character_dialogue/schema.rs:83`.
  Runtime-plan producer changes and the pending sema graph regression have
  **not compiled or executed**. Core tests do not establish those consumers.
- Not run for this in-flight change: workspace check, workspace Clippy,
  `just test-workspace`, `just test-doc`, and Tier 2. Their earlier failures
  in the field-name cut are historical evidence, not current results.

The intermediate source-layout counterexample failed as expected in
`awbc-source-layout-counterexample-recheck.log`. Its earlier first attempt
failed to compile because a fixture supplied Vec instead of Box; that fixture
was fixed before observing the semantic failure. The intermediate 523-test
pass with the rejected reconstruction does not describe the final model.

## Ownership and structural review

Working-copy sizes include adjacent WIP:

| Owner | Physical LOC | Bytes | Responsibility and disposition |
| --- | ---: | ---: | --- |
| core `awbc/schema.rs` | 3,116 | 94,714 | Existing exhaustive wire algebra owns structural edge enumeration. |
| core `awbc/codec/types.rs` | 1,213 | 42,961 | Existing type/constant wire owner; no second codec. |
| core `awbc/codec/code.rs` | 1,648 | 60,546 | Existing instruction codec; duplicate name data removed. |
| core `awbc/type_projection.rs` | 570 | 23,002 | Typed header validation and projection; no source-schema reconstruction. |
| core `awbc/type_projection/type_graph.rs` | 59 | 2,306 | Separate traversal state and linear graph well-formedness. |
| core `awbc/verify/structure.rs` | 2,813 | 110,121 | Program row/reference verification delegates record rules to the owner. |
| core `awbc/verify/code.rs` | 3,691 | 147,320 | Instruction/register relations use selected type fields. |
| core `awbc/vm.rs` | 2,849 | 112,404 | One program-context record constructor serves both executable producers. |
| core `awbc/tests/record_shapes.rs` | 629 | 22,441 | Separate behavioral test module. |
| core `awbc/codec/types/record_tests.rs` | 197 | 6,496 | Separate wire/serde tests. |
| runtime-plan `awbc_lower/pattern.rs` | 703 | 28,915 | Existing plan-to-AWBC type projection. |
| runtime-plan `awbc_lower/inventory.rs` | 2,180 | 87,768 | Existing string/type/constant inventory. |
| runtime-plan `awbc_lower/expr.rs` | 2,075 | 77,787 | Existing instruction producer deletes duplicate name data. |

The large existing owners remain cohesive with their algebra, verifier pass,
or execution context. The new independent graph traversal and regression
fixtures have child modules. No parallel field catalog, I/O, dependency edge,
feature, version family, or persistent validation cache was added. Zero audit
blockers does not establish completion of the source/runtime dependency
migration.

## Required continuation

Complete genuine source graph issuance and compiler/fact/plan admission;
the remaining `final_flow.rs` caller cannot use an empty or reconstructed
proof. Finish the real Dialogue, host, and data producers under the existing
[producer/schema closure request](../reviews/requests/2026-09-11-lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1.1-nominal-variant-layout-producer-and-schema-closure.md).
Private live nominal values, program-bound construction and restore,
operational value domains, ownership, and complete native/AWBC convergence
remain required. `MakeVariant` and Variant patterns still retain their own
case-name fields; this record does not claim their migration.

The whole seven-part goal remains active. These repository-resolvable
dependencies are neither external blockers nor grounds for declaring C4 or
the goal complete. Commit/push awaits a coherent validated dependency cut.

## Continuation — 2026-09-22

Supersedes the current validation and handoff state above; the earlier source
layout counterexample and its historical results remain evidence of that
implementation. Inspected `main == origin/main` at
`5aecf37033aeb653f69a80a4888886861811b37a`. The two intervening commits changed
instructions and documentation only. The dirty nominal/callable migrations
remain preserved in the existing checkout.

### Complete Variant-row boundary

AWBC verification previously checked builtin case names and payload presence
without checking the registry-required Tuple payload container. The owning
program now validates the complete row through `validate_variant_fields`:
nominal identity grammar, argument/child references, nonempty unique names,
and builtin case order, presence, container kind, and registry-owned arity.
Structural verification and finite checked-type projection consume this same
operation. The new persistent-value visitor also uses it.

The resulting test failure exposed two real consumers of the flat payload
assumption: Agent optional fields and `Progress.label`. Both now resolve the
payload item through `AwbcProgram::builtin_variant_payload_item`, using the
core case identity. ProjectCall's three attached-content cases use that same
method; its local Option-specific reader is deleted. The methods retain no
catalog or cache and do not expand nominal payload bodies.

The selected implementation cut is this complete Variant-row/consumer rule:
five explicit existing-file patches, the 110-line owning child module, and
the 138-line integration test. It excludes the pending nominal layout field,
record shape/wire, structural graph, integer conversion, source proof, Map,
and persistent-value API migrations. The independently tested shared rule
does not depend on those new representations. The maintained runtime chapter
records the common payload authority.

### Persistent-value work still in flight

`AwbcProgram::accepts_value` now uses the existing iterative core logical-value
visitor and canonical encoder while borrowing the actual AWBC type/string
rows. It handles persistent scalar/composite data, record descriptors,
nominal record bodies, variant owners/cases, exact opaque owners, Choice,
and bounded descendants. It does not reconstruct source schemas, clone a
checked-type tree, or accept a nominal record from a header without a body.
Integer width conversion now belongs to `From` implementations instead of
two projection-local mapping helpers.

The matching plan visitor now handles Map as a sequence of two-item key/value
tuples. Tests correlate source schema, plan, and AWBC digests, including
integer widths, dense Bytes, and all four source Bytes formats. A 90-node
recursive nominal value passes with an explicit sufficient depth allowance;
the normal limit and an invalid nested value reject. Choice ambiguity/work,
names/coordinates, complete nominal Variant rows, every Variant owner atom,
dangling references, and node/string/byte limits are covered.

This API currently has behavioral test consumers. Live VM/fiber/task/restore
validation has **not** migrated to it. Runtime-only families, Probe authority,
private nominal construction, snapshots, source issuance and complete upper
consumers still require implementation. A persistent digest test is not proof
of operational value admission or C5 completion.

### Validation and review

- Passed: final `cargo test -p arcweft-core --all-features`, **535 tests**
  (491 unit, 44 integration; zero doctests),
  `awbc-variant-core-recheck.log`, exit 0.
- Passed: the three registry-wide integration tests; complete AWBC owner tests,
  including VM execution of both Some and None for `Progress.label`; six new
  persistent-value tests and the extended record regressions are included in
  the full result. These ran in the preserved dirty checkout. The total is
  not attributed solely to the selected Variant cut.
- Passed with warnings: core all-target/all-feature Clippy,
  `awbc-variant-core-clippy.log`, exit 0 (128 library and 154 library-test
  warnings, 127 duplicates). Changed Rust was formatted.
- Passed: `just structure-audit-gate`, 95 packages, 2,331 Rust files,
  1,284,166 physical Rust LOC, 312 review triggers, zero blocking violations.
  `git diff --check` and the selected patch's cached applicability check pass.
- Failed during compilation: `cargo check --workspace --all-targets
  --all-features`, `cargo clippy --workspace --all-targets --all-features`,
  and `just test-workspace`, at the removed `AdapterRustType::opaque_producer`
  call in host-adapter `lib.rs:503`. Logs use the `awbc-variant-workspace-`
  prefix, with exit codes 101, 101, and 1 respectively. Later workspace tests
  did not execute. This is the previously observed adjacent Rust ADT producer
  migration, not a Variant-row acceptance pass or an external blocker.
- Not run in this cut: workspace doctests and Tier 2. The affected case-table,
  checked-projection and pure VM behavior has direct core coverage; no new
  MCP/resource protocol, subprocess, capture, rendering or device operation is
  exercised here. The full goal's milestone validation remains outstanding.
- The first full run after strict Variant admission failed three unit tests:
  the Agent fixture exposed its flat type/consumer, and two tests expected the
  old diagnostic text. The flat attached-default regression also used stale
  canonical string indices; it now changes the existing payload reference
  while retaining the real case names. Repairs keep the rejection assertions
  and add actual Progress execution. Final rechecks passed.
- Initial test-writing compile failures (the wrong fixture constructors and
  an owned/borrowed String comparison) were repaired before those runs. The
  first all-target check after the new row owner passed with two unused-import
  warnings; both imports were removed before final tests.
- Intermediate persistent-value checks passed 530 tests before the Variant
  continuation. One earlier core Clippy invocation overlapped the final part
  of that test invocation; no job-count override was used. The final Variant
  validation commands run sequentially.

The 110-line Variant module owns type-table row validation and payload
selection; it adds no I/O, dependency, version, catalog, or duplicated schema.
The existing structural verifier (2,766 LOC / 108,234 bytes) and instruction
verifier (3,664 / 145,987) delegate the shared rule and keep their complete
pass/dataflow responsibilities. Existing AWBC tests (5,067 / 181,038) keep the
fixture/VM consumer tests; the registry matrix is a separate integration test.
Their prior cohesion dispositions remain applicable.

In-flight value admission has a separate 433-line / 16,180-byte production
owner and 384-line / 12,345-byte test child. The plan visitor is 529 lines /
19,484 bytes. They borrow phase-owned tables and share the existing iterative
value encoder; they do not create a second executable type algebra. Long
exhaustive dispatch and graph fixtures remain cohesive rather than being
split solely for a lint line count. Their outstanding consumer migration is
explicit above.

Full goal acceptance remains unproven. Continue the source/producer and
program-bound value migrations under the current autonomy instructions;
the existing design material is evidence, not a reason to wait for another
assignment or create a replacement design request.

## Presentation target layout removal — 2026-09-22

Inspected main and origin/main at
`560baa52ce9901acf03a6f854bf29139111e9b63` with preserved convergence WIP.
This independent delivery removes the obsolete root layout from
`CharacterPresentationTargetEvidence::RuntimeCharacterDialogue`, its strict
wire representation and the unused layout-mismatch diagnostic. The retained
contract and both presentation catalog digests are unchanged. The runtime
consumer matches the target family and does not read a layout. The owner test
now exercises wire round-trip and rejection of the removed layout field.

Validation in the current preserved checkout (not an isolated index build):

- `cargo test -p arcweft-dialogue --all-features`: 40 passed (32 unit,
  4 integration, 4 doctests), `dialogue-tuple-delivery-tests.log`, exit 0.
- `cargo clippy -p arcweft-dialogue --all-targets --all-features`:
  `dialogue-tuple-clippy-final.log`, exit 0. Dialogue has 2 library size warnings
  and 3 test warnings including those 2; dependency warnings remain.
- The structure gate passed: 95 packages, 2,337 Rust files, 1,285,403 physical
  Rust LOC, 312 review triggers, 0 blockers; `dialogue-tuple-structure-gate.log`.
- Formatting and whitespace checks passed. No dependency or feature changed.

Only this presentation owner, the corresponding maintained specification
paragraph and this evidence section belong to the cut. The larger in-flight
Dialogue producer migration and core program/type admission work remain
uncommitted. Their local tests do not establish operational compiler/bundle
publication. Sema all-target/all-feature check passed; its selected Dialogue
suite has 11 passes and 2 unresolved `OpenEffectRow` failures at ordinary and
generic function boundaries (`dialogue-role-sema-tests.log`). The full
convergence goal and required workspace integration remain active. Earlier
workspace check/Clippy/test failures at the removed host-adapter opaque producer
are not claimed repaired by this cut.
