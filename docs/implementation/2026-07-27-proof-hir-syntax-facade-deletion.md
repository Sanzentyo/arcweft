# Proof convergence: HIR syntax forwarding-facade deletion

Date: 2026-07-27

Status: `IMPLEMENTED_VALIDATED_WITH_KNOWN_WORKSPACE_BASELINE`

## Boundary

This deletion-driven cut removes `arcweft_lang_hir::syntax`, the unreleased
forwarding facade that re-exported syntax-owned modules through HIR.

- 101 direct facade references and ten HIR group-import projections were
  migrated to `arcweft_lang_syntax` in the same compiling cut.
- The consumer set is `arcweft-lang-sema`, `arcweft-runtime-plan`,
  `arcweft-test`, and `arcweft-verify`; no compatibility re-export remains.
- `arcweft-runtime-plan`, `arcweft-test`, and `arcweft-verify` had relied on
  the HIR facade in production while declaring syntax only as a development
  dependency. Their existing workspace-inherited syntax edges are now normal
  dependencies. `arcweft-lang-sema` already declared the correct normal edge.
- One compile-fail row proves that downstream crates cannot import the removed
  facade.

All migrated types remain the same syntax-owned Rust types. No AST clone,
wrapper type, alias module, dual reader, source reparse, source gate, or
compatibility shim was introduced.

The implementation is Jujutsu change `svqruyzpxzlk` over parent Git commit
`fe05993bbf13`.

## Validation

Completed:

- `cargo fmt --all`;
- `cargo test -p arcweft-lang-hir --test public_api --all-features --
  --nocapture`: all nine compile-fail rows passed, including the removed-facade
  row;
- `cargo check -p arcweft-lang-hir -p arcweft-lang-sema
  -p arcweft-runtime-plan -p arcweft-test -p arcweft-verify --all-targets
  --all-features`: passed after the direct dependency correction;
- `cargo test -p arcweft-lang-hir -p arcweft-lang-sema
  -p arcweft-runtime-plan -p arcweft-test -p arcweft-verify --all-targets
  --all-features`: passed, including all HIR/sema/runtime-plan/verify unit,
  integration, and compile-fail suites;
- `cargo check --workspace --all-targets --all-features`: passed;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed;
- the final source-owner audit found zero production references to the removed
  HIR facade; and
- the review ZIP ledger contains 30 retained archives, zero unrecorded hashes,
  and zero ZIP files directly in the `docs/reviews/` inbox.

`just test-workspace` ran for 896.8 seconds. It passed the changed HIR crate,
the migrated consumers, the new compile-fail row, and all preceding downstream
suites, then stopped at the established
`arcweft-cli --test arcw_fixtures_check_run` baseline. The exact suite was
rerun and reported three passed and the same two failed rows:

- `spec_should_pass/check/010_capability_fs_read.arcw`; and
- `spec_should_pass/run/002_file_read_task.arcw`.

Both still require publication of the capability-owned `FsError` nominal
through the final attached HIR authority. This import-owner migration does not
change their parser, HIR, semantic publication, or execution behavior and does
not add a fallback nominal, compatibility reader, or fixture bypass.

Tier 2 is not applicable. Although the cut spans multiple crates and corrects
a public module/dependency boundary, it does not materially change runtime,
rendering, Agent, MCP, capture, persistence, or serialized behavior.

## Structural audit

The canonical audit is retained under
[`structure-audits/proof-hir-syntax-facade-deletion-2026-07-27/`](structure-audits/proof-hir-syntax-facade-deletion-2026-07-27/).
It records all 55 changed Rust files plus the largest workspace files, exact
bytes and physical LOC, classification, embedded-test markers, and the complete
dependency graph. The scan covered 3,753 files, including 1,951 Rust files and
906,390 physical Rust LOC across 95 manifests. It reported zero errors and 146
existing warnings; the warning-heading inventory is identical to the parent
audit.

The changed production hotspots are pre-existing owners whose only change in
this cut is the import path:

| Owner | Bytes | Physical LOC |
| --- | ---: | ---: |
| `arcweft-runtime-plan/src/expr.rs` | 84,582 | 2,384 |
| `arcweft-runtime-plan/src/flow.rs` | 76,950 | 2,105 |
| `arcweft-lang-sema/src/semantic.rs` | 78,417 | 2,102 |
| `arcweft-verify/src/lib.rs` | 65,285 | 1,905 |

No hotspot grew or gained a responsibility. The three syntax dependency edges
changed from `development` to `normal`; package count, feature sets, and edge
direction remain unchanged. Syntax remains below HIR, sema, runtime-plan, and
verify.

## Next boundary

The broad HIR syntax facade is now absent. The remaining Proof public switch
still depends on the corrected `01.1.1.4.1` semantic leaf-expression package:
the repository-retained return is a `NOT_READY` package-build stub, not a usable
contract. Until a decision-complete redelivery arrives, continue only with
independently provable zero-consumer deletions; do not invent the missing leaf
schema or preserve old linked/detached readers through shims.
