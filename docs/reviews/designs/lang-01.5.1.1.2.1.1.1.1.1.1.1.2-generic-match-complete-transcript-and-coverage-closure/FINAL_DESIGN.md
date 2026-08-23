# Final design

## Outcome

Generic Match has one final typed authority:

```text
accepted HIR declaration/body roles
             |
             v
final sema checking + exact owner rows + checked callable/nominal joins
             |
             +--> private pattern matrix --> coverage / unreachable / witness
             |
             +--> version-1 expression/pattern/statement/body transcripts
                                      |
                                      v
                      exhaustive CheckedMatch semantic digest
```

The transcript and matrix read the same checked rows. Neither performs name
resolution, type inference, catalog traversal, layout reconstruction, or HIR-ID
serialization. Failure before either projection prevents publication.

## Authority split

HIR owns topology: exhaustive expression/pattern/statement child edges,
nested Choice/dialogue paths, declaration roots, and the new `ViewValue` plus
non-expression body roles. It does not know sema identities.

Sema owns meaning: checked types/effects, callable joins, local coordinates,
project/entry/entity facts, accepted variant cases, checked record fields,
Stage look, View/Style/dialogue facts, pattern deconstruction, coverage, and
transcript digests. Project nominal field/layout atoms join the current
canonical runtime nominal projection; they are never copied.

Owning lower crates expose only purpose-built semantic behavior needed without
reversing dependencies—for example exhaustive tags/digests for existing closed
View values. Sema already depends on HIR, core, character, and view. No lower
crate depends on sema.

## Construction transaction

For one accepted callable declaration:

1. Obtain its exact checked callable row and HIR declaration semantic path
   index. Function/predicate/proof/flow/impl bodies retain current roots; View
   values use `ViewValue { ordinal }` under the existing View declaration key.
2. Traverse typed edges and new expression-owned body edges. Verify one path
   per live expression, pattern, statement, local, and body. Missing,
   duplicated, foreign-snapshot, cyclic, or poisoned paths fail atomically.
3. During ordinary final checking, construct every same-cut exact owner row.
   Raw names/IDs may locate candidates but cannot survive as the selected
   semantic payload.
4. Deconstruct checked Match patterns into the private coverage algebra and
   run source-ordered usefulness under the shared checked-`u64` budget.
5. If non-exhaustive, return the structured witness error and publish no
   `CheckedMatch`. Preserve deterministic unreachable diagnostics in the
   analysis report transaction.
6. For an exhaustive Match, compute bottom-up pattern, statement, body,
   expression, coverage, Match-payload, and final Match digests. A nested Match
   supplies its completed payload digest to its enclosing expression.
7. Publish the `CheckedMatch` only after all counters, rows, coverage, bytes,
   and final digest succeed. Rollback is complete on any error.

## Exact-identity rule

Every transcript atom is one of:

- a closed semantic tag owned by the enum/type being encoded;
- a canonical checked literal/value byte sequence;
- an existing accepted typed identity/digest;
- a same-cut opaque semantic ID constructed from existing accepted atoms;
- a checked ordinal within an accepted row; or
- a declaration-rooted role coordinate.

Names, spans, raw arena/snapshot IDs, source files, debug/display/Serde forms,
and enumeration of a whole catalog are never atoms. If no exact row can be
constructed, the checker fails; the transcript never substitutes spelling or
`Other`. Coverage's `Other` is only a mathematical residual constructor for an
authority-declared open/infinite domain.

## Closed publication contract

`CheckedMatchRef` remains private/non-Serde and validates snapshot liveness at
query time. `CheckedMatch`, its transcript digest, coverage result, structured
unreachable coordinates, and witness error remain compiler-local sema facts.
There is no wire/persistence implementation and no runtime reader in this cut.

The later runtime-plan design may consume declaration/body paths and semantic
identity types, but cannot reinterpret this design as a task-plan seal. The
later compiler cut consumes only successfully published exhaustive Match facts.

## Result-changing commitments

- all 27 current resolution families have exact success atoms;
- the unproduced `RecordElement` select family is deleted, not blessed;
- Character/Builtin case names and record field names cease to be semantic
  authority;
- nested Await/Choice/dialogue bodies and nested Match patterns/coverage affect
  the enclosing expression digest;
- tuples, records, arrays, symbolic sequences, Or, literals/entities plus
  Other, Never, Choice, and every accepted closed variant family use one matrix
  algorithm;
- all work accounting is checked `u64`, before work, with exact-limit success
  and one-over atomic failure; and
- current basic coverage and typed unsupported branches are removed after the
  final owners become constructible.

## Compatibility

These contracts are unreleased and compiler-local. The shape is replaced in
place at version `1`. No old reader, optional legacy field, compatibility alias,
dual writer, `V2`, migration format, or version bump is allowed.
