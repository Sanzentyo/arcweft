# Unified text visual parity structural audit — 2026-07-12

Revision measured: Jujutsu change `wyslyrwowkrnunwmlxytxuzlltskxzvm`.

Canonical command:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-visual-parity-2026-07-12
```

The audit scanned 2,643 files, including 1,256 Rust files and 622,283 physical
Rust LOC. It reports 0 errors and 131 warnings. This cut changes no Cargo
manifest or dependency edge. The warnings are existing size, facade,
embedded-test, and tracked architecture hotspots; the changed files do not add
an error-level hotspot.

## Changed Rust files

| Path | Crate/owner | Bytes | Physical LOC | Classification | Major responsibility |
| --- | --- | ---: | ---: | --- | --- |
| `crates/arcweft-cli/src/app/bundle/tests.rs` | arcweft-cli | 67,040 | 2,173 | unit tests | bundle assembly and source-defined Fx resolution tests |
| `crates/arcweft-cli/src/app/bundle.rs` | arcweft-cli | 82,470 | 2,304 | production | bundle orchestration and resource inventories |
| `crates/arcweft-cli/src/app/project.rs` | arcweft-cli | 29,031 | 853 | production | source selection, package identity, and runtime options |
| `crates/arcweft-cli/src/app/project_commands.rs` | arcweft-cli | 82,507 | 2,296 | production | project command orchestration and cached compilation |
| `crates/arcweft-cli/src/app/runtime/cli.rs` | arcweft-cli | 3,669 | 101 | production | CLI launch-profile execution |
| `crates/arcweft-cli/src/app/runtime/plan.rs` | arcweft-cli | 1,732 | 48 | production | runtime-plan command |
| `crates/arcweft-cli/src/app/runtime/profile.rs` | arcweft-cli | 14,964 | 399 | production | profiled compilation pipeline |
| `crates/arcweft-cli/src/app/runtime/run.rs` | arcweft-cli | 48,622 | 1,331 | production | runtime run orchestration |
| `crates/arcweft-cli/src/app/runtime/script_test.rs` | arcweft-cli | 6,812 | 209 | production command module | authored script-test execution; the filename heuristic classifies this as test in `file_metrics.csv` |
| `crates/arcweft-cli/src/app/runtime/serve.rs` | arcweft-cli | 6,391 | 195 | production | server runtime launch |
| `crates/arcweft-cli/src/app/verify.rs` | arcweft-cli | 17,756 | 505 | production | verification command lowering |
| `crates/arcweft-compiler/src/project.rs` | arcweft-compiler | 21,081 | 693 | production | linked project compilation and manifest package ownership |
| `crates/arcweft-runtime-plan/src/flow.rs` | arcweft-runtime-plan | 69,909 | 1,903 | production | checked runtime-flow/display lowering options and orchestration |
| `crates/arcweft-runtime-plan/src/render_text/fx/tests.rs` | arcweft-runtime-plan | 4,467 | 150 | unit tests | typed RichText Fx binding |
| `crates/arcweft-runtime-plan/src/render_text/fx.rs` | arcweft-runtime-plan | 5,832 | 171 | production | package-scoped Fx catalog and application binding |
| `crates/arcweft-text-layout/src/document_layout.rs` | arcweft-text-layout | 33,557 | 952 | production | shaped document placement, vertical geometry, and raster origins |
| `crates/arcweft-text-layout/tests/document_layout.rs` | arcweft-text-layout | 15,663 | 488 | integration tests | document layout and transformed ink/raster geometry |
| `tools/capture-text-parity-frame.rs` | repository tool | 22,009 | 623 | production cargo-script | Native/headless prepared-frame capture and scoped attachments |
| `tools/profile-css-style-parity-startup.rs` | repository tool | 6,595 | 230 | production cargo-script | CSS parity startup profiling |
| `tools/run-text-parity-gates.rs` | repository tool | 8,690 | 265 | production cargo-script | generic text-raster/full-frame/IMQ gate orchestration |
| `tools/verify-unified-text-visual-parity.rs` | repository tool | 18,871 | 556 | production cargo-script | semantic and attachment verification for the unified packet |

The changed warning-level production hotspots (`bundle.rs`,
`project_commands.rs`, `runtime/run.rs`, and `flow.rs`) already own their named
orchestration domains. This cut adds only package-identity propagation or a
fallible call-site projection to them. New visual logic is isolated in the
556–623 LOC cargo-script responsibilities, while the 171 LOC Fx catalog and
952 LOC layout module remain below their respective review thresholds.

## Largest current workspace Rust files

The largest test files are `cli_runtime_bench.rs` (255,668 bytes / 7,948 LOC),
`native_vertical.rs` (238,245 / 6,613),
`published_jlreq_class_mix.rs` (222,475 / 6,161), and
`native_samples_effects.rs` (214,626 / 5,850). They are existing integration
matrices and were not changed by this cut.

The largest production files are:

| Path | Bytes | Physical LOC | Embedded tests | Major responsibility |
| --- | ---: | ---: | --- | --- |
| `crates/arcweft-core/src/value.rs` | 84,017 | 2,500 | no | core runtime values |
| `crates/arcweft-core/src/engine/eval/calls.rs` | 89,488 | 2,481 | no | engine call evaluation |
| `crates/arcweft-lang-sema/src/checker/expr.rs` | 94,248 | 2,469 | no | expression type checking |
| `crates/arcweft-cli/src/toolchain_profile.rs` | 75,712 | 2,463 | yes | toolchain profile commands |
| `crates/arcweft-lang-sema/src/checker.rs` | 85,502 | 2,456 | no | semantic checker orchestration |
| `crates/arcweft-core/src/awbc/product_step.rs` | 93,512 | 2,430 | no | Product AWBC execution |
| `crates/arcweft-runtime-driver/src/session.rs` | 93,063 | 2,408 | no | bundle session orchestration |
| `crates/arcweft-bundle/src/container.rs` | 78,267 | 2,389 | yes | AWFB container codec |
| `crates/arcweft-cli/src/app/debug.rs` | 77,792 | 2,376 | yes | debug commands |
| `crates/arcweft-runtime-plan/src/expr.rs` | 83,504 | 2,363 | no | runtime expression lowering |

## Dependency fan-in and fan-out

Unique normal/dev/build dependency edges in the generated structured graph:

| Crate | Fan-in | Fan-out | Change in this cut |
| --- | ---: | ---: | --- |
| `arcweft-runtime-plan` | 7 | 9 | none |
| `arcweft-compiler` | 4 | 13 | none |
| `arcweft-cli` | 0 | 65 | none |
| `arcweft-text-layout` | 5 | 7 | none |

The public `RuntimePlanLowerOptions` contract gains package identity, but it
stays in the existing `runtime-plan -> compiler -> CLI` dependency direction.
Project compilation owns the manifest package decision; CLI `SourceSelection`
owns direct/profile selection identity; RichText Fx catalog construction only
consumes the selected value. No reverse edge or renderer/compiler coupling was
introduced.
