# Implementation and deletion order

Steps 1–7 are one atomic statement-authority cut. They may be worked through
in temporary compile-broken states, but there is no accepted intermediate
publication with two successful authorities. Step 8 is the immediately
following transcript cut. This document describes implementation order; it
does not authorize a branch, worktree, commit, or push.

## 1. Freeze prerequisites and expose their final read-only surface

- Confirm the selected dirty `CheckedEvaluatedEffect*` operation algebra and
  sealed `CheckedCallExecutionSource` are present; do not redesign them.
- Confirm accepted control-transfer evidence/targets are present. Promote
  final target types/accessors to public read-only sema visibility required by
  the payload; keep coordinate constructors private.
- Add compile-time inventory tests for the selected effect and control schemas.
- Delete/leave deleted obsolete effect success models and `checked_break_role`;
  never repair those paths as a bridge.

Exit condition: the two prerequisite typed authorities are uniquely named and
have no alternate successful producer.

## 2. Syntax cut

- Add `SyntaxDialogueMarkName` and make rich-text marker forms plus Trigger mark
  attachments use it.
- Enforce required dot, one identifier, no attributes/multiple arguments, and
  typed recovery.
- Remove Select trailing-`?` detection, range stripping, attachment field,
  grammar-projection field, formatter success, and tests. Prefix Try remains.
- Update parser diagnostics and canonicalizer/formatter tests so removed syntax
  fails closed.

Exit condition: no syntax success product carries a mark String/PublicId or a
Select propagation Boolean.

## 3. Final HIR cut

- Add the HIR mark ordinal/ID/name/row and the one content-owned catalog.
- Make content lowering enumerate accepted marker tags in source order and
  validate all catalog invariants atomically.
- Resolve line-plan mark uses through that catalog in both ordinary and
  dialogue-candidate lowering paths.
- Replace `HirTriggerPattern` with `HirTrigger`; remove Mark pattern allocation,
  edge, local, and publication; rename `Expr` to `Expression`.
- Replace unsafe raw identity with accepted/recovered typed identity and share
  the exact absolute `@unsafe.*` projection helper across both statement
  lowerers.
- Remove Select `propagates_error` across HIR statement/thread records,
  evaluation views, source-index projection, child/body edges, recovery,
  limits, and tests.
- Add `enclosing_choice_lifecycle(StmtId)` to typed HIR topology.

Delete before moving on: old Trigger enum/constructors, Mark pattern success,
raw unsafe success identity, Select propagation field/helpers, and every old
fixture constructor.

Exit condition: HIR compiles with one Trigger/mark/unsafe/Select schema and
all 35 statement/child/body inventories are exhaustive.

## 4. Registration, preparation, coordinates, and specialized checked rows

- Add `StandardStatementIngressTypeId`, `StatementIngressTypeRoleId`, closed
  `StatementIngressTypePublicationInput`, and `TypeKind::StatementIngress`
  with semantic tag `88` and inner tags `0..2`.
- Add exhaustive behavior for the new type atoms: normalization, visitors,
  type digest, matching/compatibility, openness/poison checks, display-only
  diagnostics, and environment digest.
- Have `TypeCheckEnv::new()` contribute exactly the three fixed rows. Consume
  them into `RegisteredStatementIngressTypes`; reject missing, duplicate,
  mismapped, open, recovered, poison, Named, and conflict states.
- Split analyzer draft stores for short-lived immutable selector borrows, then
  implement the deterministic Entry-seeded declaration worklist: seed one
  declaration's contextual patterns, drop the selector borrow, complete its
  ordinary patterns/expressions/selected calls/statements, and propagate its
  exact event digest/contributors over newly selected edges.
- Factor the one private Entry root seed, prepare Include edges, handle
  recursion/equal-contributor deltas with bounded checked accounting, and
  independently recompute the completed graph before consuming Event proofs.
- Add the borrowed `StatementScrutineeTypeAuthority` and seed all eight pattern
  roles before pattern analysis. Add exact Timeout/Expression/Select Bind
  post-checks.
- Add mark coordinate issuance and any private move-only prepared marker.
- Construct `CheckedTrigger`, `CheckedSelectStatement`,
  `CheckedSelectBranchHead`, and `CheckedUnsafeAudit`; consume all temporary
  equality/root/mark proofs.

Exit condition: final specialized checked rows exist only after all child
facts and seals succeed; no contextual type map or prepared proof is published.

## 5. Replace the complete checked statement authority

