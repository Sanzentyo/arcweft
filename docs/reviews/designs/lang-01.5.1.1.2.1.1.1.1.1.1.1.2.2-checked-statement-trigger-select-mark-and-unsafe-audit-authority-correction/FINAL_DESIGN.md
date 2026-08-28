# Final design

This file is the sole normative answer. If another member differs, this file
and `machine/final_contract.json` control; the validator rejects disagreement.
This contract directly replaces the unreleased statement, Trigger, Select,
dialogue-mark, and unsafe-audit paths named below. It does not preserve a
compatibility reader or parallel model.

## 1. One authority chain

The final chain is:

```text
syntax selector / prefix Try / absolute unsafe ID
  -> final HIR identities and child topology
  -> registration-owned standard ingress record
  -> private contextual pattern seeds
  -> checked child facts
  -> one CheckedStatementPayload
  -> purpose-built statement transcript
  -> compiler-local mark-ID projection
  -> runtime admission
```

Every transition is typed. Source spelling, terminal-name lookup, `PublicId`
reparsing, raw HIR IDs, raw group paths, `Any`, `Other`, `Named` stand-ins, and
whole-analysis digests are not semantic authorities.

## 2. Syntax and HIR

Syntax parses exactly one `SyntaxDialogueMarkName` after the required dot in
`[mark .name]`, inferred `[.name]`, and `on mark(.name)`. Attachment and final
lowering carry that node; neither stores a `String` nor reparses it.

Final HIR gives each dialogue-content mark an ordinal in source content order.
`HirDialogueContent` owns a single `marks: Box<[HirDialogueMark]>` catalog.
Accepted marker tag payloads carry `HirDialogueMarkId`; the rich-text stream and
the catalog therefore join by a typed ID. Duplicate names, a missing selector,
an invalid selector, a marker outside a content owner, and an ID/catalog
mismatch reject HIR acceptance.

`HirTrigger` has nine closed variants. `Mark` carries only
`HirDialogueMarkId`; it has no pattern child. `Expression` replaces the old
`Expr` spelling. `Recovered(HirTriggerIssue)` is HIR poison and has no checked
success representation.

Select bind heads are `Bind { binding, source }`. Suffix-question-mark
stripping and `propagates_error` are deleted from syntax attachment, final HIR,
source-index projection, child edges, and tests. Ordinary prefix
`HirExprKind::Try` is the sole propagation authority.

Unsafe lowering resolves an exact absolute `@unsafe.*` identity in both HIR
lowering paths and stores `HirUnsafeAuditIdentity`. Missing, invalid,
non-absolute, and wrong-family references retain typed recovery issues and
cannot reach checked success.

## 3. Registration owns standard statement ingress

`registration/model.rs` owns the sole immutable
`RegisteredStatementIngressTypes` inside `RegisteredTypeCheckEnv`. Its four
borrowed accessors return exact `TypeKind`s. `input` is exactly
`TypeKind::entity_ref(EntityKind::Input)`.

The standard environment publishes the other three types through a closed
typed input, not a name lookup:

```rust
pub enum StandardStatementIngressTypeId {
    TaskEvent,
    ScopeExit,
    FrameBoundary,
}

pub enum StatementIngressTypeRoleId {
    Task,
    Scope,
    Frame,
}

pub struct StatementIngressTypePublicationInput {
    role: StatementIngressTypeRoleId,
    ty: StandardStatementIngressTypeId,
}
```

The sole mapping is `Task -> TaskEvent`, `Scope -> ScopeExit`, and
`Frame -> FrameBoundary`. The semantic type algebra gains the closed atom
`TypeKind::StatementIngress(StandardStatementIngressTypeId)`. These atoms are
ordinary exact, hashable, transcriptable semantic types; they are not nominal
names and they do not depend on runtime-core Rust structs. The standard
`TypeCheckEnv::new()` transaction contributes exactly these three input rows.
`ProjectRegistrar` validates and consumes them into the fixed record and drops
the input rows. External/source-backed environment inputs cannot add roles.

