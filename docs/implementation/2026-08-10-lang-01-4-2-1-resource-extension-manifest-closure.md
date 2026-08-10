# Lang-01.4.2.1 resource extension-manifest closure

Date: 2026-08-10

Inspected Git commit: `61a257fb5a7e38a670e0926f94c1742f649244cf`

Working-tree state at inspection: dirty on `main` with this coherent resource
manifest, loader, compiler-handoff, bundle, and runtime cut. The sole
`D:/git/arcweft` worktree and normal shared Cargo target were used. Cargo used
four build jobs and normal incremental compilation.

Supersedes: the implementation-pending Lang-01.4.2.1 status recorded in
`2026-07-22-reviews-zip-intake-audit.md`. The rejected Lang-01.4.2 archive
remains superseded; this cut implements the verified repository-reconciled
Lang-01.4.2.1 return.

## Performed

Added the Sans-I/O `arcweft-resource-manifest` crate as the single strict JSON
wire authority for extension resource types. One duplicate-preserving parse,
closed V1 DTO lowering, typed source maps, canonical encoding, exact descriptor
digest validation, bounded deterministic work accounting, and atomic aggregate
registry publication now form one path. The AWFB artifact path calls that same
decoder with a typed `EmbeddedArtifact` package expectation; the former
independent `serde_json` coordinate probe was deleted, so artifact admission no
longer parses authored JSON twice.

The current resource model owns inherent descriptor transcript/digest behavior
and codec iteration. Registry validation reports structured source-projectable
issues for duplicate identities, invalid defaults, schema/reference mismatches,
codec support, capabilities, and descriptor claims. Required defaults, empty
non-empty lists, inverted constraints, out-of-range constrained defaults, and
multi-version package selection fail closed.

The strict root project manifest now accepts one optional normalized
`resource-type-manifest` path. The project loader reads that exact root path and
explicit typed dependency seeds only, checks UTF-8 with the exact invalid byte
offset, decodes every selected coordinate, publishes once against the supplied
immutable engine base, and retains both source-backed manifests and the one
published `Arc<ResourceTypeRegistry>`. An absent root field performs no probe,
and conventionally named undeclared files remain ignored. Compiler and LSP
registration consume the loader-published registry rather than constructing a
second empty registry.

AWFB now owns required startup section
`BundleSectionKind::ResourceTypeManifests = 22`, schema 1. Its exact AWRM
framing contains sorted canonical manifests, per-entry raw digests, and the
final registry digest. Empty sets omit the section. Decode checks section
bounds, raw digest before JSON, canonical bytes, strict coordinate ordering,
trailing/truncated/count failures, aggregate publication, and the final digest
before exposing the registry.

Runtime-driver session options now carry the explicit engine base registry.
AWFB construction, replay, materialized hot swap, runtime-host, native/web
players, CLI ingress, and release-cache decode use that base. A session retains
both the engine base needed for later patch reconstruction and the final
registry reconstructed for the active bundle. No section-22 compatibility
reader, empty-registry fallback, source reconstruction, or alternate runtime
model was added.

## Validation passed

All Cargo validation used `--jobs 4`.

- `cargo fmt --all -- --check` and `git diff --check` passed.
- `cargo check --workspace --all-targets` passed after the final single-reader
  switch.
- focused Clippy for resource-model, resource-manifest, launch,
  project-loader, bundle, runtime-driver, and runtime-host passed with
  `-D warnings`.
- `cargo clippy --workspace --all-targets -- -D warnings` passed. Clippy-found
  responsibility excess was decomposed into schema-kind source-map binders,
  `TopologyFreezeInput`, and a resource-manifest LSP diagnostic projector;
  no warning suppression was added.
- `cargo test -p arcweft-resource-manifest` passed: 1 unit, 13 manifest
  contract, 6 publication contract, and doc tests. The test bodies completed in
  0.58, 0.02, and 0.04 seconds respectively.
- `cargo test -p arcweft-bundle --test resource_type_manifest_section` passed
  7 tests, including required code 22, omit-empty, raw-digest-first,
  noncanonical JSON, entry reorder, truncation/count overflow, trailing bytes,
  and final registry digest checks; the test body completed in 0.01 seconds.
- the bundle product focused section-22 round trip passed and rejects implicit
  decode without a supplied engine base.
