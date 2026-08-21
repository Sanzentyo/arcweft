# Generic `CheckedMatch` authority

## Sole semantic owner

Every live `HirExprKind::Match` is represented by exactly one `CheckedExpression` whose resolution is `Match(Box<CheckedMatch>)`. The generic fact is not View-specific and replaces `Structural` for all Match expressions.

HIR remains the structural/source owner. The existing final-analysis rows remain the sole type/effect owners:

- enclosing `CheckedExpression.ty()` and `.effects()` own Match result and aggregate effects;
- scrutinee, guard, and arm-value `CheckedExpression` rows own their normalized types and child effects;
- `CheckedPattern.ty()` owns pattern type; and
- `CheckedBinding.ty()` owns every local type.

`CheckedMatch` stores only:

- exact scrutinee `ExprId`;
- dense source-ordered arm identity `(owner ExprId, ordinal)`;
- exact `ScopeId`, `PatternId`, optional guard `ExprId`, value `ExprId`, and HIR `LocalId` sequence;
- dense binding output ordinal plus the local's checked ownership disposition; and
- one `CheckedMatchCoverage` containing exhaustiveness and the unique sorted unreachable-arm set.

It does not store duplicate `TypeKind`, effects, per-arm coverage, source spans, AWBC IDs, or View coordinates. There is no `arm_expression`, detached AST, inferred `TypeId`, positional sidecar, or View copy.

## Construction and completeness

`CheckedMatch::try_from_hir` reads the accepted HIR Match and fails unless:

1. owner is a live Match in the accepted HIR snapshot;
2. the scrutinee and every arm field are read from that exact HIR row;
3. arm ordinals are exactly `0..len` and fit `u32`;
4. every referenced expression, pattern, scope, and local belongs to the same HIR/semantic generation;
5. every referenced expression/pattern/binding fact exists;
6. guard type is exactly Bool;
7. every arm value type agrees with the enclosing Match result type under ordinary sema rules;
8. local output ordinals are exactly HIR `locals` order;
9. ownership classification is total for every local; and
10. the sole coverage result agrees with accepted pattern/case reachability.

Final semantic publication then performs a bidirectional completeness check: every live HIR Match has exactly one Match resolution, and every Match resolution points to a live equal HIR Match. `Structural`, missing, duplicate, stale, or mismatched Match facts abort publication.

## Digest without copied facts

`FinalSemanticAnalysis::checked_match_digest` hashes exact HIR coordinates and reads normalized type digests from the same final-analysis maps at digest time:

- Match result type from owner expression;
- scrutinee/guard/value types from child expressions;
- pattern types from checked patterns;
- local types from checked bindings; and
- ownership/coverage from `CheckedMatch`.

An absent or stale referenced fact is a digest error, not a default. This commits to the complete checked meaning without duplicating authority inside `CheckedMatch`.

## Ownership projection

`RegisteredSemanticWorld::checked_ownership(&TypeKind)` is the sole inherent classifier. It recursively returns `Copy`, `SnapshotClone`, or `Rejected(reason)` for Borrowed, Unique, Affine, MustDrop, FrameLocal, NonCloneable, or NonSnapshot.

Ordinary Match accepts all dispositions. View selector publication accepts only `Copy` and `SnapshotClone`; both are normalized to bundle disposition `SnapshotClone` because the driver installs an owning snapshot-clone value. Generic language semantics are not narrowed by View persistence.

Result, Option, tuple, closed variant, record, sequence, and opaque values are admitted recursively through existing type/value-class/persistence owners. A typed Need handle is snapshot-clone only when its verified constructor arguments are recursively snapshot-clone.

## Checked View reference and source roles

The checked View catalog stores only `CheckedMatchRef { expression, semantic_digest }`. It resolves that reference against the same `FinalSemanticAnalysis`. Arms/bindings/coverage/types/effects are never copied into the catalog.

Source spans remain owned by the HIR source index. The compiler projects only typed generated-code roles—Match site, arm, binding output, and producer—into `ViewReactiveSourceMapEntryV1`. Source positions never become semantic/runtime identity.

## One-way runtime-plan projection

The compiler converts the resolved fact into `RuntimeViewMatchSelectorSeed` within the same atomic runtime semantic-fact projection. The seed contains exact IDs, semantic type identities, and checked-match digest but no coverage/effects/ownership/source spans or copied type graph. Runtime-plan finalization rewrites type identities through the one existing `RuntimePlan` type table.

This row is codegen input, not a second checked authority. `arcweft-runtime-plan` has no sema dependency and never receives `CheckedMatch` directly. A seed missing a same-batch type/fact, with non-dense order, or with a stale digest is rejected before `RuntimePlan` publication.
