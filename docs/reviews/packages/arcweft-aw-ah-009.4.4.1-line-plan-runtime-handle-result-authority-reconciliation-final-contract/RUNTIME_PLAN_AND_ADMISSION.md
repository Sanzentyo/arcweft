# RuntimePlan construction and admission

## 1. Sole owner and no second tree

Typed syntax and final HIR continue to own `HirLinePlan`.  Runtime lowering does
not copy that node graph.  It consumes the HIR once and writes ordinary
`FlowOp`s, existing line-task nodes, exact local declarations, handle-site
facts, and one result target into the existing RuntimePlan graph.

The sole runtime owner is:

```text
RuntimeDialogueContentPlan
  └── RuntimeLineTaskGroupId
        └── LineTaskGroup
              ├── captures
              ├── activation_ops: [FlowOp]
              ├── result_type
              ├── handle_sites
              ├── root / nodes
              ├── cancel_rules
              └── cleanup
```

The parent flow owns:

```text
FlowOp::Dialogue {
    content,
    result: RuntimeDialogueResultTarget { ty, pattern },
}
```

There is no runtime `LinePlan`, statement reader, source text, callee-name
recognizer, or fixture-specific branch.

## 2. Exact lowering of the primary fixture

The maintained fixture is unchanged:

```arcw
let (_, cue) = alice(voice=auto)[聞いて。[p]]
with:
    let actor = alice.stage.acquire(scope=line)
    let cue = at(0.42s):
        actor.look(.worried, crossfade=120ms)
    let voice = line.voice_handle()
    out (voice, cue)
```

The final RuntimePlan shape is:

```rust
LineTaskGroup {
    captures: [],
    activation_ops: [
        FlowOp::LineOperation {
            binding: Some(RuntimePattern::Bind { local: actor_local, .. }),
            operation: RuntimeLineOperation::AcquireActor {
                site: site_0,
                character: alice_id,
                scope: RuntimeLineHandleScope::Line,
            },
        },
        FlowOp::LineOperation {
            binding: Some(RuntimePattern::Bind { local: cue_local, .. }),
            operation: RuntimeLineOperation::Schedule {
                site: site_1,
                delay: RuntimeExpr::Duration(420_000_000),
                child: child_0,
                captures: [RuntimeExpr::Local(actor_local)],
            },
        },
        FlowOp::LineOperation {
            binding: Some(RuntimePattern::Bind { local: voice_local, .. }),
            operation: RuntimeLineOperation::VoiceHandle { site: site_2 },
        },
        FlowOp::CommitDialogueResult {
            value: RuntimeExpr::Tuple([
                RuntimeExpr::Local(voice_local),
                RuntimeExpr::Local(cue_local),
            ]),
        },
    ],
    result_type: tuple(VoiceHandle, CueHandle),
    handle_sites: [
        site(site_0, StageActor, StageActorHandle<alice>, None),
        site(site_1, Cue, CueHandle, Some(child_0)),
        site(site_2, Voice, VoiceHandle, None),
        site(site_3, Cue, CueHandle, None), // actor.look inside child_0
    ],
    root: sequence(start(child_0)),
    nodes: [
        LineTaskNode::Child {
            trigger: LineTaskTrigger::Scheduled(site_1),
            join_policy: Join,
            cancel_policy: CancelAndJoin,
            scope: action_0,
            ..
        },
        LineTaskNode::Action([
            FlowOp::LineOperation {
                binding: None,
                operation: RuntimeLineOperation::ActorLook {
                    site: site_3,
                    actor: RuntimeExpr::Capture(0),
                    look: RuntimeExpr::CharacterLook(alice_worried),
                    crossfade: RuntimeExpr::Duration(120_000_000),
                },
            },
        ]),
    ],
    ..
}

FlowOp::Dialogue {
    content: content_id,
    result: RuntimeDialogueResultTarget {
        ty: tuple(VoiceHandle, CueHandle),
        pattern: RuntimePattern::Tuple([
            RuntimePattern::Discard,
            RuntimePattern::Bind { local: outer_cue, .. },
        ]),
    },
}
```

