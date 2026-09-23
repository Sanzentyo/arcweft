# Nominal graph document boundary — 2026-09-11

Inspected `main` and `origin/main` at
`45fe11dc8328647826184035a92cc79835b2b93e`, with an empty index and the
preserved callable migration. Nominal C1 implementation is in progress; the
unlinked graph model and traversal drafts are not validated implementation.
This supplements the [argument reconciliation](2026-09-11-nominal-graph-argument-reconciliation.md)
and [accepted C1-C6 review](2026-09-09-accepted-rust-nominal-gap-review.md).

## Distinct canonical document grammars

Current `RuntimeTypeSchema::try_layout_hash` encodes a standalone schema after
`arcweft.nominal-schema\0` and shortest-varint version 1. The accepted graph
sketch reuses that prefix but places an arbitrary 32-byte root semantic
identity next, followed by a definition set. These are different document
grammars without a discriminant; the shared `TypeLayoutHash` does not provide
that distinction in the byte transcript.

Use `arcweft.nominal-schema-graph\0` followed by shortest-varint version 1
for the new graph document. This identifies the graph grammar itself. Keep
the standalone schema domain and its existing bytes. Both use the one core
primitive writer and exhaustive schema encoder. All graph atoms, order and
reachability remain as accepted, including the ordered definition arguments
from the reconciliation above. No derived hash or configured limit becomes
an identity atom, and no version is incremented.

A standalone schema operation cannot resolve `NominalRef`. It must return a
typed missing-graph error before publishing bytes or a digest. A graph
operation resolves the exact nominal/semantic pair against its validated
definitions. It never falls back to the standalone tree's `Named` resolver.

## Admission and operation bounds

The validated graph privately retains the explicit `RuntimeSchemaLimits`
selected at construction. This is admission policy, not a second type catalog
or persisted plan table. Raw serde definitions must pass graph construction;
the validated graph has no `Deserialize` implementation.

Admission checks the whole input's scalar, collection, depth and node limits,
then unique identity joins, source-ordered fields/cases, shapes and references,
then bounded canonical encoding. Definitions and member/schema nodes consume
the node allowance. Reference edges are finite schema nodes; they do not
recursively unfold the referenced definition during this structural check.

Each public layout operation owns one traversal/encoding allowance. In
particular, `try_layouts` shares its node and encoded-byte allowance across
every root, returning the complete result or an error. It cannot reset the
allowance for each root or publish earlier hashes after a later failure.
Reachability includes generic arguments and uses a visited set for nominal
definitions while still visiting every schema edge in each reached body.

Schema walking and graph cleanup must not depend on recursive native calls.
This includes dropping rejected inputs: an input rejected for excessive depth
can itself be much deeper than the selected limit. The graph owns iterative
cleanup of its consumed definitions on both failed admission and normal drop.

The frozen package remains unchanged. These corrections do not award C1
completion; implementation, exact transcripts, recursion/argument tests,
limit negatives, and all consumer migrations remain required.

## Implementation checkpoint

The current dirty implementation now connects the graph definition, admission,
reference validation, iterative schema traversal/cleanup, and reachable layout
encoder in `arcweft-core::entry::schema`. `RuntimeTypeSchema` has the five new
variants and the AWBC entry-schema codec reads/writes their complete atoms.
These entry-schema tags are distinct from the existing AWBC executable type
and constant table tags; those tables have not gained a new carrier family.

Performed and passed: `cargo fmt -p arcweft-core`;
`cargo test -p arcweft-core --lib --tests --all-features` (426 passed,
none failed: 393 library tests and 33 integration tests); and
`cargo check --workspace --all-targets --all-features` (exit 0 with warnings).
`cargo clippy -p arcweft-core --all-targets --all-features` also passed with
warnings; no workspace-wide C1 Clippy or structural push gate is claimed.
The 15 new tests cover graph identity/order/arguments/recursion, exact schema
and AWBC fragments, shared layout allowances, invalid raw serde identity,
noncanonical field IDs, and deep admission/rejection/drop. Current logs have
the `nominal-graph-` prefix under the ignored validation directory used by the
[writer cut](2026-09-11-canonical-schema-writer.md).

This is uncommitted C1 work. It does **not** yet provide
`RuntimeNominalSchemaGraph::accepts_value`, complete standalone value validation
for the new structural schemas, layout-bearing nominal variant identity, or
the nominal record-layout shape/field migration. The complete value walker
must account for opaque payload work while keeping exact opaque type
arguments and producer identity; it cannot accept an unvisited payload or
restart accounting. The remaining C1 consumers and tests must be migrated
before accepting this cut. C2-C6 are also still outstanding.

