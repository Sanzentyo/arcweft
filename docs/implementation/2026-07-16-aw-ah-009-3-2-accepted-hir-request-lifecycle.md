# AW-AH-009.3.2 accepted HIR and request lifecycle

## Status and basis

The implementation-ready cuts are implemented in Jujutsu change `slxmvsqo` on
Git base `f4982c203ee0`. The source package is
`arcweft-aw-ah-009.3.2-accepted-hir-request-lifecycle-production-reconciliation-final-contract.zip`
with SHA-256
`8701ff3ae6024cd62c33c4b36abdfa358bfa30aa93209655870c475eea1dd40d`.

Cuts 1 through 5 and the production-ready portion of cut 7 are implemented.
Cut 6 remains an explicit ordered integration boundary: it depends on the exact
authored-call carrier from AW-AH-009.3.1. This cut does not redesign or forge
that carrier. The existing signature result builder is temporarily invoked only
behind the new accepted request acquisition, worker, validation, and
publication lifecycle. No second parser, HIR, project, accepted source registry,
compatibility URI accessor, cancellation tombstone, or cache-miss lowering path
was added.

## Implemented contract

- `HirProjectModule::try_new` binds every project module to the exact retained
  `SourceDocument`; the panicking constructor is removed. An accepted snapshot
  owns one `Arc<HirProject>`, exact source registry, typed source-to-module
  reverse index, and checked footprint.
- Project loading has typed inclusive limits of 4,096 documents and 8,388,608
  aggregate UTF-8 bytes. Enumeration and reads retain maximum-plus-one evidence
  with checked arithmetic.
- LSP document, profile, overlay, and accepted-source maps use `LspUriKey`, and
  open document/overlay versions retain exact `i32` protocol versions.
- Profile rebuild uses the exact retained `LoadedProfileTopology` transaction.
  Registration borrows the one assembled HIR project, then candidate publication
  atomically binds profile key, semantic world, accepted project, exact overlay
  coverage, generation, and generation-owned caches.
- Identical source bytes with a new protocol version publish a metadata-only
  generation reusing the exact world/project `Arc`s. A changed, incomplete,
  mismatched, or concurrently replaced candidate leaves the prior accepted
  generation and caches untouched.
- `AcceptedDocumentHirLease`, `SignatureRequestStamp`, and the ordered acquisition
  path retain exact protocol document, accepted document, module, HIR project,
  world, revisions, profile state, generation, and overlay identities. Stale
  requests cannot be redirected to a newer accepted generation.
- One server-owned request registry owns cancellation controls and deadline
  tokens. Admission is capped at 32 requests, uses a fixed 250 ms deadline, one
  scheduler, a FIFO queue, and four workers. `ActiveRequest::drop` is the only
  active-map/deadline-token cleanup path; unknown cancellation IDs are not
  retained.
- Open/change/close, accepted replacement, profile remapping, workspace removal,
  and shutdown synchronously cancel the affected controls and invalidate the
  old generation before it can publish. Shutdown closes admission, drains queued
  guards outside locks, joins workers and scheduler, and checks that the registry
  is empty.
- Exact topology failures remain all-or-nothing and are projected to bounded,
  owner-relative LSP diagnostics. Character definition uses the retained
  accepted project instead of reloading or reconstructing a module path.

## Structural result

The canonical report is
[`structure-audits/aw-ah-009-3-2-accepted-hir-request-lifecycle/`](structure-audits/aw-ah-009-3-2-accepted-hir-request-lifecycle/).
It scanned 3,045 files, including 1,517 Rust files and 698,037 Rust physical LOC,
and reported zero error-level violations and 129 repository-wide warnings.

