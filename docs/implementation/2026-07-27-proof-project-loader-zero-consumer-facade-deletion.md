# Proof convergence: project-loader zero-consumer facade deletion

Date: 2026-07-27

Status: `IMPLEMENTED_VALIDATED_WITH_KNOWN_WORKSPACE_BASELINE`

## Boundary

This deletion-driven cut removes two unreleased project-loader APIs with no
Rust or documentation consumer:

- `LoadedProject::into_sources`; and
- `load_discovered_with_limits`.

The final source owner is the bound `LoadedProject`, observed through its
borrowed `sources()` projection. Consumers that need bounded explicit loading
already use `discover_manifest` and `load_with_limits`; the removed helper was
only their unused composition. No consuming clone path, renamed forwarding
function, compatibility alias, or wrapper was introduced.

One downstream compile-fail row proves that the removed free function cannot
be imported and that `LoadedProject` no longer exposes the consuming method.
This is compiler/type evidence, not a source-text gate.

## Validation

Completed:

- `cargo fmt --all`;
- `TRYBUILD=overwrite cargo test -p arcweft-project-loader --test public_api
  --all-features -- --nocapture`: both compile-fail rows passed;
- `cargo test -p arcweft-project-loader --all-targets --all-features`:
  passed, including 136 unit tests, four dependency-direction tests, both
  compile-fail rows, and six release-trust end-to-end tests;
- `cargo check --workspace --all-targets --all-features`: passed; and
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed.

`just test-workspace` ran for 460.3 seconds. It passed the changed loader,
the new compile-fail row, and every preceding downstream suite, then stopped at
the established `arcweft-cli --test arcw_fixtures_check_run` baseline. The
exact suite was rerun and reported three passed and the same two failed rows:

- `spec_should_pass/check/010_capability_fs_read.arcw`; and
- `spec_should_pass/run/002_file_read_task.arcw`.

Both rows still require publication of the capability-owned `FsError` nominal
through the final attached HIR authority. The removed loader conveniences do
not change parsing, HIR construction, semantic publication, capability I/O, or
execution. No fallback nominal, compatibility reader, fixture bypass, or
source gate was added.

The push-cut checks also passed:

- `cargo fmt --all -- --check`;
- `git diff --check`; and
- the review ZIP ledger contains 30 retained archives, zero unrecorded hashes,
  and zero ZIP files directly in the `docs/reviews/` inbox.

Tier 2 is not applicable. This isolated public-API deletion does not change
runtime, rendering, Agent, MCP, capture, persistence, network behavior, or a
serialized contract.

## Structural audit

The canonical audit is retained under
[`structure-audits/proof-project-loader-zero-consumer-facade-deletion-2026-07-27/`](structure-audits/proof-project-loader-zero-consumer-facade-deletion-2026-07-27/).
The final pass scanned 3,761 files, including 1,953 Rust files and 906,351
physical Rust LOC across 95 manifests. It reported zero errors and 146 existing
warnings; its complete warning inventory is byte-identical to the parent
compiler-facade audit.

Current changed-file metrics are:

| Owner | Classification | Bytes | Physical LOC |
| --- | --- | ---: | ---: |
| `arcweft-project-loader/src/project.rs` | production | 27,278 | 824 |
| `arcweft-project-loader/tests/public_api.rs` | test | 246 | 6 |
| `arcweft-project-loader/tests/ui/removed_zero_consumer_project_facades.rs` | test | 236 | 9 |

No manifest, dependency edge, feature, or crate boundary changed. `project.rs`
remains below the production warning threshold; its existing embedded unit
tests cover the project loader's bounded and exact/one-over behavior.

## Next boundary

The corrected Proof `01.1.1.4.1` semantic leaf/expression package remains
`DESIGN_BLOCKED`, so bound `ParsedSource`, Items fragments, linked HIR/project,
and final leaf-expression authority are not guessed. The next independently
provable deletion candidates are the syntax-only private projections and
zero-consumer accessors already audited against `main`.
