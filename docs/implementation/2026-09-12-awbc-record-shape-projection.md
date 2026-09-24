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

## Dialogue producer continuation after the Variant push

The Variant cut was committed and pushed as
`560baa52ce9901acf03a6f854bf29139111e9b63`; `main` and `origin/main` matched and
the index was empty. The work below is a subsequent **uncommitted** producer
migration, not part of that commit or the preceding 535-test result.

The retained role table's authored `DialogueStage` collides with an existing
View occurrence-state declaration at the same standard path. The current
View carrier is `std.dialogue.stage`, Plain/SnapshotOnly; the configuration
role requires `std.character_dialogue`, Plain/ConstantAndSnapshot. They cannot
be one declaration. The occurrence-state source type is now
`DialogueOccurrenceStage`; the authored configuration role keeps the accepted
role-table name. The field coordinate remains `stage`. Core owns the runtime
name, sema reads it, and the affected View chapter, manifest example and LSP
fixture were migrated. The immutable package mirrors remain unchanged.

The existing `CharacterDialogueRuntimeRole` coordinates are reused. A private
sema registry projects all six exact Standard/Domain, zero-arity opaque rows
from the accepted world before either registrar publishes its environment.
It checks the producer, value class and persistence, derives ordered Style,
retains the world stamp, and contributes its semantic/checked projection digest
to the environment digest. Both registrar paths and their structured errors
were updated. Callable schemas borrow the registry directly through
`DialogueSchemaContext` and reject a custom-field registry from another world.
All six role-related Named placeholders and both local Style compositions
were replaced by those borrowed accepted types.

The older package sketches an intermediate `TypeKind::CharacterDialogueRole`.
Current dialogue schema construction already occurs after accepted-world
publication in the shared callable resolver. Supplying its real registry
directly preserves the complete accepted types and eliminates the need to
introduce and then normalize another placeholder family. This is the selected
source boundary, not a spelling-based resolver or a partial role inventory.

Core now also has `RuntimeProgramTypes`, a borrowed selection of the existing
native or AWBC program. Producer-side persistent admission uses source semantic
identities against that one program, shares the existing value encoder, and
rejects missing or duplicate AWBC identities. It stores no catalog, schema,
generation capability, or fallback. It does not grant operational publication
authority. Dialogue has not yet consumed this context; its schema/constructor
and policy-Variant migration is the next implementation step.

Actual evidence for this continuation:

- Passed: core all-target/all-feature check,
  `dialogue-program-types-check.log`, exit 0; the subsequent typed index-overflow
  diagnostic compiled in the focused test run.
- Passed: two `program_types::tests` tests, proving source identity selection,
  native/AWBC canonical digest parity, mismatch rejection and duplicate-ID
  rejection; `dialogue-program-types-tests.log`, exit 0.
- Passed: 17 existing `value::opaque::tests`, including the dialogue View field
  owner/tamper checks; `dialogue-occurrence-stage-tests.log`, exit 0.
- Failed during dependency compilation: sema all-target/all-feature check at
  Dialogue `schema.rs:83`, which still lacks the mandatory nominal Variant
  layout. The new role registry, registrar/factory changes and sema regression
  have **not compiled or executed**. They must be checked once the actual
  Dialogue policy producer is migrated. No dependency bypass was used.
- Changed Rust formatting and `git diff --check` passed. Full core tests,
  Clippy, workspace tests and structural review have not been rerun for this
  unfinished producer migration; earlier results are not promoted to it.

The Dialogue codec still has the old root/custom/inline nominal wrappers and
layout-only typed values. It must become the retained exact 18-slot opaque
tuple with two-slot custom entries, actual role/custom type admission and
context-derived layouts for all four policy Variant families. No placeholder
layout, tuple-tag/JSON policy bridge, or empty source proof was introduced.

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
## Program-bound Dialogue producer continuation — 2026-09-22

The independent presentation cut above is now pushed as
`7e06d60009dcda046f032a1682a6850828725cb2` (main equals origin/main).
The rest of this section describes preserved, uncommitted implementation.

Supersedes the earlier statement that Dialogue has not consumed
`RuntimeProgramTypes`. The schema now borrows that actual native/AWBC authority,
all six exact role type and payload type references, the ordered Style type,
custom field type references, accepted defaults digests, CharacterCatalog and
ViewRegistry. Construction resolves every reference, including unused role
payloads/custom fields. It checks exact role producer/class/persistence and
computes the View digest from the borrowed registry. Bindings are structural
trusted-integrator inputs, not compiler/bundle publication capabilities.

Encoding/decoding uses the exact Character-derived opaque owner and 18-slot
Tuple. The caller supplies no root owner/layout. Custom entries are two-slot
Tuples; inline failure is a direct Variant. The old root/custom/inline record
wrappers, Dynamic custom schema, duplicated value nominal/layout metadata,
root layout APIs and descriptorless domain digest are removed. All four policy
families derive real layouts from the complete policy schema graph, including
the active exact RichText owner in ordered Style. The graph is discarded after
header derivation; no second executable nominal catalog is retained. The
maintained chapter records why Voice Id remains its validated voice.* String
(the existing identity family has no Voice declaration), correcting the older
package's EntityRef cell without modifying that frozen package.

Role admission checks the exact opaque value and then its body against the
payload source type in the same program. Custom admission validates its complete
value against its descriptor's source type. Style entity references require the
Style family. Domain/catalog checks cover all four contract digests, membership,
look/custom View compatibility, canonical order, limits and canonical re-encode.
Schema-owned bytes/digests follow full admission. Domain equality/hash use the
immutable fields. Local wrappers only normalize and bound data; they do not
claim active program admission. Structured edits and normalization now traverse
opaque payloads while retaining their exact owners.

Core's borrowed type context gained require_type for recursive references.
RuntimeValue::try_digest_with_limits uses the existing iterative encoder and
one logical-value budget across the whole opaque payload. It proves bounded
persistent encoding, not nominal or producer meaning. Large nested program
errors are boxed while preserving their structured source errors.

Sema's role implementation now compiles. The Proof-return registration prelude
retains its already projected role registry until final publication. The
accepted source-type accessor is available to production graph projection;
previously it was test-only. Two empty boxed-slice test arguments and an unused
import were corrected. The new role/occurrence identity test passed.

Actual validation, all sequential Cargo commands with normal parallelism:

- Dialogue library check passed. An initial test compile found a fixture calling
  nonexistent CharacterManifest::id; it now uses character().
- Final `cargo test -p arcweft-dialogue --all-features`: 40 passed (32 unit,
  4 integration, 4 doctests), dialogue-tuple-delivery-tests.log. New evidence
  covers native/AWBC digest parity, all policy cases, complete nested nominal
  body/header rejection, current contract digests, outer owner/arity, two-slot
  custom order/type checks, unused reference preflight, and opaque patching.
- Core program_types owner tests: 3 passed, dialogue-program-bounds-tests.log;
  includes shared opaque payload node/sequence limits and unchanged digest bytes.
- Final Dialogue all-target/all-feature Clippy passed, with 2 production
  function-size warnings and 3 test warnings including those 2. A prior run
  exposed large error and unused-self warnings; those were corrected. The
  static-helper conversion briefly had 7 missed multiline calls; final check
  and tests pass after repair. Dependency warnings remain.
- Sema all-target/all-feature check passed, dialogue-role-sema-recheck2.log
  (19 lib warnings; 4 test warnings, 3 duplicated). Its first selected Dialogue
  suite was 11 pass / 2 OpenEffectRow failures. The complete selected suite now
  passes 13/13 in dialogue-role-sema-tests-recheck.log after the correction below.
- Structure gate before that final private effect-projection correction:
  95 packages, 2,337 Rust files, 1,285,403 physical Rust LOC, 312 triggers,
  0 blockers. No new dependency edges or features. The schema owns admission
  and its fixed codec (965 lines / 36,459 bytes at that check); child modules own
  policy graph derivation (167 / 6,522) and program role references (155 / 5,781).
  The 669-line / 24,339-byte fixture module builds both actual program forms and
  exercises shared scenarios. The two long production functions retain one
  atomic admission and one fixed-field decode responsibility; no arbitrary split
  or public API widening was used to suppress the size warnings.

