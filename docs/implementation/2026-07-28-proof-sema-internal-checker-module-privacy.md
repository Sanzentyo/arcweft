# Proof convergence: sema internal checker-module privacy

Date: 2026-07-28

Status: `IMPLEMENTED_VALIDATED_WITH_KNOWN_WORKSPACE_BASELINE`

Jujutsu change audited: `urzvnpxwmlkuyvnqtmrrltstnsqvovns`

## Boundary

This deletion-driven cut removes three zero-consumer public namespaces from
`arcweft-lang-sema`:

- `arcweft_lang_sema::borrow`;
- `arcweft_lang_sema::fact_layer`; and
- `arcweft_lang_sema::lifetime`.

All items in these modules were already `pub(crate)` or private. Repository
Rust, tests, examples, documentation, and all 30 retained review packages have
no external module-path consumer or public-visibility requirement for them.

The files and their behavior remain active crate-internal owners:

- `borrow` is consumed by the checker and checker child modules;
- `fact_layer` is consumed by checker/module and semantic traversal; and
- `lifetime` is consumed by the checker and its child modules.

Only the root declarations changed from `pub mod` to `mod`. No item visibility
was reduced below `pub(crate)`, and no internal consumer was redirected.
Public semantic entrypoints and responsibility modules such as `check`,
`checker`, `types`, `env`, and `diagnostics` remain public.

No root re-export, compatibility alias, forwarding module, wrapper, source
gate, or removed-syntax diagnostic replaces the deleted namespaces.

## Direct evidence

The new sema trybuild row imports exactly the three former module paths. It
compiled before their root visibility changed and now receives one E0603 error
for each private module. This is direct Rust visibility evidence rather than a
source-text gate.

The complete sema test suite exercises all active borrow, lifetime-registry,
fact transfer, suspension-boundary, and semantic traversal consumers after the
visibility reduction.

The current implementation summaries were corrected to distinguish public
semantic boundaries from these crate-private checker owners. The historical
`docs/reviews/pro_review21.md` remains unchanged; it already proposed `mod
borrow` and `mod lifetime` and is preserved as historical review material.

## Contract boundary

This cut does not alter semantic facts, borrow/lifetime rules, diagnostics,
limits, runtime behavior, serialization, or source identity.

The corrected Proof 01.1.1.4.1 package remains only partially
implementation-ready pending
[`01.1.1.4.1.1`](../reviews/requests/2026-07-27-seq-proof-01.1.1.4.1.1-source-owner-and-semantic-consistency-correction.md).
No blocked source-owner, pathless-variant, Duration comparison,
overflow-owner, region, or leaf-expression decision is inferred here.

## Validation

Completed:

- `cargo fmt --all` and final `cargo fmt --all -- --check`: passed;
- exact internal-module API compile-fail row: passed;
- `cargo check -p arcweft-lang-sema --all-targets --all-features`: passed;
- `cargo test -p arcweft-lang-sema --all-targets --all-features`: passed,
  including 1,119 unit tests and all integration/compile-fail suites;
- `cargo clippy -p arcweft-lang-sema --all-targets --all-features -- -D warnings`:
  passed;
- `cargo check --workspace --all-targets --all-features`: passed;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed; and
- `git diff --check`: passed.

`just test-workspace` ran for 882.8 seconds. It passed the changed sema suite,
the new compile-fail row, and every preceding workspace suite. It stopped only
at the established `arcweft-cli --test arcw_fixtures_check_run` baseline. The
exact suite was rerun and reported three passes plus the same two failures
present at the parent revision:

- `spec_should_pass/check/010_capability_fs_read.arcw`; and
- `spec_should_pass/run/002_file_read_task.arcw`.

Both rows await final attached-HIR publication of the capability-owned
`FsError`. This cut does not touch that owner and adds no fallback nominal,
fixture bypass, compatibility reader, or source gate.

Tier 2 is not applicable: this is an isolated semantic-crate visibility
reduction and does not change runtime, render, Agent, MCP, capture, or persisted
data behavior.

The final ZIP ledger contains 30 retained `docs/reviews/**/*.zip` archives,
zero unrecorded hashes, and zero root-inbox ZIPs. No returned
Proof 01.1.1.4.1.1 correction archive exists.

## Structural audit

The canonical audit is retained under
[`structure-audits/proof-sema-internal-checker-module-privacy-2026-07-28/`](structure-audits/proof-sema-internal-checker-module-privacy-2026-07-28/).
It scanned 3,798 files, including 1,962 Rust files and 905,958 physical Rust
LOC across 95 manifests. It reported zero errors and 146 existing warnings.

Current changed-file metrics are:

| Owner | Bytes | Physical LOC | Classification |
| --- | ---: | ---: | --- |
| `arcweft-lang-sema/src/lib.rs` | 912 | 44 | semantic crate facade |
| `arcweft-lang-sema/tests/api_compile.rs` | 2,025 | 57 | API compile-fail driver |
| `arcweft-lang-sema/tests/ui/internal_checker_modules_private.rs` | 69 | 3 | API compile-fail fixture |
| `arcweft-lang-sema/tests/ui/internal_checker_modules_private.stderr` | 978 | 35 | deterministic compiler diagnostic |
| `docs/implementation/README.md` | 132,528 | 1,900 | implementation-state documentation |
| `docs/implementation/phase-0-1-workspace.md` | 49,244 | 529 | implementation-state documentation |

No production file size, dependency edge, manifest, feature, opcode, public
data type, or serialized format changed.

## Next boundary

Agent REPL `diagnostics`, `error`, and `evidence` are separately audited facade
module-path candidates, but their root-reexported public types are active and
must be preserved. Any privacy cut must retain those deliberate facade exports.
The public `command` responsibility module has many production consumers and
must remain public. Proof semantic leaf readers and accepted Dialogue exteriors
remain frozen until their correction/replacement authority is ready.
