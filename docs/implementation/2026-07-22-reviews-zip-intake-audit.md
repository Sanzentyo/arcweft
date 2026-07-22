# `docs/reviews` ZIP intake audit — 2026-07-22

## Why this audit exists

Several returned contract ZIPs had been placed directly under `docs/reviews/`
without a package-specific intake record. Work resumed from the active dirty
tree and the most recently named package without first reconciling that
directory, so those packages were not surfaced promptly. This was a workflow
failure, not evidence that the files were absent.

This audit is the repository-visible intake ledger. The outer archive hashes
and every archive's internal manifest/hash list were verified. No package was
treated as implementation-ready merely because its filename said
`final-contract`.

## Inventory

| Package | SHA-256 | Intake state | Next action |
| --- | --- | --- | --- |
| [`arcweft-lang-01.1.1.1-final-contract-c957a61e4a0b.zip`](../reviews/packages/arcweft-lang-01.1.1.1-final-contract-c957a61e4a0b.zip) | `024A13F98A7F46764A79CCBBD8F7ED317C30A4F5E24332E6AE1E2FF7B2A7E18C` | Active implementation | Complete exact prefix/postfix propagation source evidence, typed boundary checking, tooling projection, full contract matrix, Tier 2, and structural audit. |
| [`arcweft-lang-01.1.1.2-project-nominal-type-resolution-production-reconciliation-final-contract.zip`](../reviews/packages/arcweft-lang-01.1.1.2-project-nominal-type-resolution-production-reconciliation-final-contract.zip) | `FF695EADEF1A4C833D86F53CA5E9010C7DF3D3643418109980B0E9F1D6CFE1AB` | Implemented substrate; final adapter publication cut remains | The shared resolver and entity-family projection are implemented. Complete Lang-01.1.1.2.2 through the same accepted world. |
| [`lang-01.1.1.2.1-entity-family-applied-type-projection-final-contract.zip`](../reviews/packages/lang-01.1.1.2.1-entity-family-applied-type-projection-final-contract.zip) | `FDAFDCA7B5D6682504A901274EB05C7B74C19816063C9A959C69DE0157A01906` | Received, internally verified, and implemented in the active cut | `Ref<EntityFamily>` now uses the same checked contextual projection as `Speaker` and `SpeakerPreset`; focused resolver, callable, entry, persistence, fixture, and LSP tests pass. |
| [`2026-07-22-lang-01.1.1.2.2-final-contract-main-4fd6331d.zip`](../reviews/packages/2026-07-22-lang-01.1.1.2.2-final-contract-main-4fd6331d.zip) | `4518DC6D81A6435B7514CE7BDCD3887DF87A857A8BC9EAA5DF14DF62DBD59C95` | Received; internally verified and implementation-ready | Its 197-row matrix replaces string nominal carriers with accepted Rust-package/environment owner-qualified identities, projects publications through the accepted world, and migrates Rust ADT metadata atomically. Implement it after the active entity-family cut is validated to avoid overlapping resolver edits. |
| [`Lang-01.3.1.2.1-typed-stream-runtime-wire-contract-correction-final-contract.zip`](../reviews/packages/Lang-01.3.1.2.1-typed-stream-runtime-wire-contract-correction-final-contract.zip) | `66809A1280A507F69BB78D9DF3BF7AF227A91CD68B86CF8771CBF9EE20AA856A` | Superseded by Lang-01.3.1.2.2 | Do not implement its last-group-only curried projection. |
| [`arcweft-lang-01.3.1.2.2-curried-external-stream-final-contract.zip`](../reviews/packages/arcweft-lang-01.3.1.2.2-curried-external-stream-final-contract.zip) | `D1BD7FB5301509CA88BE7C9D3662942CA88472D11143499C0C3067D626DF9418` | Received; internally verified and implementation-ready after Cut 0 re-intake | Its 168-row matrix preserves every curried argument group through signature, product, runtime, ABI 2, codec 8, host, save, and hot-reload boundaries. Re-read current callable/sema/runtime paths after the active Try cut because the package pins `5821a3ca479b5b89ca6ede997b9cf4f42f6280a6`. |
| [`arcweft-lang-01.4.2-resource-extension-manifest-wire-contract-final-contract.zip`](../reviews/packages/arcweft-lang-01.4.2-resource-extension-manifest-wire-contract-final-contract.zip) | `01F308C08FE818E247E41E94278EB2D69D5A12AC597794A9109390840C0D95D3` | Superseded by the repository-grounded Lang-01.4.2.1 redelivery | Do not implement the unvalidated package. |
| [`arcweft-lang-01.4.2.1-resource-extension-manifest-repository-reconciliation-final-contract-main-5821a3ca.zip`](../reviews/packages/arcweft-lang-01.4.2.1-resource-extension-manifest-repository-reconciliation-final-contract-main-5821a3ca.zip) | `7DC625446304FE2FFA73027AA518853DF56982BD486347D6F81142D8EAF6ACC0` | Received; internally verified and implementation-ready | After Lang-01.1.1.2 and the existing resource registry/retained-identity substrate, implement the Sans-I/O strict manifest crate, single decoder/encoder, atomic registry publication, AWFB section 22, and runtime digest validation. Public `res` authority switching remains a later cohort. |
| [`arcweft-lang-01.5.1.2-typed-content-root-admission-final-contract.zip`](../reviews/packages/arcweft-lang-01.5.1.2-typed-content-root-admission-final-contract.zip) | `CA72FD70C657A11B7BECDB331D131177B6DEFD6094D034BBECFC3AF1A232E1C0` | Safe binary-topology subset implemented; root-family wording superseded | Retain only the already verified substrate and use Lang-01.5.1.2.1 for the closed family switch. |
| [`arcweft-lang-01.5.1.2.1-content-root-family-source-elimination-reconciliation-final-contract-main-5821a3ca.zip`](../reviews/packages/arcweft-lang-01.5.1.2.1-content-root-family-source-elimination-reconciliation-final-contract-main-5821a3ca.zip) | `C91C2C635C13EB68D46C5D0D4A6F3ECDE0192546BE32175EB2A24FAA54FDE699` | Received; internally verified and implementation-ready after dependency re-intake | Its 160-row matrix closes roots to Character, Flow, View, Action, Activity, Asset, Signal, Metric, Layer, and exact configured resources; `Source`, callable, Stream-return, and name heuristics remain excluded. |

