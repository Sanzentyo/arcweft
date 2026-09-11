# Data reflection schema projection owner — 2026-09-11

Base: `ad67cf150d4a6bae50ec7062e4c3827fe4de80a0` on the existing `main`
checkout, equal to `origin/main` when inspected. The callable/effect and nominal
Variant migrations are still dirty. This independent relocation does not
accept those migrations or claim a buildable complete working tree.

## Result and ownership

Core now owns `From<&arcweft_data::TypeShape> for RuntimeTypeSchema` and the
corresponding field, case, byte-format, tag-style and integer-repr conversions.
The sole sema caller uses that conversion. Its old exported free function and
private format/repr helpers are deleted. Core's existing data dev-dependency
becomes a normal dependency; no feature, version, codec tag or lockfile changes.

Data owns format-neutral reflection declarations. Core owns runtime schemas
and their canonical writer. The core-to-data edge lets semantic producers and
lower runtime adapters share the conversion without an adapter-to-sema edge
or a second mapping. The maintained crate map records this ownership.

The projection preserves the existing algebra and all reflection metadata:
scalar widths, byte formatting, nested containers, declaration/wire names,
default/skip flags, record policy, enum tagging and discriminants. `Named`
remains a reference. Conversion neither invents a nominal/semantic identity
nor admits unresolved definitions as a nominal graph. The data decoder's exact
identity, schema-bound value admission and layout publication still belong to
the active [producer closure request](../reviews/requests/2026-09-11-lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1.1-nominal-variant-layout-producer-and-schema-closure.md).

This cut is the complete relocation of an existing, context-free authority. It
does not provide a temporary layout or partial Variant constructor. Only its
module declaration is staged from the larger dirty `entry/schema.rs`; the
other nominal/validator changes remain outside this commit.

## Validation

Logs use the `data-schema-projection-*` prefix under the ignored
`.arcweft-local/validation/2026-09-11-effect-row-formulas/` directory. All Rust
commands use the existing checkout, so these are working-tree results, not an
isolated execution of the staged tree.

- Passed: focused core projection tests, two tests. The final tests verify
  reflected record validation, exact integer width, sequences, schema serde
  round trip, recursive name retention, and layout sensitivity to enum and
  wire metadata. The final focused run followed a test-only revision that
  removes reliance on the separate pending Option-validator change.
- Passed: `cargo test -p arcweft-core --all-features --lib --tests`, 446 tests
  (413 library and 33 integration), before that final test-only revision.
  No core production code changed between this run and the final focused pass.
- Passed with warnings: `cargo clippy -p arcweft-core --all-targets
  --all-features`. This is not a warning-free result.
- Passed: `cargo test -p arcweft-core --all-features --doc`, zero doctests.
- Passed: formatting of changed Rust, `git diff --check`, staged diff check,
  and review of explicit staged paths/hunks.
- Passed: `just structure-audit` and `just structure-audit-gate`: 95 packages,
  2,297 Rust files, 1,273,892 physical Rust lines, 309 review triggers, zero
  blocking violations. Cargo metadata confirms the dependency edge.
- Failed: sema all-target/all-feature check stops at the already incomplete
  Dialogue `RuntimeVariantIdentity::Nominal` layout initializer.
- Failed: workspace all-target/all-feature check and Clippy, `just
  test-workspace`, and `just test-doc` stop at missing production layouts in
  Dialogue `character_dialogue/schema.rs:83` and runtime-accelerator
  `external.rs:543`. The sema/compiler changes behind these dependencies are
  not claimed to have compiled. Workspace tests did not reach execution.
- Not run: Tier 2. Its runtime prerequisites are currently uncompilable; the
  existing missing-layout diagnostics already identify the prerequisite
  failure. No native, AWBC or restore producer-closure pass is claimed.

The independent relocation is committed with those broader failures
explicitly unresolved. It preserves every current transformation and consumer,
while the graph/Variant cut remains uncommitted pending complete migration.
The convergence goal remains active.

## Structure review

Current complete file measurements, including preserved work elsewhere:

| Owner | Physical lines | Bytes | Responsibility and disposition |
|---|---:|---:|---|
| core `entry/schema/data.rs` | 138 | 4,905 | Context-free reflection projection; one exhaustive mapping, no state or I/O |
| core `entry/schema/data/tests.rs` | 151 | 4,984 | Behavior and canonical-metadata regression tests for that projection |
| core `entry/schema.rs` | 1,164 | 37,665 | Existing schema integration; this cut adds only the private module declaration |
| sema `final_analysis/nominal_schema.rs` | 2,505 | 102,089 | Accepted project nominal projection and its checked analysis context; removes the independent reflection mapping |
| sema `final_analysis.rs` | 216 | 11,835 | Narrow final-analysis exports; removes the obsolete helper export |

The remaining large sema owner retains source/semantic nominal projection and
its shared admission context. That cohesive operation is not split at a numeric
threshold; the independent data mapping is now in its actual lower owner.
Metadata reports normal dependency/consumer counts of core 11/29, data 1/18
and sema 16/8 (declared normal dependencies, including optional dependencies).
The new edge does not introduce I/O or a higher semantic dependency into core.

No design deviation or compatibility reader is introduced by this relocation.
