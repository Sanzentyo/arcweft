# Arcw Project Architecture Implementation

Date: 2026-06-23

Source package: `D:/sanze/Downloads/arcweft-arcw-project-architecture.zip`

Baseline requested by package: `59979465caaa24c816a2c2470c305dee58b5bb3e`

Implementation revision during audit:

- Jujutsu change: `yqxuvnvwuvpsutszouuqpqwqruuxzpwq`
- Current draft commit during audit: `914a7bbe2a44e4b906b19adc6dcee5beafde6aa6`

## Acceptance Criteria

- `arcw.toml` is a Cargo-like project manifest with package metadata and build roots.
- Project source loading is recursive under the configured source root.
- File paths map to typed Arcweft module paths; non-root modules must declare the matching `mod` path.
- `use` declarations become typed module graph edges for name introduction only;
  eager/lazy body demand is compiler query state, not import syntax.
- Module graph compile units are deterministic and SCC-based.
- HIR has a module-preserving project container and a linked transitional view for existing semantic/runtime passes.
- `arcw check` is package-oriented.
- `arcw compile FILE` is a direct rustc-like single-source route.
- `arcw build` writes deterministic project metadata and textual runtime plan artifacts.
- `arcw run` discovers `arcw.toml` upward, falls back to the project root when no launch profile exists, and uses project compilation for that route.
- Native adapter conformance fails before bundle/task execution when a required host call has no registered native implementation.

## Implementation Notes

- Added Sans I/O crate `arcweft-project` for manifests, source inventories, module graph edges, and compile units.
- Extended `arcweft-project-loader` with filesystem project discovery/loading while keeping the data model in `arcweft-project`.
- Added `arcweft-lang-syntax::ast::module_path` and kept import syntax as
  ordinary name-introduction without `UseMode` / `UseDependencyMode`.
- Added `arcweft-lang-hir::project::HirProject`.
- Added `arcweft-compiler::project` for split parse/lint/HIR project compilation and in-process unit cache keys.
- Added `arcw check`, `arcw build`, and `arcw compile` command paths in `arcweft-cli`.
- Updated runtime/profile compilation so manifest-discovered project runs use the project compiler and linked HIR/runtime plan.
- Kept `native-player` enabled by default through the existing `arcweft-cli` default feature set.
- Added host adapter preflight as inherent `HostCallPolicy` behavior instead of a local helper or extension trait.

## Smoke Projects

Temporary smoke projects were created under ignored `target/codex/` paths and are not repository fixtures:

- `target/codex/arcw-project-smoke`
- `target/codex/arcw-project-multimodule`

Validated commands:

```bash
cargo run -p arcweft-cli -- check --manifest-path target/codex/arcw-project-smoke/arcw.toml
cargo run -p arcweft-cli -- compile target/codex/arcw-project-smoke/src/main.arcw --emit plan -o target/codex/arcw-project-smoke/target/main.plan
cargo run -p arcweft-cli -- build --manifest-path target/codex/arcw-project-smoke/arcw.toml
target/debug/arcw run --runner headless --mode drain --steps 4
cargo run -p arcweft-cli -- check --manifest-path target/codex/arcw-project-multimodule/arcw.toml
cargo run -p arcweft-cli -- build --manifest-path target/codex/arcw-project-multimodule/arcw.toml
target/debug/arcw run --runner headless --mode drain --steps 4 --flow flow.main
```

The multimodule smoke project confirmed 2 loaded modules and 2 compile units.

## Validation

Passed:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-project -p arcweft-project-loader -p arcweft-lang-hir -p arcweft-compiler -p arcweft-host-adapter -p arcweft-runtime-host -p arcweft-cli --lib
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

Structural audit result:

```text
files scanned: 1365
Rust files: 760
Rust physical LOC: 365983
package manifests: 87
violations: 0 error(s), 88 warning(s)
dry-run: no report files written (use --write DIR)
```

The 88 warnings are existing ownership-size warnings, not new error-level audit violations.

## Structural Measurements

Changed Rust files measured from the current checkout:

