# Primary request copy

Repository path:
`docs/reviews/requests/2026-07-28-seq-proof-01.1.1.4.1.1.1.1.2-call-recovered-argument-schema-correction.md`

Repository blob SHA:
`162a83984b27b8458e3380a15c17642111b080cc`

## Scope

Correct E12 `Call` and C01-C03 argument contradictions while preserving all
accepted non-E12 expression schemas and matrices.

The provisional payload had no representation for recovered names/values,
ordering poison, unresolved dot evidence, signature-focus punctuation, or
explicit call type applications. The correction must close those result-changing
gaps without fabricating names/IDs, dropping arguments, reparsing source, or
creating parallel HIR/readers.

## Required cases

- missing callee;
- missing positional value;
- missing/invalid named name;
- missing named value;
- missing spread value;
- missing/invalid associated member;
- duplicate named argument;
- positional after named;
- spread not last;
- dot value-first/nominal-second classification;
- explicit `::` nominal-only classification;
- explicit call type applications including `collect<Vec<T>>()`, `foo::<T>()`,
  and member forms, distinct from associated-receiver generic arguments;
- project-aware bare generic arity failure;
- source ownership for callee/associated parts, argument whole/name/value,
  punctuation, recovery insertions, and cursor boundaries;
- shared-resolver facts and signature projection retaining recovered form/name;
- exact limits, rollback, retry, and deletion-driven migration.

Missing values use real root-owned `RecoveryOperand` children. Missing/invalid
names are typed non-ID states. Ordered explicit type arguments use qualified
`TypeId`; present-invalid arguments retain poisoned IDs and only missing slots
lack an ID.

Tests traverse:
`ParsedSource -> attached semantic node -> staged final HIR transaction ->
shared resolver/source query -> commit/rollback`.

No hand-constructed-only proof, alias, wrapper, extension trait, compatibility
shim, dual reader, source reparse, source gate, CSS/Takumi path, old syntax
diagnostic, static Capacity-only dispatcher, detached old HIR, or obsolete
Dialogue repair is authorized.