- the runtime-driver focused AWFB extension-registry test passed; it
  reconstructs against the supplied base and exposes the exact final digest.
  Its test body completed in 0.03 seconds.
- full resource-model, resource-manifest, launch, bundle, and runtime-driver
  package suites and doc tests passed. The combined build took 6 minutes 10
  seconds after the requested clean; individual test groups completed in
  seconds or less.
- project-loader library tests passed 146/146, including six new explicit-path,
  no-scan, UTF-8, coordinate, dependency-seed, and unresolved-package rows.
- existing compiler registration tests retain `Arc::ptr_eq` identity across
  the accepted launch profile, and the workspace check proves the new loader
  and LSP consumers compile against that boundary.
- structured `cargo metadata --format-version 1 --no-deps` was written to
  `target/resource-manifest-metadata.json`.
- `structure-audit-gate` passed with 95 workspace packages, 2,020 Rust files,
  999,125 physical Rust LOC, 182 review triggers, and zero blocking
  violations. Generated evidence is retained under
  `structure-audits/lang-01-4-2-1-resource-extension-manifest/`.

The longer wall times above were compiler/link work after `cargo clean` with
the repository's four-job cap. The focused test bodies remained 0.01--0.58
seconds; no test-body performance defect was inferred from build duration.

## Non-green broader rows

`cargo test -p arcweft-compiler` remains non-green only in the pre-existing
`view_product` integration target: one test passes and six fail. The failures
expect the retired/unfinished View lowering behavior (`Image` callable,
literal-text diagnostic codes, old stage/cardinality, and old recovery
projection). This cut changes no compiler source or compiler tests and does not
restore an old View lowerer or add a temporary builtin. The same row was
already recorded by the preceding Agent cut and belongs to the returned
Lang-01.5.1 View work.

`cargo test -p arcweft-project-loader` passes its 146 implementation tests but
remains non-green in one of four pre-existing `dependency_direction` tests:
the current metadata graph reports `arcweft-lang-sema` reaching
`arcweft-core`. This cut changes neither crate's manifest and the canonical
structure audit reports zero blocking violations for the new resource-manifest
edges. No unrelated dependency graph was rearranged to make this slice green.

Because these known independent rows fail, `cargo test --workspace` was not
rerun as a redundant aggregate after their exact package failures were
captured. No Tier 2 row specific to resource-manifest publication is declared
by the returned package; the exact production limits are exercised by the
focused manifest/model suites.

## Structural disposition

The new crate facade is 34 LOC. Its largest responsibility modules are strict
decode (1,047 LOC), closed shape validation (1,001 LOC), and atomic publication
(921 LOC), all below the 1,200-LOC review trigger and separated by state/API
boundary. Bundle framing is isolated in a 287-LOC module instead of being
embedded into the already-large bundle facade/product owners.

Touched large bundle, CLI, LSP, loader, runtime-driver, and runtime-host files
retain their existing cohesive ingress/orchestration responsibilities. They
only select or transport the typed manifest registry; none copies the JSON
schema, registry rules, or canonical encoder. `topology/loader.rs` remains the
single exact-path/filesystem transaction owner, while JSON and registry
semantics stay below it. `session.rs` retains only engine-base/final-registry
state needed by construction, replay, and hot swap. The 2,708-LOC session
integration test continues to follow that same public session boundary. No
touched production file grew by 300 physical LOC in this cut.

## Request dispatch and explicit non-goals

No design request is dispatched from this cut: Lang-01.4.2.1 is returned,
verified, and implementation-ready, and this slice closes it. Public `res`
syntax/HIR/sema authority remains outside this manifest contract and is not
inferred here.

For still-unreturned correction contracts, dispatch must use the existing
narrow file under `docs/reviews/requests/`, attach every parent/previous return
named by that request, explain the exact compiler-exposed missing boundary, and
require one design-only ZIP with `OPEN_QUESTIONS=0`, exact typed owner/ABI/codec
and save allocations, full producer/consumer/deletion matrices, and no code
overlay. Do not split one serialized boundary across assignees or implement an
inferred placeholder while waiting.

This cut does not implement Lang-01.4.2 public `res` syntax, CSS/Takumi,
removed-syntax-only diagnostics, source gates, compatibility aliases, dual
readers, shims, or any correction cohort that the repository ledger still
classifies as unreturned.
