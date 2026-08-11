# Atomic lowering, freeze, rollback, and retry order

## One transaction

```text
ParsedSource
-> parser-owned PendingExpressionProjection / projected start event
-> ExpressionProjection::{Try, Select}
-> central AttachedExpressionNode
-> staged final HIR/source/diagnostic/work transaction
-> freeze
-> sole source/diagnostic query
-> commit or complete rollback
```

## Deterministic preflight and staging order

1. Validate `SourceDocumentIdentity` (document ID, revision, retained length)
   and checked source byte arithmetic.
2. Resolve the central attached expression and validate one snapshot, exact
   kind, projected start/event owner, `Target`, and `SelectedMember` component
   shape. `Name` must carry parser-validated `SyntaxName`; `Missing` must carry
   the exact zero-width `MissingName` component. Any impossible conversion is a
   hard invariant failure.
3. Recursively lower the target and obtain its typed child allocation/source/
   poison manifest. Parentheses contribute authored span geometry but no HIR
   expression ID.
4. For `Name`, preflight the member spelling against
   `HirLimit::NameBytes`, then call the unchanged `HirName::try_new`. `Missing`
   charges zero and does not call the constructor.
5. Compute the singular root poison with target-propagation precedence and
   independently derive the member diagnostic obligation from
   `HirSelectedMember`.
6. Checked-add retained syntax diagnostics plus staged HIR diagnostics and
   preflight `HirLimit::Diagnostics`.
7. Checked-add expression allocations and preflight
   `HirLimit::Expressions`.
8. Checked-add every arena allocation and preflight
   `HirLimit::TotalSlotsPerModule`.
9. Checked-add slot-Whole metadata and component manifest rows; validate typed
   uniqueness. There is no separate source-component limit.
10. Reserve deterministic qualified IDs from the attached syntax identity,
    including a distinct Try ID before its outer Select ID where `?.` is used.
11. Stage payloads, poison states, slot metadata, source components, and
    owner-keyed diagnostics in descendant-before-ancestor order.
12. Freeze validates: every child is live and same-module; source components
    match kind/payload; every Missing member has exactly one root diagnostic;
    every Name has none; no diagnostic owner repeats; every primary/site is
    exact; and no propagation-only parent diagnostic exists.
13. Publish all staged maps/vectors/counters in one commit. Only after commit do
    read-only source/diagnostic queries observe the new owners.

A preflight error is selected in the order above. Work-order perturbation may
change internal evaluation order only after all inputs are captured; it cannot
change the specified error precedence or any committed identity/result.

## Rollback

Any failure removes, as one unit:

- root, Try, target, and other reserved IDs not previously committed;
- payloads and poison states;
- slot-Whole metadata and all components;
- diagnostics and freeze obligations;
- scopes/locals/candidates/results touched by recursive child lowering; and
- expression, total-slot, name-byte, diagnostic, and source-manifest work
  deltas.

No rolled-back owner is queryable. Existing committed children remain
unchanged when a parent fails. A retry with the same source, syntax snapshot,
module, and HIR snapshot derives the same qualified IDs and diagnostic owners.
The diagnostic map deduplicates by qualified owner; retry never appends.

## Module-freeze truth table

```text
Target clean + Name     root clean; no root diagnostic
Target clean + Missing  MissingOperand(SelectedMember); one root diagnostic
Target poison + Name    RecoveredChild(Target); child diagnostic only
Target poison + Missing RecoveredChild(Target); child diagnostic then root member diagnostic
```

The `Missing` obligation is payload-derived, so target poison cannot suppress
it. `MissingOperand(Target)` and a synthetic target child are absent from every
E13 branch.
