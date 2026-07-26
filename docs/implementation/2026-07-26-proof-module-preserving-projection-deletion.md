# Proof module-preserving projection deletion

Date: 2026-07-26
Status: implementation validated; unrelated workspace baselines recorded
Jujutsu change: `xtxzozospylnqtppnyxqswvtmyrmlqmz`

## Decision

Proof-concurrency v6.1.1.4 has a repository request but no returned design ZIP.
The final semantic leaf-expression payload therefore remains design-blocked and
is not inferred in this cut.

Projection consumers that do not require a crate-global semantic merge can be
migrated independently. They now read the accepted, module-preserving
`HirProject` directly. No linked-view replacement helper, compatibility alias,
dual reader, source gate, or placeholder HIR payload was added.

## Removed flatten-only readers

- CLI project reports sum flow counts across canonical project modules instead
  of reading `CompiledProject::linked_hir()`.
- LSP dialogue cascade projection enumerates dialogues across canonical project
  modules while retaining one project-global ordinal for the runtime line
  display catalog.
- The one-module accepted-profile fixture obtains the exact crate-root HIR
  module from `CompiledProject::hir_project()` instead of cloning the linked
  view's source document.

The LSP evidence uses distinct root/child speakers and line colors. A
module-local ordinal reset selects the root runtime-catalog record and fails the
test.

## Intentionally retained boundary

`HirProject::linked_module()`, `HirModule::append_module_body()`, and
`CompiledProject::linked_hir` remain production semantic inputs for resolver,
type-checker, verifier, style/runtime-plan, and selected recovery-query paths.
Deleting them before the final expression/item payload and project-semantic
entry points exist would remove behavior or create a second adapter authority.
Their deletion remains part of the Proof v6.1.1.4 public authority switch.

The retained references in the directly inspected files are:

- CLI verification input in `project_commands.rs`;
- the recovered-signature test's single semantic-query input in
  `profiles/state/tests.rs`.

Neither is a projection-only reader and neither was repaired or wrapped here.

## Review-package inbox

At this cut, all 29 ZIP files under `docs/reviews/` have case-insensitive
SHA-256 matches in package-specific implementation notes. No Proof v6.1.1.4
return ZIP is present; only its request Markdown exists.

## Validation

Passed:

- `cargo fmt --all -- --check`
- LSP canonical project dialogue-order unit test: 1 passed
- LSP cascade test module: 4 passed
- LSP child-module global-dialogue-ordinal hover test: 1 passed
- existing LSP line-option hover test: 1 passed
- accepted-profile identical-rebuild test: 1 passed
- CLI multi-module project flow-count test: 1 passed
- CLI project-command test module: 6 passed
- `cargo check -p arcweft-cli -p arcweft-lsp --all-targets --all-features`
- `cargo clippy -p arcweft-cli -p arcweft-lsp --all-targets --all-features -- -D warnings`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- CLI library and binary tests: 198 passed
- CLI `runtime_native_options`: 3 passed
- CLI `check_core_cli`: 4 passed
- CLI `native_style_parity_sample`: 1 passed
- CLI `release_trust_json`: 5 passed
- CLI `responsive_stage_placement`: 4 passed
- CLI persistent-cache build goldens: 2 passed
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`: 3,680 files,
  1,936 Rust files, 906,210 Rust physical LOC, 94 manifests, 0 errors, 146
  repository-wide warnings

The normal workspace test command was run and its unrelated baseline failures
were preserved rather than folded into this cut:

- `just test-workspace` stopped in
  `arcweft-rust-abi-macros::compile_fail::rejects_unsupported_abi_shapes`
  when copying `reject_lifetime_generic_type.rs` returned Windows `os error 3`.
  The checked-in source fixture exists and the exact compile-fail target passes
  1/1 when rerun alone. No file in that crate is changed here.
- The CLI recipes that follow the failed workspace recipe were run separately.
  Every selected recipe passed except `arcw_fixtures_check_run`, which retained
  its existing 3-pass/2-fail baseline for `010_capability_fs_read.arcw` and
  `002_file_read_task.arcw`. The capability-owned `FsError` nominal
  publication remains blocked on the Proof public HIR switch and was already
  recorded by the preceding assertion-deletion cut.

Tier 2 was not run. This cut does not change runtime, render, Agent, MCP, or
capture behavior and does not meet the repository's Tier 2 risk condition.

## Changed-file structure snapshot

Measured from this checkout before the broad gates:

| Path | Classification | Bytes | Physical LOC | Responsibility |
|---|---:|---:|---:|---|
| `crates/arcweft-cli/src/app/project_commands.rs` | production orchestration | 82,236 | 2,276 | project command reports, build/check artifacts, verification dispatch |
| `crates/arcweft-cli/src/app/project_commands/tests.rs` | unit tests | 12,715 | 379 | project command/cache behavior |
| `crates/arcweft-lsp/src/features/cascade.rs` | production feature plus unit tests | 33,062 | 917 | accepted dialogue cascade projection and source-path selection |
| `crates/arcweft-lsp/src/profiles/state/tests.rs` | unit tests | 51,665 | 1,472 | accepted-profile publication and cache lifecycle |
| `crates/arcweft-lsp/src/session/tests.rs` | unit tests | 82,124 | 2,393 | LSP session protocol behavior |

`project_commands.rs` was already above the 1,200-LOC warning threshold. This
cut changes one report projection and adds its tests in the existing external
test module; it does not extend the file's orchestration responsibilities.
