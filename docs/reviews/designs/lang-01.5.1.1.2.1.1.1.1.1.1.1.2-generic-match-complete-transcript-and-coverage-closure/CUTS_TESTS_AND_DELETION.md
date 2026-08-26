# Compile-clean cuts, tests, and deletion

## Cut order

Each cut must compile and pass its scoped tests before the next begins. A
replacement constructor and all of its consumers land in the same cut; no
temporary source resolver, permissive `Other`, dual reader, or parallel model
may bridge cuts.

### C1 — HIR topology and declaration bridge

1. Add closed HIR roles for `ViewValue { ordinal }` and the 19
   expression-owned non-expression root families in `source_inventory.json`.
2. Extend the existing semantic path builder to traverse View parameters/
   source-ordered values and Await/Choice/dialogue statement/pattern bodies.
3. Reuse the existing nested path segments and statement/body child roles;
   append only genuinely new roles.
4. Delete the `ViewItem => MissingBody` branch. Retain `MissingBody` for extern
   capability and trait requirement.
5. Prove one accepted-rooted path per live child, deterministic source
   order, cycle/duplicate/foreign-snapshot rejection, and checked ordinal
   conversion.

No sema type moves into HIR.

### C2 — construct exact checked owners

1. Add opaque same-cut IDs/rows from `SCHEMAS.md` in their legitimate sema or
   lower owner.
2. Change final checker constructors to retain exact project-item, entry,
   variant case, record field, method/field, StageLook, View modifier,
   typed-binding, and rich-text meaning.
3. Make Character/Builtin/project case vectors typed source-order rows and make
   selected variant resolution retain the selected row.
4. Join project field rows to the one canonical runtime nominal projection;
   add environment-field rows at environment record construction.
5. Add owner-defined exhaustive tags/digests for Effect, Agent/Progress field,
   View/Style closed types without adding reverse dependencies.

After all C2 consumers compile, delete:

- string case vectors and selected case-name authority;
- transcript-facing project item/entry raw owner/public spelling authority;
- `CheckedSelectResolution::Method`/`DialogueView`/`Field` name-only fields;
- unproduced `CheckedSelectResolution::RecordElement` and constructor/readers;
- `StageLook(HirName)` open fallback;
- unchecked View modifier names; and
- record-pattern transcript name lookup.

Diagnostic spellings may remain in explicitly named diagnostic-only fields.

### C3 — complete semantic transcript

1. Replace `TranscriptHasher` with fallible checked-`u64` writes.
2. Add exhaustive expression-shape atoms and exact payload writers for all
   27/8/7 resolution families.
3. Add checked pattern-field/case/entity/typed-binding atoms.
4. Add private statement/body/rich-text digests over C1/C2 facts.
5. Compute nested Match payloads bottom-up and include pattern/coverage meaning
   in the Match expression digest.
6. Keep `CheckedMatchRef` private and non-Serde.

Then delete `UnsupportedIdentity`, every typed unsupported success branch,
source-spelling writes (including Effect display text), saturating accounting,
lossy/sentinel length conversions, and infallible transcript conversions.

### C4 — one complete private coverage engine

1. Add the private Matrix/PatternVector/deconstruction/domain/constructor/
   sequence-partition/witness types.
2. Implement checked domain construction, specialization, default,
   usefulness, source-ordered Or diagnostics, canonical witness
   reconstruction, and the all-`u64` transaction.
3. Extend bracket-pattern seeding to `TypeKind::Seq`.
4. Atomically return hard non-exhaustiveness with a structured witness.

After differential and domain tests pass, delete current `CoverageAtom`,
`CoverageShape`, `CoverageDomain`, `basic_coverage`, scalar coverage witnesses,
`UnsupportedCoverage`, `max_coverage_states`, and all old readers/helpers.

### C5 — publication and exposed consumers

1. Make the final analysis query publish only a fully exhaustive, completely
   transcribed `CheckedMatch`.
2. Update sema exports/reports/tests and later compiler-facing internal APIs to
   the one final schema.
3. Run repository structural checks proving no deleted type/variant/function or
   source/raw fallback remains.
4. Do not create runtime/wire/persistence/task-plan consumers.

## Positive test matrix