The OpenEffectRow failures were premature projection of an as-yet-uninferred
project body into a concrete expression effect set. Preparation now computes
intrinsic effects separately. Known fixed/bounded rows contribute their actual
constant effects; an unknown row is deferred only when the exact Project
schema, checked declaration, staged body and UnboundedInference contract agree.
The selected call graph still owns that callee edge. Existing callable closure
and final call sealing must publish the complete closed row. Unknown fixed or
detached rows still fail; no complete unknown row is replaced with purity.
The broader effect suite ran: 80 passed / 9 failed in
dialogue-call-effect-regressions.log. Failures remain in inferred callback rows,
escaping callback rows and curried continuation constraints (UnknownRow and
OpenEffectRow). This is incomplete application-specific symbolic effect
publication, not a reason to weaken the final closed-row checks. Ordinary
function/explicit callback propagation tests in that suite pass.

Operational publication and execution remain incomplete: compiler/bundle
issuance of these role payload bindings, ordinary Project/source graph layout
migration and semantic-batch proof transport, host-adapter result admission,
runtime-accelerator Variant migration, live/native/AWBC/restore typed value
boundaries and the other convergence-goal branches still require completion.
In particular runtime-driver's RuntimeCharacterDialogue presentation target
still rejects without a decoded value; library tests do not establish that
runtime path. Workspace check/Clippy/test-workspace have not been promoted from
the previously recorded host-adapter failure to success. No new goal completion,
blocker, compatibility reader or source-layout reconstruction was introduced.

Additional unblocked checks: accepted_rust passes 8/8 after correcting the
previously uncompiled mixed-graph fixture to current enum payload syntax
`More(Branch<T>)` / `Empty(Unit)`; dialogue-unblocked-accepted-rust-recheck.log.
The first run was 7 pass / 1 recovered-HIR fixture failure. The complete sema
unit suite was then run for the registration/call-preparation boundary:
829 passed / 24 failed (dialogue-unblocked-sema-full-tests.log). The failures
cover the nine higher-order cases above, saved function-value/continuation
origins and contextual constructor/ordinary-call type evidence. Integration
tests and doctests were not reached by that failed full command. This is the
current full baseline for continued repair, not a complete sema acceptance.

Runtime-plan all-target/all-feature checking now reaches final_flow.rs:964 and
fails because the existing semantic batch call lacks the new fifth source-graph
proof argument. The prior Dialogue dependency compilation failure is gone.
The next nominal migration must carry an actual source-issued graph through
compiler semantic facts and correlate all Project/Rust source layouts and
Entry/ownership consumers; neither an empty graph nor target-row reconstruction
can replace that proof. Log: dialogue-unblocked-runtime-plan-check.log.

Sema all-target/all-feature Clippy also completed successfully,
dialogue-unblocked-sema-clippy.log (1,265 lib warnings; 1,459 lib-test warnings,
1,247 duplicated). These warnings were not suppressed and this is not a
warning-free result. No further Cargo sessions from this check remain active.

## Occurrence-stage namespace delivery — 2026-09-22

Inspected main/origin at `7e06d60009dcda046f032a1682a6850828725cb2`.
The authored configuration role and the live View stage previously shared the
source name DialogueStage despite different producers and persistence rules.
The occurrence role is now DialogueOccurrenceStage. Core owns its canonical
name; sema's constant/projections/standard world, the LSP source fixture, the
View chapter and manifest use it. The existing Stage enum and stage field
coordinates stay the same. The configuration role retains DialogueStage.

This cut contains those six implementation/document paths, the selected
CharacterDialogue namespace paragraph, and this evidence. New role registration,
producer tuple/policy/body admission, and the core nominal graph work are still
preserved WIP. The semantic identity changes with the canonical occurrence name
under the unreleased version-one contract; no alias or old reader is retained.

Evidence in the preserved checkout: 17 opaque/View/codec/save tests pass in
`dialogue-occurrence-stage-delivery-tests.log`; sema all-target/all-feature check
passes; the selected CharacterDialogue/role tests pass 13/13. Sema Clippy passes
with 1,265 lib and 1,459 lib-test warnings (1,247 duplicated), recorded in
`dialogue-unblocked-sema-clippy.log`. Its complete unit suite is 829 pass / 24
fail in function-value/currying/contextual inference work, not a full success.
The LSP fixture was migrated but not executed: runtime-plan dependency checking
currently fails at the missing source-graph proof argument (final_flow.rs:964).
No new dependency, feature, instruction rule or build artifact is included.

The occurrence namespace cut is pushed as

df19f953bd34778161595d5649ffd6b4c5929b09

(main equals origin/main; index empty). All recorded validation commands are
terminal. Remaining work and failed checks above keep the full goal active.

## Canonical reducer Result delivery — 2026-09-22

Inspected main/origin at df19f953bd34778161595d5649ffd6b4c5929b09.
The root reducer previously validated a builtin Result case but then passed its
outer one-item Tuple to the Reduction reader. Correct Result::Ok(Reduction)
therefore failed. It now consumes the existing canonical builtin-case API and
passes the inner item. No alternate Result encoding or fallback is retained.

The independent regression accepts an admitted Reduction in canonical Ok and
rejects the bare value, Option wrapper, and an extra Tuple wrapper. The focused
root command passes 6 tests in root-canonical-result-delivery-tests.log,
including that regression and the five program-admission tests in preserved
WIP. Earlier full core tests pass in root-program-admission-core-full.log;
core all-target/all-feature Clippy passes with 128 lib and 155 lib-test warnings
(127 duplicated) in root-program-admission-core-clippy.log. That full run and
Clippy preceded the new standalone regression. Tests ran in the preserved
checkout; no separate staged-index build is claimed. The selected diff contains
only the Result consumer, its standalone test and this evidence.

The larger root/program admission migration and source graph transport remain
uncommitted required work. This cut does not claim complete reducer result ABI,
source nominal lowering, workspace validation, or whole-goal acceptance.


## Stable Project type identity delivery — 2026-09-23

Inspected main/origin at ceebaa5d29a7fe341c41fbccfd5183405bcd43e0.
The two checked-type encoders included the entire source-set revision in a
Project nominal identity. A reducer body-only edit consequently changed the
source graph layout and Entry binding, despite unchanged declarations and data.
Project nominal identity now retains its world/package, module, owner path,
kind, name and ordered arguments without the ambient source revision. Revision
continues to belong to the exact accepted declaration/generation checks. The
version-one encoding evolves in place; there is no legacy reader or alias.

The new regression establishes stable identity/layout after an implementation
edit, rejects that foreign-revision checked declaration even after a same-key
cache hit, and observes a changed layout after a field type change while the
declaration identity stays stable. Both encoder paths have the same rule.

Actual evidence in the preserved checkout: the focused regression passes;
compiler Entry tests pass 17/17, including the former body-only binding failure
and a new constructor-free nested Event payload test; source iterator tests
pass 2/2; bundle runtime resource codecs pass 10/10. Logs use the
source-stable- prefix in the existing ignored validation directory. Full sema
unit tests are 830 pass / 24 fail, with the exact same failure names as the
previous higher-order/contextual-inference baseline. Six-crate all-target /
all-feature Clippy passes (core, sema, runtime-plan, compiler, bundle and
runtime-driver), with existing and reported warnings; this is not warning-free
or workspace-wide Clippy acceptance. Workspace all-target/all-feature check
still fails at host-adapter's removed AdapterRustType::opaque_producer call.

The structural gate reports 95 packages, 2,341 Rust files, 1,286,397 physical
Rust LOC, 310 review triggers and zero blockers. The digest owner still defines
one checked-type grammar; the paired encoders share the nominal identity rule.
No dependency, feature, runtime catalog or generation-bypass API was added.
The selected cut contains only the two digest hunks, the focused regression
and this evidence. No separate staged-index build is claimed. Root/program
admission, source graph transport/definition closure and consumer migrations
remain preserved WIP; their passing tests do not imply whole-goal completion.

