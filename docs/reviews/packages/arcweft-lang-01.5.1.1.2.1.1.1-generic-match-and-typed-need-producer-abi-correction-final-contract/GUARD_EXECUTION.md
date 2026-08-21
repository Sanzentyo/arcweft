# Guard execution correction

## Current-source defect

Current verification treats `AwbcMatchArm.guard` as a callable with exactly one scrutinee parameter and Bool result, and checks its effects. Current `AwbcVm::execute_match_terminator` pattern-matches and installs captures but never calls that guard. The runtime-plan expression seed, however, preserves each HIR arm's optional guard.

Using the AWBC Match terminator for the new View selector would therefore produce verifier/VM semantic divergence. This contract closes the divergence without expanding scope to redesign generic AWBC Match.

## Sole selector lowering

A View Match selector SHALL use explicit ordinary instructions and `Branch`, never `AwbcTerminator::Match`:

```text
entry:
    evaluate scrutinee once -> r_scrutinee
    Jump arm_0_test

arm_i_test:
    TestPattern pattern_i, r_scrutinee -> r_matches
    Branch r_matches, arm_i_bind, arm_i+1_test

arm_i_bind:
    EnterScope scope_i
    BindPattern pattern_i, r_scrutinee, Declare
    if no guard: Jump arm_i_select
    evaluate guard_i exactly once -> r_guard
    Branch r_guard, arm_i_select, arm_i_reject

arm_i_select:
    MakeTuple source-ordered local registers -> r_payload
    MakeVariant result_type, case=i, payload=r_payload -> r_result
    Return r_result

arm_i_reject:
    ExitScope scope_i
    Jump arm_i+1_test

no_match:
    Trap(TypeMismatch, "verified View Match had no selected arm")
```

Pattern failure does not enter the scope. Guard false exits it before trying the next arm. The selected path returns an owning tuple; frame destruction then removes all arm-local registers.

## Effect authority

The guard expression is an ordinary checked expression. Its exact effects remain in its child `CheckedExpression`; the enclosing Match expression owns the aggregate effects. `CheckedMatch` stores only the guard `ExprId`. Runtime-plan lowering resolves the same child fact and emits ordinary AWBC calls/instructions. No guard-effect row is copied into View or bundle schemas.

## Validation and tests

Structural validation rejects any generated View selector containing `AwbcTerminator::Match`, a nonempty `AwbcMatchArm` range, or a guard function reference. Differential tests run the same patterns and guards through the existing pure runtime-expression evaluator and the explicit selector chain and require identical selected arm and bindings. Required rows include false guard followed by a later matching arm, side-effect accounting exactly once, guard wrong type, guard trap, all guards false, and callee-frame destruction after a true guard.