| ID | Required executable evidence |
|---|---|
| `T01_ALL_RESOLUTIONS` | one exhaustive compile-time/behavior fixture per 27 expression, 8 value, 6 surviving select, 13 pattern, and 35 statement families; adding an enum variant makes the writer fail to compile |
| `T02_ENTITY_CASE_IDENTITY` | project Entity, Character case, builtin case, project/Option/Result cases: equal names/different accepted IDs differ; changed diagnostic spelling/equal ID stays equal; payload type and case order differ |
| `T03_RECORD_FIELD_JOIN` | source-to-declaration ordinal mapping, project runtime field ID, nominal semantic ID, layout and field type sensitivity; authored name/format perturbation with equal checked row is invariant |
| `T04_MATRIX_DIFFERENTIAL` | generated small finite Bool/Option/Result/enum/Choice/product/array/sequence matrices agree with an independent enumerating oracle for usefulness, redundancy, exhaustiveness and witness membership |
| `T05_WITNESS_AND_LIMITS` | nested tuple/record/array/sequence witnesses; literal/entity/open `Other`; Never; deterministic witness ordering; exact-limit success and one-over atomic error for all 11 counters |
| `T06_ALL_BODY_ROOTS` | function/predicate/proof/flow/trait impl/inherent/View parameter/default/body/value paths and every Await/Choice/dialogue root; View no longer `MissingBody`; body reorder/sensitivity and raw-ID/span perturbation invariance |
| `T07_DELETION_CLEAN` | compile and structural checks prove old coverage, unsupported branches, source resolution helpers, `RecordElement`, mixed counters, legacy/version aliases, persistence/task-plan types absent |

## Detailed semantic differentials

- HIR arena compaction, snapshot regeneration, raw Expr/Pattern/Stmt/Item/Local
  IDs, spans, comments, whitespace, literal radix, and equivalent formatting do
  not change a digest when checked meaning/path is equal.
- Declaration path, callable contract/join, nominal semantic identity/layout,
  field/case ID, field/case order, payload type, operator, effects, accepted
  identity, arm order, Or order, child order, body order, guard class, or result
  expression meaning changes the appropriate digest.
- Integer radix equivalence, duration-unit normalization, f32/f64 exact bits,
  canonical text/Ruby/tag payload, entity identity, line/text-key identity, and
  Style/View closed variants each receive equality/difference pairs.
- Nested Match pattern/guard/coverage changes affect its enclosing expression
  and outer Match digest.
- False guard dominates; dynamic guards are usefulness-tested but do not cover;
  earlier useful Or alternatives make later overlaps unreachable at the exact
  alternative coordinate.
- Tuple and record cartesian coverage, fixed arrays, symbolic exact/rest
  Vec/Slice/Seq partitions, closed enums, Choice, Never, and open residuals
  cover empty/singleton/overlap/exhaustive/non-exhaustive cases.
- Sequence tests include zero, adjacent cut points, bounded gaps, unbounded
  tail, huge minima without huge allocation, and nested product elements.

## Limit matrix

For each limit below, one fixture reaches exactly the limit and succeeds, one
attempts limit+1 and returns its exact typed error before work, and one repeats
the failure to prove determinism/no publication:

```text
max_arms
max_matrix_rows
max_or_alternatives
max_pattern_nodes
max_expression_nodes
max_depth
max_sequence_partitions
max_specializations
max_unreachable_rows
max_witness_nodes
max_transcript_bytes
```

Also test `u64` addition/multiplication/conversion overflow without allocating.
No test configures an explicit Cargo job count.

## Compile/structure negatives

Compile-fail tests must reject constructing private `CheckedMatchRef` fields,
serializing `CheckedMatchRef`, forging opaque semantic IDs, constructing a
variant selection with a case row not owned at its ordinal, and constructing a
record pattern row with mismatched layout/field/type.

Repository structural checks use typed API/compilation and exact deleted
symbol inventories, not source spelling as positive acceptance. They fail if a
public/persisted Match DTO, Serde derive, task-plan seal, whole-catalog digest,
legacy/V2/version-not-1 marker, old coverage model, wildcard success match, or
parallel transcript/coverage reader is introduced.

## Validation commands for implementation

Run from repository root, without explicit Cargo job counts:

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p arcweft-lang-hir
cargo test -p arcweft-lang-sema
cargo test --workspace
```

If independent test commands are deliberately run in parallel, record that
coordination; never pass `--jobs`, `-j`, or `CARGO_BUILD_JOBS` to ordinary
commands.

## Completion rule

Implementation is complete only when C1–C5 are compile-clean, all positive/
negative/differential/limit tests pass, every deletion target is absent, and
no compatibility or persistence scope has appeared. A blocked tier is reported
as blocked/not-run, never as passed.