## Source graph and root/program continuation — 2026-09-23

Supersedes the earlier statement that compiler/runtime-plan proof transport is
absent. This remains uncommitted work in the preserved main checkout. The two
small delivered cuts in this continuation are canonical Result consumption
(ceebaa5d29a7fe341c41fbccfd5183405bcd43e0) and stable Project type identity.
The latter's full delivered SHA is recorded below after publication.

Root state/event roles now retain identity/semantic identity/layout without a
schema copy. Both role headers are correlated before initializer evaluation or
snapshot acceptance. Initial values, event batches, committed reducer state,
and restored values use RuntimeProgramTypes against the actual native/AWBC
rows. Atomic event failure preserves state and transition cursor. Core's AWBC
codec and both entry verifiers consume the same role contract. Compiler role
issuance and driver replay recording/execution were migrated; no replay schema
copy remains. The mock root evaluators exercise admission through real native
and AWBC type/domain tables, not instruction-level reducer ABI execution.

Project nominal projection now retains its exact checked source request and an
Arc of the complete graph. The TypeShape expander, its named recursive leaves,
and the copied RuntimeTypeSchema were deleted. Entry schema digest hashes the
source layout in its distinct version-one domain; stable type identity repairs
body-only Entry binding drift. Ownership's private top-level nominal validator
uses that graph. Recursive ownership classification and nested aggregate value
admission are still incomplete; this is not C5 completion.

The shared graph projector now owns Project, joined Rust, Character and closed
registered variant definitions under nominal_schema/graph.rs and its children.
Accepted Rust's actual world/generation lease remains; the generic graph error
and limits use RuntimeNominalGraphProjection names. A source-only API obtains
closed variant layouts from the accepted semantic shape catalog. Source field
and case types are retained, not inferred from normalized or AWBC rows. Core
try_merge rejects disagreeing source definitions and keeps exact Bytes formats.
Graph equality compares the defined type graph, independently of policy limits.

RuntimeResolvedNominal and closed variant facts carry their original graph.
Compiler publication completes an owned RuntimeNominalDefinition inventory
from the final source projections, traversing typed arguments, record fields
and case payloads with a visited set and work bound. This supplies definitions
used solely by Entry signatures or nested types. The previous expression-driven
record/variant domain emitters were removed. Final flow lowering passes the
real merged proof to the atomic core semantic batch. The current completion
API consumes the owned facts before compiler publication; further whole-goal
construction/API reconciliation must preserve atomic publication and one final
program type/domain authority.

Bundle's distinct executable compatibility transcript now includes record
shape, explicit field coordinates and optional names. The core shape owns the
shared semantic tag. This is not a recomputation of the source layout hash.
The strict wire fingerprint regression covers distinct empty forms and field
names with a compact section round trip.

Actual validation (all sequential Cargo commands, no jobs override):

- root-program-admission-core-full: 542 passed (498 unit, 44 integration), before
  the standalone Result and graph merge additions; source-final-core-tests:
  500/500 final core unit tests pass. Core integration coverage was not rerun
  after the new graph join/shape tag exposure.
- source-final-runtime-plan-tests: 68/68 final unit tests pass. Earlier
  source-graph-runtime-plan-tests passed 87 tests including integrations and
  compile-fail fixtures before the source-definition closure was added.
- source-stable-sema-tests: 830 passed / the same 24 earlier failed test names;
  the new mixed Project->Rust graph/value and generation rejection tests pass.
  No whole-sema integration/doc success is claimed after that failed command.
- source-stable-entry-tests: 17/17 pass. Earlier runs were 14/2 (missing Event
  domain), then 15/1 (binding stability). Both defects were repaired. The new
  payload-only nested nominal source fixture compiles and verifies.
- source-stable-iterator-tests: 2/2 pass; source-stable-bundle-tests: 10/10 pass.
  The first bundle test compile used a wrong fixture type name AwbcFieldType;
  it was corrected to AwbcRecordField before the passing rerun.
- source-stable-clippy: six selected crates pass all-target/all-feature Clippy.
  Library warnings: core128, sema1255, runtime-plan149, compiler71, bundle12,
  driver12. Lib-test warnings include core155 (127 duplicate), sema1449
  (1237 duplicate), runtime-plan151 (148 duplicate), compiler77 (68 duplicate),
  bundle13 (11 duplicate), driver12 (12 duplicate). Integration warnings remain.
- source-stable-workspace-check: FAILED at host-adapter lib.rs503, the removed
  AdapterRustType::opaque_producer API. Structural Rust results need their real
  program/type admission; an opaque placeholder would be incorrect.
- source-stable-structure-gate: 95 packages, 2341 Rust files, 1,286,397 physical
  Rust LOC, 310 ownership-review triggers, zero blocking dependency violations.
  No new crate edges, features or I/O ownership were added. No workspace
  Clippy/test-workspace or full convergence acceptance is claimed.

Ownership review measurements at that gate: core root.rs1131LOC/40977bytes
(production) and root/tests.rs447/16320 (test); program_types.rs257/9631 and
schema/nominal.rs451/14245 (production). Sema nominal_schema.rs2183/88343 retains
source request/seal relations; graph.rs586/24300 owns the mixed source traversal.
Runtime-plan semantic_facts.rs10522/399578 retains the generation-bound fact
vocabulary, with the new nominal_definitions.rs153/5751 owning definition closure.
Compiler lower.rs7888/334004 remains the checked-source normalization boundary.
These are existing cohesive cross-family owners; this change does not split
fields by file size or create public wrappers to pass metrics. The new child
uses existing record/variant models and the source provider; executable tables
remain core-owned. Source-definition and producer APIs still require the full
goal's remaining consumer reconciliation rather than a declaration of closure.

Required remaining work includes Rust ADT compiler normalization and host-result
admission; recursive ownership and nested retention; all program-bound live and
snapshot C5 gates; operational CharacterDialogue compiler/bundle/driver role
binding; the 24 symbolic-effect/continuation/contextual inference failures; and
the retained Match/View/task-plan/scheduler sequence. Those repository-resolvable
items keep the original seven-part goal active, not blocked or complete.

Stable Project type identity was pushed as
265ccea1da9c51afd8916240422ff4b673383e08.
main equals origin/main; the index is empty. All recorded validation processes
are terminal. The original goal remains active with the required work above.

## Rust callable overload registration repair — 2026-09-23

An actual multi-function Rust ADT compiler fixture failed registration with
NonContiguousOverloads: bool_node had overload 4 although it was the first
callable at that path. AdapterManifest assigned ordinals across the entire Rust
manifest. Ordinals now follow each adapter callable path, including earlier
Rust package publications and explicitly declared adapter functions. Separate
paths start at zero; repeated paths continue their existing inventory.

The focused cut contains the two manifest implementation hunks, a standalone
primitive-returning regression for interleaved paths across two Rust packages,
and this evidence. Opaque-carrier removal and the structural Rust normalization
migration remain separate uncommitted work. No branch or worktree was created.

Validation against the current working tree: cargo test -p
arcweft-adapter-context --all-features passed 25 tests (23 unit, two integration;
zero doctests); cargo clippy -p arcweft-adapter-context --all-targets --all-features
passed without warnings. Changed Rust was formatted and the staged whitespace
check passed. Logs: rust-overloads-adapter-tests.log and rust-overloads-clippy.log
in the ignored 2026-09-11-effect-row-formulas validation directory. The separate
Rust compiler integration test also passed after this fix. These focused checks
do not replace the still-required full convergence checks; the goal stays active.

## Accepted Rust source ADT normalization — 2026-09-23

