# View exported-part production contract correction

- Date: 2026-07-16
- Package: `arcweft-seq-06.11d.2.1.1.1.1-view-exported-part-production-contract-correction-final-contract.zip`
- Package SHA-256: `b4662f3ecd79c157ee93656a173e9809fff31696aaded1fedb9411cdb1e9732e`
- Package basis: Git `8984661d5679efccf7a16255f921530cd0b7cacc`
- Production base for this increment after final validation rebase: Jujutsu
  change `mytryolq` / Git `8a6d4a62a138`
- Working change: Jujutsu change `rqmwxyuq`
- Status: Increment 1 is implemented; the complete correction remains open

## Package intake

All twelve archive members were read before production changes. The archive
digest above was recomputed from the provided ZIP. The package has no open
result-changing decisions and requires eight small compiling increments. This
note records only the first coherent increment and does not redefine completion
around that subset.

The source-bound exported-part parser, HIR, semantic checking, program builder,
and ordinary removed-syntax recovery from the preceding reconciliation remain
intact. No compatibility alias, dual reader, old-spelling recognizer, or source
gate was introduced.

## Increment 1 implementation

The numeric identity that previously served incompatible semantic and registry
roles is split into:

- `ViewId(PublicId)` for stable authored/public View ownership;
- `ViewProgramId(PublicId)` for stable accepted-program identity;
- `AcceptedViewProgramRevision([u8; 32])` with nonzero, canonical lowercase-hex
  serialization; and
- opaque, non-serializable `ViewRegistryId(u32)` for process-local dense slots.

`arcweft-view::view` is now a small facade over dedicated `identity` and
`registry` responsibility modules. Entity ownership and private fragment/host
references use `ViewRegistryId`; authored `ViewCall` and `ViewProgram` values use
`ViewId`; and value-program snapshots use `ViewProgramId` instead of a numeric
definition position. Manual Serde exists only for stable identities and the
accepted revision. A compile-fail rustdoc proves that `ViewRegistryId` does not
implement `Serialize`.

Registry descriptors have private fields and explicit anonymous-Rust,
public-Rust, and Arcweft construction paths. Registration validates duplicate
public identity and capacity before mutation. Retired Arcweft slots become
permanent tombstones and are never reused. Focused tests cover anonymous/public
capabilities, candidate-first duplicate rejection, stable string codecs,
revision codecs, and stale-slot non-aliasing.

Runtime mount creation and restore now retain the accepted stable
`ViewProgramId`; they no longer synthesize program identity from a definition's
dense vector index. The temporary product resource still carries a string until
the package's product/catalog increments replace that boundary.

## Minimal API deviation

The package lists `ViewDescriptor::arcweft` and `ViewRegistry::retire_arcweft`
as crate-private inside `arcweft-view`, while replacement orchestration is owned
by the separate `arcweft-runtime-driver` crate. A crate-private constructor
cannot be called across that boundary.

The descriptor constructor and dense slot/index access remain crate-private.
`ViewRegistry` instead exposes registry-owned typed `register_arcweft` and
`retire_arcweft` operations that accept stable `ViewId`/`ViewProgramId` values
and neither require nor return a dense slot. This is the smallest dependency-
correct adaptation and does not create a compatibility surface.

## Verification evidence

All Cargo commands use `CARGO_INCREMENTAL=0`.

- `cargo fmt --all` — passed;
- `cargo fmt --all -- --check` — passed;
- `cargo test -p arcweft-view --lib` — 56 passed;
- `cargo test -p arcweft-view --doc` — two compile-fail rustdocs passed,
  including the non-serializable dense registry slot contract;
- `cargo check -p arcweft-runtime-driver --lib` — passed;
- `cargo clippy -p arcweft-view --all-targets --all-features -- -D warnings`
  — passed;
- `cargo clippy -p arcweft-runtime-driver --lib --all-features -- -D warnings`
  — passed;
- the focused `ViewMountState` snapshot/restore unit test — passed;
- the exact runtime nested-mount round-trip integration test — passed. The
  command wrapper reached its timeout immediately after Cargo printed the
  successful one-test result; no Cargo or rustc process remained;
- `cargo test -p arcweft-runtime-driver --test view_runtime` — 11 passed and
  four pre-existing fixture-validation failures. Three fixtures omit required
  `ViewTextBlockResource` owners and the standard dialogue fixture uses a
  resource identity rejected by the existing canonical encoder. These failures
  occur in the pre-existing `encode_canonical_section` call before the new typed
  program-ID adaptation. The changed nested-mount snapshot/restore case passed.

## Structural audit

