# Lang-01.5.1.3 final contract package

Status: **READY_FOR_IMPLEMENTATION**  
Inspected Git commit: `0c8cb74dd96116a8b987cc419c9a280b6cabe4a4`  
Scope: generated-artifact runtime-binding identity, projection, catalog, fail-closed runtime gates, revision correlation, and tests.

This package is the standalone implementation contract for the attached Lang-01.5.1.3 request. It consumes the already accepted external-module metadata and Activity reconciliation facts. It does **not** add Rust dynamic-library loading, WASM instantiation, process spawning, artifact discovery, metadata decoding, or artifact execution semantics.

## Decisive result

The final model has one immutable, exact `GeneratedArtifactBindingKey`, one canonical `GeneratedArtifactBindingLaunchProduct`, and one host-owned Sans-I/O fixed-slot catalog. The selected-profile launch product stores the complete key once and carries canonical Activity selections including `ActivityImplementationId`. Runtime plans and function values carry only the product-local typed `GeneratedArtifactBindingId`; no-profile compilation carries no product rather than a fabricated empty topology. Registration compares the host's complete claimed key against the selected requirement before mutating a slot. Resolution checks the active topology before checking presence. Missing, stale, unselected, kind-mismatched, or structurally mismatched bindings never fall back through a callable spelling, Activity spelling, mount, basename, adapter profile, or filesystem path.

The generated function identity is added directly to Arcweft-owned `AdapterFunction` and propagated through the existing semantic and runtime-plan records. No extension trait, parallel side map, compatibility reader, alias, dual resolver, or last-known-good path is permitted.

## Reading order

1. `FINAL_CONTRACT.md` — normative behavioral contract.
2. `RUST_API_SHAPES.md` — final public and cross-crate Rust shapes.
3. `CRATE_AND_FILE_DELTA.md` — owning crates, dependency direction, and file-level edits.
4. `CANONICALIZATION_AND_CODEC.md` — key coverage, ordering, schema, and strict decoding.
5. `TOPOLOGY_RUNTIME_FLOW.md` — accepted-topology-to-runtime projection and pre-host gates.
6. `ERRORS_AND_LIFETIME.md` — machine codes, mismatch ordering, stale precedence, and LSP lease rules.
7. `TEST_MATRIX.md` — complete positive, negative, stale, codec, and no-partial-work matrix.
8. `IMPLEMENTATION_ORDER.md` and `DELETION_MATRIX.md` — landing sequence and mandatory removals.
9. `TRACEABILITY.md` — request-to-decision closure.
10. `REPOSITORY_EVIDENCE.md` and `VERIFICATION.md` — inspected authority and actual package validation.

`FINAL_STATUS` and `OPEN_QUESTIONS.md` are machine-readable sidecars. `OPEN_QUESTIONS.md` is exactly `none`.

## 日本語要約

受理済み metadata を実行時に名前で引き直すのではなく、loader が同じ transaction 内で完全な binding requirement を生成し、compiler/runtime はその requirement のローカル ID だけを運びます。host は requirement ID と完全 key を提示して既構築 binding を登録し、call・first-class function apply・Activity start の直前で完全相関を検証します。古い topology/LSP generation、未登録、未選択、異種 export、artifact/export/hash の不一致はいずれも host work を開始する前に失敗します。
