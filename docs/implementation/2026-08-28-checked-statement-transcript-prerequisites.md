# Checked statement transcript prerequisites

Date: 2026-08-28

Status: accepted implementation direction; implementation in progress

## Evidence at the decision point

The accepted generic-Match C3 contract requires statement and body semantic
digests, but `CheckedStatement` at Git
`b502aa4bcc3c194166e285540d07d46748d8f44e` retains only `effects` plus the
sparse lowering-oriented `CheckedStatementRole`. Its `Ordinary` branch merges
statement meaning that the transcript must distinguish. In addition,
`CheckedEffectField` retains a `String` name and fabricates positional
`arg{ordinal}` names from HIR call syntax.

The current facts therefore cannot be hashed as final semantic authority.
Doing so would require source-name reconstruction, raw-ID hashing, or an
unsupported/`Any` success branch. All three are rejected.

## Required implementation order

1. Move evaluated-effect operands onto callable-owned open-slot identities and
   sealed execution sources. Delete field-name reconstruction.
2. Add one HIR-owned control-transfer index to the existing root-local
   semantic topology. It resolves successful `out`, `break`, and `continue`
   statements to typed targets before sema publication.
3. Replace the sparse statement role with one final checked non-child payload
   authority. It retains only semantics not already owned by typed HIR
   children and body projections.
4. Seal evaluated effects, control targets, Include Flow targets, locale,
   unsafe-audit, trigger, Select, and named-scope facts after accepted roots,
   structural edges, and final call applications exist.
5. Build one atomic semantic-transcript catalog over expressions, patterns,
   statements, and bodies. The lazy Match-only builder is not the final
   publication authority.

This ordering is a prerequisite relationship, not a request to preserve an
intermediate compatibility model.

## HIR control-transfer authority

The topology owns root-local rows equivalent to:

```text
statement + Out      -> dialogue line-plan output application
statement + Break    -> Loop/While/WhileLet/For body owner
statement + Continue -> Loop/While/WhileLet/For body owner
```

Scope parentage is validation and target-selection evidence. It is not copied
into sema, and sema must not repeat label or scope resolution.

Output continuations are conceptual line-plan owners rather than executable
body rows, so they receive their own accepted-rooted output coordinate. Loop
targets join the existing `HirSemanticBodyOwner` and
`StableCheckedBodyCoordinate` authority.

Current final HIR retains label uses on control-transfer statements but has no
produced loop-target label declaration, and production line-plan lowering does
not currently produce a label. A labeled transfer therefore rejects as an
unresolved typed target. The implementation must not hash the label use or
silently treat it as the nearest unlabeled target.

## Final checked statement payload

`HirStmtKind::semantic_transcript_tag()` remains the exhaustive 35-shape
authority. The checked payload is grouped by non-child meaning rather than
copying the HIR statement algebra. Required payload families are:

- structural;
- assignment and assertion;
- defer outcome;
- evaluated effect;
- checked iteration;
- output and loop-control targets;
- trigger and Select branch semantics;
- unsafe audit;
- source locale and named/anonymous scope;
- Include Flow target;
- suspension and yield.

Expression, pattern, type, local, statement, and body children continue to
come from HIR typed edge/projection authorities. No copied statement AST or
parallel body model is introduced.

## Transcript boundary

The final transcript cut introduces private, non-Serde version-one
`CheckedStatementSemanticDigest` and `CheckedBodySemanticDigest` newtypes.
One memoized, cycle-checked graph constructs expression, pattern, statement,
and body digests after all checked rows are complete. A failure publishes no
partial transcript catalog or `CheckedMatch` row.

Statement and body records start with accepted-rooted stable coordinates.
Body traversal consumes `HirSemanticBodyRow` in source order; it never infers a
body from path prefixes or fabricates a `BodyId`. Heterogeneous Thread children
retain an explicit expression-versus-statement child-kind tag.

There is no transcript `Any`, wildcard identity, source-spelling fallback, or
`UnsupportedIdentity` success path. Coverage's typed open-domain `Other`
constructor remains coverage-only and is unrelated to transcript identity.

## Implemented prerequisite cuts

Evidence was refreshed on 2026-08-29 at Git
`fe7cbcc5cfa2f737ee0ac10e5c0c7dbebcf18f0a` with unrelated user WIP and the
current four-file sema coordinate cut present in the working tree.

- HIR now resolves each accepted `out`, `break`, and `continue` statement to
  one root-local typed target. Labeled transfers reject because no target-label
  declaration authority currently exists.
