# Single-value View Match selector ABI

## Exact signature and input

For a reactive Match over `Need<T>`, the parent runtime journal projects current state into one ephemeral ordinary four-case `NeedState<T>` variant:

```text
NotStarted        -> state.NotStarted(Tuple([]))
Pending(progress) -> state.Pending(progress)
Ready(payload)    -> state.Ready(payload)
Cancelled         -> state.Cancelled(Tuple([]))
```

The synthetic state identity/digest commits to `T`; it is inserted as a `RuntimePlanTypeSeed` in the same type batch and interned by `AwbcInventory`. It is not a Need handle, endpoint record, View value, presentation value, or copied type table.

The selector signature is exactly:

```text
params = [input_state_type]
result = Some(selector_result_type)
```

The runtime driver projects one journal state, verifies `input_state_type_digest`, and passes that value as the sole parameter. The selector does not evaluate the producer expression, subscribe, start a task, or execute the selected View body.

## Result type

For `N` source arms, runtime-plan interns one synthetic nominal variant:

```text
Variant Nominal(owner = generated selector identity) {
  case 0 "arm_00000000": Tuple(binding types for arm 0),
  ...
  case N-1 "arm_XXXXXXXX": Tuple(binding types for arm N-1),
}
```

Case ordinal equals source arm ordinal. Every case has a tuple payload; zero bindings uses `Tuple([])`, never absent payload. Heterogeneous arms work because each case has its own tuple type. Nested Result/Option and other recursively snapshot-clone closed values use ordinary RuntimeValue/AWBC variants.

## Construction owner and API

The compiler constructs `RuntimeViewMatchSelectorSeed` from `CheckedMatchRef` and the same `FinalSemanticAnalysis`, then inserts it into the existing atomic runtime semantic-fact input. Runtime-plan finalization resolves all expressions/patterns/locals and rewrites `RuntimeSemanticTypeId` to the one `RuntimePlanTypeId` table, producing `RuntimeViewMatchSelector`.

`arcweft-runtime-plan::awbc_lower::view_match::ViewMatchSelectorBuilder` receives only `&RuntimePlan`, `&RuntimeViewMatchSelector`, and `&mut AwbcInventory`. It returns `LoweredViewMatchSelector` containing function, input/result types and digests, checked-match digest, and source-ordered cases. It never names sema/View/bundle types and never receives `CheckedMatch` directly.

## Function body

The input parameter is evaluated zero additional times. For each arm in source order the builder emits `TestPattern`, `Branch`, `EnterScope`, `BindPattern(Declare)`, optional ordinary guard evaluation and `Branch`, `MakeTuple`, `MakeVariant`, and `Return`. A failed pattern or false guard exits the temporary scope and continues. After the last failure it emits a stable no-match Trap.

The selector does not evaluate the arm's View body. The selected case tells the View evaluator which precompiled `ViewInstructionRange` to execute.

## Frame lifetime proof

The sole return coordinate is the register named by `AwbcTerminator::Return`. Current VM semantics clone that value before `FiberState::finish_return`; `finish_return` pops the callee frame and transfers the owning value. `RuntimeValue::Variant` owns its tuple recursively, so no register/scope/frame pointer survives.

The bundle section never stores output registers. `AwbcRegisterId` occurs only inside the AWBC function body/frame layout.

## Core verification

`AwbcProgram::verify_view_match_selector` checks:

- Synthetic kind, deterministic/non-suspending flags, one parameter, one result;
- exact input state row/digest and result nominal owner/digest;
- case count, dense ordinal, generated name, and tuple payload for every arm;
- tuple length/item types/depth/closed checked-type validity;
- every return constructs that result type;
- `MakeVariant.case` equals the source arm/cross-section case;
- every tuple uses source-ordered binding registers from its arm only;
- no affine/borrowed/unique/must-drop/frame-local/non-cloneable/non-snapshot output;
- no `AwbcTerminator::Match` or `AwbcMatchArm.guard`; and
- canonical code/type digest consistency.

Bundle validation compares the verified owner to `ViewMatchSelectorBindingV1` and `CheckedMatchRef`; core does not depend on bundle/View.

## Decode and transactional install

The driver performs read-only validation before local mutation:

1. exact site, active generation, checked-match digest, function, input/result digests;
2. `RuntimeValue::Variant` with exact nominal owner;
3. case ordinal/name in range and equal to bundle arm;
4. `Some(RuntimeValue::Tuple)` payload;
5. exact tuple count, distinguishing missing and extra values;
6. recursive value/type match for every output `AwbcTypeId`;
7. dense output ordinals, exact View local coordinates, no duplicate local, `SnapshotClone` disposition;
8. valid selected body range;
9. stage all values in `LocalInstallTransaction`; and
10. one revision-checked commit.

Dropping/rejecting a transaction changes nothing. Commit conflict also changes nothing. Closed errors distinguish stale site/digest/generation, malformed nominal/case/payload, missing/extra/wrong nested value, ownership rejection, duplicate local, invalid body, no-match Trap, and commit conflict.
