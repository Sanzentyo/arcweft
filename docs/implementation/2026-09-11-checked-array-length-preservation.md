# Checked Array length preservation — 2026-09-11

Base: `8ff15c2907ed83ebe7206d955651c207c1c09a9e`, on the existing `main`
checkout, equal to `origin/main`. Callable/effect and nominal producer/schema
work remains dirty and is preserved outside the staged cut.

## Behavior and ownership

Normalized Array types retain a concrete length, and core plan types retain
that length as `u64`. Checked-type projection erased it into an unconstrained Sequence.
AWBC already has an Array row with that length, yet checked-type reification
rejected the row. Sema separately checked an outer array's length; nested
arrays inside a sequence lost the constraint.

`RuntimeCheckedType::Array { item, length }` now retains the exact item
predicate and length. Its ordinary recursive value admission checks both at
every nesting level. Zero-length arrays and the full `u64` type-length domain
remain representable without allocating values of the declared length.
Lengths are compared without truncation or an integer sentinel.

All checked Array producers now preserve that node: normalized type facts,
core plan projection, sema ownership projection and AWBC reification. The
existing runtime-plan checked-type interner emits the existing AWBC Array
row. The checked semantic transcript uses tag 24, followed by the fixed-width
length and recursive item transcript. Different lengths and unconstrained
Sequence remain distinct when interning. AWBC Array tag 31 and its existing
item/length fields are unchanged; all contract versions remain 1.

The sema-only `array_length` helper, outer validation branch and private
`ArrayLengthMismatch` error are deleted. Sema uses the common checked carrier
error for an invalid array at any depth. It retains source collection-family
and recursive ownership evidence for its own classification responsibility.

The staged paths contain only this complete checked-type projection and
validation change, its consumer migrations, their regression tests and
this record. The generic builtin/Array schemas, nominal graph and mandatory
nominal Variant layouts remain in the separate coupled migration. This cut
does not claim persistent schema projection or program-bound restore closure.

## Validation

Commands ran in the existing dirty checkout; they do not constitute an
isolated execution of the staged tree. Logs share the ignored prefix
`.arcweft-local/validation/2026-09-11-effect-row-formulas/`.

- Passed: core all-target/all-feature check before the schema addition,
  `array-checked-core-check.log`.
- Passed: final core all-feature library/integration run, 482 tests
  (449 + 33), `array-final-core-tests.log`.
- Passed: both core AWBC Array tests after adding the nested-array regression,
  `array-final-awbc-focused-tests.log`. They cover actual plan projection,
  canonical AWBC encode/decode/re-encode, zero/two/maximum lengths, wrong
  element type, wrong length and nested array rejection. The nested test is
  additional to the preceding full run.
- Passed with existing warnings: final core Clippy,
  `array-final-core-clippy.log`; changed-crate formatting and diff review.
- Passed: final structural screening/gate, 95 packages, 2,306 Rust files,
  1,276,334 physical Rust lines, 311 review triggers and zero blockers,
  `array-final-structure-audit.log` and `array-final-structure-gate.log`.
- Failed: workspace check and Clippy, `array-schema-workspace-*.log`, plus
  `just test-workspace` and `just test-doc`, `array-final-test-*.log`. Each
  reaches the still-missing production nominal Variant layouts in Dialogue
  `character_dialogue/schema.rs:83` and runtime-accelerator `external.rs:543`.
- Not executed: the migrated sema ownership test and new runtime-plan
  normalized/interner tests, because those crate prerequisites fail to
  compile. Their expected behavior is not reported as a test pass.
- Not run: Tier 2 while those workspace prerequisites remain uncompilable.
  No compiler, generation, native application or restore milestone is claimed.

The broader schema development initially attempted an unavailable `Box<Schema>`
wire reader. It was corrected to use the schema reader and the shared nesting
scope before the passing core run. That failed log is retained as
`array-schema-core-check.log`.

## Structural disposition

Current complete owner measurements include preserved work outside this cut:

| Owner path under `crates/` | Physical lines | Bytes |
| --- | ---: | ---: |
| `arcweft-core/src/pattern.rs` | 2,906 | 106,096 |
| `arcweft-core/src/plan.rs` | 1,413 | 49,964 |
| `arcweft-core/src/awbc/type_projection.rs` | 531 | 21,441 |
| `arcweft-core/src/awbc/tests.rs` | 5,013 | 178,979 |
| `arcweft-core/src/awbc/tests/array.rs` | 103 | 3,906 |
| `arcweft-lang-sema/src/ownership.rs` | 2,277 | 89,373 |
| `arcweft-runtime-plan/src/semantic_facts.rs` | 10,483 | 399,091 |
| `arcweft-runtime-plan/src/semantic_facts/tests.rs` | 2,929 | 105,894 |
| `arcweft-runtime-plan/src/awbc_lower/pattern.rs` | 689 | 28,153 |
| `arcweft-runtime-plan/src/awbc_lower/pattern/tests.rs` | 24 | 880 |

`pattern.rs` contains 680 embedded test lines and sema `ownership.rs` contains
333. Paths named `tests` are test owners; the remaining paths are production.

The existing pattern owner gains one real checked type and its semantic/value
rules. Plan and AWBC owners gain projections of that same type. Sema deletes
its duplicate validation; no new state, I/O, dependency, facade or mutable
catalog is introduced. Tests follow the respective projection owners, with
the new AWBC and interning regressions in their own test modules. The large
existing owners remain review triggers; the change adds no unrelated
orchestration or transport responsibilities.

The nominal producer/schema request and every other convergence acceptance
criterion remain open. No design deviation is introduced by retaining the
already-declared exact array length through checked execution.