| Path | Bytes | Physical LOC | Class | Embedded test LOC |
| --- | ---: | ---: | --- | ---: |
| `crates/arcweft-lang-hir/src/project.rs` | 21,349 | 582 | production | 292 |
| `crates/arcweft-lsp/src/profiles/accepted_project.rs` | 37,684 | 1,086 | production | 228 |
| `crates/arcweft-lsp/src/profiles/state.rs` | 27,396 | 804 | production | 328 |
| `crates/arcweft-lsp/src/profiles/caches.rs` | 5,469 | 177 | production | 0 |
| `crates/arcweft-lsp/src/profiles/environment.rs` | 12,784 | 338 | production | 0 |
| `crates/arcweft-lsp/src/requests/registry.rs` | 17,182 | 527 | production | 135 |
| `crates/arcweft-lsp/src/requests/signature.rs` | 18,139 | 567 | production | 0 |
| `crates/arcweft-lsp/src/requests/executor.rs` | 7,376 | 229 | production | 0 |
| `crates/arcweft-lsp/src/session.rs` | 37,230 | 918 | production | 0 |
| `crates/arcweft-lsp/src/session/signature.rs` | 16,830 | 427 | production | 0 |
| `crates/arcweft-project-loader/src/project.rs` | 29,732 | 890 | production | 208 |

The relevant normal-dependency fan-in/fan-out values are HIR `10/3`,
project-loader `2/15`, and LSP `0/26`. No crate boundary, Cargo feature, external
dependency, unsafe block, source gate, compatibility module, or deprecated
accessor was added. The previous 1,168-LOC mixed profile cache owner was deleted;
accepted lifecycle state and semantic caches now have separate responsibility
modules.

## Validation

Rust commands used `CARGO_INCREMENTAL=0`. Full changed-crate tests passed on
ancestor `8a6d4a62a138`; after the final conflict-free rebase to
`f4982c203ee0`, LSP 126/126 and the combined changed-crate Clippy smoke passed:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-launch -p arcweft-lang-hir -p arcweft-project-loader --all-targets
cargo test -p arcweft-lsp --lib
cargo clippy -p arcweft-lsp --all-targets --all-features -- -D warnings
cargo clippy -p arcweft-lang-hir -p arcweft-launch -p arcweft-project-loader -p arcweft-compiler -p arcweft-lang-sema -p arcweft-tooling --all-targets --all-features -- -D warnings
cargo check -p arcweft-cli -p arcweft-compiler -p arcweft-lang-sema -p arcweft-tooling --no-default-features
cargo metadata --format-version 1 --all-features --no-deps
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/aw-ah-009-3-2-accepted-hir-request-lifecycle
```

The focused results were HIR 46 unit tests plus 3 project-symbol, 1 public-API,
2 style, and 2 style-environment tests; launch 14 unit plus 1 character-manifest test;
project-loader 123 unit, 3 dependency, and 6 end-to-end tests; and LSP 126 unit
tests. Clippy passed with warnings denied for every changed production crate.
Typed Cargo metadata confirmed normal fan-out of HIR `3`, project-loader `15`,
and LSP `26`; the audit's reverse edges provide the fan-in values above. The
toolchain was Rust/Cargo 1.96.0 (`ac68faa20`, `30a34c682`) and Jujutsu 0.41.0.

Workspace check was attempted but this checkout does not contain
`web/assets/noto-sans-jp-vf.ttf`, which `arcweft-player-scene` includes at compile
time. CLI `--no-default-features --all-targets` Clippy is independently blocked
because existing CLI tests import the optional `arcweft-runtime-driver` while
that feature is disabled; the CLI production target and the other changed
crates pass the focused check above. Full workspace tests, Tier 2 MCP stdio, and
exact visual goldens were not run because of the missing asset and because this
cut does not change the Tier 2 or visual rendering risk areas.

## Explicit remaining boundary

[AW-AH-009.3.1 call-surface syntax production reconciliation](../reviews/requests/2026-07-16-aw-ah-009.3.1-call-surface-syntax-production-reconciliation.md)
must land its exact authored-call/range carrier before cut 6 can replace the
temporary existing signature result builder with the sema-backed query. That
follow-up must pass the accepted document, document-bound HIR, registered world,
exact call carrier, and this cut's sole cancellation flag to `SignatureQuery`,
use the same final stamp/publication validator around cache access, and delete
the old word-at-position/Rust-adapter successful path in the same compiling cut.
Until then this implementation does not claim AW-AH-009.3 semantic signature
help complete.
