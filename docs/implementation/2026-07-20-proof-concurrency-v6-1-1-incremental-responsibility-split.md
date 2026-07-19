# Proof concurrency v6.1.1 — incremental syntax responsibility split

## Outcome

The incremental syntax subsystem now has a 12-line public facade and a
responsibility module for the database, snapshots, transaction validation, and
session-local node identities. The existing 21 database unit tests moved to a
separate test module. No parser rule, public API path, public type, diagnostic,
serialization shape, identity allocation rule, reconciliation rule, or
transaction behavior changed.

The split is intentionally mechanical:

- `incremental.rs` declares the subsystem modules and preserves the existing
  `arcweft_lang_syntax::incremental::*` exports;
- `incremental/database.rs` owns the unchanged parse-database implementation;
- `incremental/database_tests.rs` owns the unchanged unit-test cases;
- `limits.rs`, `reconcile.rs`, and `shape.rs` retain their existing
  responsibilities.

This removes the structural warning on the former 1,270-line production file
and removes its embedded 838-line test module. At this cut, the facade is 304
bytes and 12 physical lines, the database module is 13,897 bytes and 432
physical lines, and the external unit-test module is 27,392 bytes and 836
physical lines.

Moving the database implementation below the facade made two members that were
formerly visible to descendant modules inaccessible to the sibling
reconciliation module. Their visibility is now `pub(super)`, which restores
access only within the private `incremental` subsystem; neither member is
public outside that module, and the external crate API remains unchanged.

## Completion boundary

This cut does not claim Proof concurrency v6.1.1 Stage 2. It adds no new
grammar identity, attachment, error, or transaction type. Stage 2 remains
blocked until the private Stage 1 shadow grammar has typed descendants for the
remaining accepted top-level declaration families, including the outstanding
extern-module, dialogue-defaults, live-source, style, and public entity
families.

## Verification

Validation was run from parent revision `27995a0e3bb9`:

- `CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-syntax --all-features
  --lib incremental -- --nocapture`: 27 passed;
- `CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-syntax --all-features
  --test parse_error_incremental -- --nocapture`: 1 passed;
- `CARGO_INCREMENTAL=0 cargo check -p arcweft-lang-syntax --all-targets
  --all-features`: passed;
- `CARGO_INCREMENTAL=0 cargo clippy -p arcweft-lang-syntax --all-targets
  --all-features -- -D warnings`: passed;
- `cargo fmt --all -- --check`: passed;
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write
  docs/implementation/structure-audits/proof-concurrency-v6-1-1-incremental-responsibility-split-2026-07-20`:
  scanned 3,294 files, 1,689 Rust files, 776,647 Rust physical lines, and 92
  package manifests; reported 0 errors and 130 existing warnings;
- `git diff --check`: passed.

The first focused test compile exposed the sibling-module visibility described
above and exited with four privacy errors. After restricting the affected
members to `pub(super)`, the repeated focused command and every subsequent gate
passed.

Tier 2 is not required for this structure-only syntax-crate cut because it does
not span a public contract or affect runtime, render, Agent, MCP, or capture
behavior.