- evaluated-effect open fields now retain callable-owned `OpenArgumentId`
  identities. Positional names are not fabricated and runtime spelling is
  projected only at the compiler boundary.
- sema now issues affine control-transfer evidence from the HIR topology and
  accepted root catalog. Line-plan output coordinates append the disjoint
  `output=0x01, dialogue-line-plan=0x00` suffix; loop transfers retain both the
  exact `HirLoopTargetFamily` and the existing checked body coordinate.

These are prerequisites only. The sparse statement role and the lazy
Match-only transcript builder remain scheduled for deletion by the following
statement-seal and atomic transcript cuts.

The remaining Trigger, Select, dialogue-mark, unsafe-audit, and exhaustive
statement-payload authority is closed by the accepted
[Lang-01.5.1.1.2.1.1.1.1.1.1.1.2.2 design](../reviews/designs/lang-01.5.1.1.2.1.1.1.1.1.1.1.2.2-checked-statement-trigger-select-mark-and-unsafe-audit-authority-correction/README.md).
Implementation must consume that design as one deletion-driven typed boundary;
it does not permit a temporary Trigger/Select payload, string mark resolver,
or unsafe-ID fallback.

## Evaluated-effect final-call authority cut

Implementation evidence was refreshed on 2026-08-29 from Git
`18f36ee5de97939f259b28c31b66f1a625e17978`. The checkout also contained
unrelated user WIP in Agent, CLI, player, renderer, compiler project tests,
documentation, and samples; that state was neither staged nor rewritten by
this cut.

The evaluated-effect prerequisite now uses the final checked call application
as its only operand authority:

- sema retains each physical operand as a C1-issued stable execution source
  plus its solution-closed type;
- open log/event fields retain `OpenArgumentId` and are named only when the
  compiler projects the accepted binding into the runtime plan;
- `Drop` policy classification consumes the accepted registered value or
  semantic enum owner/case and retains its dynamic fade operand without HIR
  argument reconstruction;
- contextual enum constructor heads are a generic expected-enum call form for
  project, `Option`, `Result`, and registered closed enums; constructor
  identity is semantic owner digest plus case ordinal, never diagnostic
  spelling; and
- runtime reachability treats terminal/prefix effect calls and non-call policy
  metadata as non-value semantic carriers. Their owning children remain live,
  but the carriers publish neither a runtime type fact nor a runtime value fact.

The same cut closes the adjacent ownership boundaries exposed by end-to-end
validation:

- checked Drop invocation is the closed `Drop`/`DropOptional`/explicit-policy
  algebra. Explicit policy excludes the derived default and `OnDrop`; fade is
  either `LogicalDuration` or one checked Duration operand;
- accepted enum constructor parameters bind by accepted payload-field semantic
  identity. Record labels are lookup-only, and tuple constructors fabricate no
  parameter names;
- HIR runtime reachability consumes a complete
  `HirRuntimeExpressionProjection` table. Structural value retention and
  selected-call result/callee retention are disjoint, missing rows reject, and
  no retain fallback exists;
- inline `[at duration call=...]` syntax has one token-topology-derived timed-cue
  payload that reuses the ordinary DialogueCall expression. Malformed forms
  retain typed recovery and never downgrade to ordinary arguments;
- line-plan `at(duration) { ... }`, bare `at(duration):`, and
  `let cue = at(duration):` share the ordinary Call plus callback-Closure
  authority. Authored bare forms remain `Statement(StmtId)` roots rather than
  discarding their expression-statement owner or fabricating a direct
  line-plan expression edge; and
- inline Delay and line-plan At share one runtime schedule-operation builder.
  Delay retains both checked Duration type and schedule-handle type, while AWBC
  callable and Flow entry blocks retain their distinct accepted safe-point
  kinds.

The obsolete line-plan TimedCue item and its two expression-child roles are
deleted. Generic inline RichText timed-cue parsing remains because it is a
different maintained source construct. The large evaluated-effect,
dialogue-line-plan, compiler variant, and runtime semantic-fact owners were
split into private domain modules with their existing facades retained.

Runtime-plan effect facts retain only executable application, operand source,
closed type, operation, and final policy data. The old source-argument
reconstruction, runtime `policy_source`, dialogue-site expression duplicate,
and standalone effect-call runtime facts are deleted. There is no semantic
`Any`, source-name fallback, or case-specific `.Cancel`/`.Stop` reachability
exception.

