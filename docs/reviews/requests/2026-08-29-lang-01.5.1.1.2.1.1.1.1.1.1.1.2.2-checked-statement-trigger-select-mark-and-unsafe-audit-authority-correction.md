# Lang-01.5.1.1.2.1.1.1.1.1.1.1.2.2 — checked statement trigger, Select, mark, and unsafe-audit authority correction

Status: `OPEN_DESIGN_REQUEST`

## Parent, split reason, and precedence

This is an independently throwable design-gated child of
[Lang-01.5.1.1.2.1.1.1.1.1.1.1.2](2026-08-22-lang-01.5.1.1.2.1.1.1.1.1.1.1.2-generic-match-complete-transcript-and-coverage-closure-correction.md)
and its accepted
[final semantic owner correction](2026-08-23-lang-01.5.1.1.2.1.1.1.1.1.1.1.2.1-final-semantic-owner-construction-and-seal-correction.md).
The parent correctly requires one private statement/body transcript over all
35 final-HIR statement families, but current source still lacks constructible
final payloads for Trigger, Select, dialogue marks, and unsafe audits. Those
four gaps meet at the same statement-seal and accepted-coordinate boundary;
splitting them would leave either a string/name join, a raw HIR identity, or a
second statement model.

Apply this precedence:

1. current source, maintained stable documentation, and accepted typed
   contracts;
2. the accepted generic-Match expression/pattern/statement/body graph and its
   C1 semantic paths;
3. the implemented checked call/effect and control-transfer prerequisites;
4. this correction for Trigger, Select, dialogue marks, unsafe audits, and the
   complete checked statement payload; and
5. historical sketches, pseudocode, source spellings, or filenames.

The accepted generic-Match graph remains authoritative: statement transcripts
use the exhaustive `HirStmtKind::semantic_transcript_tag()`, typed child/body
roles, and a minimal non-child checked payload. This request does not add a
copied statement AST. The accepted control direction also remains
authoritative: HIR selects `out`, `break`, and `continue` targets, and
`SemanticCoordinateIndex::control_transfer_evidence` issues the checked target.
The accepted evaluated-effect replacement is an input, not a decision reopened
here:

```rust
pub struct CheckedEvaluatedEffectOperand {
    source: CheckedCallExecutionSource,
    ty: TypeKind,
}

pub struct CheckedEvaluatedEffect {
    application: CheckedCallApplicationSite,
    operation: CheckedEvaluatedEffectOperation,
}

pub enum CheckedEvaluatedEffectOperation {
    Log { /* closed fields */ },
    SignalWrite { /* closed fields */ },
    MetricWrite { /* closed fields */ },
    EmitEvent { /* closed fields */ },
    Panic { /* closed fields */ },
    Fail { /* closed fields */ },
    Bail { /* closed fields */ },
    Ensure { /* closed fields */ },
    Drop { /* closed fields */ },
}

pub struct CheckedEffectField {
    open_argument: OpenArgumentId,
    value: CheckedEvaluatedEffectOperand,
}

pub enum CheckedDropFade {
    ConstantNanos(u64),
    Operand(CheckedEvaluatedEffectOperand),
}
```

Dialogue final sites retain the sealed effect and expose no public structural
expression ID. Do not redesign or duplicate those types in the returned
contract.

## Repository evidence at dispatch authoring time

The following is evidence, not proposed design. It was inspected on `main` at
Git commit `f236099c8207b5b6ac283ba2f48e999955bd3e0f`, equal to
`origin/main` when inspected. The checkout also contained unrelated user WIP
and an in-progress sibling evaluated-effect cut. That dirty state is not
authority and must not be reset, overwritten, or folded into this request.

1. `crates/arcweft-lang-hir/src/stmt.rs` has exactly 35 `HirStmtKind`
   families. `semantic_transcript_tag()` assigns the closed source-order range
   `0x0700..=0x0722`; `Error=0x0722` is rejection-only.
2. The same file stores
   `HirTriggerPattern::{Input(PatternId), Event(PatternId), Signal { target,
   value }, Timeout(ExprId), Mark(PatternId), Select(PatternId), Task(PatternId),
   Scope(PatternId), Expr(ExprId)}`. `Mark` is not a pattern semantically, and
   `Expr` is an abbreviation on an otherwise semantic enum.
3. `crates/arcweft-lang-hir/src/stmt/thread.rs` stores
   `HirSelectBranchHead::Bind { binding, source, propagates_error: bool }`,
   plus `Frame`, `Event`, and `Recovered`. Syntax parsing removes a trailing
   `?` from the source expression and copies it into that Boolean.
4. Maintained grammar and the implemented generic Try direction use prefix
   `try`; `docs/02-runtime/device-streams.md` uses
   `value = try stream.next()`. There is no accepted postfix-question Try
   expression. The live Select Boolean is stale surface and semantic state,
   not a compatibility requirement.
5. `crates/arcweft-lang-hir/src/dialogue_application/content.rs` already owns
   `HirDialogueContentId { owner: ExprId }`; rich text already owns contiguous
   `HirRichTextTagId { content, ordinal }`. It has no mark-only identity or
   mark-name/catalog join.
6. `crates/arcweft-lang-sema/src/checked_rich_text/checker.rs::check_marker`
   strips `.` from an opaque `HirRichTextValue`, reparses it as `PublicId`, and
   publishes `CheckedRichTextAction::Marker(PublicId)`.