The compiler now retains accepted Rust metadata and its exact world/HIR lease in
the same structural nominal source model as Project declarations. The native
RuntimePlan and AWBC use the accepted source graph for unit, tuple, record,
newtype, generic and recursive instances; no Rust HIR owner or opaque producer
is fabricated. A real adapter-manifest/source integration compiles seven Rust
return types, preserves two distinct Node<T> instantiations, and rejects
incorrect nested values and wrong empty variant payload forms on both plans.
The plan validates the accepted Rust generation again when consumed. The
nominal source proof still needs full host result transport, live value gates,
snapshot restore and ownership work before C1-C6 can be called complete.

Focused evidence: rust-nominal-core-tests (500 unit + 44 integration pass),
rust-nominal-runtime-plan-tests (68 unit + 19 integration pass),
rust-nominal-sema-tests (9 accepted-Rust pass), rust-nominal-cache-tests
(21 compiler integration pass), rust-nominal-entry-tests (17 pass), and
rust-nominal-clippy (four selected crates, all targets/features exit0 with
warnings) in the ignored validation directory. These were run before the last
dead-error/typed-error cleanup; check the final changed bytes before publishing.
No workspace acceptance is claimed. The unrelated sema baseline still has the
same 24 known failures. The full convergence goal stays active.

## Program-backed live host-result admission — 2026-09-23

The existing native Plan and AWBC type tables now expose bounded live-value and
snapshot-candidate checks alongside canonical encoding. Live admission accepts
exactly owned SnapshotOnly and affine opaque values; snapshot admission permits
SnapshotOnly plain values but rejects affine handles. Canonical constant encoding
keeps its previous rejection and diagnostic. The shared traversal still checks
nested children and scalar validity. `RuntimeProgramTypes` selects the existing
program by semantic ID without retaining a parallel schema.

Raw native HostCall resume validates the returned value against its retained
Plan-local result type before any pattern binding, even with no binding. AWBC
validates against the pending call signature before assigning a register or
resuming; a discarded Unit result is checked too. A wrong raw result fails the
fiber with a host/ABI diagnostic. Focused native and AWBC regressions and the
full `arcweft-core` suite pass (503 unit tests plus all integration, compile-fail
and doctest groups). All-target/all-feature core Clippy exits zero with existing
warnings. A strict `-D warnings` run failed first in unchanged dependency
crates on new Clippy lints; this is not a strict-lint acceptance claim.

At this checkpoint, the manifest host registry and TaskOutcomeContract still
held finite checked result predicates, and `arcweft-host-adapter` still called
the removed Rust opaque-producer API. The next section records their subsequent
program-bound migration. No whole-workspace validation was claimed here.

## Program-bound host and task transport integration — 2026-09-23

The next integration replaces manifest-host result predicate projection with
the manifest contract digest, call mode and result semantic identity. The
registry now rejects nested `Need` after the outer suspend modality is removed;
there is no Rust opaque-producer reconstruction. `TaskOutcomeContract`
distinguishes explicit standalone finite contracts from program-owned semantic
rows. `BoundTaskOutcome` retains the exact Plan or AWBC program and validates
live payloads and Result carriers through its type table. The native bridge,
desktop pending work and Agent controller use that bound context. AWBC task
lowering reuses an admitted semantic row rather than interning a detached
checked predicate; native Await and AWBC task resumes use program admission.
Root-command pending results retain their program context through live routing,
recording and replay, including ignored successes.

Current focused evidence: `arcweft-core` full suite passes 507 unit tests plus
all integration, compile-fail and doctest groups after the standalone-contract
and Agent protocol-record regressions. `arcweft-host-adapter` passes eight tests and
`arcweft-adapter-desktop` passes seven. `arcweft-agent-runner` passes 65 after
the shared AgentResourceBody fixture was updated to canonical unary payload
tuples and both program admissions were aligned with the existing core
`accepts_protocol_record` authority. Compiler and runtime-driver all-target,
all-feature checks pass; `arcweft-runtime-driver` passes 67 unit tests and its
integration groups after the root-command acknowledgement rollback. No
all-workspace result is claimed.

The current `arcweft-runtime-host` check stops earlier in its dependency graph
at `arcweft-runtime-accelerator/src/external.rs:543`: shaped `data.decode` still
tries to synthesize a nominal Variant from a `TypeShape` digest and now lacks
the mandatory program-owned layout. The active producer-closure request
explicitly rejects substituting the digest for layout. Astra Max and Luna Max
traced the missing selected-program result context and the lossy DataShape
descriptor contract; the typed data producer and codec path require a full
source/program-bound reconciliation. This compile failure is not presented as
an external blocker or as a successful runtime-host validation. C5/C6 restore,
standalone constructor closure, hot replacement and broader convergence
acceptance remain unfinished; the goal stays active.

## Data producer reconciliation selected for implementation — 2026-09-23

The Data boundary cannot obtain a nominal enum layout from a `TypeShape`
semantic digest: the selected Plan/AWBC program owns the exact nominal graph,
semantic type, layout and case ordinals. The final path therefore needs codec
metadata on the existing type/field/case rows, a `DataShape<T>` witness bound to
that program and semantic row, and one graph-aware data codec view over those
rows. The standalone data reflection graph can use the same lower codec API;
it does not become a second persistent runtime schema. The existing raw Record
transport drops payload, rename, policy and generic-instance data and must be
deleted when its replacement is connected.

The public source operations must distinguish typed decode/encode from a
dynamic `DataValue` result and return `Result<_, DataError>` for ordinary codec
errors. Native expressions and AWBC intrinsic signatures must carry the exact
selected input and result type rows to the operation. The data value algebra
must retain Option/tuple form, typed map keys and ordering; a field default
requires an admitted value or exact pure callable, not the existing bool flag.
Core owns runtime-value conversion and admission; concrete codecs own byte
conversion. This is the chosen integration direction, not a validation claim.
The in-flight data owner and external-call-context edits have not yet been
compiled together, and the full native/AWBC codec, restore and mutation-failure
acceptance remains outstanding.

The graph-aware `arcweft-data` trait now supplies `ShapeRef` plus a selected
`ShapeAccess`, and `Value` distinguishes Option, Tuple and ordered typed-key
Map. The reflection derive registers recursive and mutually recursive nodes
without identifying types by spelling. The nine concrete codec crates compile
with all targets and features, and their all-feature test suites pass, including
format-specific rejection of unsupported shapes. `arcweft-data` and derive
tests also pass. These results establish the standalone codec boundary; they
do not validate the source `data.*` calls or selected-program runtime producer.
At this checkpoint HEAD is `233ac21d8664da4a71dbff4150bc5b78b2a568ab`
on `main`, with 352 dirty/untracked paths observed; the implementation remains
uncommitted and in progress.

The selected codec policy belongs to each typed use of a logical type, not
solely to the shared semantic type row. For example, separate nominal fields
may use the same `Seq<Bytes>` row while declaring Base64 and Hex for its Bytes
element. The selected representation is a codec-only occurrence tree attached
to the existing root/field/case/argument edges. It carries presentation
attributes and child occurrence structure, while semantic IDs, nominal
identity, layout, field IDs, case ordinals and MapKind stay on their existing
type/domain rows. Admission zips the occurrence tree with those typed child
slots and the original source schema proof before publication. A borrowed
runtime view can index `(type row, occurrence)` pairs for graph-aware codecs;
it does not retain a second declaration graph or infer a policy from observed
values. The earlier plan to put a single BytesFormat on a shared Bytes row is
superseded by this use-site decision. Implementation and acceptance of this
occurrence boundary are still pending.

## Data codec and default-provider checkpoint — 2026-09-23

The codec-only occurrence tree is now admitted with the selected Plan/AWBC
type graph. A borrowed `RuntimeProgramDataShapes` view retains field/case/child
occurrences separately from semantic rows, so two fields sharing one Bytes node
can select different Base64 and Hex representations. `arcweft-data` exposes a
graph-aware `ShapeRef`/`ShapeAccess` codec boundary; the nine concrete codec
crates and save/HTTP callers use it. Missing or skipped record fields request
their original record occurrence and field ordinal from an explicit default
provider. A bare `has_default` flag cannot synthesize a runtime value. CSV,
Arrow/Parquet and Avro use the same request boundary in their own decoders.