Missing, duplicate, mismapped, open, recovered, poisoned, `Named`, or
conflicting rows fail registration. The fixed record and all four type semantic
digests enter `RegisteredEnvironmentDigest` domain version `1`. This is one
closed standard-type algebra plus one role record, not a second nominal catalog
or a free-form extension registry.

## 4. Contextual pattern preparation

`StatementScrutineeTypeAuthority` is a private borrowed view with the exact
four fields selected in the request: registered standard types, final HIR
project, HIR topology, and `PreparedEntrySemanticAuthority`. It owns no
`TypeKind`, is not `Clone`, is never published, and is dropped immediately
after contextual pattern seeding and statement validation.

The current analyzer order cannot soundly answer Event reachability because it
seeds statement patterns without Entry context. A global “finish every call,
then seed Event” reorder is also insufficient: selected call resolution may use
a local bound by an Event pattern. Implementation therefore uses one private,
move-only Entry-seeded declaration worklist:

1. resolve types, callable schemas, non-contextual locals/pattern seeds, exact
   stateful Entry `goto` roots, and prepared Include Flow edges;
2. enqueue each stateful Entry root with that Entry's already resolved event
   `TypeKind` and exact Entry item identity;
3. for a declaration's first incoming event digest, briefly borrow
   `StatementScrutineeTypeAuthority`, seed that declaration's contextual
   patterns, drop the borrow, and complete its patterns, expressions, calls,
   and statements;
4. propagate the same digest and contributing Entry identities over each newly
   selected project-call edge and prepared Include edge; equal later inputs
   merge, unequal inputs reject before publication;
5. handle recursion with deterministic declaration-key worklist/SCC state and
   continue propagating newly discovered equal Entry contributors without
   retyping the declaration;
6. reject every Event-bearing declaration reached by zero stateful Entries,
   independently recompute reachability over the completed selected graph, and
   consume the resulting proofs while sealing the one checked Entry catalog.

Entry-root resolution is factored out of the present Entry checker into one
private helper shared by worklist preparation and the final Entry seal. The
worklist's declaration→digest/contributor state is proof-construction scratch,
not a public or retained contextual-type catalog; it is consumed after the
independent completed-graph check. A statement reached from zero stateful
Entries, from a recovered Entry, or from Entries with unequal event
`TypeKind`s rejects. Multiple reachable Entries with the same
`SemanticTypeDigest` succeed.

The final Entry seal compares the selected digest to every corresponding
`CheckedStatefulEntry::event().semantic_type()` in the single
`CheckedEntryCatalog`; the consumed proof and traversal scratch are then
dropped.

Choice Select context is obtained from a typed
`HirProjectEvaluationTopology::enclosing_choice_lifecycle(StmtId)` query. It
must identify exactly one accepted enclosing Choice lifecycle; the expected
type is always `TypeKind::entity_ref(EntityKind::ChoiceOption)`. The query does
not scan source or infer from a terminal name.

The exact role table is in `SCRUTINEE_TYPE_SOURCES.md`. Pattern checking writes
the selected type only into the ordinary checked pattern/local facts. Statement
construction re-reads those facts and consumes equality proofs; it never copies
a contextual `TypeKind` into statement payload.

## 5. Final checked statement authority

`CheckedStatement` contains exactly `effects` plus one
`CheckedStatementPayload`. The payload has the fifteen variants and tags in
`HIR_AND_SEMA_SCHEMAS.md`. Constructors are crate-private and all published
access is read-only. `CheckedScopeIdentity` uses the accepted body coordinate;
it does not retain source spelling. `CheckedIncludeFlowTarget` carries the one
`CallableDeclarationDigest` already owned by the checked callable catalog.

