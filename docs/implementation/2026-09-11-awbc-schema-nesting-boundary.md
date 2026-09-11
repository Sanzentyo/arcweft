# AWBC schema nesting boundary — 2026-09-11

Base: `9a7819cf75debc92b1b331d1c4b4763c17134320` on the existing `main`
checkout, equal to `origin/main` when inspected. Callable/effect and nominal
producer work remains dirty and is preserved outside this cut.

## Result

Unary runtime schemas recursively called their own AWBC reader without entering
the existing decode nesting allowance. A long Option/Seq chain could therefore
recurse past the selected bound before a collection reader checked depth.

`Reader::read_nested` now owns the enter/read/leave scope, restoring depth on
both success and ordinary failure. The existing item and string-table readers
use it, and every `RuntimeTypeSchema` read enters the same scope before reading
its tag. Root schema nodes count toward the bound. Collection scopes retain
their existing charges; consumed collection items and input bytes are not
refunded on failure.

This replaces the duplicated scope handling and closes the whole schema-reader
boundary. It does not add a Choice-only guard, another reader, a format tag,
version, manifest or dependency. The writer and existing encoded bytes are
unchanged in this cut. The caller still selects `AwbcDecodeBudget`.

The staged metadata file contains only that reader scope and the new nesting
test module. Pending graph/atom/Choice rows and validation-work limits remain
unstaged. The independent tests use the already accepted Unit/Bool/Option/Seq
schema rows, so this fix does not depend on those pending additions.

## Validation

All commands ran in the existing working tree with preserved WIP. These are
not isolated executions of the staged tree. Logs are under the ignored
`.arcweft-local/validation/2026-09-11-effect-row-formulas/` directory.

- Passed: `cargo test -p arcweft-core --all-features
  awbc::codec::metadata::nesting_tests --lib`, three tests. They reject
  20,000-level Option and Seq inputs at depth eight before constructing the
  excess nodes, exercise exact/one-over depth and byte round trip, and verify
  depth restoration after failed schema and collection reads.
- Passed: final `cargo test -p arcweft-core --all-features --lib --tests`,
  467 tests (434 library and 33 integration), log
  `awbc-schema-nesting-core-tests.log`.
- Passed with warnings: core all-target/all-feature Clippy, log
  `schema-choice-final-core-clippy.log`; the subsequent workspace Clippy also
  compiled the three new core tests before the downstream failure.
- Passed: formatter, ordinary and staged diff checks, and review of the three
  explicit Rust paths/hunks.
- Passed: `just structure-audit` and `just structure-audit-gate`: 95 packages,
  2,301 Rust files, 1,275,240 physical Rust lines, 311 review triggers, zero
  blocking violations. The totals include unrelated preserved WIP.
- Failed: workspace all-target/all-feature check, workspace Clippy and
  `just test-workspace`. They stop at the unresolved production nominal
  Variant layout initializers in Dialogue `character_dialogue/schema.rs:83`
  and runtime-accelerator `external.rs:543`. Workspace tests do not reach
  execution. These remain real failures of the complete working tree.
- Not run: doctests for this private-reader cut; it changes no Rustdoc examples
  or public API declarations. Tier 2 remains unrun while its workspace runtime
  prerequisites fail to compile. No execution/restore or full C1 success is
  claimed from this codec fix.

## Ownership and remaining work

Complete current owner measurements are metadata codec 1,524 lines/55,881
bytes, wire reader/writer 631/18,491, and nesting tests 84/2,508. The existing
large metadata owner retains one wire projection responsibility. This cut adds
one shared reader scope and keeps its tests in a dedicated responsibility
module. It adds no I/O, generation state or parallel schema catalog.

The nominal graph/Variant/Choice migration and all other convergence acceptance
remain open. Their broader validation failures are not accepted as passes, and
they are not included in this commit. No design deviation is introduced by
enforcing the existing decode budget on every recursive schema read.