The writer-cut workspace test failure (28 compiler tests, predominantly open
effect rows) remains unresolved. That failed run is not a passed C1 workspace
gate. No C1 ownership success, full validation milestone, or completed goal is
claimed by this checkpoint.

## Value-validation continuation

Supersedes only the checkpoint statement that standalone validation of the
new structural schemas is missing. This continuation remains uncommitted on
the same inspected `main` commit.

The canonical value visitor now performs schema checks and encoding in one
iterative traversal. A single `ValueBudget` accounts for the entire value,
including opaque payloads, reduction commands, and Agent predicate nodes.
The same bounded writer accounts for every output byte and text atom; there
is no per-opaque payload writer or reset budget. The digest remains private
until the whole visit succeeds.

Borrowed views over ordinary, dense and columnar sequence storage avoid
cloning a row or string before its limits are checked. Standalone Tuple,
Result, RecordValue and ExactOpaque schemas use this visitor. The previous
recursive schema validator and recursive value/Agent-predicate encoders are
deleted. Tree `Named` lookup remains confined to the standalone schema
context, and unresolved `NominalRef` still requires its validated graph.
Error locations follow the active value path without copying the whole path
at every node.

Passed for this continuation: `cargo fmt -p arcweft-core`;
`cargo test -p arcweft-core --lib --tests --all-features` (433 passed,
none failed: 400 library tests and 33 integration tests); and
`cargo check --workspace --all-targets --all-features` (exit 0 with warnings).
Seven new value tests cover the shared opaque budgets, Agent structure/text,
columnar storage parity, integer widths, borrowed strings, nested error paths,
and successful/rejected 20,000-level values under explicit limits. Logs use
the `value-validation-` and `schema-value-validation-` prefixes in the same
ignored validation directory.

`RuntimeNominalSchemaGraph::accepts_value`, nominal variant layout propagation,
record-layout migration and the remaining C1-C6 work are still outstanding.
Inspection confirms that project facts already retain
`RuntimeResolvedNominal::layout` and plan `ProjectNominal.layout`; use those
existing authorities. Variant domain rows, closed builtin/character owners
and AWBC variant identity do not yet retain the required layout. Their
complete producer/consumer migration must precede accepting a nominal value
under the new graph; do not substitute a zero or semantic hash for its layout.

`cargo clippy -p arcweft-core --all-targets --all-features` passed with
warnings. The canonical structural gate also passed: 95 packages, 2,294 Rust
files, 1,272,588 physical Rust lines, 309 review triggers and zero blocking
violations. This screening includes preserved WIP and is not full C1 acceptance.

The decomposed owners are the borrowed storage view (212 lines/8,083 bytes),
value budget (112/4,444), iterative encoder (479/18,846), and schema predicate
context (368/14,664). `entry/schema.rs` is now 1,162 lines/37,482 bytes.
The common visitor owns stack/byte-order state; the predicate context owns
expected child schemas; both borrow the sole runtime storage and schema
authorities. The long exhaustive visitor is retained as one state machine.
No public view API, manifest, feature or dependency edge is added.

The touched `value.rs` remains at the existing upper trigger (3,690 lines,
132,201 bytes). This edit adds only a private module and crate-visible view
exports; it adds no evaluation, I/O, transport or persistence state to that
owner. The new storage projection lives in its own responsibility module,
and its tests exercise real dense/columnar consumers through validation and
canonical bytes. This is the disposition for that integration touch, not a
claim that every existing responsibility in `value.rs` was redesigned.

## Record-layout continuation

The complete existing record-layout responsibility is now covered by
[nominal record shape and field authority](2026-09-11-nominal-record-layout-shape-and-field-authority.md).
The source-shape enum and its name/count rule moved from the draft graph
module to `entry/schema/record_shape.rs`; the graph and executable layout use
the same type and rule. Compiler projection, AWBC projection, project-record
fact admission, layout deserialization, and native value field diagnostics
were migrated together. This replaces only the earlier statement that the
record-layout shape/field migration is outstanding. Graph value acceptance,
nominal Variant layout propagation, the rest of C1, and C2-C6 remain required.

## Typed graph value continuation

[Nominal Variant producer closure](2026-09-11-nominal-variant-producer-closure-gap.md)
supersedes the remaining statement that graph `accepts_value` is missing.
The working tree now implements it through the shared iterative value visitor,
including exact graph layouts/cases, typed reference lookup, operation-local
layout memoization and complete opaque payload accounting. The real builtin
Option/Result tuple payload representation is also validated. Core library and
integration tests pass (444), as do core Clippy and the structural gate.
The mandatory Variant layout migration remains incomplete at external producer
construction: the workspace check currently fails in Dialogue and data decoding.
The linked correction request tracks that coupled producer/schema boundary;
the Rust work is not yet committed or accepted as complete C1.