7. `CheckedDialogueLinePlan` retains `Box<[PublicId]>` plus a separate
   `Box<[CheckedDialogueMarkHandler]>`. Final analysis recursively walks line
   plan statements, resolves `HirTriggerPattern::Mark(PatternId)`, reparses the
   relative entity-pattern spelling, and constructs a statement-to-`u32`
   side table.
8. The compiler copies that side table to
   `RuntimeDialogueMarkHandler { statement: StmtId, ordinal: u32 }`.
   `arcweft-runtime-plan/src/final_flow/line_plan.rs` re-reads HIR to prove the
   trigger is `Mark`, looks up the statement in that side table, and only then
   obtains the already-existing typed `RuntimeDialogueMarkSeedId`. Core,
   AWBC, and line-task execution already use typed runtime mark IDs and do not
   need a label.
9. `HirUnsafeAudit` retains `id: HirIdRefValue`. The maintained language rule
   requires an absolute `@unsafe.*` identity. `arcweft-id` already owns the
   exact `UnsafeAuditId` and owner-issued `AcceptedUnsafeAuditSemanticId` under
   `b"arcweft.id.accepted-unsafe-audit-semantic.v1\0"`.
10. `crates/arcweft-verify/src/lib.rs` confirms
    `CheckedStatementRole::UnsafeAudit`, then re-reads the HIR audit and uses
    `id_ref_label` to render absolute, relative, family-relative, and recovered
    references into strings. That permits shapes semantically forbidden by the
    maintained contract to reach a later consumer.
11. `CheckedStatement { effects, role }` and `CheckedStatementRole` are sparse.
    `Ordinary` merges most statement meaning, while Trigger and Select have no
    checked row at all. Current statement preparation also does not seed
    Trigger or Select `Frame`/`Event` pattern types.
12. `semantic_coordinate.rs` is the existing cohesive owner of accepted-rooted
    coordinates. Its suffix grammar currently assigns `0` to body coordinates
    and `1,0` to dialogue-line-plan output targets. The same module already
    issues affine control-transfer evidence.

The responder must refresh this evidence against current `main`, record the
full Git SHA and dirty state, and identify any intervening accepted contract.
If a later accepted contract already supplies every owner below or contradicts
one of these selected decisions, stop and report the exact conflict instead of
returning a parallel design.

## Selected correction: one typed authority chain

The returned design must implement the following ownership. Names and field
shapes in this section are normative; visibility may only be narrowed. No
alias for a deleted type is permitted.

HIR records shown as `pub` remain read-only downstream vocabulary with
crate-private constructors. Final checked statement/payload/coordinate types
are public read-only sema projections because compiler and verifier consume
them; fields and constructors remain private. The existing
`CheckedControlTransferTarget`, `CheckedOutputTarget`, and
`CheckedLoopControlTarget` prerequisite types must be promoted to the same
read-only visibility when placed in the public payload, without exposing a raw
coordinate constructor. `RuntimeTriggerAdmission` is public only inside the
compiler-to-runtime-plan typed input boundary and has no external constructor.

### 1. Syntax parses a dialogue-local mark selector once

The rich-text parser and the `mark(...)` trigger attachment must project the
same validated local selector leaf. Introduce one syntax-owned
`SyntaxDialogueMarkName`-equivalent parsed identifier. It represents the
identifier after the required leading dot, not a `String`, `PublicId`, entity
reference, or pattern. Its source range remains syntax/source-index evidence.

`[mark .name]`, inferred `[.name]`, and line-plan `on mark(.name)` must all use
that typed selector. Marker tags with attributes, multiple arguments, missing
dot, missing name, or an invalid identifier retain typed syntax recovery and
cannot mint a HIR mark. No downstream layer may call `strip_prefix`,
`parse_public_id`, `to_string`, `Display`, or a source-range reader to recover
the selector.

### 2. HIR owns mark identity, catalog order, and Trigger shape