Important consequences:

- the actor value is captured when the schedule operation executes, after
  actor acquisition and before cue issuance;
- `actor.look` is a typed child action, not a parsed callback body;
- the unbound `actor.look` result remains owned by the child/line scope; it is
  not the same as explicit `_`;
- `out` commits an exact tuple; the outer pattern is not evaluated until the
  dialogue closes successfully;
- the discarded voice handle is dropped at result publication, not by reading
  its debug label.

## 3. Source-order lowering rules

For each `HirLinePlanItem` in source order:

| HIR item | RuntimePlan output |
|---|---|
| `let p = stage.acquire(...)` | one `LineOperation::AcquireActor`, exact site, `binding=Some(p)` |
| `let p = at(delay): callback` | callback action nodes plus one `LineOperation::Schedule`, exact site and capture expressions |
| `let p = line.voice_handle()` | one `LineOperation::VoiceHandle`, exact site, `binding=Some(p)` |
| `actor.look(...)` statement | one `LineOperation::ActorLook`, `binding=None` |
| `let _ = handle_expr` | operation with `binding=Some(RuntimePattern::Discard)` or ordinary `Drop`, causing immediate typed drop after successful evaluation |
| `drop(value)` | existing drop operation; affine token lookup and ledger transition happen in the original drop implementation |
| `out expr` | one `FlowOp::CommitDialogueResult { value: expr }` and termination of the completing activation path |

A source item after an unconditional `out` is rejected by final analysis as an
unreachable line-plan item.  Conditional/cancellation control is represented
by ordinary `FlowOp` control flow; every completing path must reach exactly one
commit and every non-completing nonlocal path must reach zero.

When the maintained source form has no authored `out`, final analysis fixes the
line result to `Unit` and lowering appends one exact
`CommitDialogueResult { value: Unit }` to the normal activation path.  This is
a typed semantic synthesis, not a string result, fake effect, or source-parsed
fallback.

## 4. Capture construction

There are two capture classes:

1. **group captures**: values from the parent flow needed by activation ops,
   mark callbacks, cancellation, or cleanup;
2. **scheduled child captures**: values evaluated at the exact `at` source
   position and stored in the issued cue's live state.

The schedule operation evaluates:

```text
1. delay expression
2. delay conversion and checked deadline
3. capture expression 0
4. capture expression 1
...
5. issuance ordinal allocation
6. child live-state insertion
7. CueHandle construction
8. binding or implicit scope registration
```

Failure at steps 1–4 mutates nothing.  Failure at step 5 leaves no ledger or
child entry.  Steps 6–8 are one transaction.

Child functions receive exactly the scheduled capture vector; they do not
clone the whole activation environment.  AWBC function signatures use those
exact capture types.

## 5. Producer/type projection

The compiler runtime semantic projection maps final semantic types as follows:

| Checked type | Runtime projection |
|---|---|
| `StageApi<C>` | `Err(NonValueLineCapability::StageApi)` if requested as a value; stage calls lower directly while the capability is in checked call facts |
| `LineContext` | `Err(NonValueLineCapability::LineContext)` if requested as a value; method lowers directly in line context |
| `CharacterLook<C>` | exact existing entity-reference checked type |
| `StageActorHandle<Exact<C>>` | exact `RuntimeOpaqueTypeOwner` for `std.line.stage_actor_handle` and Character C |
| `StageActorHandle<Any>` | producer-wide owner, never accepted by `ActorLook` without exact narrowing |
| `CueHandle` | exact opaque owner `std.line.cue_handle` |
| `VoiceHandle` | exact opaque owner `std.line.voice_handle` |

The generic RuntimePlan runtime-type table remains the single producer/type
authority.  The line lowering code references those accepted type ids; it does
not keep a producer map of its own.

## 6. Construction API

Owner: existing RuntimePlan construction module.