## Data projection and runtime value atom continuation

The independent [data reflection projection relocation](2026-09-11-data-schema-projection-owner.md)
is committed and pushed as
`9a7819cf75debc92b1b331d1c4b4763c17134320`. Core owns `From<&TypeShape>`;
the former sema function/export and metadata conversion helpers are gone.
This supplies the lower mapping owner without claiming data nominal identity
or graph admission. The Variant and callable/effect work remains dirty.

The in-flight schema now additionally represents Never, Duration, Progress,
EntityReference and AgentValue. Canonical layout tags are 31–35; the existing
AWBC schema row uses tags 30–34, following its existing zero-based tag grammar.
Both formats remain version 1. Schema serde, graph budget/reachability/drop,
canonical layout encoding, value validation and AWBC schema rows cover these
atoms together. Never admits no value but can occur beneath an absent Option.
Duration, Progress and EntityReference retain their actual runtime families,
with the existing scalar byte and string budgets.

AgentValue is its existing closed recursive value algebra, not an open schema.
The borrowed logical value owner supplies `is_agent_value_node` to both the
checked-type predicate and schema validation. The latter visits every sequence
item and record field through the shared iterative visitor and one value
allowance. This preserves i64/u64 widths and finite f64, accepts the existing
EntityReference leaf, and rejects tuples, nominal/opaque/operational values
and other integer widths. No second root-kind policy or copy of nominal
definitions is introduced.

The source inventory also establishes that normalized Agent operational types,
Range/Iterator/Map/Need/Stream/Parser/ThreadHandle/Shared/Reference/Function
already fail `RuntimeNormalizedType::checked_type` through its typed
`RuntimeUnsupportedTypeShape` boundary. This is evidence for the existing
checked variant payload admission rule, not a new universal persistence
exclusion. Complete normalized schema projection, Choice, builtin variants,
ordered arguments, fixed-array constraints and producer/generation correlation
still need to be reconciled before the producer closure is complete.

The existing Dialogue role contract resolves the prior uncertainty about
Style: it is the derived Choice of EntityReference and the exact opaque
RichText role. Six authored role declarations come from accepted standard
nominals in one accepted world. The current unconstrained typed-style API is
not that final role authority. The later accepted semantic-fact resolution
defines generation issuance as a public trusted-integrator structural boundary;
operational publication additionally needs compiler/bundle evidence. Its
private-constructor sketches must not be reintroduced as an unforgeability
requirement across crates.

Validation for the atom continuation, logs `schema-value-atoms-*` in the same
ignored directory:

- Passed: schema-focused library run, 44 tests; core all-feature library and
  integration run, 453 tests (420 + 33), including the exact AWBC atom rows.
- Passed: final atom group, five tests, after a Clippy-only or-pattern/doc
  cleanup. Coverage includes scalar-family mismatches, exact/one-over value
  limits, Never/Option, checked-AgentValue parity and nested rejection paths,
  and a real nominal graph Variant record payload containing Duration.
- Passed with warnings: final core all-target/all-feature Clippy. The two new
  warnings in the borrowed view were corrected; existing warnings remain.
- Passed: formatter, diff check and canonical structural gate: 95 packages,
  2,298 Rust files, 1,274,283 physical Rust lines, 309 review triggers, zero
  blocking violations.
- Failed then corrected: the first core check found the AWBC schema writer's
  exhaustive match missing the new atoms. Both writer and reader were migrated
  before the passing test/Clippy runs.
- Not rerun after the atom additions: workspace check/Clippy/tests/doctests
  and Tier 2. The preceding projection-cut runs failed at the unresolved
  Dialogue/data production layout initializers; Tier 2 was not run. Those
  failures remain unresolved and are not converted into current passes.

Current complete owner measurements: schema integration 1,174 lines/37,964
bytes; value predicate context 542/21,847; atom tests 295/9,514; borrowed value
view 241/9,093; pattern owner 2,813/102,215; AWBC metadata codec 1,505/54,823.
The pattern owner now delegates the AgentValue node rule to its value owner
instead of keeping a second match table. Its recursive checked-value traversal
is otherwise unchanged. The AWBC owner extends its existing schema row codec;
it does not acquire generation state, another carrier or another layout writer.
These responsibilities remain cohesive. The full nominal producer migration
is still uncommitted and the convergence goal remains active.

## Choice validation continuation

The working tree now includes ordered `RuntimeTypeSchema::Choice` (layout tag
36, existing AWBC schema-row tag 35, both version 1). Its alternatives use the
same schema traversal, graph reachability/admission, canonical writer and serde
surface. Empty, singleton and ordered alternatives retain their distinct
schema transcripts.

