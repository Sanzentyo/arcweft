# AW-AH-009.4.4.1 — line-plan runtime handle/result authority reconciliation

## Sequence and precedence

This is a narrow mandatory correction beneath AW-AH-009.4 CharacterDialogue
Cut 4 runtime lowering and the maintained post-Try convergence item 6.

Inspected production baseline:
`15ad861a954249a9430b32d53ae0fc79c019a4f0`.

Preserve the accepted CharacterDialogue domain, typed dialogue-content
application, `DialogueLine<R>` non-escaping operation rule, existing
`DialogueContentSpec`, `LineTaskGroup` reducer topology, final runtime-plan type
graph, generic opaque producer authority, and structured/AWBC parity. Redesign
one of those only when a concrete repository-evidenced flaw requires it.

Current repository-wide policy takes precedence over historical package
version increments: every Arcweft-owned version marker remains exactly `1`,
and unreleased shapes are replaced in place with no old reader.

## Split reason

Typed syntax, final HIR, and sema now preserve the primary fixture's complete
line plan, source-ordered bindings, scheduled callback, exact handle types, and
`out (voice, cue)` result. Executable lowering cannot be completed locally
without inventing authority:

- `StageApi`, `StageActorHandle`, `CueHandle`, and line-context handle
  operations have checked callable types but no typed runtime operation/value
  producer;
- `actor.look` has no Sans-I/O presentation/stage command owner;
- a `LineTaskGroup` child fiber receives a cloned capture environment and has
  no typed channel for committing the plan's `out R` to the suspended Dialogue
  owner;
- `FlowOp::Dialogue` has no result pattern/continuation value coordinate;
- the remaining `LineEffectRequest::RegisterHandle` and `LineOutRequest`
  carriers store strings and cannot satisfy exact opaque handle types or
  arbitrary `R`; and
- adding ordinary pure intrinsics would erase scheduling, scoped lifetime,
  cancellation, activation identity, and handle-drop behavior.

These decisions must be designed together because the handle token identity,
line-task activation, result commit point, cancellation cleanup, and AWBC
snapshot form constrain one another.

## Required decisions

1. Define the sole runtime value owner for `StageApi`, `StageActorHandle`,
   `CueHandle`, `VoiceHandle`, line context, and stage-look values. State which
   are non-values, exact opaque values, or another existing runtime value
   family; define producer identity, payload, equality, ownership, nesting,
   persistence, and checked-type validation without source strings.
2. Define the stable activation-scoped identities for actor, scheduled cue,
   and voice handles, including deterministic construction, multiple handles
   at one source site, replay, save/restore, hot replacement, and stale
   generation rejection.
3. Define exact lifetime ownership for `scope=line`, ignored `_` bindings,
   explicit drop, normal completion, cancellation, failure, return/goto, and
   joined/detached child work. No destructor may be inferred from a display
   label.
4. Define the final RuntimePlan owner for a typed line plan. Decide how the
   source-ordered setup statements, scheduled children, local declarations,
   captures, cleanup, and one completing `out R` are represented without a
   second detached LinePlan tree.
5. Define how `let (_, cue) = dialogue[...] with: ...` receives `R`. Select the
   sole result pattern and commit boundary across `FlowOp::Dialogue`,
   `DialogueState`, line-task completion, cancellation, and nonlocal control.
   State whether setup/result production occurs before presentation,
   atomically at activation, or at another exact phase.
6. Define `at(Duration): callback` as a typed scheduling operation: evaluation
   order, callback capture, returned `CueHandle`, trigger time, zero/negative/
   overflow handling, cancellation, joined completion, and callback failure.
   It must not lower as a synthetic wait or an unexecuted pure function.
7. Define the Sans-I/O command owner for `actor.look(look,
   crossfade=Duration)`, including exact Character/look identity, actor
   ownership proof, host request/output, ordering, cancellation, failure, and
   native/Web/headless behavior. Do not make the renderer parse a callable or
   handle label.
8. Define how line-context `voice_handle()` relates to the exact active
   dialogue/voice lifecycle, including absence of voice, lazy start, host
   failure, cleanup, and returned handle identity.
9. Replace or delete the string-valued `RegisterHandle`/`DropHandle`/
   `LineOutRequest` routes where this cut crosses them. If any remain for an
   unrelated released boundary, identify that external consumer and prove
   they cannot be selected by typed line-plan lowering.