`arcweft-lang-hir::dialogue_application` owns these final records:

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirDialogueMarkOrdinal(u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirDialogueMarkId {
    content: HirDialogueContentId,
    ordinal: HirDialogueMarkOrdinal,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirDialogueMarkName(HirName);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirDialogueMark {
    id: HirDialogueMarkId,
    name: HirDialogueMarkName,
    tag: HirRichTextTagId,
}
```

Constructors are crate-private. `HirDialogueMarkOrdinal` exposes its `u32`
only through a typed accessor. `HirDialogueMarkId` exposes `content()` and
`ordinal()`. `HirDialogueMarkName` is HIR-local lookup evidence. It may cross
the checked/compiler boundary only as the diagnostic projection described
below; it never selects a mark and never enters semantic identity or a
transcript.

Evolve `HirDialogueContent` in place to own
`marks: Box<[HirDialogueMark]>`. Evolve the accepted marker-tag payload in
place so an accepted marker tag carries its `HirDialogueMarkId`. The content
constructor validates all of the following atomically:

- every mark belongs to this exact `HirDialogueContentId`;
- mark ordinals are contiguous from zero and follow marker-tag source order;
- every `tag` belongs to this content and denotes one accepted marker tag;
- a marker tag and mark row form a one-to-one relation;
- mark names are valid, nonempty, and unique within this content; and
- no recovery tag/name receives a successful mark identity.

The dialogue-application lowering transaction first projects accepted marker
selectors in content order, mints this catalog, and resolves line-plan mark
uses against the catalog before publishing the application. Identical names in
different dialogue applications are valid because the content owner differs.
An unknown or duplicate local name is typed recovery and prevents final sema
publication.

Replace `HirTriggerPattern` directly with:

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirTrigger {
    Input(PatternId),
    Event(PatternId),
    Signal {
        target: ExprId,
        value: Option<PatternId>,
    },
    Timeout(ExprId),
    Mark(HirDialogueMarkId),
    Select(PatternId),
    Task(PatternId),
    Scope(PatternId),
    Expression(ExprId),
    Recovered(HirTriggerIssue),
}
```

`Recovered` is HIR poison only; it has no checked or transcript success tag.
`HirTriggerIssue` is a closed source-independent recovery enum and includes
missing/malformed trigger, unknown dialogue mark, and mark-outside-dialogue-
application cases. It retains no attempted spelling.

`HirStmtKind::On` owns `HirTrigger`. `Mark` has no `PatternId`, pattern arena
slot, pattern local, `TriggerPattern` child edge, or publication step.
`HirStatementChildRole::TriggerPattern` remains exactly for `Input`, `Event`,
`Select`, `Task`, and `Scope`; Signal retains its existing typed target/value
roles; Timeout and Expression retain `TriggerExpression`. Mark contributes its
typed HIR payload and no evaluated child. Rename `Expr` to `Expression`; do
not retain an alias or deprecated variant.

### 3. Select propagation belongs only to prefix Try

Replace the accepted HIR branch head in place with:

```rust
pub enum HirSelectBranchHead {
    Bind {
        binding: HirSelectBindingLocal,
        source: ExprId,
    },
    Frame {
        pattern: PatternId,
        locals: Box<[LocalId]>,
    },
    Event {
        pattern: PatternId,
        locals: Box<[LocalId]>,
    },
    Recovered,
}
```

Delete `propagates_error` from syntax projection, attachment, HIR, evaluation
views, source-index matching, tests, sema, compiler, and tooling. A trailing
question mark in a Select binding is invalid current syntax; it is not
accepted, warned, normalized, or routed through a compatibility parser.

`value = try stream.next() => ...` lowers `source` to the ordinary
`HirExprKind::Try` child. The existing checked Try carrier/boundary and the
expression transcript are the sole propagation authority. Select stores no
second Boolean, result/error projection, or Try summary.

### 4. Sema issues one accepted-rooted dialogue-mark coordinate

`crates/arcweft-lang-sema/src/semantic_coordinate.rs` owns:

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StableCheckedDialogueMarkCoordinate {
    application: CheckedSemanticPath,
    ordinal: HirDialogueMarkOrdinal,
}
```

Its constructor is private. The only issuer is:

```rust
impl SemanticCoordinateIndex<'_, '_> {
    pub(crate) fn dialogue_mark(
        &self,
        mark: HirDialogueMarkId,
    ) -> Result<StableCheckedDialogueMarkCoordinate,
             SemanticCoordinateIndexError>;
}
```

The issuer resolves `mark.content().owner()` through the existing accepted
expression path and trusts the opaque HIR constructor only after same-
generation/module validation. The canonical bytes are exactly:

```text
CheckedSemanticPath::canonical_bytes(application)
|| u8(2)  // checked dialogue-mark coordinate suffix
|| u8(0)  // dialogue-content mark family
|| u32_le(mark.ordinal)
```

Suffix `2` follows the existing body suffix `0` and output-target suffix `1`.
No BLAKE3 domain is added for this structural coordinate. The accepted
application path makes equal local ordinals in different applications
different coordinates. The name and `HirRichTextTagId` are excluded.

Final checked rich text and the final line plan use this coordinate directly:

```rust
#[derive(Clone, Debug)]
pub struct CheckedDialogueMark {
    coordinate: StableCheckedDialogueMarkCoordinate,
    diagnostic_name: HirDialogueMarkName,
}

pub enum CheckedRichTextAction {
    // existing closed non-marker variants unchanged
    Marker(CheckedDialogueMark),
}

pub struct CheckedDialogueLinePlan {
    effect_sites: Box<[CheckedDialogueEffectSite]>,
}
```

The source-ordered `CheckedRichTextAction::Marker` tokens are the sole final
checked mark inventory; `CheckedDialogueLinePlan` does not copy a second mark
slice. Delete `CheckedDialogueMarkOrdinal`, `CheckedDialogueMarkHandler`, the
`PublicId` mark slice, and the statement-to-mark side table. If rich-text
checking must precede accepted-root issuance internally, use one private
move-only prepared marker carrying `HirDialogueMarkId` and consume it during
the existing final-analysis seal. It must never be public, clonable into the
final report, or retained beside the stable coordinate.

`PreparedDialogueLinePlan` is reduced in the same cut to its prepared effect
sites only. Its constructor and `into_parts` return neither marks nor handlers;
`PreparedDialogueApplication` reaches any move-only prepared marker solely
through its prepared rich-text report. Thus neither the prepared line plan nor
the final line plan retains `marks`, `mark_handlers`, a statement ID, or an
ordinal side table.

`CheckedDialogueMark.diagnostic_name` is a nonsemantic display projection. The
compiler may copy it to the existing
`RichTextControl::Mark::diagnostic_name`; equality, mark selection, handler
lowering, transcript construction, and runtime event routing use only the
typed coordinate/runtime ID. Renaming a mark and all of its uses can therefore
change diagnostic output without changing semantic identity.

`CheckedDialogueMark` implements `PartialEq`/`Eq` on `coordinate` only, like
other accepted rows with a diagnostic label; it does not derive name-sensitive
semantic equality. `diagnostic_name()` is an explicit display accessor.

During one compiler semantic-fact construction transaction, enumerate marker
actions in checked content order, assign the existing contiguous
`RuntimeDialogueMarkId`, and build a private temporary
`StableCheckedDialogueMarkCoordinate -> RuntimeDialogueMarkId` map. After all
dialogue applications have been projected, project each reachable
`CheckedTrigger` into the lower-layer execution fact:

```rust
pub enum RuntimeTriggerAdmission {
    Input,
    Event,
    Signal,
    Timeout,
    Mark(RuntimeDialogueMarkId),
    Select,
    Task,
    Scope,
    Expression,
}
```

`arcweft-runtime-plan/src/semantic_facts.rs` owns this value under its ordinary
active-statement fact lookup; the map key `StmtId` is same-generation lookup
evidence and is not persistent identity. A checked Mark coordinate absent from
the exact owning content rejects. Drop the temporary coordinate map before
publishing runtime semantic facts. Runtime line-plan lowering consumes
`RuntimeTriggerAdmission` and the existing typed HIR children; it does not
reclassify the trigger or resolve a label. Delete
`RuntimeDialogueMarkHandler`, `RuntimeDialogueApplication.mark_handlers`, its
constructor parameter/accessor, the compiler copy loop, and runtime-plan
statement-side lookup. Core `RuntimeDialogueMarkId`, AWBC mark IDs, content
events, and `LineTaskTrigger::Mark` remain the lower typed runtime authority.

`wait(mark(.name))`, if admitted by the final suspension-expression cut, must
resolve through this same HIR mark catalog and stable coordinate. The current
`RuntimeWaitTarget::Mark(String)` is not an alternative semantic authority and
must not be used to implement or transcript line-local marks. The return must
either migrate that admitted path to the same typed runtime mark ID in the
same cut or prove with an executable rejection that it remains unadmitted;
there is no string fallback.

### 5. Sema validates scrutinee types and publishes only non-child meaning

`crates/arcweft-lang-sema/src/final_analysis/model.rs` owns these final types:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedTrigger {
    Input,
    Event,
    Signal,
    Timeout,
    Mark(StableCheckedDialogueMarkCoordinate),
    Select,
    Task,
    Scope,
    Expression,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedSelectStatement {
    Operand,
    Branches(Box<[CheckedSelectBranchHead]>),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedSelectBranchHead {
    Bind,
    Frame,
    Event,
}
```

All constructors are private to final analysis. There is no checked recovery
variant. Trigger tags are Input=0, Event=1, Signal=2, Timeout=3, Mark=4,
Select=5, Task=6, Scope=7, Expression=8. Select statement tags are Operand=0
and Branches=1; Select-head tags are Bind=0, Frame=1, Event=2. These tags exist
only within their enclosing version-one statement transcript domains.

The exact scrutinee `TypeKind` is already owned by the corresponding
`CheckedPattern`; Timeout and Expression types are already owned by their
`CheckedExpression`; Signal target type is owned by its checked target
expression; and Select Bind type is owned by its checked source expression and
binding local. The accepted statement transcript consumes those facts through
typed HIR child roles and their checked child digests. No final compiler,
runtime-plan, verifier, transcript, or tooling consumer needs an independent
copy. Therefore the final Trigger/Select payload stores only non-child family,
mark coordinate, branch-head kind, and source order. It does not publish
`CheckedStatementScrutinee { role, ty }` or
`CheckedTriggerExpression { kind, ty }`.

Preparation still needs one closed ephemeral role switch so it can select the
right expected type before checking each pattern. Own it in
`final_analysis/analyzer/preparation.rs`, not the public model:

```rust
pub(crate) enum StatementScrutineeRole {
    TriggerInput,
    TriggerEvent,
    TriggerSignal,
    TriggerSelect,
    TriggerTask,
    TriggerScope,
    SelectFrame,
    SelectEvent,
}
```

This enum is a private exhaustive control input, not stored phase state and not
encoded. Put the one borrowed selector beside it in
`final_analysis/analyzer/preparation.rs`:

```rust
pub(crate) struct StatementScrutineeTypeAuthority<'a> {
    standard: &'a RegisteredStatementIngressTypes,
    project: HirExecutableProjectView<'a>,
    topology: &'a HirProjectEvaluationTopology,
    entries: &'a PreparedEntrySemanticAuthority<'a>,
}
```

`StatementScrutineeTypeAuthority` is a view, not a map or published catalog.
It owns no `TypeKind`, has no `Clone`, and is dropped after pattern seeding and
statement validation. Extend the existing
`final_analysis/analyzer/patterns.rs::PatternSeedContext` and
`analyzer/preparation.rs` statement-context pass in place so every successful
Trigger and Select pattern is seeded before pattern analysis.

`crates/arcweft-lang-sema/src/registration/model.rs` owns the sole standard
ingress role schema, as an immutable field of `RegisteredTypeCheckEnv`:

```rust
pub struct RegisteredStatementIngressTypes {
    input: TypeKind,
    task: TypeKind,
    scope: TypeKind,
    frame: TypeKind,
}
```

Its fields and constructor are private; read-only accessors return borrowed
`TypeKind`s. The ordinary registered-environment transaction constructs this
schema and includes it in `RegisteredEnvironmentDigest`. `input` is validated
to be exactly `TypeKind::entity_ref(EntityKind::Input)`. `task`, `scope`, and
`frame` are the exact non-`Named`, non-poison standard semantic types selected
by the corresponding typed environment publication roles. The returned
contract must name those three publication-role ID types and their existing or
new in-place registration inputs; it may not use path/name lookup to fill the
schema. Missing, duplicate, open, recovered, or conflicting role publication
rejects registration, so there is no default or detached success schema. This
fixed record is the role registry, not a second nominal catalog.

The expected-type switch is exact:

| Role | Sole expected-type source |
| --- | --- |
| Trigger Input | `RegisteredStatementIngressTypes::input()`, exactly `TypeKind::entity_ref(EntityKind::Input)` |
| Trigger Event / Select Event | the unique event `HirEntryTypeBinding::ty()` reachable for the statement's accepted executable roots, read through `PreparedEntrySemanticAuthority::ty(TypeId)` |
| Trigger Signal | first check the target; require `TypeKind::entity_ref_with_value(EntityKind::Signal, T)` and seed the optional value pattern with exactly `T` |
| Trigger Select | exactly `TypeKind::entity_ref(EntityKind::ChoiceOption)` from the enclosing HIR Choice lifecycle owner |
| Trigger Task | `RegisteredStatementIngressTypes::task()` |
| Trigger Scope | `RegisteredStatementIngressTypes::scope()` |
| Select Frame | `RegisteredStatementIngressTypes::frame()` |
| Timeout | exactly `TypeKind::Duration` on the checked expression child |
| generic Expression | exactly `TypeKind::Bool` on the checked expression child |
| Select Bind | the checked source expression result after ordinary prefix-Try checking; the resolved binding local receives that exact type |

For Event roles, topology enumerates every stateful Entry whose accepted root
can reach the statement, obtains that Entry's unique
`HirEntryMember::EventType(HirEntryTypeBinding)` through the existing typed
member inventory, and reads its already resolved `TypeKind` from the prepared
Entry authority. Zero reachable stateful Entry event types, a recovered Entry
role, or unequal candidate `TypeKind`s rejects before pattern publication.
The final-analysis seal consumes this preparation proof by comparing the
chosen type's `SemanticTypeDigest` with every corresponding
`CheckedStatefulEntry::event().semantic_type()` in the one
`CheckedEntryCatalog`; it retains no event-type side table.

Do not select a nominal with `TypeKind::Named`, terminal-name lookup,
`source_label`, a hard-coded project type name such as `GameEvent`, or
`Any`/`Other`.

After pattern checking, the statement constructor re-reads the completed
`CheckedPattern.ty` and proves it equals the selected contextual expected type,
then consumes the proof. Signal additionally proves target payload equality.
Timeout and Expression prove their checked child types are `Duration` and
`Bool`. Select Bind proves that the checked binding local type equals the
checked source expression result type after ordinary prefix-Try analysis. No
proof is retained as another `TypeKind`. The source-ordered head slice is the
only checked branch inventory; its array index is the canonical ordinal. The
constructor enumerates that slice and proves each index agrees with
`HirStatementChildRole::{SelectBinding,SelectSource,SelectPattern}` and
`HirStatementBodyRole::SelectBranch`. It does not retain a
`CheckedSelectBranchOrdinal` or a second branch record.

### 6. Unsafe audit uses the existing absolute identity family

Replace `HirUnsafeAudit.id: HirIdRefValue` with:

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirUnsafeAuditIdentity {
    Accepted(UnsafeAuditId),
    Recovered(HirUnsafeAuditIdentityIssue),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirUnsafeAuditIdentityIssue {
    Missing,
    InvalidReference,
    NonAbsolute,
    WrongFamily,
}

pub struct HirUnsafeAudit {
    identity: HirUnsafeAuditIdentity,
    reason: Option<ExprId>,
    has_safety_doc: bool,
}
```

Both ordinary statement lowering and dialogue-candidate statement lowering
must perform the same typed projection. Only `HirIdRef::Absolute` whose
normalized ID constructs `UnsafeAuditId` succeeds. Relative,
family-relative, wrong-family, empty, missing, and recovered references remain
typed recovery and poison the final statement. Source-index queries retain the
source site for diagnostics/repairs, but source-index evidence is not the
semantic identity.

Final analysis owns:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedUnsafeAudit {
    id: UnsafeAuditId,
    has_safety_doc: bool,
}
```

Its private constructor accepts the HIR `UnsafeAuditId`, validates that the
optional reason child is pure `String` when present, and retains only the
non-child SAFETY-documentation bit. `semantic_id()` is a read-only projection
that calls `UnsafeAuditId::semantic_id()`; no caller supplies and no checked row
copies digest bytes. Reason presence/type remains owned by the typed
`HirStatementChildRole::UnsafeReason` edge and its `CheckedExpression`.

Missing reason or SAFETY documentation remains visible to the existing
dev/test/release verifier policy so it can report a repair obligation; neither
is silently fabricated or treated as a release success. The verifier obtains
the ID and SAFETY bit from `CheckedStatementPayload::UnsafeAudit` and reason
presence from the accepted typed statement-child inventory. It does not re-read
or render the HIR ID. Delete `id_ref_label`; tooling may render
`CheckedUnsafeAudit.id()` for humans, but no rendered value is fed back into
verification, compiler lowering, transcript construction, or lookup.

The statement transcript encodes `AcceptedUnsafeAuditSemanticId` projected
from the accepted ID, the SAFETY-documentation bit, the optional typed
reason-child digest through
`HirStatementChildRole::UnsafeReason`, and the unsafe body digest. It never
encodes `@`, an ID spelling, `HirIdRefValue`, a span, or a recovery label.

## Complete final checked statement authority

The above rows must enter the one final statement model. Replace
`CheckedStatementRole` directly; do not retain an alias, `Ordinary`, optional
old field, compatibility constructor, or dual validator.

```rust
pub struct CheckedStatement {
    effects: EffectSet,
    payload: CheckedStatementPayload,
}

pub enum CheckedStatementPayload {
    Structural,
    Assignment(Box<CheckedAssignment>),
    Assertion(CheckedAssertionDisposition),
    Defer(DeferOutcome),
    EvaluatedEffect(Box<CheckedEvaluatedEffect>),
    Iteration(Box<CheckedIteration>),
    ControlTransfer(CheckedControlTransferTarget),
    Trigger(CheckedTrigger),
    UnsafeAudit(CheckedUnsafeAudit),
    Select(CheckedSelectStatement),
    SourceLocale(LocaleTag),
    Scope(CheckedScopeIdentity),
    Include(CheckedIncludeFlowTarget),
    Suspension(Box<CheckedSuspensionStatement>),
    Yield,
}

pub enum CheckedScopeIdentity {
    Anonymous,
    Named,
}

pub struct CheckedIncludeFlowTarget {
    declaration: CallableDeclarationDigest,
}
```

`CheckedStatement` has one crate-private constructor. It obtains the
`EffectSet` from the existing completed statement/child effect fold and
validates payload-specific effects (including the sealed evaluated operation)
before publication; a caller cannot pair an arbitrary effect set with a
payload. Keeping this accepted aggregate is not permission to copy child
types, identities, or effects into individual payload variants.

`CheckedScopeIdentity::Named` records the semantic presence of a name, not its
spelling; the accepted body coordinate is the scope identity. Include resolves
through the existing checked Flow/callable catalog and retains its accepted
`CallableDeclarationDigest`. Yield is a unit payload: construction validates
the yielded expression type against the enclosing
`CheckedFunctionExecution::StreamFactory` item type, then consumes that proof.
The typed expression child digest and function contract remain the two
legitimate owners; the statement does not copy a second `TypeKind`. The scope
and Include types are named here only to close the 35-family statement carrier;
the returned design must reuse an already accepted exact type if current
`main` has landed one with equivalent ownership.

Payload tags are exactly Structural=0, Assignment=1, Assertion=2, Defer=3,
EvaluatedEffect=4, Iteration=5, ControlTransfer=6, Trigger=7, UnsafeAudit=8,
Select=9, SourceLocale=10, Scope=11, Include=12, Suspension=13, Yield=14. The
HIR statement tag is always encoded first, so `Structural` does not repeat the
old `Ordinary` collapse. It is permitted only for the explicit whitelist
below.

### Exhaustive 35-family production matrix

| `HirStmtKind` | final checked payload |
|---|---|
| Assertion | `Assertion` |
| Let | `Structural` |
| Assign | `Assignment` |
| LetElse | `Structural` |
| LetChoice | `Structural` |
| LetScope | `Structural` |
| LetActionReceive | `Structural` |
| Return | `Structural` |
| Out | `ControlTransfer(Output)` |
| Goto | `Structural` |
| DeferBlock | `Defer` |
| Defer | `Defer` |
| Yield | `Yield` |
| Signal | `Structural` |
| LifetimeSet | `Structural` |
| Wait | `Suspension` |
| On | `Trigger` |
| UnsafeLifetime | `UnsafeAudit` |
| Choice | `Structural` |
| If | `Structural` |
| IfLet | `Structural` |
| Match | `Structural` |
| While | `Structural` |
| WhileLet | `Structural` |
| For | `Iteration` |
| Close | `Structural` |
| Select | `Select` |
| SourceLocale | `SourceLocale` |
| Scope | `Scope` |
| Include | `Include` |
| Break | `ControlTransfer(Loop)` |
| Continue | `ControlTransfer(Loop)` |
| Expression | `EvaluatedEffect` only for the exact sealed effect; otherwise `Structural` |
| ProofCall | `Structural` |
| Error | reject; no `CheckedStatement` |

Return/Goto/Close and other Structural rows retain their expression meaning
through typed child digests. If implementation evidence shows one of these
families has non-child meaning absent from HIR typed children, the returned
design must add one general final payload family and migrate all affected rows;
it may not put a source string, raw ID, or special-case flag into Structural.

## Producer and consumer migration

The returned owner/consumer matrix must be complete and at minimum cover:

- syntax rich-text projections, Trigger attachments, Select branch
  attachments, parser diagnostics, formatter/canonicalizer, and syntax tests;
- HIR dialogue content/rich-text records, statement/thread records, final
  lowering for ordinary and dialogue-candidate blocks, statement evaluation
  plans, child/body edges, source index/projection, recovery, transaction
  limits, and tests;
- sema rich-text checking, pattern seeding, expression/statement preparation,
  `semantic_coordinate` and its catalog issuer, final-analysis model,
  validation, type visitation, report publication, semantic transcript, and
  project-index summaries;
- compiler checked-statement consumption, dialogue-content projection,
  rich-text marker projection, runtime semantic-fact construction, persistent
  diagnostics that inspect statement families, and compiler tests;
- runtime-plan semantic facts, validation, final-flow line-plan lowering,
  AWBC lowering inputs, and runtime-plan tests;
- `arcweft-verify`, CLI/LSP unsafe-audit summaries/actions, and verifier tests;
  and
- all direct constructors and fixtures of every deleted type.

Compiler and runtime-plan must consume final checked rows. They may still use
HIR IDs privately to locate the same-generation statement/expression, but raw
IDs never choose semantic meaning and never enter persistent/transcript
identity. `arcweft-core` remains lower and Sans I/O; sema must not depend on
compiler, runtime-plan, verifier, CLI, or adapters.

## Atomic implementation and deletion order

The following is the required edit/deletion order. Steps 1 through 7 form one
compile-clean authority cut and must not land as independently successful
parallel models.

1. Confirm the selected evaluated-effect types above and the existing
   `CheckedControlTransferEvidence`/`CheckedControlTransferTarget` prerequisite
   are present. Do not repair `checked_break_role` or the old effect enum.
2. Add the typed syntax mark-selector projection and remove Select trailing-`?`
   parsing/projection. Update syntax diagnostics and canonical tooling so
   removed syntax fails closed.
3. Evolve HIR dialogue content with the mark catalog; replace
   `HirTriggerPattern` by `HirTrigger`; replace unsafe-audit identity; remove
   `propagates_error`; update both HIR lowering paths, typed edges/evaluation,
   source projection, recovery, limits, and invariants.
4. Add the registered standard-ingress schema, the borrowed statement
   scrutinee-type authority, and the accepted-rooted mark coordinate; complete
   contextual pattern seeding. Construct final `CheckedTrigger`,
   `CheckedSelectStatement`, and `CheckedUnsafeAudit` rows. Consume Entry-event
   and private prepared-mark proofs into final rows before publication.
5. Replace `CheckedStatementRole` and every `PreparedStatementFact` success
   path with `CheckedStatementPayload`; consume the existing affine control
   evidence and sealed evaluated effects. Implement the exhaustive 35-row
   matrix with direct Rust matches and no wildcard success arm.
6. Switch checked rich text, compiler, runtime-plan, verifier, project index,
   and all tests to the final rows. Project mark ordinals directly into the
   existing runtime content issuer.
7. Delete, before the cut compiles:
   `HirTriggerPattern`, `propagates_error`, trailing-`?` Select helpers,
   marker `PublicId` parsing, `CheckedDialogueMarkOrdinal`,
   `CheckedDialogueMarkHandler`, `RuntimeDialogueMarkHandler`, prepared/final
   line-plan mark and handler fields/accessors/constructor parts, runtime
   dialogue `mark_handlers` fields/accessors/constructor parameters, recursive
   handler collection, runtime HIR trigger recheck, `CheckedStatementRole`,
   `checked_break_role`, unsafe `id_ref_label`, old constructors/validators,
   and every obsolete test success branch.
8. In the next atomic transcript cut, build the accepted parent’s single
   memoized expression/pattern/statement/body graph from these final rows.
   Encode all 35 statement tags and payloads, seal all Match rows only after
   the complete catalog succeeds, and delete the lazy Match-only transcript
   builder. An error publishes neither a partial transcript catalog nor a
   `CheckedMatch`.

If source dependency constraints make steps 1–7 too large for one commit, the
return may identify smaller commits only when each commit has one final owner
and no old successful reader. A private move-only prepare/seal state is allowed;
a public bridge, adapter, compatibility alias, optional old field, or two
successful payload enums is not.

## Required positive tests

The final contract and later implementation must include at least:

1. one fixture for every successful Trigger family, proving exact pattern or
   expression type and body/local publication;
2. Signal with and without a value pattern, including payload-type equality;
3. dialogue marks in nested line-plan groups, two marks, same local name in
   different applications, and direct projection to the expected runtime mark
   ID without a handler side table;
4. coordinated renaming of a marker tag and all its local uses leaves semantic
   digests equal, while reordering marks or swapping handler references changes
   the appropriate coordinate/digest;
5. Select Operand and source-ordered Bind/Frame/Event branches, with binding,
   pattern, local, body, and array-index-to-HIR-role equality without a stored
   branch ordinal;
6. `value = try source =>` proves propagation is owned by the existing checked
   Try child and changes the source expression digest when Try is removed;
7. absolute `@unsafe.*` admission, semantic-ID sensitivity, String reason,
   present/absent metadata reporting, and verifier repair behavior from the
   checked payload;
8. HIR arena allocation/ID/span/source-format perturbation with equal mark,
   unsafe-audit, Trigger, Select, statement, and body semantic digests;
9. a generated exhaustive fixture over all 35 `HirStmtKind` variants proving
   one payload or rejection row each, with `Error` the sole rejection-only
   HIR family; and
10. compiler/runtime-plan/verifier integration proving consumers never need a
    source spelling or public structural expression ID.

## Required negative tests and structural gates

Include executable rejection tests for:

- malformed, missing-dot, attributed, duplicate, unknown, cross-application,
  recovered, and mark-outside-dialogue selectors;
- a forged HIR mark with foreign content/tag, noncontiguous ordinal, duplicate
  tag, duplicate name, or wrong application owner;
- trailing `source?` in Select, including proof that the parser does not strip
  it or set hidden propagation state;
- recovered Select head, a typed child/body role whose branch index disagrees
  with source-order enumeration, wrong binding/source type, Frame/Event role
  swap, missing contextual ingress type, and incompatible Entry event schemas;
- wrong Input/Event/Select/Task/Scope/Frame pattern type, non-Signal target,
  wrong Signal payload, non-Duration Timeout, and non-Bool Expression trigger;
- relative, family-relative, wrong-family, malformed, missing, and recovered
  unsafe-audit IDs; non-String or effectful reason; and forged semantic-ID
  bytes;
- missing/duplicate/foreign control evidence and effect applications;
- one mutation per 35-family payload matrix row, including Structural used for
  a non-whitelisted family and any successful `Error` row;
- exact-limit and one-over mark/branch/transcript accounting with no partial
  publication; and
- compile-API/trybuild unavailability tests proving old public constructors,
  types, fields, and variants cannot be used, plus typed behavior, codec, and
  structured dependency-graph checks proving semantic consumers accept only
  the final authority.

Source inventories and searches are design/review evidence only. They are not
production acceptance rules. Typed constructors, compile-API failures,
behavior, transcript perturbation, codecs, structured dependency graphs,
compiler checks, and runtime tests are acceptance evidence.

## Non-goals and prohibitions

- No production Rust patch is returned in the design archive.
- Do not redesign accepted generic Match coverage, evaluated-effect operation
  selection, control-transfer topology, runtime scheduling, task persistence,
  or public AWBC wire shape.
- Do not add `Any`, transcript `Other`, `UnsupportedIdentity` success, a source
  string, debug/display/Serde encoding, raw HIR ID, public structural `ExprId`,
  copied statement AST, copied body graph, or whole-catalog digest.
- Do not add a legacy reader, deprecated variant, compatibility alias, optional
  old field, version bump, `V2`/`V3` name or domain, or a second mark/Trigger/
  Select/unsafe-audit model. Every Arcweft-owned version marker remains `1`.
- Diagnostic names/ranges may remain in the source index or a clearly
  nonsemantic display projection. They never select or validate semantic
  meaning.
- No new worktree, branch, workspace checkout, commit, or push is authorized by
  this request.

## Required returned archive

Return exactly:

`arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1.2.2-checked-statement-trigger-select-mark-and-unsafe-audit-authority-correction-final-contract.zip`

The archive has one top-level wrapper named exactly as the ZIP basename and
contains:

- `README.md` with reading order and inspected Git evidence;
- `FINAL_DESIGN.md` as the sole normative answer;
- `DECISION_REGISTER.md` distinguishing repository evidence from selected
  design and tracing every decision above;
- `HIR_AND_SEMA_SCHEMAS.md` with exact Rust-shaped types, visibility,
  constructors, invariants, and tags;
- `SCRUTINEE_TYPE_SOURCES.md` naming the exact Entry/Choice/task/scope/standard
  owner and accessor for every Trigger and Select role;
- `MARK_COORDINATE_AND_TRANSCRIPT.md` with the exact coordinate bytes and
  statement/rich-text transcript grammar;
- `OWNER_CONSUMER_MATRIX.md` with every producer, consumer, dependency edge,
  and deletion target;
- `IMPLEMENTATION_AND_DELETION_ORDER.md` with compile-clean cut boundaries;
- `TEST_MATRIX.md` covering every positive, negative, perturbation, limit, and
  all-35 exhaustiveness case;
- `SOURCE_EVIDENCE.md` with current paths, symbols, blob identities, and search
  inventories;
- `machine/final_contract.json` containing the closed type/tag/matrix/deletion
  inventory;
- a repository-aware validator plus negative self-tests that mutate every
  mandatory gate;
- a byte-identical copy of this request, covered by the manifest;
- `MANIFEST.txt`, `VALIDATION_REPORT.md`, `FINAL_STATUS`, and
  `OPEN_QUESTIONS`.

The archive may claim `READY_FOR_IMPLEMENTATION` only when
`OPEN_QUESTIONS` is exactly `none`, every contextual scrutinee type source is
constructible from current or same-cut legitimate typed owners, and the
validator proves the all-35 matrix is exhaustive. Reopen the produced ZIP and
rerun wrapper, member, manifest, request-mirror, status, source-SHA, and
validator checks against its actual bytes. Report design validation separately
from repository Cargo validation; no unrun command may be reported as passed.