| Path | Bytes | LOC | Embedded test LOC | Role |
| --- | ---: | ---: | ---: | --- |
| `crates/arcweft-cli/src/output.rs` | 48439 | 1382 | 0 | production report types; pre-existing large file, this change removes old check report code |
| `crates/arcweft-runtime-host/src/native_task.rs` | 27848 | 805 | 98 | production native task bridge and unit tests |
| `crates/arcweft-cli/src/app/project.rs` | 24172 | 719 | 86 | production project/profile selection and checking |
| `crates/arcweft-host-adapter/src/lib.rs` | 21640 | 628 | 171 | production host adapter registry/policy and unit tests |
| `crates/arcweft-compiler/src/project.rs` | 17229 | 562 | 0 | production project compiler driver |
| `crates/arcweft-cli/src/app/project_commands.rs` | 15845 | 501 | 32 | production project CLI commands and unit tests |
| `crates/arcweft-cli/src/app/verify.rs` | 17133 | 493 | 22 | production verifier CLI |
| `crates/arcweft-project/src/graph.rs` | 14778 | 470 | 121 | production module graph and unit tests |
| `crates/arcweft-project-loader/src/project.rs` | 12600 | 384 | 27 | production project filesystem loader and unit tests |
| `crates/arcweft-lang-syntax/src/ast/module_path.rs` | 10595 | 332 | 52 | production typed module paths and unit tests |
| `crates/arcweft-cli/src/app/runtime/profile.rs` | 11627 | 309 | 0 | production runtime/profile compilation |
| `crates/arcweft-project/src/manifest.rs` | 6018 | 227 | 43 | production project manifest and unit tests |
| `crates/arcweft-runtime-host/src/bundle_runner/session.rs` | 8222 | 217 | 0 | production bundle session |
| `crates/arcweft-lang-syntax/src/ast/common.rs` | 5657 | 215 | 0 | production AST common nodes |
| `crates/arcweft-project/src/sources.rs` | 4979 | 178 | 0 | production source inventory |
| `crates/arcweft-runtime-host/tests/bundle_runner.rs` | 5777 | 164 | 0 | integration test |
| `crates/arcweft-lang-hir/src/project.rs` | 4855 | 144 | 29 | production HIR project and unit tests |
| `crates/arcweft-cli/src/app/commands.rs` | 3397 | 113 | 0 | production CLI command enum |
| `crates/arcweft-cli/src/app.rs` | 4602 | 110 | 0 | production CLI dispatch |
| `crates/arcweft-lang-hir/src/lib.rs` | 716 | 22 | 0 | facade |
| `crates/arcweft-compiler/src/lib.rs` | 274 | 17 | 3 | facade |
| `crates/arcweft-lang-syntax/src/ast.rs` | 180 | 12 | 0 | facade |
| `crates/arcweft-project-loader/src/lib.rs` | 300 | 10 | 0 | facade |
| `crates/arcweft-project/src/lib.rs` | 287 | 9 | 0 | facade |

Largest workspace Rust hotspots remain pre-existing generated/test-heavy or broad modules:

- `crates/arcweft-text-layout/src/vertical_orientation.rs`: 12400 LOC, generated lookup-style production data.
- `crates/arcweft-cli/tests/check/cli_runtime_bench.rs`: 7946 LOC, integration tests.
- `crates/arcweft-cli/tests/check/agent_observe_native/native_vertical.rs`: 6283 LOC, integration tests.
- `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_class_mix.rs`: 6162 LOC, integration tests.
- `crates/arcweft-cli/tests/check/agent_observe_native/native_samples_effects.rs`: 5652 LOC, integration tests.

## Remaining TODOs

- Persistent on-disk incremental cache format is intentionally not implemented; the new cache trait is in-process only.
- Resolver/typechecker entry points still consume the linked HIR view. `HirProject` is the preserved migration boundary for future module-aware semantic passes.
- Web/native player visual smoke for richer game bundles remains covered by existing player-specific work, not by this package cut.

## Design Deviations

No intentional deviations from the package architecture were kept. The package apply script was not used directly because it resolved patch paths incorrectly in this local shell; the equivalent patches were applied and reconciled manually.
