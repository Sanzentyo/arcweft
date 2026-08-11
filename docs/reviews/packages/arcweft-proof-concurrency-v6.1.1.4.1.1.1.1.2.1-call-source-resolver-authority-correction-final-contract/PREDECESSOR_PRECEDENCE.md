# Predecessor precedence

Precedence is result-based, not filename-based.

1. Current repository source at the audited main controls existing production
   types, syntax, limits, work accounting, and consumers.
2. The v6.1.1.4.1.1 source-owner package controls database-qualified IDs,
   `HirSourceIndex`, `HirSourceQuery`, source-site validation, source presence,
   liveness, and single-reader policy.
3. The v6.1.1.4.1.1.1.1 tail/generator package controls `SyntheticRole`,
   `RecoveryOperand`, owner-kind admission, ordinal admission, generator order,
   liveness payloads, and atomic rollback.
4. The v6.1.1.4.1 leaf package controls the closed final-HIR expression family,
   known-family poison, Call semantic child order, value-first/nominal-second
   direction, Dialogue/RichText context limits, and deletion-driven switch,
   except where the later correction requests identify a concrete contradiction.
5. AW-AH-009.3.3.4 controls associated receiver classification, explicit `::`,
   bare generic arity, capacity precedence, zero-resolver terminal failures, and
   deletion of the string Capacity helper.
6. AW-AH-009.3.3.3.1 controls physical-versus-retained overload accounting,
   subject to AW-AH-009.3.3.4 precedence for bare `Vec`.
7. Current `ArgumentListSyntax::active_argument_slot`, `CallTargetFacts`,
   `CallableLimits`, and `ResolverWork` control production cursor/fact/limit/work
   behavior where the older Proof package used a conflicting provisional sketch.
8. This package controls only the corrected E12/C01-C03 final shape and migration.

No lower-precedence row may restore an obsolete reader, replace the shared
resolver, change current grammar, duplicate an index/limit, or lower a known
Call to generic Error.