`RuntimeSchemaLimits::max_validation_work` is an explicit u64 policy for one
complete value admission, appended to the existing version-1 AWBC limit row.
The general engine policy selects 262,144, matching its existing logical-node
capacity. Root execution policies require it to be nonzero. This schema API
does not claim to implement the older named 65,536-unit checked-generation
validator or the remaining C3 generation/publication boundary.

One mutable validation-work owner charges each expected node and each Choice
candidate before evaluation. Choice edges consume validation depth. Failed
branches never refund work. Every alternative is evaluated in source order;
later work/depth or invalid-schema failures cannot be hidden by earlier
success. Zero matches retain ordered mismatch evidence, and multiple matches
report the first two successful ordinals after all candidates finish. Nominal
branches use the same graph lookup and layout-operation memo as other values.

The existing iterative visitor now schedules candidate and completion frames.
Candidate visits write to a non-publishing sink. Once exactly one alternative
has validated the same borrowed value, its canonical bytes are emitted once.
Logical value nodes, strings and output bytes retain their separate caller
allowance; graph layout work retains its graph admission allowance. Opaque
payloads remain semantically atomic during candidate probing and are traversed
once for the actual value and byte allowance. No probe receives a fresh
validation-work counter or graph layout context.

Nested ordinary mismatch causes remain typed and ordered. Their owning
`RuntimeSchemaChoiceMismatch` releases nested Choice causes iteratively, so
returning and dropping a 20,000-level mismatch does not use recursive drop glue.
The internal cause is extracted only during destruction; live callers have
read-only ordinal/cause access. Arbitrary deep clone/debug/equality operations
are not claimed by this traversal/drop evidence.

The independent [AWBC schema nesting fix](2026-09-11-awbc-schema-nesting-boundary.md)
is committed and pushed as
`663d60720861b838d864317b43a644442c4b823b`. It enforces the existing decoder
depth allowance on every schema read, including unary paths, and shares one
enter/read/leave scope with collection readers. It does not include the
pending Choice/graph/limit-row additions. The coupled nominal migration remains
dirty.

Validation for this continuation:

- Passed: initial schema group, 52 tests, followed by graph Choice and decoder
  coverage. Final core all-feature library/integration run: 467 tests (434 +
  33), log `awbc-schema-nesting-core-tests.log` in the existing ignored log
  directory. Focused nesting tests also passed separately (three tests).
- Passed: exact Choice schema/AWBC bytes and truncation; unique, empty and
  ambiguous alternatives; ordered nested mismatches; shared work across
  sibling choices; later work/depth failure after earlier matches; invalid
  schema failure; exact graph nominal layout rejection; opaque physical
  accounting; 20,000-level successful, work-exhausted and ordinary-mismatch
  paths including mismatch destruction.
- Passed with existing warnings: final core Clippy. New local style warnings
  were corrected. Formatter and ordinary/staged diff checks passed.
- Passed: final structural screening and gate: 95 packages, 2,301 Rust files,
  1,275,240 physical Rust lines, 311 review triggers, zero blocking violations.
- Failed: current workspace all-target/all-feature check and Clippy, and
  `just test-workspace`, at Dialogue/schema.rs:83 and data external.rs:543
  missing production nominal Variant layouts. The workspace test recipe did
  not reach execution. No downstream sema/compiler success is inferred.
- Not run for the Choice work: workspace doctests and Tier 2 while those
  prerequisites remain uncompilable. No C1 or producer-closure completion.

Current complete owner measures: schema integration 1,250 lines/40,371 bytes;
iterative value/candidate visitor 715/27,377; predicate context 630/24,227;
validation-work owner 56/1,684; Choice tests 252/8,025. The larger schema
integration still owns the public algebra, limits, errors and common bounded
writer. Its existing embedded canonical-byte tests exercise that private
writer. Traversal, graph work, reflection projection and predicate algorithms
remain in their responsibility modules; no unrelated state or I/O was added.
The visitor retains one continuation machine and one physical value-prefix
owner instead of a second schema-specific value walker. These are the
dispositions for the newly triggered schema integration size/test review.

The remaining work includes builtin variants and the rest of normalized schema
projection, complete nominal/opaque arguments and array constraints, and all
producer/generation correlation and plan/AWBC/restore migration. Dialogue's
standard role registration/substitution decision has now been read: six
accepted rows publish atomically, Style is derived, and internal typed role
coordinates are substituted before final semantic publication. No fallback
name recognition or speculative role layout was implemented here.

## Builtin and fixed Array schema continuation

