# Incremental hot-swap bundle completion audit (2026-06-24)

Source package:
`D:/sanze/Downloads/arcweft-incremental-hot-swap-bundle-2026-06-23.zip`

This audit compares the package implementation order and verification plan
against the current repository state. It treats user-directed syntax changes as
authoritative: `asset set`, `AssetSetRef<T>`, `lazy use`, `eager use`, and
`hot checkpoint` are not v1 source syntax.

## Implementation order status

| Package cut | Current status | Evidence |
|---|---|---|
| Cut 1: remove import execution policy | Implemented | `UseMode`/`UseDependencyMode` removed; `lazy use`/`eager use` are parser diagnostics; typed `UseTree` is used for dependency discovery. |
| Cut 2: incremental model | Implemented | `arcweft-project::{artifact,fingerprint,incremental}` provide typed keys, digests, query kinds, snapshots, and invalidation reports. |
| Cut 3: persistent cache I/O | Implemented for object/record storage and build-output records | `arcweft-project-loader::cache` owns filesystem CAS, `.awci` records, locks, stats/verify/explain/prune/fetch; `arcw build` now writes metadata/plan/snapshot/AWFB records and reuses verified Program AWFB records for identical inputs. |
| Cut 4: compiler query and module object | Implemented as the conservative linked-HIR slice | `arcweft-compiler::{incremental,object,link,reachability,content_partition}` exist; `arcw build --watch` retains an in-memory compile-unit cache. Cross-invocation compiler query reuse is excluded pending design. |
| Cut 5: content DSL | Implemented with user-directed syntax corrections | `content` declarations, typed roots, content builtins, project graph edges, release dynamic-goto rejection, and internal `FiniteRefSet` reachability exist. Source-level `asset set` is explicitly rejected. |
| Cut 6: AWFB v1 | Implemented for the product container | Fixed-header AWFB, section index, size/digest/bounds checks, zstd output limits, external descriptors, product-only `.awfb` decode, inspection/export separation, duplicate-id rejection, and patch bundle container rules exist. Compact resource codecs are excluded pending design. |
| Cut 7: bytecode artifact verification | Implemented for structured bytecode plus compact validation sidecar | Structured bytecode ABI/layout/entry/reference verification gates runtime/player construction; `AWBC` carries a verified compact validation table. Final executable compact AWBC is excluded pending design. |
| Cut 8: patch model and generation runtime | Implemented for content-only/code-compatible live apply and restart fallback | AWFB patch plans, add/replace/remove operations, carrier payloads, base/target root validation, compatibility labels, generation pinning, and `BundleSession` hot-swap paths exist. True code-generational execution is excluded pending design. |
| Cut 9: CLI watch and dev transport | Implemented for polling/watch artifacts and native endpoint transport | `arcw build --watch`, `arcw run --watch`, patch sidecars, native in-process endpoint apply/restart, and native binary one-shot `--patch-transport` exist. Windowed event-loop live patch stream is excluded pending design. |
| Cut 10: external content/release manifest | Implemented for release manifests, cache fetch, policy, and signing workflows | `.awfr` manifests, deterministic fetch plans, file/http/https/cache mirrors, retry/budget/timeout/network policy, signature policy, Ed25519 envelope verification, key epoch rotation/revocation, and `arcw sign-bundle` exist. Product-grade patch target manifest/signature publication is excluded pending design. |

## Excluded design requests

These are intentionally outside the current goal until review responses provide
the missing implementation design:

- `docs/reviews/requests/2026-06-24-code-generational-hot-swap-design.md`
- `docs/reviews/requests/2026-06-24-awbc-executable-compact-table-design.md`
- `docs/reviews/requests/2026-06-24-persistent-compiler-query-cache-design.md`
- `docs/reviews/requests/2026-06-24-windowed-native-live-patch-design.md`
- `docs/reviews/requests/2026-06-24-product-resource-section-codecs-design.md`
- `docs/reviews/requests/2026-06-24-patch-target-manifest-signature-design.md`

## Verification status

The implementation note records the focused commands already run. The latest
reviewable gate status after the final request split was:

```bash
git diff --check
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features
cargo +nightly -Zscript tools/arcweft-structure-audit.rs --root . --write docs/implementation/structure-audit-incremental-hot-swap-2026-06-23
```

Current structural audit result: `0` errors, `97` warnings across `784` Rust
files and `385567` physical Rust LOC.

## Known non-blocking validation notes

- The source fixture command
  `cargo test -p arcweft-cli --test arcw_fixtures_check_run spec_should_pass_check_fixtures_pass_after_refactor --quiet`
  still fails before reaching the changed fixture because that test invokes
  `arcw check <path>`, while the current CLI exposes manifest-based
  `arcw check [--manifest-path <MANIFEST>]`. The fixture content was covered by
  sema fixture tests recorded in
  `docs/implementation/incremental-hot-swap-bundle-2026-06-23.md`.
- Negative product-path and inspection-export smokes are expected to fail with
  explicit product/codec errors, proving that product `.awfb` does not fall
  back to legacy JSON.

## Remaining TODOs

All remaining TODOs for this bundle goal are design-blocked and are listed in
the excluded design requests above. No additional implementation-sized TODO is
currently identified by this audit.