## Dependency order

1. Validate the implemented Lang-01.1.1.2.1 entity-family projection, then
   apply the returned Lang-01.1.1.2.2 package. The latter projects adapter/Rust
   callable publications through the same accepted world instead of equating
   `Named` and accepted nominal identities. It must not introduce a second
   resolver or alter Lang-01.1.1.2.1 work, poison, or source accounting.
2. Re-intake and apply the returned Lang-01.3.1.2.2 correction before Stream
   runtime/AWBC/host/save work. Its pinned baseline predates the active
   propagation and shared-callable changes, so the Cut 0 owner comparison is
   mandatory.
3. Re-intake and apply Lang-01.5.1.2.1 after the ordinary-function/generator and
   external-Stream Source-elimination prerequisites, before closed content-root
   admission and remaining source-content deletion.
4. Apply the repository-validated Lang-01.4.2.1 after Lang-01.1.1.2 and the
   resource registry/retained-identity substrate, before adding the public
   extension-manifest decoder, canonical encoder, or AWFB publication.

All named follow-up requests in this intake have returned. There is currently
no design request to send. The implementation order is the active
Lang-01.1.1.2.1 validation followed by Lang-01.1.1.2.2. The returned adapter
publication, Stream, content-root, and resource-manifest packages are
production work and must not be sent again. A new request may be named only
after a concrete implementation ambiguity has been recorded in an independently
throwable file under `docs/reviews/requests/`.

## Ongoing intake rule

At task start and each reviewable push cut point, compare every recursive
`docs/reviews/**/*.zip` archive against package-specific intake/completion notes
and recorded SHA-256 values. A ZIP directly under `docs/reviews/` is an inbox
item: inspect it before selecting the next production slice, then move it to
`docs/reviews/packages/`. Classify every archive as implementation-ready,
active, blocked by a named request, superseded/duplicate, or invalid as
delivered. Do not silently leave a returned archive outside the task dependency
graph.