The independent builtin Tuple constructor correction is pushed as
`8ff15c2907ed83ebe7206d955651c207c1c09a9e`; its
[record](2026-09-11-builtin-payload-tuple-admission.md) separates the completed
core case-registry rule from the pending schema migration. The complete
checked Array producer/reification change is pushed as
`35ba257b7505aa2e88bc05ecc708ebdf8e75b522`; its
[record](2026-09-11-checked-array-length-preservation.md) includes the deleted
sema-only outer-length check, actual core plan/AWBC tests and the unexecuted
upper-layer test limitations. `main` and `origin/main` match the latter SHA.

The pending schema algebra now represents every core builtin family with
`RuntimeTypeSchema::Builtin(RuntimeBuiltinSchema)`. Its private owner and
ordered payload-item schemas are admitted through `try_new`; unit cases and
the one-item Tuple ABI come from the same core registry used by checked types
and values. Construction and serde reject a wrong payload-schema count;
rejected owned schemas are dismantled iteratively. No per-instance copy of
case labels, ordinals or payload-presence rows is retained. Option and Result
factories instantiate this same schema and their old enum variants are gone.

Canonical schema tag 20 now writes the builtin owner tag, full case count,
and each canonical ordinal/name/presence row plus its inner schema when
present. AWBC schema tag 19 writes the builtin owner and the exact ordered
payload-item table. The old standalone Result schema tags (canonical 27 and
AWBC 26) are removed. The AWBC reader checks count before reading/allocating
items and keeps the shared nesting scope. Versions stay 1, with no old reader.
Schema traversal, graph work accounting, definition reachability and value
validation use all canonical cases. Values require the exact owner, ordinal,
name, payload presence, unary Tuple and recursive item predicate.

Fixed arrays use `RuntimeTypeSchema::Array { item, length: u64 }`. Canonical
schema tag 37 and AWBC schema tag 36 retain fixed-width length followed by
the item schema. The value visitor checks exact logical length, including
dense storage, before visiting elements. Array length mismatch retains its
full-width expected length and nested path and is an ordinary Choice
mismatch. One item schema edge is traversed regardless of the declared length:
even an empty array requires its complete nominal item definition and its
layout includes that definition. Schema size accounting does not allocate or
charge a materialized value array merely because its declared length is large.

Final validation for these current inputs:

- Passed: the full core all-feature library/integration run, 482 tests
  (449 + 33), `array-final-core-tests.log`. The two later focused core AWBC
  Array tests passed, including an additional nested-length regression.
- Passed: all seven builtin structural fixtures through the actual value
  constructor; wrong owner/name/ordinal/presence/item and flattened/empty/wide
  tuple rejection; construction/serde count checks; exact canonical and AWBC
  bytes, removed tag and truncation rejection; ordered graph references; exact
  work/node/depth bounds; 20,000-level schema/drop and wire-rejection cases;
  array full-width length and empty-array nominal graph closure.
- Passed with warnings: final core Clippy. Passed: formatting, diff review,
  final structural screening and gate (95 packages, 2,306 Rust files,
  1,276,334 physical Rust lines, 311 review triggers, zero blockers).
- Failed: workspace check, workspace Clippy, `just test-workspace` and
  `just test-doc`, at the still-missing Dialogue and data production Variant
  layouts. The migrated sema and runtime-plan tests did not execute. Tier 2
  remains not run while its prerequisites fail to compile.

Initial stale schema tag, golden bytes and budget fixtures, a private test
import/nested test declaration, and an unavailable `Box<Schema>` wire read
were corrected before the corresponding passing core runs. Failed logs remain
under the existing ignored validation directory. The passed tests include
preserved WIP and are not isolated staged-tree executions.

The schema changes remain uncommitted with the coupled nominal migration.
They describe supplied structural payload types; they do not authenticate
standard semantic signatures or complete generation correlation. In
particular, real `AgentResourceBody::BytesBase64` contains an
`AgentBinaryBody`, whose `encoding` and `data` fields are owned by the Agent
field registry. No fixture Bool schema is used as its production replacement.
The complete Agent DTO/schema rule, data descriptor identity and value shape,
Dialogue active role descriptors, compiler Character/base-environment graph
production and C3/C5 admission/restore remain open under the existing request.

## Agent type-argument continuation

`690a406023b1a1822bff6bad17588254b762a1b8` is pushed on main and retains
Probe result types through the existing `RuntimeAgentTypeProjection`, checked
types, normalized facts, ownership, AWBC interning/reification and structural
verification. The [implementation record](2026-09-11-agent-probe-type-argument-preservation.md)
contains the exact scope and validation. This resolves the earlier normalized
blanket Agent rejection and the result-erasing checked Agent representation;
it does not claim runtime Probe target/result correlation or complete Agent
record payload admission.