```rust
impl RuntimePlanBuilder {
    pub(crate) fn add_line_task_group(
        &mut self,
        input: RuntimeLineTaskGroupInput,
    ) -> Result<RuntimeLineTaskGroupId, RuntimePlanBuildError>;

    pub(crate) fn add_dialogue_op(
        &mut self,
        flow: RuntimeFlowId,
        content: RuntimeDialogueContentPlanId,
        result: RuntimeDialogueResultTarget,
    ) -> Result<(), RuntimePlanBuildError>;
}

pub(crate) struct RuntimeLineTaskGroupInput {
    pub captures: Box<[RuntimeLocalDeclarationId]>,
    pub activation_ops: Box<[FlowOp]>,
    pub result_type: RuntimePlanTypeId,
    pub handle_sites: Box<[RuntimeLineHandleSite]>,
    pub root: RuntimeLineTaskNodeId,
    pub nodes: Box<[LineTaskNode]>,
    pub cancel_rules: Box<[LineCancelRule]>,
    pub cleanup: LineTaskCleanup,
}
```

`LineTaskGroup::new` remains crate-private and performs the same admission as
the builder.  Tests cannot construct an unchecked group through public struct
fields.

## 7. Admission obligations

Admission is one pass over the accepted type graph plus bounded control-flow
analysis.

### 7.1 Type and owner

- every local/capture/result id exists;
- every expression's stored checked type equals its accepted runtime type;
- line capabilities occur only as operation facts, never values;
- each handle result type is opaque, exact for its producer, affine, and
  snapshot-only;
- exact StageActor Character equals site Character;
- each Character look is an exact entity ref owned by that Character;
- all opaque payload validators are registered through the existing owner.

### 7.2 Sites and schedule topology

- site ids are dense `0..n`, unique, and source ordinals strictly increase;
- a site kind agrees with every operation that references it;
- a scheduled site references exactly one child and that child's trigger
  points back to the same site;
- no non-schedule operation references a scheduled child;
- child capture count and type vector equal its action function parameters;
- the child belongs to the same group and is reachable from the root;
- no action node is reachable through both joined and detached ownership paths;
- delay expressions produce `Duration` and are not evaluated as pure metadata.

### 7.3 Result graph

For the bounded CFG of activation, admitted cancellation-result handlers, and
cleanup/nonlocal exits, dataflow state is:

```rust
enum CommitCount { Zero, One, Many }
```

Join is exact (`Zero+One` alternatives remain a set); admission rules are:

- every normal completing exit has only `One`;
- duplicate path state `Many` is rejected;
- cancellation explicitly declared as completing must have only `One`;
- cancellation/failure/return/goto paths declared non-completing have only
  `Zero`; a committed value on such a path is abandoned during unwind;
- commit expression type equals group `result_type`;
- parent `RuntimeDialogueResultTarget.ty` equals group `result_type`;
- target pattern is admissible for `R`, including affine moves and discards;
- no child action may contain `CommitDialogueResult` unless its function role
  is the single activation owner or an admitted same-activation cancellation
  result function.  Ordinary scheduled/mark children are rejected.

### 7.4 Cleanup/control

- every affine site is covered by an owner slot on every path;
- normal, cancelled, and failed close each run exactly one cleanup selection;
- joined work reaches terminal state before publish;
- detached work has no line-scope affine capture and no result-cell access;
- return/goto cannot skip ledger unwind;
- cleanup actions cannot reacquire a result-owned handle after publish begins.

## 8. Structured errors, never fallback

At minimum, builder/lowering diagnostics distinguish:

```text
line.non_value_capability_projection
line.missing_handle_producer
line.wrong_handle_value_class
line.wrong_handle_persistence
line.duplicate_handle_site
line.site_kind_mismatch
line.schedule_child_mismatch
line.callback_capture_type_mismatch
line.result_missing
line.result_duplicate
line.result_type_mismatch
line.result_pattern_mismatch
line.result_commit_from_child
line.detached_affine_capture
line.cleanup_path_uncovered
line.limit_exceeded
```

None may select `RuntimeExpr::String`, `LineEffectRequest`, an ordinary
intrinsic, a fake task spec, or a no-op call as recovery.
