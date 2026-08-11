# Repository evidence and verification scope

## Selected repository state

- Repository: `Sanzentyo/arcweft`
- Latest default-branch commit: `5018912852a45e96f48735767021bf858ffcd493`
- Commit message: `Delete obsolete HIR facade and assertion carrier`
- Connector comparison `5018912852a45e96f48735767021bf858ffcd493...main`: identical, zero commits ahead/behind
- Parent ZIP baseline `ac9ce44fe9423efd85280e26832dd30c725b3b34` is 22 commits behind this main

## Current owners inspected

| Path | Blob SHA | Evidence used |
|---|---|---|
| `AGENTS.md` | `e91f99213dde67953beda6aa078c370a8dc4541d` | direct replacement, inherent owner behavior, no wrappers/source gates, ZIP workflow |
| `crates/arcweft-lang-hir/src/identity.rs` | `b198ecc728b3e586b3e1ea7b7b89ca1f1c0a5d1b` | typed IDs, current raw SyntheticKey owner, SyntheticRole, HirLimit maxima |
| `crates/arcweft-source/src/document.rs` | `e1b1a545d28f62704a7e7b517620b85b6ffe73b6` | revision-bound source identity and 8,388,608 registration-source maximum |
| `crates/arcweft-lang-syntax/src/ast/pattern.rs` | `d610e071d43d378db500181f091528ff6a6a639f` | `Pattern::Variant { path: Option<_>, ... }` and pathless syntax |
| `crates/arcweft-lang-syntax/src/expr/numeric.rs` | `e7a5b88c7b20aae8ae08ea24f86b61a717ecf15d` | raw current integer/sequence substrate and u128 overflow limitation |
| `docs/implementation/2026-07-27-proof-01-1-1-4-1-ready-claim-redelivery-intake.md` | `af33fd302a02b85c81bf0647d60201b0fbb597c9` | five adjudicated contradictions and blocked public switch |
| `docs/implementation/2026-07-27-proof-obsolete-reference-hir-deletion.md` | `1f05223445dfbf7063c91101e07a70fff0ff6461` | final type-region owner deliberately awaits this correction |
| follow-up request | `30b8ca02ece5187545219f046151a56683871544` | exact required decisions/output |

The full latest `AGENTS.md` and complete Rust skill were read. No production file was edited.

## Parent ZIP validation

- local bytes: 64,523
- SHA-256: `61e2ee166bff158fe83dcf1484b7b9380a81f60d865377503400d27d238cc708`
- members: 20
- all 19 non-self parent manifest entries matched byte length and SHA-256
- all members were extracted and searched for the adjudicated source/Duration/path/limit/Synthetic contradictions

## Verification boundary

This return validates design consistency, package integrity, exact request copies, complete matrices, traceability, and current repository owner shapes. It does not claim that production Rust compiles against these not-yet-implemented public types. Runtime/workspace tests are therefore not applicable to this design-only archive. The implementation contract explicitly requires them at the public switch.