The current complete core run passes 486 tests (453 + 33), with core Clippy
passing with warnings. Structure screening/gate reports 95 packages, 2,307
Rust files, 1,276,590 physical Rust lines, 311 review triggers and zero
blockers. Workspace check, Clippy, test-workspace and doctests still fail at
the separate production nominal layouts; the direct Agent runner test attempt
also stops at Dialogue compilation. Its migrated tests and the sema/runtime-plan
tests are unexecuted. No additional Agent schema atom or protocol DTO schema
was added in this continuation.

## Data witness and codec-use continuation — 2026-09-23

Inspected local `main` at `233ac21d8664da4a71dbff4150bc5b78b2a568ab` with
361 dirty paths, including the preserved coupled semantic, nominal, data,
Dialogue and runtime changes. The following is implementation work in progress,
not a C5 completion claim.

The core `RuntimeDataShape` retains the exact `RuntimeProgramOwner` and the
selected `DataShape<T>` semantic row. It derives `T` from that immutable row;
the source child is not inferred from an observed value or a TypeShape digest.
Live equality and admission compare the exact retained executable. Generic
serde and generic persistent value hashing reject the live witness. The AWBC
value DTO stores shape and child semantic identities, and explicit program-bound
restore rebinds them only after the enclosing artifact/generation check.

Codec properties now use a finite `RuntimeCodecUse` occurrence tree attached
to the existing type/domain owners. It contains no semantic/type IDs, layouts,
scalar widths, field identities or case ordinals. Admission zips policies to
the original logical rows, and nominal references return to the original
nominal domain. A borrowed transient ShapeAccess index distinguishes two uses
of a shared `Seq<Bytes>` row, preserving, for example, Base64 and Hex policies.
The original source graph's canonical version-1 layout operation commits the
retained wire names, record policies, tags, repr, discriminants and field
annotations. The graph remains a construction proof and is not retained as a
second runtime catalog. `has_default` remains an annotation; it is not an
admitted constant or callable and cannot manufacture a missing field value.

The earlier focused `cargo test -p arcweft-core data_shape --all-features --quiet`
run passed seven tests before the occurrence-tree changes. That result covers
the carrier and primitive adapter at that checkpoint. The subsequent nominal
occurrence, schema-policy correlation and source-layout tests still require
the current coupled core/AWBC validation; the earlier pass is not evidence for
their final inputs.

Restore work still required within the original C5 scope:

- Product/Fiber, queued task events and retained Dialogue values now carry the
  selected owner through DataShape restoration. The older context-free
  `AwbcRuntimeValueSnapshot::into_runtime_value` entrypoint remains while its
  remaining consumers are migrated.
- `line_task/handle.rs` still has context-free saved-value conversions. These
  need the same exact owner and typed expected context before publication.
- The existing nominal-record DTO still lacks source semantic identity, and
  `nominal_into_live` still uses the raw constructor. Passing an owner through
  the recursive decoder alone does not establish the complete nominal restore
  proof. Replace that route with typed program admission and remove obsolete
  context-free conversions before claiming C5 complete.

These are remaining implementation obligations, not external blockers or a
request for a separate design assignment.

### Same-day restore and admission continuation

Supersedes the preceding restore-work inventory. Local `main` remains at
`233ac21d8664da4a71dbff4150bc5b78b2a568ab`; a later observation counted 376
dirty paths in the same coupled working tree.

`RuntimeNominalRecordValue` now retains its source semantic identity alongside
nominal name and layout. Original Plan/AWBC constructors and accepted layout
construction supply that identity, and predicates, source-graph admission,
canonical value bytes and the explicit snapshot DTO preserve and check it.
Snapshot record and nominal-variant candidates pass the selected program's
snapshot admission before publication. No name/layout-to-semantic resolver was
introduced. The obsolete tree-only nominal-role validator, which had no source
semantic identity and no repository callers, was removed.

All recursive AWBC value restores now require `RuntimeProgramOwner`; the
context-free restore entrypoint and its optional-owner implementation are
deleted. Line-task callers and driver captures pass their selected owner. The
driver's closed dialogue-action token instead retains the existing typed core
action value directly. The internal line-handle token no longer claims a
source nominal identity with a fixed hash: its existing opaque owner encloses
the token owner's exact six-field tuple ABI, with strict typed decoding and no
legacy nominal reader.