Sol-max review selected statement authority for every line-plan construct that
entered through ordinary statement lowering. The current bare-At producer now
uses that final path. The following checked-statement cut must finish the same
normalization for the older Let/Out projections, then delete copied
`LinePlanLetValue`, `LinePlanOut`, and `DialogueLinePlanLet` rows. The dormant
`HirLinePlanItem::Expression`, Option, and TimelineAssert families require a
producer audit; a variant with no genuine expression-owned producer is removed
rather than retained as a compatibility path.

## Validation of the evaluated-effect final-call cut

Passed on 2026-08-29:

- `cargo test -p arcweft-lang-syntax --all-targets`: 676 library tests, one
  compile-API test, and three public parser-authority tests;
- `cargo test -p arcweft-lang-hir --all-targets`: 900 passed and eight ignored
  library tests, one public final-HIR test, four project-symbol tests, and one
  compile-API contract test;
- `cargo test -p arcweft-lang-sema --all-targets`: 548 library tests, 12
  compile-API tests, and four integration tests;
- `cargo test -p arcweft-compiler --test evaluated_effects`: two end-to-end
  ordinary/dialogue tests through structured runtime plans and AWBC;
- `cargo test -p arcweft-runtime-plan --lib`: 54 tests;
- compiler/runtime-plan all-target checks from the independent Luna-max
  inventory audit;
- all-target checks across syntax, HIR, sema, runtime-plan, and compiler, plus
  clippy across the same five packages; clippy completed with the repository's
  existing warning inventory and no command failure;
- package formatting, repository diff checks, and deleted-symbol/debug-macro
  scans; and
- `just structure-audit` plus `just structure-audit-gate`: 2,273 files, 95
  workspace packages, 270 review triggers, and zero blocking violations.

## Structural review

The canonical audit scanned 2,273 files and 95 workspace packages, reported
270 review triggers, and found zero blocking violations. Both the screening
and `--fail-on-blocking` gate passed.

`semantic_coordinate.rs` remains the cohesive owner of accepted-rooted checked
coordinate types and their canonical byte grammar; issuance and topology joins
remain separated in `semantic_coordinate/catalog.rs`. The cut adds only
crate-private control-target coordinates and does not widen a public API, copy
HIR scope state, or invert the HIR-to-sema dependency. Splitting these
coordinate types by statement family would scatter one canonical grammar, so
the existing owner is retained.

`final_analysis/tests.rs` remains the integration-fixture owner for private
final-analysis publication behavior. The two added tests exercise the same
accepted-root/topology/report fixture authority as the surrounding coordinate
tests. Moving only these rows would duplicate the private fixture harness or
introduce a test-only access seam, so the cohesive integration owner is
retained despite the upper LOC review trigger.

## Validation of the checked control-target coordinate cut

Passed:

- both focused output-target and four-loop-family tests (one test each);
- `cargo test -p arcweft-lang-sema --tests`: 542 library tests, 12 compile-API
  tests, and 4 integration tests;
- `cargo check --workspace --all-targets --all-features`;
- `cargo clippy --workspace --all-targets --all-features`, with existing
  warnings and no command failure;
- package formatting and repository diff checks; and
- `just structure-audit` plus `just structure-audit-gate`.

Blocked environment validation:

- the first `just test-workspace` attempt exhausted the build volume after the
  workspace `target` cache had grown to 296,527,421,327 bytes;
- `cargo clean` removed 146,998 regenerated files (276.2 GiB), without touching
  tracked files or user WIP; and
- clean and cached retries both reached Windows error 1455 while concurrently
  linking the broad workspace test inventory: the configured paging file was
  too small to map the generated `rlib` metadata. No Arcweft assertion or
  compile diagnostic preceded those environment failures.

Tier 2 was not run because this sema-private coordinate cut does not change a
runtime, renderer, Agent, MCP, capture, protocol, or persisted public contract.

## Checked statement, executable ingress, and canonical mark cut

Implementation evidence was refreshed on 2026-08-29 from Git
`a8234b4d2c695aeea6121f1a6d846334f7df17b6`. The implementation was performed
directly on the existing `main` checkout. Unrelated user WIP in Agent, CLI,
player, renderer, compiler project infrastructure, documentation, and samples
remained in the working tree and was not used as an authority for this cut.

