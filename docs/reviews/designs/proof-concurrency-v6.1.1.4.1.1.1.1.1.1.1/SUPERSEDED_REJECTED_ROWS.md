# Superseded rejected rows

The following claims from the two rejected returns are deleted rather than
renamed or retained as compatibility evidence:

- `HirSelectedMember::Invalid`, attached Invalid/ErrorNode member branches,
  invariant-only Invalid payload tests, Invalid diagnostics/source rows, and
  invalid attempted-name accounting;
- a standalone `AttachedSelectExpr`, delimiter enum, OptionalDot flag, CST walk,
  and second projection/source reader;
- direct lowering of `?.` to a two-slot Select and every row that omits the
  postfix Try identity or its Operand/Operator source sites;
- `target..member`, `target..`, compact repeated-dot producers, compact 128/129
  dot limits, and their nested Select/source/diagnostic totals;
- unnamed/test-only missing-target producers and all E13
  `RecoveryOperand(Target)` synthetic-child rows;
- `MissingOperand { role: Target }` as authored-child propagation;
- parser recovery-record deltas, E13 syntax diagnostic deltas, detached-parser
  128/129 evidence, and shadowed 256/257 recovery evidence;
- source-query validation that checks document/revision/length before role
  applicability/ordinal;
- `HirWorkBudget`, `HirNameConstructionError`, or any E13-specific name
  constructor;
- root diagnostics keyed by `(owner, role)`, duplicate root diagnostics, and
  tests comparing nonexistent diagnostic error payloads;
- combined Expressions/TotalSlots tests where one deterministic failure masks
  the other; and
- any adjacent sidecar requirement, alias, wrapper, compatibility reader,
  source reparse, source gate, CSS/Takumi branch, or removed-syntax diagnostic.

The earlier useful decisions—authored Select target, missing member without a
fabricated name, `Whole`/`Target`/`SelectedMember`, owner-keyed diagnostics,
payload-derived member obligation, independent limits, and deletion-driven
migration—are fully restated in this standalone package.