Policy admission now checks metadata budgets before allocating the borrowed
occurrence index and bounds every appended occurrence. A failed construction
does not cache a partial index. A canonical nominal body cannot be its own
`NominalRef`; only occurrence edges may use that back-edge. Added regression
coverage includes selected Plan/AWBC enum policies and payload positions,
shared Bytes policies, recursive nominal resolution, budget rejection/retry,
semantic/layout/field snapshot tampering, canonical semantic-identity bytes,
and line-handle token ABI tampering. The core full run before this follow-through
passed 527 of 528 unit tests; the sole failure was the obsolete nominal-document
golden lacking the new optional-policy marker. That version-1 golden is updated.
After the nominal identity/DTO/token migration and two corrected test-source
compile errors, the centrally run complete core suite passed: 535 unit tests
and every integration, compile-fail and doctest group, exit 0. Cross-crate
checks remain centrally coordinated. The core pass does not claim completion
of the still-integrating C5 producer/codec path.

The new core owners received the required size/growth review. Measured physical
sizes at this checkpoint are:

| Path below `crates/arcweft-core/src/` | Bytes | LOC | Classification |
| --- | ---: | ---: | --- |
| `program_types/data_shapes.rs` | 44,393 | 1,079 | production; no embedded tests |
| `entry/schema/codec_use.rs` | 26,461 | 695 | production; no embedded tests |
| `value/awbc_save.rs` | 38,905 | 945 | production; no embedded tests |
| `value/awbc_save/tests.rs` | 11,304 | 303 | test |

The two new production modules grew from zero at the inspected base. The
occurrence adapter owns one borrowed index and its typed projection/validation;
the codec-use owner owns one policy algebra, source-schema correlation, bounded
traversal and canonical transcript. The save module owns the existing recursive
DTO boundary and program-bound admission, with tests following that boundary.
None owns I/O, a second semantic catalog or a persisted duplicate graph.
Core already depended on data; this work adds no dependency edge. Public APIs
were not widened to split files. Their cohesion is retained; the coupled cut's
canonical structural scanner and dependency report remain part of final
validation. Field-level Bytes overrides through graph references and actual
default-value producers still require the data/codec integration evidence.

### Accepted Rust codec policy and default producers — 2026-09-23

Supersedes the preceding field-default producer gap. Inspected local `main`
at `233ac21d8664da4a71dbff4150bc5b78b2a568ab`; the coupled working tree was
dirty (408 paths at the first observation of this continuation). No branch,
worktree, staging, commit or push was performed by this owning agent.

`arcweft-data-derive-support` now owns the shared Rust data attribute parser,
rename rules and source validation used by both Reflect and ArcweftType.
ABI declarations preserve resolved wire names, record unknown-field policy,
enum tag/content/repr/discriminants, field bytes format, and default/skip
intent. Accepted record and variant metadata carry those properties through
generic instantiation into the original nominal source graph. The same v1
canonical source layout commits every property. Bytes keeps its actual ABI
type; use-site formats are field/occurrence policy. Signed discriminants use
canonical decimal text in v1 ABI JSON, avoiding serde's internally tagged
integer narrowing. The shared parser and ABI validation reject ambiguous
wire names, tag collisions and invalid repr cases before publication.

Field defaults join the original registered Rust callable allocation. The
source proof checks purity, a closed empty effect row, nullary inputs, exact
instantiated result type and uniqueness, then binds the same allocation to
the checked callable generation. Program IDs commit that complete accepted
declaration/signature/role/result proof rather than hashing a path alone.
Runtime-plan emits an ordinary pure helper plus its existing program binding;
the source codec policy retains the exact `default_program` edge. Both Plan
and AWBC execute the selected helper through the admitted backend, validate
the returned runtime value against the field row, then use the Sans-I/O data
default provider. Metadata collection never executes the factory or calls
arbitrary `Default` to manufacture a constant.

Path defaults use real `#[arcweft_export(pure)]` functions. Trait defaults and
skipped fields require a registered explicit pure Default constructor. A local
`impl Default` can export that real wrapper with the attribute; the new
`arcweft_export_default!(pure, pub fn default_flag() -> bool)` declaration
also wraps existing primitive/foreign implementations without orphan impls.
Generic fields resolve the corresponding concrete result type after
instantiation. The macro generates the actual core Default call and metadata;
no second callable catalog or runtime resolver is introduced. Source HIR has
no field-default syntax, so none was invented in this Rust producer slice.

Integration testing found that older Rust enum derives preformatted tag/repr
into `Value::Record`/Number, disagreeing with the typed codec algebra. Derives
now preserve `Value::Enum`; TypeShape remains the owner of wire tag/repr
projection. Explicit `RuntimeCodecUse::Newtype` marks the transparent wire
view of a Rust single-slot enum tuple payload while the executable value keeps
its exact Tuple1 ABI. `transparent_child` exposes that source proof; a plain
Tuple1 or nominal reference alias does not imply transparency. JSON/YAML/TOML
enum payload traversal is integrated by the codec owner so field Bytes
overrides and referenced child shapes are retained.

