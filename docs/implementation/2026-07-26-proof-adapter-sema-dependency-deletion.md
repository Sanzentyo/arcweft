# Proof adapter semantic dependency deletion

## Source and decision

This deletion-driven cut follows the accepted
`arcweft-proof-concurrency-v6.1.1-typed-ast-proof-block-hir-runtime-identity-final-contract.zip`,
SHA-256
`1b7de5f2c10a5b29d67c72011e4272df9a76af8907fd21fe162de54809fc69ef`.
It removes a normal dependency path that would otherwise make the Proof public
HIR authority part of the runtime-host build graph.

Cargo feature unification previously allowed the normal path

```text
arcweft-runtime-host
  -> arcweft-adapter-context
  -> arcweft-lang-hir / arcweft-lang-sema / arcweft-lang-syntax
```

whenever a language-tooling package enabled `arcweft-adapter-context/sema` in
the same workspace graph. The manifest data and its semantic projection have
different owners. Keeping them behind features in one crate did not preserve
the runtime/compiler layer boundary.

The repository decision is therefore:

- `arcweft-adapter-context` owns only language-free adapter manifest data,
  registry policy, typed host-call metadata, and deterministic manifest codecs;
- the new `arcweft-adapter-sema` bridge owns projection into generated source,
  HIR external declarations, semantic nominal/callable facts, and checker
  effect environments; and
- runtime crates depend only on `arcweft-adapter-context`.

This is a direct replacement. The removed `sema` feature, old manifest methods,
and old generated-source URI are not preserved through aliases, forwarding
methods, re-exports, or dual readers.

## Implemented boundary

- The complete source-backed registration implementation moved from
  `arcweft-adapter-context::manifest::registration` to
  `arcweft-adapter-sema::registration`.
- `AdapterSemanticRegistration<'_>` is the explicit projection context for one
  admitted `AdapterManifest`. It owns effect declaration, target-effect
  availability, and deterministic source-backed fact construction.
- Compiler, project-loader, CLI, and LSP consumers now call the bridge directly.
- Generated adapter semantic documents now use the final owner URI
  `arcweft-generated://adapter-sema/{ordinal}`.
- The obsolete adapter-context semantic feature and all optional language-layer
  dependencies were deleted from its manifest.
- A Cargo-metadata integration test walks normal dependency edges under
  `--all-features`, reports the exact path on failure, and rejects:
  - `arcweft-runtime-host` reaching syntax, HIR, runtime-plan, or compiler; and
  - `arcweft-adapter-context` reaching syntax, HIR, or sema.

Historical implementation notes retain the commands and feature names that
were true for their recorded revisions. They are not current build guidance.

## Proof sequencing boundary

This cut does not make the rejected Proof 01.1.1.4 return complete and does not
invent semantic leaf-expression payloads. The corrected 01.1.1.4.1 package is
still required before the final HIR public authority switch can close. The
protected Proof working changes remain intentionally separate from this
workspace-compiling dependency deletion.

## Verification

The reviewable cut was measured from Jujutsu change `upvnvnnv` over
`main@8dcaead11925bc0ece4e56342f9d2399ef50e658`.

The focused validation passed:

- `arcweft-adapter-context`: 17 unit tests and one language-free public API
  integration test;
- `arcweft-adapter-sema`: 10 registration/digest tests after data-owner-only
  duplicates were removed;
- `arcweft-project-loader`: 136 unit tests, four dependency/API tests, one
  public compile-fail test, and six release-trust tests;
- compiler project-cache transaction: 11 tests;
- `arcweft-runtime-host`: 34 unit tests and all integration targets
  (1 AWBC product, 6 bundle runner, 1 dependency direction, and 1 View
  interaction test); and
- the all-feature Cargo-metadata dependency test independently passed after it
  was tightened to use Cargo's exact `workspace_members` IDs.

The following compilation and lint gates passed:

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The first full runtime-host test attempt encountered Windows `LNK1106` with
only 160 MiB free on the workspace drive. Inspection found 345.6 GiB of
regenerable Cargo target artifacts, including 143.6 GiB of incremental state.
`cargo clean` removed 266,844 build-artifact files and recovered that space;
the same all-target runtime-host suite then passed from a clean build with one
build job. No source, package, or protected Proof working change was removed.

`just test-workspace` completed the non-CLI workspace suites, CLI lib/bins, and
the selected CLI integrations before the final fixture matrix reported the two
existing Proof public-switch gates:

- `tests/fixtures/arcw/spec_should_pass/check/010_capability_fs_read.arcw`;
- `tests/fixtures/arcw/spec_should_pass/run/002_file_read_task.arcw`.

An exact rerun of `arcw_fixtures_check_run` produced 3 passes and those 2
failures. Both require the typed `FsError` member omitted by the rejected Proof
01.1.1.4 package. This cut does not restore the detached extern-capability
reader, add a global/Named fallback, or otherwise hide that known gate. An
initial 30-minute workspace attempt timed out while linking; its orphaned
Cargo/rustc/link PIDs were identified and stopped before the controlled
60-minute rerun that produced the result above.

`just test-tier2` passed all 46 selected tests: 22 MCP stdio tests, one slow
Agent-observe test, 16 native auxiliary-capture cases, two visual-smoke cases,
one checked-in golden-integrity case, and four exact PNG/imq goldens.

The canonical structural audit passed with 0 errors and 146 warnings across
3,692 files, 1,939 Rust files, 906,494 physical Rust lines, and 95 manifests.
The exact reports are in
[`structure-audits/proof-adapter-sema-dependency-deletion-2026-07-26/`](structure-audits/proof-adapter-sema-dependency-deletion-2026-07-26/).

The new bridge has three normal consumers (`arcweft-cli`, `arcweft-lsp`, and
`arcweft-project-loader`), one compiler dev consumer, and eight normal outgoing
dependencies. Its measured Rust files are:

| File | Classification | Bytes | Physical LOC | Responsibility |
|---|---:|---:|---:|---|
| `src/lib.rs` | production facade | 347 | 9 | bridge boundary and registration module |
| `src/registration.rs` | production | 10,659 | 279 | semantic projection context, generated document, external facts |
| `src/registration/input.rs` | production | 49,037 | 1,249 | one transactional manifest-to-environment projector |
| `src/registration/input/digest.rs` | production | 23,138 | 648 | deterministic semantic input digest |
| `src/registration/input/source.rs` | production | 42,039 | 1,131 | generated source renderer and exact source map |
| `src/registration/tests.rs` | unit test | 22,147 | 585 | semantic projection and identity matrix |

`input.rs` is 49 lines over the 1,200-LOC warning threshold and below the
2,500-LOC error threshold. It was reviewed rather than split in this cut: the
file is one stateful projection transaction that preserves a shared source map,
accepted owner, declaration order, nominal inventory, callable schemas, Rust
metadata, and tooling validation. Splitting those mutually dependent phases
now would add context plumbing and duplicate identity/error projection during
an ownership-only deletion. Further feature growth must first extract a
cohesive 300–800 LOC responsibility module; this warning is not an exemption
from future review.