- Replace `CheckedStatementRole` directly with
  `CheckedStatementPayload`; change `CheckedStatement` to exactly effects plus
  payload.
- Add `CheckedScopeIdentity` and `CheckedIncludeFlowTarget`; reuse the accepted
  locale, suspension, assignment, assertion, defer, iteration, evaluated
  effect, and control-transfer types.
- Resolve Include once into a move-only accepted callable proof and consume it
  both for reachability and payload construction.
- Implement one explicit Rust match with all 35 `HirStmtKind` arms in the exact
  matrix. `Error` returns failure. No `_` success arm.
- Validate the completed statement effect fold and payload-specific effect
  invariants in the sole constructor.

Delete before moving on: `CheckedStatementRole`, `Ordinary`, old constructor
parameters/accessors/validators, `PreparedStatementFact` success rows that
represent the old model, `checked_break_role`, and every old fixture.

Exit condition: every successful statement has one payload row and every
consumer compiles against it or is intentionally still broken awaiting step 6.

## 6. Rich text, compiler, runtime-plan, verifier, and tooling migration

- Change `CheckedRichTextAction::Marker` to `CheckedDialogueMark`; make its
  source-ordered actions the only checked mark inventory.
- Reduce prepared and final line plans to effect sites; remove all mark/handler
  parts and recursive handler collection.
- In compiler content projection, enumerate marker actions, issue existing
  contiguous runtime mark IDs, build the temporary coordinate map, project
  every reachable Trigger, and drop the map before publishing facts.
- Add `RuntimeTriggerAdmission` to runtime-plan semantic facts and switch
  validation, final-flow line-plan lowering, AWBC inputs, persistent
  diagnostics, and tests to it.
- Remove runtime HIR trigger rechecks and label lookup.
- Switch verifier and CLI/LSP unsafe summaries/actions to checked payload plus
  typed reason/body children; keep policy diagnostics for missing reason or
  SAFETY documentation.
- Switch project-index summaries and compiler statement classification to the
  final payload.
- Add executable rejection for `wait(mark(.name))` at the final
  suspension/runtime-plan admission boundary; never invoke the legacy string
  target.

Exit condition: every downstream semantic consumer reads final checked rows;
raw HIR lookup is generation-only and never selects meaning.

## 7. Mandatory deletion closure and repository validation

Delete all remaining live definitions, fields, accessors, constructor
parameters, helpers, imports, and success tests for:

```text
HirTriggerPattern
Select propagates_error and trailing-question helpers
marker PublicId parsing/string reconstruction
CheckedDialogueMarkOrdinal
CheckedDialogueMarkHandler
prepared/final line-plan marks and handlers
RuntimeDialogueMarkHandler
RuntimeDialogueApplication.mark_handlers
recursive handler collection and runtime statement-side mark lookup
CheckedStatementRole / Ordinary
checked_break_role
unsafe id_ref_label and HIR-re-reading verifier constructors
```

Run formatting, targeted owner/consumer tests, workspace check, full tests,
Clippy with warnings denied, rustdoc, dependency-direction checks, and
repository searches. Distinguish passed, failed, blocked, and not-run tiers.

Exit condition: steps 1–7 are compile-clean and test-clean with deletion
searches proving only design/history and deliberate compile-fail assertions
retain old names.

## 8. Transcript and generic-Match closure

- Build one memoized expression/pattern/statement/body graph from the final
  rows using the grammar in `MARK_COORDINATE_AND_TRANSCRIPT.md`.
- Encode all 35 HIR statement tags, all 15 payload tags, all Trigger/Select
  tags, typed child/body roles, rich-text Marker coordinate, unsafe semantic
  identity, and effects.
- Seal all Match rows only after the complete transcript catalog succeeds.
- Delete the lazy Match-only transcript builder and any old statement/body
  grammar.
- Keep current five `CheckedSelectResolution` and 26
  `ViewSpecifiedValue` inventories; do not reintroduce predecessor-stale rows.
- Verify cancellation, checked accounting, N/N+1 limits, permutation
  stability, and atomic failure.

Exit condition: transcript/catalog publication is atomic; no partial catalog or
`CheckedMatch` exists on error.

## Compile-clean boundary rule

If physical implementation must use multiple local commits, each commit must
still have one final owner and no old successful reader. Private move-only
prepare/seal states are allowed. Public bridges, adapters, aliases, optional
old fields, duplicate enums, and fallback readers are forbidden. This design
task itself creates no commit.