10. Define RuntimePlan construction/admission and verifier obligations for
    result type `R`, local/capture types, handle producers, activation owner,
    schedule topology, exactly-one completing result, and cleanup/control
    paths. Missing proof must be a structured lowering error, not a fallback.
11. Define the in-place AWBC schema, codec, verifier, VM, line-task reducer,
    suspension, and fiber snapshot forms. Keep every version marker `1` and
    delete replaced discriminants/readers in the same atomic cut.
12. Define structured/AWBC behavioral parity, including exact observation and
    host-request ordering across activation, scheduled callback, dialogue
    advance, cleanup, and result binding.
13. Define bundle/save/replay/hot-replacement behavior for an active dialogue
    with scheduled handles and an uncommitted or committed result. State
    transactional failure precedence for type, producer, generation, and
    activation mismatches.
14. Provide a compile-clean implementation interleave and exact deletion
    matrix covering the temporary checked-only `Named` handle spellings,
    runtime semantic projection exclusions, string effect carriers, fixture
    exceptions, and any obsolete tests.
15. Provide bounded work accounting for plan items, locals, handles, scheduled
    callbacks, captures, child nodes, cleanup actions, result size/depth,
    queued host commands, and save/restore validation.

## Required consumer inventory

Inspect and cover at least:

- maintained dialogue, line-plan, scoped-handle, cancellation, runtime-core,
  AWBC parity, save/replay, and presentation chapters;
- the AW-AH-009.4 CharacterDialogue final contract and its RUN-037 row;
- final syntax/HIR/sema dialogue application and `HirLinePlan` owners;
- checked callable identities for StageMethod, `at`, line context, and drop;
- compiler runtime semantic owner inventory and dialogue application facts;
- `arcweft-runtime-plan` final expression/flow/dialogue/line-task and AWBC
  lowering;
- `arcweft-core` RuntimeValue/type graph, FlowOp, DialogueState, line-task
  reducer/child fibers, effects, AWBC schema/codec/verifier/VM/snapshots;
- dialogue, presentation, runtime-driver, native/Web/headless, bundle, replay,
  hot-reload, Agent observation, and CLI consumers; and
- every current line-task/Dialogue/AWBC parity test plus the two maintained
  dialogue-with-plan fixtures.

## Required tests

- the unchanged
  `spec_should_pass/run/011_dialogue_line_value_and_handle_discard.arcw`
  fixture through check, structured execution, AWBC execution, and CLI;
- the simpler mark-triggered `current_pass/check/011_dialogue_with_plan.arcw`;
- actor acquire, look command, cue scheduling, voice handle, `out` tuple,
  destructuring, and `_` discard with exact runtime types;
- multiple same-site handle activations and save/replay identity stability;
- dialogue advance before/at/after cue trigger, cancellation, callback failure,
  host rejection, and cleanup ordering;
- missing voice, invalid look owner, wrong actor activation, stale generation,
  wrong opaque producer, malformed result pattern/type, duplicate result,
  missing result, and oversized plan/result/capture cases;
- native/AWBC differential traces for commands, effects, result binding,
  status, diagnostics, and final state;
- codec/tamper/snapshot/hot-replacement transactional tests; and
- compile-fail/API proof that the replaced string handle/out route and any
  temporary runtime fallback cannot be selected.

## Constraints and non-goals

- Do not revive the detached/source-parsed LinePlan model, raw statement
  reader, callee-name recognizer, source-site exception, or fixture allowlist.
- Do not synthesize string handles, debug-label results, fake task specs,
  no-op stage calls, or a pure `at` intrinsic that does not schedule work.
- Do not add a second runtime value algebra, copied opaque-producer table,
  dynamic/untyped result slot, compatibility alias, shim, dual reader, or
  version greater than `1`.
- Do not redesign RichText, CharacterDialogue patch/default semantics, View,
  Await/Need, Const, timeout, CSS, or Takumi in this correction.
- Keep lower crates Sans I/O and all scheduling/replay behavior deterministic.

## Expected output

Return one independently usable design-only archive named
`arcweft-aw-ah-009.4.4.1-line-plan-runtime-handle-result-authority-reconciliation-final-contract.zip`.
It must contain `OPEN_QUESTIONS.md` exactly `none`, exact Rust-shaped owners and
APIs, RuntimePlan/AWBC/save schemas, command and result phase timelines,
identity/lifetime/failure tables, deletion and implementation interleaves,
bounded limits, and the complete positive/negative/tamper/parity test matrix.
Do not include a production code overlay.