Current focused checks pass: the full `arcweft-data` and nine-codec all-feature
test invocation (including new shared-Bytes and default-provider tests),
`arcweft-config` (eight tests), save/HTTP codec tests, and `arcweft-core` (538
unit tests plus its integration and compile-fail groups). The full
`arcweft-runtime-plan` all-feature suite passes 71 unit tests, three
compile-fail tests and its remaining integration groups. `arcweft-agent-runner`
passes 66 tests both with default and all features after serializing its
admitted RAG context from the typed payload for stable JSON key order. These
are local working-tree results; they do not establish source/native/AWBC
end-to-end Data acceptance or workspace-wide validation.

The outstanding integration remains material. Source DataFormat nominal
projection and generic effect-row completion are under repair; the accelerator
still has to replace its inferred descriptor/nominal path with selected-program
value conversion and `Result<_, DataError>` construction. Rust field-default
metadata now retains the declared provenance through sema/ABI, but its
accepted pure callable binding, Plan backend execution and full producer test
are in progress. No C5/C6 or full convergence completion is claimed. The
observed checkout remains `main` at
`233ac21d8664da4a71dbff4150bc5b78b2a568ab`; this checkpoint has not yet
been committed or pushed.

## Integrated verification in progress — 2026-09-23

The Rust pure default producer now has an accepted nullary callable proof,
Plan/AWBC binding, and result verification. The accelerator now uses the
selected program's DataShape witness and returns typed DataValue/DataError
results. JSON, YAML, and TOML traverse enum payload occurrences, including
per-field Bytes presentation. Focused producer, accelerator, and codec suites
passed; the remaining generic-call constraint regressions are still under
repair. This supersedes the outstanding producer/accelerator description in
the previous checkpoint.

The compiling checkout passed `cargo check --workspace --all-targets
--all-features` and `cargo clippy --workspace --all-targets --all-features`
(warnings retained). The canonical structural gate scanned 96 packages and
reported 321 review triggers and zero blocking violations. The first
`just test-workspace` attempt stopped while linking test binaries: its log
records LLVM `no space on device` and MSVC LNK1180 insufficient disk space.
The D: volume had about 323 MB free; `target/debug/incremental` alone held
174,231,363,966 bytes. A further `cargo clean` removed 278,628 generated
files (264.9 GiB) and restored about 273 GB free. The workspace test has not
passed; the next attempt will use `CARGO_INCREMENTAL=0` as an environment-only
capacity measure while keeping the repository's test profile, features, and
normal Cargo parallelism. These results precede the ongoing generic-scope
edits and require final revalidation before a coherent commit.

## Structural owner review — 2026-09-23

Inspected `main` at `233ac21d8664da4a71dbff4150bc5b78b2a568ab`. The dirty
checkout contains 450 paths (335 modified, 6 deleted, 109 untracked). The
canonical gate log at
`.arcweft-local/validation/2026-09-23-structure-audit.log` reports 96 packages,
321 review triggers, and zero blocking violations. Measurements compare
complete physical files at that base with the current working tree; byte counts
are base-to-current and LOC counts include blank lines.

| Owner | Bytes, base → current | Physical LOC, base → current | Responsibility and disposition |
| --- | ---: | ---: | --- |
| `crates/arcweft-runtime-accelerator/src/external_data.rs` | 0 → 56,737 | 0 → 1,482 (+1,482) | Private selected-program Data call boundary: signature and witness admission, codec dispatch, typed-value conversion, and default execution share one `DataCall`/shape context. Keep together; there is no second schema or independently owned runtime state. The companion tests are in `external_data_tests.rs`. `SIZE001` remains a nonblocking owner review. |
| `crates/arcweft-codec-json/src/lib.rs` | 28,776 → 50,768 | 808 → 1,397 (+589) | One JSON parser/emitter and its `ShapeRef`-aware wire projection; `arcweft-data` remains the shape authority. Keep format-specific parsing and projection together in this cut. `SIZE001` and the `SIZE002` facade-size review remain open; no claim is made that the 250-LOC facade target is met. |
| `crates/arcweft-codec-yaml/src/lib.rs` | 25,894 → 45,931 | 694 → 1,250 (+556) | One YAML parser/emitter and graph-aware projection; source preflight is already isolated in `source_preflight.rs`. No separate state or dependency owner warrants a further split here. `SIZE001` and `SIZE002` remain open reviews. |
| `crates/arcweft-codec-toml/src/lib.rs` | 26,529 → 46,073 | 732 → 1,267 (+535) | One TOML parser/emitter and graph-aware projection; source preflight is already isolated in `source_preflight.rs`. No separate state or dependency owner warrants a further split here. `SIZE001` and `SIZE002` remain open reviews. |
| `crates/arcweft-runtime-scheduler/src/lib.rs` | 23,190 → 38,438 | 647 → 1,050 (+403) | One deterministic Sans-I/O scheduler state machine owns pending, in-flight, join, cancellation, and terminal-task transitions; host I/O remains in adapters. Keep the state transition owner intact. `SIZE002` remains open because the root exceeds the 1,000-LOC facade review threshold. |
| `crates/arcweft-lang-sema/src/callable/constraints.rs` | 92,898 → 106,480 | 2,281 → 2,590 (+309) | One affine callback/checkpoint driver closes the candidate constraint transaction. Its 1,583 embedded test LOC exercise those private close-once invariants. Keep the driver as one algorithm and record this as the cohesion disposition for the upper `SIZE001` trigger; `TEST001` is review-only. |
| `crates/arcweft-runtime-host/src/native_task.rs` | 47,292 → 60,605 | 1,331 → 1,679 (+348) | Native task routing, completion, and standard host adapters form one host-side lifecycle; scheduler state stays in the Sans-I/O scheduler crate. Its 460 embedded test LOC cover that bridge. Keep the lifecycle together; `SIZE001` and `TEST001` remain nonblocking reviews. |
| `crates/arcweft-data/src/raw.rs` | 29,492 → 40,684 | 811 → 1,130 (+319) | Recursive typed/raw conversion shares one graph-aware traversal and field-default request context. It remains the format-neutral Data boundary with no duplicate schema authority; no split is indicated. |

These dispositions follow the current state and dependency boundaries rather
than the LOC numbers alone. The codec and scheduler `lib.rs` facade-size notices
remain explicit open reviews; this audit does not claim a facade decomposition.
No Cargo command was run for this structural review.

## Data and codec owner cuts merged to main — 2026-09-23

The existing `main` checkout started this harvest at
`233ac21d8664da4a71dbff4150bc5b78b2a568ab`, with 452 expanded
dirty/untracked paths before selective staging. The remaining working tree
is still dirty; these commits establish only the listed completed owners.

