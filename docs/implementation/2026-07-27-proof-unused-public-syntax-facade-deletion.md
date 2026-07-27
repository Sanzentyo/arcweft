# Proof convergence: unused public syntax facade deletion

Date: 2026-07-27

Status: `IMPLEMENTED_VALIDATED`

## Context

After the detached fragment payload deletion at Git commit
`bc0b081755d8a8894bc0e05971c46ec40380bf3e`, a workspace-wide audit of every
top-level public function in `arcweft-lang-syntax` found exactly two APIs whose
only occurrence was their own definition:

- `types::parse_where_clause_list`, which parsed arbitrary raw text into a
  detached `Vec<WhereClause>` at synthetic base offset zero; and
- the complete `cst::path` projection module, which scanned a CST for rooted
  path spellings and included the obsolete `parent::` alias in its public
  result vocabulary.

Neither API had a production or test consumer. No implementation or design
document refers to either boundary. Retaining them would preserve unused public
syntax authorities and, in the path module, an alias-specific projection that
the final grammar and diagnostics do not need.

## Deleted authority

- delete `parse_where_clause_list` directly;
- retain the crate-private `parse_where_clauses_at`, because declaration and
  signature parsers use its document-relative base offset;
- delete `cst/path.rs`, including `CstPathRootKind`, `CstPathRoot`, and
  `cst_path_roots`;
- remove the public `cst::path` module declaration; and
- add compile-fail evidence that external callers cannot import either removed
  facade.

No replacement wrapper, alias recognizer, compatibility module, source gate,
or removed-syntax diagnostic is introduced.

## Scope boundary

This cut does not delete public parsers with current consumers. In particular,
`parse_type_ref`, `parse_fn_signature`, `parse_expr`, `parse_cst`, and
document-bound fragment parsing remain in use. The final attached syntax/HIR
authority switch remains gated on the corrected Proof `01.1.1.4.1` contract;
this deletion chooses no missing leaf, call, path-root, or scope payload.

## Validation

The implementation is Jujutsu change
`nspxzqzvowzkmpkpxyqytlsykvnmxxmr` over parent Git commit
`bc0b081755d8a8894bc0e05971c46ec40380bf3e`.

The final checkout passed:

- `cargo fmt --all -- --check` and `git diff --check`;
- the syntax public-API trybuild matrix with the new
  `removed_unused_syntax_facades.rs` row;
- all 494 syntax library tests and every syntax integration suite;
- strict all-target/all-feature syntax Clippy;
- `cargo check --workspace --all-targets --all-features`; and
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`.

`just test-workspace` ran for 1,051.3 seconds. Every workspace, CLI,
integration, and compile-fail component before the Arcweft fixture gate passed,
including the new public-API row. The fixture gate retained its exact existing
three-pass/two-fail baseline:

```text
tests/fixtures/arcw/spec_should_pass/check/010_capability_fs_read.arcw
tests/fixtures/arcw/spec_should_pass/run/002_file_read_task.arcw
```

Those failures are the already recorded external-capability `FsError`
publication gap and do not import either deleted API. The persistent-cache
build CLI golden suite after that nonzero recipe step was run separately and
passed both tests.

The canonical structural audit generated reports under
[`structure-audits/proof-unused-public-syntax-facade-deletion-2026-07-27/`](structure-audits/proof-unused-public-syntax-facade-deletion-2026-07-27/).
It scanned 3,745 files, including 1,948 Rust files and 906,402 Rust physical
LOC across 95 manifests, and found zero errors and 146 existing warnings. Its
warning report is line-for-line identical to the parent audit.

Current changed production owners are `cst.rs` at 12,512 bytes / 429 physical
LOC and `types.rs` at 34,954 bytes / 1,083 physical LOC. The deleted
`cst/path.rs` was 2,487 bytes / 78 physical LOC. The compile-fail harness and
new Rust fixture are 16 and 6 physical LOC. No file crosses a new threshold,
no dependency edge changes, and no production responsibility is added.

The recursive review-package ledger contains 30 ZIPs; every exact SHA-256 is
recorded, the unrecorded count is zero, and the root review inbox is empty.

Tier 2 does not apply. Both deleted public APIs had zero consumers, and the cut
does not affect runtime, renderer, Agent, MCP, capture, persistence, or a
serialized contract.

## Next boundary

After validation and push, re-evaluate current `main` and the Proof `01.1.1.4.1`
return state. Continue deleting zero-consumer/source-free readers without
guessing the blocked final HIR schema.