The canonical audit ran after rebasing the increment onto Git
`8a6d4a62a138`:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

It scanned 3,031 files, 1,506 Rust files, 694,449 Rust physical LOC, and 90
package manifests. The result was 0 errors and 128 repository-wide warnings.
Exact reports are checked in under
`docs/implementation/structure-audits/view-exported-part-correction-increment1-2026-07-17/`.

Exact current-checkout metrics for every changed Rust file are:

| Path | Bytes | Physical LOC | Role | Embedded test LOC |
| --- | ---: | ---: | --- | ---: |
| `crates/arcweft-cli/src/app/bundle/tests.rs` | 81,579 | 2,545 | integration fixture adaptation | n/a |
| `crates/arcweft-runtime-driver/src/view_runtime/evaluator.rs` | 63,540 | 1,553 | View evaluation/mount construction | 0 |
| `crates/arcweft-runtime-driver/src/view_runtime/part.rs` | 2,289 | 74 | accepted program/part boundary | 0 |
| `crates/arcweft-runtime-driver/src/view_runtime.rs` | 41,154 | 1,068 | runtime orchestration/snapshot restore | 0 |
| `crates/arcweft-view/src/entity.rs` | 6,861 | 242 | entity ownership slots | 0 |
| `crates/arcweft-view/src/lib.rs` | 7,669 | 166 | crate facade | 2 |
| `crates/arcweft-view/src/program.rs` | 33,131 | 979 | authored program/builder | 244 |
| `crates/arcweft-view/src/tests/display_frame.rs` | 7,150 | 238 | display tests | n/a |
| `crates/arcweft-view/src/tests/entity_reactive_registry.rs` | 7,462 | 240 | entity/registry tests | n/a |
| `crates/arcweft-view/src/tests/fragment_layout.rs` | 5,970 | 212 | fragment/layout tests | n/a |
| `crates/arcweft-view/src/value_program.rs` | 26,986 | 849 | value program/mount snapshots | 280 |
| `crates/arcweft-view/src/view/identity.rs` | 6,551 | 229 | stable identities/codecs | 54 |
| `crates/arcweft-view/src/view/registry.rs` | 8,456 | 271 | dense registry/descriptors | 87 |
| `crates/arcweft-view/src/view.rs` | 3,092 | 96 | responsibility facade/host IDs | 0 |

The largest current workspace Rust files are test files: CLI runtime bench
(256,474 bytes/7,970 LOC), native vertical observe (238,805/6,620), published
JLREQ class mix (220,473/6,109), native samples/effects (214,731/5,850), and
compiler tests (180,052/5,363). They are unchanged by this increment.

`arcweft-view` has 12 dependency fan-in edges and eight fan-out edges. The one
new edge is the package-authorized normal dependency on `blake3`; it will own
the canonical accepted-revision transcript in Increment 6. The new identity
and registry modules remain below the ordinary 300-800 LOC responsibility
target. The adapted runtime evaluator and CLI test file remain warning-level
legacy hotspots, but this increment changes only their ID-boundary call sites
and adds no new responsibility to them.

## Post-rebase verification

After rebasing onto Git `8a6d4a62a138`, the increment was verified again with
`CARGO_INCREMENTAL=0`:

- `cargo fmt --all -- --check` — passed;
- `cargo test -p arcweft-view --lib` — 56 passed;
- `cargo test -p arcweft-view --doc` — two compile-fail rustdocs passed;
- the exact runtime nested-mount round-trip integration test — passed; and
- `cargo clippy -p arcweft-view -p arcweft-runtime-driver --all-targets
  --all-features -- -D warnings` — passed.

## Remaining correction increments

The following package requirements are explicitly not complete:

1. multi-source schema-v2 SourceMap ownership and direct
   `arcweft-bundle -> arcweft-source` dependency;
2. bundle complete-product typestate and source-index validation;
3. typed product definition/call tables and immutable runtime
   `ViewProgramCatalog`/`ViewDefinitionIndex` authority;
4. typed static instruction inventory, canonical semantic transcript, and
   accepted program revision derivation;
5. semantic occurrence reconciliation, opaque direct-boundary capability, and
   persistent-owner rejection for anonymous Rust Views;
6. six-phase candidate-first replacement with exact rollback, generation, and
   targeted cache/trace invalidation;
7. final ordinary-parser rejection accounting with no historical recognizer;
   and
8. contextual Style application edges plus atomic LSP rename, symbols,
   semantic tokens, limits, and the remaining test matrix.

The correction must remain open until those increments and their Tier-0/Tier-1,
codec, runtime, replacement, tooling, structural, and migration gates pass.