The sparse statement role has been replaced by one sealed checked-statement
authority. `CheckedStatementPayload` covers the complete live success algebra:
structural, assignment, assertion, defer, evaluated effect, iteration, output,
loop control, trigger, Select, unsafe audit, source locale, scope, Include,
suspension, and yield. Its expression, pattern, type, local, body, and target
rows are issued from the final HIR topology and accepted semantic roots. The
older independently writable Trigger, Select, mark, unsafe, and sparse-role
carriers are deleted; public consumers receive opaque read-only views.

The accepted executable ingress is also typed end to end. Entry and Include
use prepared seed, worklist, SCC, and delta authorities rather than reparsing
names or rooting every declaration. Choice `OnSelect` enters through the same
typed statement seeding path. `AcceptedDeclarationSemanticId` deliberately
excludes revision-bound fact digests so declaration identity and accepted
generation evidence remain separate.

Sol-max structural review found and closed an under-accounted draft worklist
before this cut was committed. The final worklist keeps one pending contributor
delta per declaration, extracts each declaration's source-owned typed call and
Include candidates once, caches its adjacency once, and propagates each
edge/contributor pair exactly once. Every candidate probe, retained edge,
declaration row, contextual statement visit, contributor fact, pending-delta
write, queue push/pop, and propagation visit is checked and charged before its
allocation or write. With `D` declarations, `S` statement rows, `X` expression
probes, `I` Include rows, `M = X + I`, and `K` Entry contributors, the production
bound is the checked `D + 2S + X + M + 4DK + MK` sum. N+1 rejects before a
partial fact, queue row, cached edge, or seal can publish.

The completed-graph verifier owns fresh facts, pending deltas, adjacency, and
accounting and repeats the traversal from immutable typed evidence. It does not
read the mutable worklist cache; equality is checked before the move-only seal
can be split for statement and Entry consumers. A checked call site whose
prepared graph node is missing rejects as a typed invariant, while a structural
HIR carrier without an expression fact is correctly not a call candidate.

The ingress implementation is physically divided into the parent orchestration
and seal facade plus declaration inventory, worklist/accounting, independent
verification, and unit-test child modules. The analyzer's embedded test module
was also moved to `analyzer/tests.rs`, reducing the production orchestrator from
2,566 to 917 physical lines without widening visibility or changing semantics.

The remaining new statement owners are cohesive domain boundaries:
`statement_scrutinee.rs` is one short-lived select/seed/validate/proof
transaction, `statement_seal.rs` is one exhaustive late producer, and
`statement_effects.rs` contains the pre-call preparation and post-call final
fold of the same move-only effect authority. `semantic_coordinate.rs` remains
the sole accepted coordinate grammar. Compiler `lower.rs` and runtime-plan
`semantic_facts.rs` extend their existing atomic sema-to-runtime projection and
schema transactions; trigger facts now enter the same constructor as dialogue
facts, and the post-hoc dialogue projection mutation is deleted. No parallel
trigger classifier, mark-name join, sparse statement catalog, or fallback
`Any` path remains.

Final HIR now has 31 statement variants: 30 success shapes plus `Error`.
Unproduced `LetChoice`, `LetScope`, `LetActionReceive`, and `DeferBlock`
families and their consumers were removed instead of being retained as
compatibility scaffolding. Resource accounting uses exact checked limits with
charge-before-write behavior; no saturating or partial-publication path
remains.

Runtime reachability is a lazy query over reached expressions. Final sema owns
the typed `Structural` versus `Call(Static | RuntimeReceiver)` disposition,
and a call edge is admitted only from the exact `CheckedCallSite::HirCall`.
Presentation-owned or unreachable Call-shaped syntax therefore does not need
a selected call, while a reached unselected HIR Call rejects. The previous
eager projection map, raw `is_hir_call` flag, and trait-method-wide plan
fallback are deleted. Checked `for` iterator witnesses retain their exact
statement-origin semantic edges, so required Impl-method bodies become
reachable without rooting unrelated trait methods.

The maintained language surface is the authority for dialogue marks. The only
authored forms are `[mark @.point]` and `on mark(@.point)`; the reserved wait
form is `wait(mark(@.point))`. Syntax projects the shared lexer-owned
entity-reference grammar, HIR retains a typed relative suffix, and sema issues
the accepted mark identity. Removed `.point` and inferred `[.point]` forms do
not receive a compatibility reader. Unknown dot heads never become marker,
proxy, or custom-effect identities. Formatter proxy inference, raw declaration
scans, and `type|struct|proxy` shorthand discovery were deleted; explicit
object syntax remains.

