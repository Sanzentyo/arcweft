# Proof View project projection deletion

Date: 2026-07-26

## Scope

This cut removes the flattened `HirModule` reader from compiler-owned View
validation and sidecar lowering. It is deletion-driven preparation for the
Proof-concurrency v6.1.1 project authority switch and does not depend on the
pending `01.1.1.4` semantic leaf-expression payload contract.

`ViewProjectLowerer` now retains the accepted `HirProject` and derives one View
inventory directly from its canonical module iterator. The same inventory is
used for authored-View validation and sidecar lowering. The old
`linked_hir` field, constructor parameter, and compiler call argument were
deleted; no adapter, compatibility reader, or replacement flattening helper was
introduced.

The existing Style integration fixture also stopped reading the linked module
to locate module-local View ranges. It resolves each View through its owning
module in the accepted project instead.

## Preserved invariants

- `HirProject::modules()` is backed by the canonical module-path `BTreeMap`.
  The crate root's empty path sorts first, followed by child modules in
  canonical order.
- Each `HirModule::view_declarations()` iterator retains source declaration
  order. The direct project traversal is therefore order-equivalent to the
  deleted `linked_module()` View projection.
- A single collected inventory feeds validation and lowering, so the program
  ID, definition order, and global instruction ordinals cannot observe
  different traversals.
- View source spans remain bound to the owning module's exact
  `SourceDocumentIdentity`; no range rebasing or source-text reparse is used.

The multi-module behavior test deliberately supplies project files in
`z -> root -> a` order. It proves that accepted authored definitions remain
`root source order -> a -> z`, that the program identity is derived from the
root's first View, that instruction spans advance across module boundaries,
and that root/child View spans retain their distinct source documents.

## Explicit non-goals

- `CompiledProject::linked_hir` remains while semantic resolution, type
  checking, Style/FX/runtime-plan lowering, and other production consumers
  still require the transitional linked module.
- The pending `01.1.1.4` leaf-expression schema is not inferred here.
- This cut does not change View language syntax, HIR payloads, runtime
  behavior, bundle codecs, or compatibility policy.

## Validation

Completed:

- `cargo test -p arcweft-compiler --test view_product` (`7 passed`)
- `cargo test -p arcweft-compiler --test style` (`5 passed`)
- `cargo check -p arcweft-compiler --all-targets --all-features`
- `cargo clippy -p arcweft-compiler --all-targets --all-features -- -D warnings`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`
  - 3,681 files
  - 1,936 Rust files
  - 906,387 physical Rust LOC
  - 94 package manifests
  - 0 errors and 146 ownership-review warnings
- design-package ZIP inbox hash audit
  - 29 archives inspected
  - 0 unrecorded hashes
  - 0 returned `01.1.1.4` archives

`just test-workspace` exercised the workspace suites but did not complete as a
single green recipe. Its first command retained the previously observed
Windows parallel-fixture copy race in
`arcweft-rust-abi-macros::rejects_unsupported_abi_shapes`: copying the existing
`reject_lifetime_generic_type.rs` fixture returned `os error 3`. The exact test
passed `1/1` immediately when rerun alone.

The CLI recipes skipped after that stop were run individually. CLI lib/bins
(`198 passed`), `runtime_native_options` (`3 passed`), `check_core_cli`
(`4 passed`), `native_style_parity_sample` (`1 passed`),
`release_trust_json` (`5 passed`), `responsive_stage_placement` (`4 passed`),
and the persistent-cache goldens (`2 passed`) were green. The inherited
`arcw_fixtures_check_run` gate remained `3 passed / 2 failed` for
`010_capability_fs_read.arcw` and `002_file_read_task.arcw`; both still require
the capability-owned `FsError` nominal publication already documented by the
preceding Proof cuts. Neither broad-gate failure touches the changed View
projection path.

Tier 2 is not required for this projection-only compiler change: it does not
alter runtime, render, Agent, MCP, or capture behavior.

## Design deviations and remaining work

There is no design deviation. The final public HIR/project authority switch,
assertion identity propagation, AWBC codec, and save/replay identity remain
open. The `01.1.1.4` returned ZIP is still required before implementing the
semantic leaf-expression payload boundary.