The new syntax crate has production dependencies only on `syn`, `quote` and
`proc-macro2`, with exactly the two macro crates as workspace consumers.
Cargo metadata confirms Rust ABI's new production edge to data is for its
actual Bytes type registration; no syntax/HIR/sema/runtime edge was added to
either macro or ABI owners. Public APIs were widened for real metadata
transport and backend execution needs, not file splitting. Source parser
code was moved out of data-derive and its obsolete duplicate paths removed.

Current ownership measurements (physical LOC, excluding child modules):

| Owner path | Bytes | Base LOC | Current LOC | Responsibility |
| --- | ---: | ---: | ---: | --- |
| `arcweft-data-derive/src/expand.rs` | 35,767 | 1,103 | 926 | production macro expansion |
| `arcweft-data-derive-support/src/attrs.rs` | 14,105 | 0 | 476 | shared production source grammar |
| `arcweft-rust-abi-macros/src/lib.rs` | 21,051 | 593 | 572 | production export expansion |
| `arcweft-lang-sema/src/env/rust_metadata.rs` | 41,208 | 871 | 1,221 | accepted metadata lifecycle/catalog |
| `arcweft-lang-sema/src/callable/catalog/defaults.rs` | 6,872 | 0 | 189 | checked default proof and existing-catalog indexes |
| `arcweft-core/src/pure/program.rs` | 6,283 | 0 | 157 | selected Plan pure-program execution boundary |

These production modules have no embedded test blocks. The metadata owner
crosses the 1,200-LOC and 300-LOC-growth review thresholds: its source input,
accepted immutable rows, generation join and instantiated views remain one
cohesive publication lifecycle; field policy is retained on those rows, not
in unrelated state or a copied catalog. Name and codec/default validation,
source graph projection and callable proof have separate existing ownership
boundaries. No I/O or general AST traversal was added to that lifecycle.
The canonical workspace structural scanner and broad integration matrix are
centrally coordinated with the full C5 cut.

Focused evidence before the final coupled rerun:

- `cargo test -p arcweft-core pure_program_external --all-features --quiet`
  passed its selected-backend/owner/result regression.
- `cargo check -p arcweft-rust-abi -p arcweft-adapter-context -p
  arcweft-adapter-sema -p arcweft-lang-sema -p arcweft-compiler --all-targets
  --quiet` passed at the complete Bytes projection checkpoint.
- `cargo test -p arcweft-data-derive-support -p arcweft-data-derive -p
  arcweft-rust-abi -p arcweft-rust-abi-macros --quiet` passed after explicit
  primitive/foreign wrapper and compile-fail additions: 13 ABI tests,
  four shared-parser tests, three default metadata tests and all remaining
  integration/compile-fail/doctest groups.
- `cargo test -p arcweft-data --all-features --quiet` passed all groups,
  including typed tagged/repr enum round trips and existing compile-fail cases.
- Focused all-target/all-feature Clippy on the four syntax/ABI packages passed
  with warnings (including the retained long reflection expansion and data
  dependency style warnings). This is not a warning-free workspace claim.
- Source/selected codec parity initially failed on i128 JSON buffering and
  enum preformatting, both repaired above. Its next failure isolated concrete
  enum Bytes payload traversal; the codec owner supplied that repair. The
  final compiler source-policy/default fixtures and core transparent policy
  test are being rerun against the coupled result, not counted as passed here.

Final producer checkpoint supersedes that pending rerun: `cargo test -p
arcweft-compiler --test project_cache_transaction rust_ --quiet` passed 8/8,
including source/Reflect/Plan/canonical-AWBC JSON parity, all seven policy
layout changes, malformed ABI policies, actual Path/Trait/primitive/foreign/
generic default execution and wrong/missing/duplicate/effectful producer
rejection. An intermediate added effect test used the malformed label `io`
and failed in fixture registration; corrected to the valid `io.read`, it now
exercises default admission and passes. `cargo test -p arcweft-core data_shape
--all-features --quiet` passed 16/16, including exact default requests/results,
tampered bindings and explicit Tuple1 transparency/arity rejection. The
earlier Tuple1 fixture type mismatch and missing Plan root-policy validation
arm were repaired before this pass. Final targeted rustfmt and
`git diff --check` passed. A later local observation counted 417 dirty paths
in the same preserved coupled checkout. The owning producer slice is frozen
for centralized workspace checks and publication; the broader C5 semantic
regression/integration work is not claimed complete by these focused passes.

Current integrated status and owner review are maintained in [the latest AWBC record-shape projection checkpoint](2026-09-12-awbc-record-shape-projection.md).