| Boundary | Pushed full SHA | Validation |
| --- | --- | --- |
| Graph-aware Data shape/value/default contracts, reflection derive, and nine codec adapters | `76a40ff652a9c994322be491a3d607f25cbc1640` | All-feature tests and all-target Clippy for `arcweft-data`, derive/support, and all nine codec crates passed; Clippy retained warnings. |
| Rust ABI codec policy and pure field-default metadata | `96e39d38f49a099d049bfc9ef3f4320f905372b9` | All-feature Rust ABI/macro tests and all-target Clippy passed; Clippy retained linker warnings. |
| Typed Map/Option config merging | `3daf1cfa5675a07a10528af4259ad6b1fca6fb3d` | Eight config tests and all-target Clippy passed. |
| HTTP codec ShapeRef/ShapeAccess use | `7e4be12354f5fb56a52340f9a93291fa54ef72e8` | Eight negotiation tests and all-target Clippy passed. |
| Save envelope ShapeRef/ShapeAccess use | `56524270979a61635a5a5168510340d7160283ce` | Save tests, including compile-fail coverage, and all-target Clippy passed. |
| Rust ABI build fixture migration | `9c330783c3b55b817764315b5c9bda6029746705` | Two Rust ABI build tests and all-target Clippy passed. |
| Shared Data derive ownership documentation | `7ed03ab9bd5ee187a92d33d59bd82eefe4c91e0c` | Crate map reviewed against the admitted derive owner. |
| Program-owned core runtime types and outcomes | `0f06b58efd93bfcb60cb136b431f7c0214e3b0d2` | Core unit and integration groups passed; core Clippy passed. |
| Scheduler-bound task outcomes | `c951cd3c80e089d8efd025f133f79268daf8067c` | Fourteen scheduler tests and Clippy passed. |
| CharacterDialogue runtime schema and policies | `226ce68d1587aaf8d232dccb7baf3bf96650c1fc` | Focused core dialogue tests and Clippy passed. |
| Program-owned nominal/default RuntimePlan facts | `2599581c882a0758b09b55a00b8ef6a92ff` | RuntimePlan unit, compile-fail, and integration groups and Clippy passed. |
| Typed Rust ABI adapter metadata | `337082aa1b43fbe9324c0324c84500b8ef6a92ff` | Adapter context tests and Clippy passed. |
| Typed Rust nominal metadata registration | `a35240c8ee8cd41be4fb22c277d867863f8147cc` | Eleven tests and Clippy passed. |
| Selected-program external Data shapes | `1625e023c472f5e55dcc232188423fb326794a4c` | Ninety-one accelerator tests and Clippy passed. |
| Typed runtime resources in bundles | `1e910b92369f876fc1f2dd34de18d2e05774f05e` | Bundle all-feature tests and Clippy passed. |
| Host adapter task outcome binding | `39499f907511fb3deca0e439230917bdf42479fa` | Eight tests and Clippy passed. |
| Native task completion and program outcomes | `0de97d2751a1ba0584800e14129161a27d9439a0` | Runtime-host library check and Clippy passed; package tests deferred during concurrent sema edit. |
| Root command result program binding | `8f3b8cff25c3c0a5be27c0a3638e87067ae521e6` | Runtime-driver library check and Clippy passed; package tests deferred during concurrent sema edit. |
| Agent controller result program binding | `c7da5aaf522287a66ad9cd8f9e63ed92d487bfdb` | Agent-runner library check and Clippy passed; package tests deferred during concurrent sema edit. |
| Desktop adapter task outcomes | `39faa16755328c72b97c64ca53b381cd50dbe856` | Seven desktop-adapter tests and all-target Clippy passed. |
| Native player task error propagation | `ace3e733c28253d5da9d52d686997f7c1d539ea9` | Player library check and Clippy passed; package tests deferred during concurrent sema edit. |
| Maintained nominal/dialogue runtime contract | `22fbb498665e8ffc99ce04c8d5e9b1fc2cdb5fed` | Changed prose, links and executable example reviewed; documentation-only check passed. |
| Nominal graph implementation evidence | `e9642599e2143ab7535b58a11fcc81a65408b479` | Historical checkpoint and referenced links reviewed; documentation-only check passed. |

Each commit staged only its named dependency closure and was pushed by a
non-forced fast-forward to `origin/main` after `git diff --cached --check`.
The local test logs are under `.arcweft-local/validation/`, including the
`2026-09-23-*-harvest.log` and later package-specific check/Clippy logs.
These are incremental owner cuts, not an integrated acceptance result. The
inspected `main` is `e9642599e2143ab7535b58a11fcc81a65408b479`, and the
working tree still has 142 Git porcelain entries. Correlated callable
constraints, effect rows, source Data integration, nominal runtime consumers,
the deferred package tests, and the remaining goal acceptance require final
verification.

## Integrated owner harvest checkpoint — 2026-09-23

Supersedes the preceding dirty-tree count as an operational checkpoint.
Inspected local `main` and `origin/main` at
`ad65af6296c0f770495a89a76694cdee6ad65841`; the Git working tree and
index were clean. The remaining owner migrations were pushed through the
following full SHAs:

- Compiler accepted nominal/Data lowering and source-policy fixtures:
  `edc4e54bb46d59722da2ff8e94c3e3642a48478b`.
- HIR/sema correlated applications, checked nominal facts, and graph-bound
  pending effect prerequisites: `4c852688f348d397be2992990008f3c5b9a113ed`.
- Native player fixture lifecycle and formatting:
  `9139f8e8702425ee26cb4175989f7418b91363fb`,
  `8c216b1e23da29a57ec12b793bd6c716d66034e4`.
- Checked LSP/verify-LSP consumers and compile-fail diagnostic fixture:
  `5c29f3d169bfafd428db43cad66c07c268d70809`.
- CLI bound-task consumers: `ad65af6296c0f770495a89a76694cdee6ad65841`.

The clean integrated HEAD passed `cargo check --workspace --all-targets
--all-features` and `cargo clippy --workspace --all-targets --all-features`
with warnings. The focused native-player library suite passed 48/48;
runtime-host and runtime-driver package test groups passed; LSP/verify-LSP
package suites passed after updating one compiler-diagnostic-only `.stderr`
fixture. Compiler Data nominal, environment-record pattern and iterator
targets passed, while `callable_execution` passed 67/81. The full sema library
suite passed 870/878; its remaining eight failures are effect-row inference
and callback/function-value cases. CLI unit tests passed 168/168, but four
fixture-suite tests still fail at their first checked/run input. These are
separate from workspace check/Clippy success. A complete `just test-workspace`
and final goal acceptance have not passed; effect rows, callable execution,
CharacterDialogue producer, Match, View, task-plan, nominal/scheduler/restore
acceptance and the applicable Tier 2 evidence remain required.

## Selected host and effect boundary continuation — 2026-09-23

Inspected `main` and `origin/main` at
`9c49b09ee78f42f2ae9683ef5973ef2969df2921` with a clean working tree
and empty index before this note. The following coherent cuts were pushed:

| Owner cut | Full Git SHA | Observed evidence |
| --- | --- | --- |
| Omitted function-effect rows in typed local/pattern bindings | `dd0a04ded372a2a18c024a5b3cd250aaf6e9b718` | Sema 873/879; six prior higher-order effect failures only; affected Clippy passed. |
| Signal `Watch<T>` setter and bodyless trait receiver facts | `7cba825fed854eaa98aadd94a95c7b2f6a736473` | Sema 875/881; six prior failures; Signal CLI run and trait CLI check fixtures passed; Clippy passed. |
| Final selected-call target effect availability | `add67ef212099a70a0e68a64c652a23c1657fb72` | Sema 878/884; six prior failures; adapter-sema target tests and affected Clippy passed. |
| Program-owned VirtualPath and selected manifest host calls | `5bd7e40a88fe96fe166dbbfa7d3a63e5d27fc49e` | Core 543/543; native/AWBC file I/O and selected-host contract 5/5; real 010 spec check; affected 11-crate Clippy and workspace rustfmt passed. |
| Explicit native CLI stdout/stderr/exit with typed `Never` | `9c49b09ee78f42f2ae9683ef5973ef2969df2921` | Adapter-context 24/24, host-adapter 9/9, runtime-host 5/5, native/AWBC CLI contract 8/8, real 001 spec run; sema 878/884 with the same six failures; affected Clippy and workspace rustfmt passed. |

The six sema failures require symbolic higher-order callback rows and
application-specific substitution; no final inference acceptance is claimed.
The CLI fixture suite also reaches `020_relative_ids.arcw`, whose named scope
is accepted by sema but lacks runtime-plan/Core/AWBC identity projection.
The remaining goal phases, `just test-workspace`, structural refresh, and
applicable Tier 2 validation have not been completed at this checkpoint.

## Higher-order rows and named scope execution — 2026-09-24

Supersedes the preceding six-failure sema and `020_relative_ids` observations.
Inspected local `main` and `origin/main` at
`3a075b9ed5f303fcd411c947c7bf49d0c84f78d6`; the working tree and index
were clean immediately after the non-forced push. The following cuts are on
`main`:

