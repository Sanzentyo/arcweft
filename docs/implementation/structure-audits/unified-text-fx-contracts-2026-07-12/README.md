# Unified text/Fx contract structural audit — 2026-07-12

Audit target: Jujutsu change `rrnvupruqzsu` over committed baseline
`3d3167ba` (`Finalize unified text, TextBox, View, and Fx design`). The Jujutsu
change ID is stable across final description/bookmark updates.

Canonical command:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-fx-contracts-2026-07-12
```

Result: 1,231 Rust files / 615,563 physical Rust LOC, 0 errors, 144 warnings.
The CSV files in this directory are the exact current-checkout measurements;
they exclude target output and VCS internals.

## Changed boundary measurements

| Path | Owner | Bytes | Physical LOC | Role / major responsibility |
| --- | --- | ---: | ---: | --- |
| `crates/arcweft-presentation/src/fx.rs` | `arcweft-presentation` | 2,478 | 60 | Intentional Fx facade |
| `crates/arcweft-presentation/src/fx/graph.rs` | `arcweft-presentation` | 34,415 | 1,075 | Typed definition, graph, ABI and semantic hashing |
| `crates/arcweft-presentation/src/fx/evaluator.rs` | `arcweft-presentation` | 30,824 | 908 | Shared Sans I/O sampler/graph evaluator |
| `crates/arcweft-runtime-plan/src/fx.rs` | `arcweft-runtime-plan` | 20,572 | 581 | HIR-to-Fx graph lowering orchestration |
| `crates/arcweft-bundle/src/fx_definitions.rs` | `arcweft-bundle` | 9,590 | 260 | Bounded deterministic FxDefinitions section codec |
| `crates/arcweft-runtime-driver/src/fx_runtime.rs` | `arcweft-runtime-driver` | 18,785 | 525 | Logical clock, retained instances, restore validation |
| `crates/arcweft-runtime-driver/src/session/fx.rs` | `arcweft-runtime-driver` | 2,226 | 65 | Session-facing Fx lifecycle/observation projection |
| `crates/arcweft-lang-hir/src/symbol.rs` | `arcweft-lang-hir` | 31,321 | 911 | Ordinary/Fx callable ownership and import resolution |
| `crates/arcweft-render-text/src/lib.rs` | `arcweft-render-text` | 2,063 | 47 | Intentional resolved-text facade |
| `crates/arcweft-text-layout/src/lib.rs` | `arcweft-text-layout` | 1,427 | 47 | Intentional layout facade |
| `crates/arcweft-text-layout/src/document_layout.rs` | `arcweft-text-layout` | 22,441 | 617 | Preliminary shaped-document layout consumer |

All newly added ordinary responsibility modules remain below the 1,200-LOC
warning threshold. Fx unit tests are isolated in
`crates/arcweft-presentation/src/fx/tests.rs`; the runtime save/load behavior is
covered by integration tests rather than embedded into the oversized session
or display modules.

The audit still warns about the pre-existing `arcweft-runtime-driver` session
(2,132 LOC) and display (1,533 LOC) orchestrators and the pre-existing bundle
root/container files (2,017/2,389 LOC). This cut did not place the Fx lifecycle
inside `session.rs`: it uses the 525-LOC runtime state module plus a 65-LOC
session extension. Later renderer/TextBox cuts must continue decomposing the
remaining warned orchestrators rather than adding new responsibilities there.

## Dependency fan-in / fan-out

Counts below are unique workspace package edges from `dependency_edges.csv`:

| Package | Fan-in | Fan-out |
| --- | ---: | ---: |
| `arcweft-presentation` | 15 | 6 |
| `arcweft-runtime-plan` | 7 | 9 |
| `arcweft-bundle` | 10 | 22 |
| `arcweft-runtime-driver` | 6 | 13 |
| `arcweft-render-text` | 16 | 4 |
| `arcweft-text-layout` | 3 | 7 |
| `arcweft-lang-hir` | 9 | 3 |

The dependency direction remains `syntax -> HIR -> runtime-plan -> bundle /
runtime-driver`, while the renderer-independent Fx contract remains in
`arcweft-presentation`. No dependency from `arcweft-core` to presentation,
View, text layout, bundle, GPU, or platform I/O was introduced.