The exhaustive production match has 35 explicit HIR arms. `Error` rejects;
there is no wildcard success arm. `Expression` publishes `EvaluatedEffect`
only when the dirty evaluated-effect WIP's already-selected sealed operation
contract accepts it; otherwise it is `Structural`. This design does not
redesign that effect contract. The exact all-35 matrix is normative in
`TEST_MATRIX.md` and machine-readable in `machine/final_contract.json`.

`CheckedTrigger`, `CheckedSelectStatement`, and
`CheckedSelectBranchHead` carry only non-child meaning. Child pattern and
expression types remain authoritative in the ordinary checked child maps.
Source order of `CheckedSelectBranchHead` is the sole branch inventory and its
array index is the ordinal proven against final-HIR child/body roles.

## 6. Dialogue mark coordinate and runtime projection

Sema issues
`StableCheckedDialogueMarkCoordinate { application, ordinal }` through
`SemanticCoordinateIndex::dialogue_mark`. Canonical bytes are exactly:

```text
CheckedSemanticPath::canonical_bytes(application)
|| u8(2)
|| u8(0)
|| u32_le(ordinal)
```

`CheckedDialogueMark` equality and hashing use only this coordinate;
`diagnostic_name` is display-only. `CheckedRichTextAction::Marker` carries this
typed mark. `CheckedDialogueLinePlan` retains only effect sites.

The compiler enumerates checked marker actions in content order, issues the
existing `RuntimeDialogueMarkId`s, builds one temporary
coordinate-to-runtime-ID map, projects reachable checked Mark triggers to
`RuntimeTriggerAdmission::Mark`, and drops the map before publishing the
runtime plan. Runtime/AWBC/core never receive the stable sema coordinate or a
string mark.

The final suspension cut does not yet admit `wait(mark(.name))` through checked
statement/runtime-plan lowering. This cut therefore must reject that form as
unsupported before executable publication. It must not call the legacy
`RuntimeWaitTarget::Mark(String)` path. A later suspension cut may admit the
surface form only by reusing the same `HirDialogueMarkId`, stable coordinate,
and compiler-local runtime-ID projection; no alternate identity is reserved.

## 7. Unsafe audit

Checked success is `CheckedUnsafeAudit { id: UnsafeAuditId,
has_safety_doc: bool }`; an optional reason remains the ordinary checked String
child. Semantic bytes use only `UnsafeAuditId::semantic_id()` plus the closed
boolean and child transcript. The verifier consumes the checked payload and
reason child. It must not re-read HIR, `id_ref_label`, source text, or a
`PublicId`.

## 8. Transcript and predecessor precedence

The purpose-built statement transcript uses version `1`, closed tags,
accepted-rooted `StableCheckedStatementCoordinate` and
`StableCheckedBodyCoordinate` bytes, length prefixes, checked child digests,
and semantic identities. Its exact grammar is in
`MARK_COORDINATE_AND_TRANSCRIPT.md`. It has no serde transcript, source
spelling, raw ID, range, wildcard, or whole-analysis digest.

This design amends only the generic predecessor's statement/body grammar and
the owners named here. It does not import two stale inventories from that
package: current accepted source has five `CheckedSelectResolution` variants
(`Method`, `DialogueView`, `AgentField`, `ProgressField`, `Field`) and 26
`ViewSpecifiedValue` variants. Tuple/record expression select tags remain
reserved, not live. Neither inventory is the new `CheckedSelectStatement`
algebra. All unrelated expression/style grammar follows later accepted
contracts and current source.

## 9. Atomicity and terminal state

Schema, producers, consumers, transcript, tests, and deletion steps 1–7 are one
atomic semantic cut. The transcript/generic closure follows as step 8 only
after that cut is compile-clean. Publication is transactional: any recovery,
type disagreement, coordinate collision, missing child, limit failure, or
Entry-seal mismatch publishes no partial final analysis or runtime plan.

There are no result-changing open questions. The selected design is
constructible from current owners plus the same-cut typed registration input,
phase reorder, and topology query specified above.