| Owner cut | Full Git SHA | Observed evidence |
| --- | --- | --- |
| Selected host implementation checkpoint | `2989007a448bd4611d48ee9f4b64ff5a34070178` | Documentation-only checkpoint of the preceding tested host cuts. |
| Symbolic higher-order effect rows per application | `ff54425fb30fbaf6c68ca061920464a24fec8f26` | Focused higher-order tests 15/15; sema library suite 885/885; all-target Clippy passed with warnings. |
| Shared HIR/sema named scope identity | `e4e0be04ddfd3d33ff024402b5370a9ebfaec8de` | HIR mixed namespace test 1/1, sema suite 885/885, HIR/sema Clippy passed with warnings. |
| Typed native and AWBC named scopes, static scope IDs, codec and restore validation | `3a075b9ed5f303fcd411c947c7bf49d0c84f78d6` | Compiler native/AWBC scope acceptance 3/3; core scope tests 11/11 and checkpoint 1/1; runtime-plan sibling/parent definition test 1/1; driver cleanup save/load test 1/1; `cargo fmt --all --check` and affected 10-crate all-target/all-feature Clippy passed with warnings. |

The CLI `current_check_fixtures_pass` suite compiled and passed the previous
`020_relative_ids` and following `021` inputs, then failed at
`022_family_relative_test_bench_ids.arcw` with an incomplete nominal type
resolution for HIR TypeId slot 17. That fixture has no Scope syntax; the
nominal failure remains unclassified and is not claimed as acceptance. The
first scope execution cut explicitly rejects `?` crossing a carrier block's
lexical Scope until its typed success/residual continuation is connected.
Native/AWBC acceptance of that propagation, the remaining CLI fixtures,
compiler callable execution, Match/View/task-plan/nominal/scheduler/restore,
the final workspace gates, `just test-workspace`, structural refresh, and
applicable Tier 2 evidence remain required.

## Scope propagation and script-root checkpoint — 2026-09-24

Inspected local `main` and `origin/main` at
`3d1da68c55361cc8c9dd59869f1d703b96680d77`. The index was empty and
the working tree had 20 porcelain entries from the in-progress HIR/sema
script-root partition and callable-state migration. The following completed
cuts were pushed separately:

| Owner cut | Full Git SHA | Observed evidence |
| --- | --- | --- |
| Preserve unresolved nominal candidate through dot fallback | `0089fb7698d876589d68ebe21739a066c5d812eb` | Sema library suite 886/886 and all-target Clippy passed with warnings. CLI fixture `022` advanced past its prior nominal TypeId error but did not pass. |
| Typed carrier continuation across lexical Scope | `9b6a1b96f248cefddfe05277530449b4b2117987` | Compiler `scope_propagation` 7/7, `named_scopes` 3/3, `try_pipe` 8/8; runtime-plan forged-fact admission 1/1; changed-crate Clippy, formatting, and whitespace checks passed. Native/AWBC results include active-scope snapshot and restore. |
| Nested test/bench script command validation | `3d1da68c55361cc8c9dd59869f1d703b96680d77` | CLI nested-scope command test 1/1, all-target Clippy, exact-file rustfmt and staged whitespace checks passed. |
| HIR Language/script-root expression partition | `14b9312c3090a595788a422d9b94791c37a15a36` | HIR library suite 901 passed, 0 failed, 8 ignored; focused regression excludes Test/Bench script expression and owner families while retaining a called Flow body; exact-file rustfmt and staged whitespace checks passed. |

The first HIR library run was 900 passed / 1 failed because the new fixture
recovered during parsing; after correcting it and adding a Flow-body preservation
assertion, the complete HIR suite passed. The CLI `current_check_fixtures_pass`
suite still stops at `022_family_relative_test_bench_ids.arcw`: the nominal
candidate error changed to ordinary callable resolution of `expect`, then to
`semantic expression ExprId(... slot: 13 ...) has no admissible final type`.
Sema execution-effect sealing was found to fold Test/Bench manifest roots outside
the selected Language owner graph; its correction remains uncommitted and
unverified against the CLI fixture. The HIR pass alone does not prove the fixture
or the whole suite passes. The compiler `callable_execution` suite remains
67/81, with fourteen native/AWBC failures across seven required families.
The program-owned callable-state migration and all later goal phases and final
workspace gates remain open.

## Script-root semantic effect partition — 2026-09-24

At `3ad1a67091105bab4967d1dc001391d53bd74191`, local `main` and
`origin/main` matched, the index was empty, and 90 working-tree entries belonged
to the in-progress callable-state migration. The HIR partition cut
`14b9312c3090a595788a422d9b94791c37a15a36` also passed affected
all-target/all-feature Clippy with warnings. The Sema script-root cut
`3ad1a67091105bab4967d1dc001391d53bd74191` passed its focused regression
1/1, the full Sema library suite 887/887, and all-target/all-feature Clippy
with warnings; exact staged paths and `git diff --cached --check` were verified
before push. Sema now excludes Test/Bench script roots from language expression,
local, type, pattern, capture, and execution-effect facts while keeping their
item facts and called Flow body facts. The CLI `022` fixture remains unrun after
this cut because the AWBC/runtime-plan callable lowerer is still under migration.
The callable execution tests, final workspace gates, and later goal phases remain
open.

## Program-owned callable state checkpoint — 2026-09-24

The integrated monomorphic callable cut was committed and pushed to `main` in
three reviewable layers:

| Layer | Full Git SHA |
| --- | --- |
| Core/AWBC callable state, execution, codec, verifier, ownership and sequence-kind layout | `a7f3573baa33e703ec42f22a95174682f0f0d617` |
| Final-HIR/Sema callable values, compiler projection and runtime-plan lowering | `94e7dafe434dfabae93339ae695bf1a844fa5423` |
| Runtime consumers, exact AWBC program lease, save/restore and corrected design contract | `ee24824429a810df4c3b2a1b89555ea4f6c094fd` |

On the integrated tree, Core library tests passed 550/550, HIR 902/902
(8 ignored), Sema 887/887, RuntimePlan 73/73, and Bundle 143/143. The
runtime-codegen exact-program-lease regression passed 1/1. Runtime-driver
callable save round-trip and foreign-program rejection each passed 1/1.
`cargo fmt --all -- --check`, `git diff --check`, and all-target/all-feature
Clippy for Core, HIR, Sema, RuntimePlan, compiler, runtime-codegen, and
runtime-driver succeeded; Clippy emitted warnings. The CLI fixture
`022_family_relative_test_bench_ids.arcw` passed direct `compile --emit check`
with zero warnings and zero obligations.
After the integrated `main` push, the workspace all-target/all-feature Cargo
check also passed on the pushed source with warnings.

Compiler `callable_execution` passed 81/87 on Native and AWBC. The six remaining
failures are the two backend cases for each of CharacterDialogue factory,
generic prefix as a monomorphic callback, and shared prefix with distinct later
types. These are required next-cut implementation gaps, not accepted failures or
skips. CharacterDialogue needs the complete typed producer and consumers;
generic/shared prefix needs scoped scheme transport and specialization across
Sema, runtime types, callable states, and both backends. The final workspace
gates and later goal phases remain open.

## Effect and CharacterDialogue integration checkpoint — 2026-09-24

Inspected local `main` and `origin/main` at
`3e28bf901925f8698ed030e8d81c80807ce1a94c`: they matched, the index was
empty, and 39 working-tree entries remained in active CharacterDialogue and
generic-predicate work. The earlier independently pushed cuts were:

| Cut | Full Git SHA | Observed evidence |
| --- | --- | --- |
| Shared Core/Sema effect-row algebra | `9778fb4615c4fd32a1ffcce5e1ddcb01236dd126` | Core focused 15/15; Sema effect-row focused 17/17; affected library check and Clippy passed. |
| Canonical scoped effect declarations | `8b95757ef726701a015d69eb706784b9464c2aa1` | Core focused 4/4, library check and Clippy passed. |
| Shared CharacterDialogue patch coordinate/operation | `b10946819ee3b7b49c5b6b9e36e18fe5be1e7238` | Shared dialogue model focused 4/4; Sema Character target focused 12/12. |
| Whole runtime-value callee reachability | `3a2b084ebe029596ff32c3416515eb73e2844199` | HIR focused 1/1; Sema Character target 12/12; compiler Character projection subsequently reached 5/5 after Core binding admission. |
| Scoped runtime type binder and function effect contract | `6e693d0075675937efe7e5205e5b9d519de34e84` | Core scope focused 4/4. |
| Exact AWBC program lease shared by session, executor and View | `d31cfc8bce2c85f479e3a5782da21c12c605518f` | Driver library check and exact-lease regression 1/1 passed. |
| Logical Character membership without a visual manifest | `90cb2d23f734dbfc276291b9dfd2e589f6b5c38a` | Character library suite 49/49. |
| Typed Core/Native/pure/AWBC CharacterDialogue operation and version-one wire | `3e28bf901925f8698ed030e8d81c80807ce1a94c` | Core all-feature library suite 577/577, including pure program backend and AWBC codec cases; compiler Character projection 5/5 on the integrated dirty tree. The first Core sweep found the opcode inventory missing `0x29`; it was corrected before this passing run and commit. |

Dialogue's generation-owned producer schema previously passed focused 10/10,
including total defaults coverage, but its new source-type admission and
compiler/bundle/session assembly remain uncommitted and are being changed.
CharacterDialogue's AWBC verifier currently checks structural target/result
types and register reads; the accepted generation producer still needs its
complete role/custom/source-type inputs, and runtime display needs the actual
opaque target rather than a fabricated empty configuration. Generic prefix
predicate and scoped scheme migration is also in progress. The remaining
`callable_execution` cases, full cross-crate gates, and later convergence phases
are not yet accepted by this checkpoint.

## Dialogue target transport and scheme opening checkpoint — 2026-09-24

Inspected local `main` and `origin/main` at
`665eafeb8f3635f97fb0a175eb16f58180fb81fa`: they matched after a
non-forced push, and the index and working tree were clean at that point.

| Cut | Full Git SHA | Observed evidence |
| --- | --- | --- |
| AWBC CharacterDialogue opaque admission | `61c91c7a77b45589a65300c9d787542f9898addb` | Core library 577/577 at the earlier checkpoint. |
| Function effect predicate retention | `f9b9e7c66c8f178c7f6b38c25216eaed328b18bd` | Sema library 877/877 at this cut. |
| Compiler/RuntimePlan Character call and Look projection | `f809d6202db3f8e95c9c65ae2de9eb0e99658008` | Compiler Character focused 8/8; RuntimePlan all-feature library 76/76. |
| Core-owned Character nominal identity and Look admission | `282a890a7f560ff4dc624518b506ff8b3e010aec` | Core all-feature library 578/578; Sema digest focused 10/10, including identity parity. |
| Generation-owned CharacterDialogue producer schema/defaults/source values | `9c78b7d101f71503dc9763a58b921981ab205925` | Dialogue all-feature library 38/38; Core Look authority replaces caller-supplied Character-to-type maps. |
| Shared function scheme opening constraint scope | `aa3cf83466ccfd444db8fbdece663e2568578460` | Sema focused 5/5 and full library 883/883; Sema Clippy exited successfully with warnings. |
| Retained dialogue target call result | `e6a20245839b8f9d3910f554d2dba441ae5710fb` | Sema focused 2/2 and full library 884/884. The obsolete metadata-only dialogue consumer was removed. |
| Distinct Call/Specialize application identities | `49e5c68bed6c0b92c8dd6cf52babd5fec739d378` | Sema focused 6/6 and full library 885/885; same-ExprId applications and nested predicates retain separate evidence. |
| Native/AWBC exact dialogue target transport and snapshot admission | `11c11bc6a3f00e11002d8c9d16c72a83cf5dce07` | Core focused target/wire/save tests 4/4 and all-feature library 582/582. |
| Target-first Compiler/RuntimePlan lowering and typed instance keys | `4f7084443c277511030fa6b3fefc3e631786e700` | Compiler Character focused 14/14; RuntimePlan all-feature library 78/78. |
| Mandatory target fixture migration and parity assertion | `665eafeb8f3635f97fb0a175eb16f58180fb81fa` | Player/Host/Compiler all-target/all-feature check passed; Player unprojected-dialogue fixture 1/1. The parity assertion is compiled but its producer-dependent execution is not yet accepted. |

The Compiler `evaluated_effects` integration run was 13/17. Three native
execution cases stopped because no accepted CharacterDialogue generation
producer is bound; the fourth found an AWBC callback verifier expecting a void
result where the ordinary callable ABI requires `Some(Unit)`. After correcting
that verifier, its focused integration test passed 1/1; the entire 17-test
suite has not been rerun and the three producer cases remain unresolved.
The next required cut assembles one generation-owned declaration from logical
Character membership, complete effective defaults, Voice/Look and role payload
types, canonical custom descriptors, and accepted View/Style resources, then
binds its producer to the exact Plan/AWBC program and display consumer.
Function scheme specialization still lacks checked-expression publication and
pending result-port execution. Match/View/task-plan/nominal/scheduler/restore
and final workspace gates remain open.

## Generation declaration and specialization provenance checkpoint — 2026-09-24

Inspected local `main` and `origin/main` at
`89ec5cb309a856f2d4a65c0b71bd09a13ced3ba2`: they matched after a
non-forced push. The index was empty; 30 working-tree entries remained in
active Dialogue role-schema, Compiler/RuntimePlan generation, and Sema
function-value consumer work. This checkpoint records pushed cuts, not final
acceptance of those active integrations.

| Cut | Full Git SHA | Observed evidence |
| --- | --- | --- |
| Descriptor-derived CharacterDialogue custom catalog digest | `9a28602d9ff8a02b63414b50ebe7849e03bdb1f2` | Dialogue focused 2/2, full 40/40, Clippy exit 0 with warnings. |
| Accepted dialogue profile product lease through runtime projection | `a833ab24c86023f6025f911ba186853f82aa5d05` | Compiler project 27/27, profile admission 6/6, affected text-proxy 1/1. The then-current full Compiler library run was 101/103; both unrelated fixture failures were subsequently corrected below. |
| Generation-owned CharacterDialogue declaration and shared default digest | `ed1fce176531438f00c8e411a861e33b09404397` | Dialogue generation focused 5/5, full 45 library + 4 integration + 4 doc tests, all-target Clippy and fmt passed. The declaration is not yet bound to an executable producer. |
| Checked function specialization result provenance | `17d040cc63925ba09808d1e4dedad53010ab0080` | Sema focused 8/8, full 887/887 plus character-nominal integration 4/4, all-target/all-feature Clippy and fmt passed. Analyzer value-use consumers remain open. |
| Compiler budget and exact AWBC execution-context fixtures | `a69701bc25424a0d53e0cf160b3c331d9176280e` | Both focused tests and full Compiler library 103/103 passed. The pure generic call has no implicit effect binding; its checked scheme/key spends 23 structural visits. |
| One accepted View registration owner shared by Bundle and driver | `2068f2e9cb48ee5e7670599173d07beaffd55882` | Bundle/driver all-target/all-feature check and View runtime integration 29/29 passed; fmt passed. |
| Dynamic CharacterDialogue target uses generation evidence | `89ec5cb309a856f2d4a65c0b71bd09a13ced3ba2` | Dialogue presentation focused 2/2 passed. RuntimePlan digest matching and dynamic display decoding remain open. |

The generation declaration now owns logical Character rows, optional visual
manifest fingerprints, exact and Any dialogue type references, role/custom
references, effective defaults, and accepted View/Style fingerprints. The
next integration must bind real role payload schemas and defaults, project all
roots through Compiler/RuntimePlan, transport the declaration and character
package metadata through AWFB, install one producer for Native/AWBC, decode
the actual target for display, and preserve generation ownership through
replacement and restore. The three producer-dependent `evaluated_effects`
cases remain unresolved and have not been claimed as passing. An external
Character without an authored HIR display-name row also needs explicit
accepted presentation input before it can produce a dialogue line; its ID
spelling is not a display-name fallback. Final workspace gates and later goal
phases are still open.