The accepted frozen design package predates this maintained-surface correction
and still contains `.point` examples. That package was not edited. Current
maintained grammar, typed source topology, and executable tests resolve the
conflict in favor of the canonical `@.` reference form.

Explicit text-proxy declarations are intentionally not implemented by this
cut. Sol-max review closed the next authority as a
`FinalSemanticAnalysis`-owned `CheckedTextProxyCatalog`, keyed by
`ProjectNominalDeclarationId`, with registered scalar identities, typed
defaults, two-phase object validation, and compiler-only materialization into
`RichTextStyle::Object`. That work is a separate reviewable cut because it
replaces the remaining explicit object metadata boundary; it must not be
combined with the completed removal of unknown-dot inference.

## Validation of the checked statement and canonical mark cut

Passed on 2026-08-29:

- `cargo fmt --all -- --check` and `git diff --check`;
- `cargo test -p arcweft-lang-syntax -p arcweft-lang-hir --all-targets
  --all-features`: syntax 681 library tests plus public API suites, and HIR 915
  passed with eight ignored plus its public API suites;
- `cargo test -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-tooling
  --all-targets --all-features`: sema 603 library tests, runtime-plan 55
  library tests, tooling 31 library tests, and all API/integration suites;
- the focused executable-ingress suite: 15 tests covering delta-only pending
  state, local call/Include probes, cached-edge accounting, exact/one-over work,
  overflow/preallocation atomicity, traversal-order convergence, and consuming
  phase seals;
- `cargo test -p arcweft-compiler --all-targets --all-features`: compiler 55
  library tests and every API/integration suite, including the two checked
  iterator-witness tests;
- `cargo check --workspace --all-targets --all-features`;
- `cargo clippy --workspace --all-targets --all-features`, with the
  repository's existing warning inventory and no command failure; and
- `just structure-audit` plus `just structure-audit-gate`: 2,296 files, 95
  workspace packages, 275 review triggers, and zero blocking violations.

The normal `just test-workspace` push gate did not complete under the host's
parallel link load. `rustc 1.96.0` first produced an internal compiler error
while resolving imports in `iterator_witness_source.rs`; already scheduled
jobs subsequently reported a missing rlib. The exact
`cargo test -p arcweft-compiler --test iterator_witness_source` rerun passed
both tests after the build lock was released, and the same test binary had
already passed in the complete compiler all-target run. This is recorded as a
host/compiler failure, not an Arcweft test failure.

After the ingress correction and complete sema/compiler revalidation, the
normal gate was retried. It again exhausted the Windows paging-file mapping
capacity while many test binaries linked: rustc reported OS error 1455 while
mapping the already built bundle and sema rlibs, followed by delayed import ICEs
in bundle/player tests. No Arcweft assertion or ordinary compile diagnostic
preceded the host failure. The final isolated compiler all-target run, including
the iterator-witness binary, passed after this retry point.

The first `just test-tier2` attempt stopped before test execution because the
build volume had only 430 MiB free while creating the CLI archive. The verified
`D:\git\arcweft\target` contained 136,837 generated files totaling 270.9 GiB;
`cargo clean` removed only those rebuildable artifacts. The clean-state Tier 2
rerun completed compilation and ran four slow MCP cases. Three passed; the
unrelated in-progress Agent Product path failed
`agent_mcp_stdio_reads_agent_trace_resource` while lowering its existing
Agent-script fixture with an AWBC pattern-binding type mismatch (`expected 9,
found 20`). The aggregate recipe stopped there, before later Agent/capture and
language production-boundary recipes. This cut does not change Agent Product,
MCP trace publication, or that fixture, and none of those paths were staged.

The language production-boundary recipes were therefore run directly. Both
ignored Flow exact/one-over cases passed. All six ignored Select cases passed:
the exact and one-over total-slot transactions, syntax-expression limit, HIR
expression limit with scope tail, source-byte preflight, and real parser
diagnostic limit.

The first Select one-over run exposed a stale test lookup: it queried
`HirDatabase::current` with the initial source revision after several later
revisions had published. Sol-max review confirmed that the database and
transaction rollback were correct and that `current` must remain exact-keyed.
Both total-slot helpers now assert that the accepted prefill's exact key remains
current and the rejected candidate's exact key does not. The production-sized
one-over and exact counterparts then passed in 193.63 and 194.61 seconds,
respectively; no database or transaction production code changed.
