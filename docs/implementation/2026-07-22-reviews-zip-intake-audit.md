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
| `arcweft-lang-01.1.1.2-project-nominal-type-resolution-production-reconciliation-final-contract.zip` | `FF695EADEF1A4C833D86F53CA5E9010C7DF3D3643418109980B0E9F1D6CFE1AB` | Active implementation | Finish the single sema nominal resolver, checker/tooling migration, Tier 2, and structural audit. |
| `Lang-01.3.1.2.1-typed-stream-runtime-wire-contract-correction-final-contract.zip` | `66809A1280A507F69BB78D9DF3BF7AF227A91CD68B86CF8771CBF9EE20AA856A` | Received, blocked by a concrete contract defect | Obtain [Lang-01.3.1.2.2](../reviews/requests/2026-07-22-lang-01.3.1.2.2-curried-external-stream-runtime-argument-projection-correction.md); the returned projection loses earlier curried argument groups. |
| `arcweft-lang-01.4.2-resource-extension-manifest-wire-contract-final-contract.zip` | `01F308C08FE818E247E41E94278EB2D69D5A12AC597794A9109390840C0D95D3` | Received, not implementation-ready | The package records no checkout, pinned HEAD, or executed command and sets `contract_agent_validated=false`. Obtain the repository-grounded [Lang-01.4.2.1 redelivery](../reviews/requests/2026-07-22-lang-01.4.2.1-resource-extension-manifest-repository-reconciliation.md). |
| `arcweft-lang-01.5.1.2-typed-content-root-admission-final-contract.zip` | `CA72FD70C657A11B7BECDB331D131177B6DEFD6094D034BBECFC3AF1A232E1C0` | Safe binary-topology subset implemented; root-family portion blocked | Obtain [Lang-01.5.1.2.1](../reviews/requests/2026-07-22-lang-01.5.1.2.1-content-root-family-source-elimination-reconciliation.md), which removes the already deleted `Source` top-level family from the content-root contract. |

## Dependency order

1. Finish Lang-01.1.1.2, which is already changing the shared type and symbol
   substrate.
2. Apply the returned Lang-01.3.1.2.2 correction before Stream runtime/AWBC/
   host/save work.
3. Apply Lang-01.5.1.2.1 before closed content-root admission and remaining
   source-content deletion. Independent safe binary-topology work may continue.
4. Apply a repository-validated Lang-01.4.2.1 before adding the public extension
   manifest decoder or canonical encoder.

The three follow-up requests are independent design tasks and may be sent in
parallel. Their production implementations remain ordered by the shared
substrate above.

## Ongoing intake rule

At each reviewable push cut point, compare every `docs/reviews/*.zip` archive
against package-specific intake/completion notes and recorded SHA-256 values.
Inspect any unrecorded or changed archive before selecting the next production
slice. Classify it as implementation-ready, active, blocked by a named request,
superseded/duplicate, or invalid as delivered. Do not silently leave a returned
archive outside the task dependency graph.
