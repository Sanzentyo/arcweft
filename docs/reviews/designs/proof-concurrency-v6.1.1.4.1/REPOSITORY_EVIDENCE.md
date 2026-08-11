# Repository evidence and verification scope

## Baseline

- Repository: `Sanzentyo/arcweft`
- GitHub main selected immediately before contract construction: `ac9ce44fe9423efd85280e26832dd30c725b3b34`
- Commit message observed: `Delete unused public syntax facades`
- AGENTS.md blob: `e91f99213dde67953beda6aa078c370a8dc4541d`
- Correction request blob: `24069d99212d33c0721df296e3a0ef74378ba208`
- Primary request blob: `6e23b72f546f3de9cd59c3d16e05a7fe12d084ed`

## Exact text and owner code inspected

| Repository path | Blob SHA | Use |
|---|---|---|
| `AGENTS.md` | `e91f99213dde67953beda6aa078c370a8dc4541d` | architecture, source-gate, compatibility and ZIP workflow |
| correction request | `24069d99212d33c0721df296e3a0ef74378ba208` | required corrections/output |
| primary request | `6e23b72f546f3de9cd59c3d16e05a7fe12d084ed` | base inventory/matrices/tests |
| `docs/implementation/2026-07-26-proof-01.1.1.4-return-intake.md` | `b857bee90e997e480f67a4fefe309d8f8747364c` | rejected-return defects |
| `docs/implementation/2026-07-27-proof-01-1-1-4-1-not-ready-redelivery-intake.md` | `24c02ecee9f0bc41635c809451badff56d6e216c` | failed redelivery status |
| `docs/implementation/2026-07-26-proof-hir-local-schema-decisions.md` | `42e9a3ea5ad0f6b25e4445b86b1d901dc0aa337d` | fixed adjacent member/IfLet/unsafe insertion owners |
| `docs/implementation/2026-07-25-proof-stage-3-deletion-driven-authority-switch.md` | `6714d8bc34ba665073c3c817638646287c7c076f` | deletion-driven consumer switch |
| `docs/implementation/2026-07-20-aw-ah-009-4-2-private-cut-2.md` | `4c6a6fa8113c924080c23b06ff17c2272c75a7a0` | accepted private Dialogue carrier evidence |
| associated-capacity blocker note | `eb19d1dcd4e59cf91d925d2d1d85a9a3b338228c` | accepted receiver route and precedence |
| `crates/arcweft-lang-hir/src/identity.rs` | `b198ecc728b3e586b3e1ea7b7b89ca1f1c0a5d1b` | qualified IDs, limits, SyntheticRole |
| `crates/arcweft-lang-hir/src/dialogue_application.rs` | `2d96c4744514c20dd4ef490ec39248451ce37af8` | exact ID/coordinate/line-plan/candidate/source carriers |
| `crates/arcweft-lang-syntax/src/expr.rs` | `31addf634e6290ffe56008843ffc73f1def2c4c5` | expression inputs plus exact Binary/Unary/ComputationBlock enums |
| `crates/arcweft-lang-syntax/src/expr/numeric.rs` | `16e56e968d9a3e04c4b81ae1080ac5e1e8bd6e98` | compact sequence/radix/suffix |
| `crates/arcweft-lang-syntax/src/reference.rs` | `166a9278d867b9ab91333a90c792238eb716aacb` | named/elided type regions |
| `crates/arcweft-lang-syntax/src/types.rs` | `552fa567a96c65758f12dcea91a2d4c8387fbb30` | typed type receiver tree after detached facade deletion |
| `crates/arcweft-lang-syntax/src/ast/module_path.rs` | `bd0d3fe0619278523d5ccea15a840d98a269056b` | four path roots and super resolution |
| `crates/arcweft-lang-syntax/src/ast/symbol_path.rs` | `d17f6d1f795a7b6d3726b74117a4a883d331e429` | external-capable project segments/aliases |
| `crates/arcweft-lang-sema/src/callable/resolver.rs` | `81aef22163d5ccb4e6250fb99101d991e5188d31` | one resolver and AssociatedType callee |
| `crates/arcweft-lang-sema/src/nominal/model.rs` | `f1efa54635aa7fd2b2f0592ee92372a3d3e76022` | full nominal receiver product |
| `crates/arcweft-lang-syntax/src/ast/flow.rs` | `c8406543ddccaa3927bbd613b75c9e26cc2dc299` | Thread name/modifier/FlowItem body |
| `crates/arcweft-lang-syntax/src/ast/dialogue.rs` | `bd344a3e4e3847cb68f56b1a5c20c0410e9c28b` | exhaustive content tokens |
| `crates/arcweft-lang-syntax/src/ast/dialogue/rich_text.rs` | `c168bc31af025db3906b0711b9bd4aae3f24150f` | tag payload/argument/recovery families |
| `crates/arcweft-lang-syntax/src/text/rich_text_tag.rs` | `688539d34b646243d9e4c1ed09e528176293c6bf` | exact RichText limits |
| `crates/arcweft-presentation/src/rich_text.rs` | `5436e923380d1363363c640ab11cb1e966f64ff5` | current builtin Fx inventory |
| `crates/arcweft-presentation/src/rich_text/authoring_schema/direct_style.rs` | `ce7aabb072f030913a1bd4fe73434a1ff9fc6410` | 8 direct-style variants and property schema |
| `crates/arcweft-presentation/src/rich_text/authoring_schema/style.rs` | `93c522addd8cb208a5ebc9f47b35fbca787a0095` | 5 style selectors |
| `crates/arcweft-presentation/src/rich_text/authoring_schema/layout.rs` | `33771896742699b3d79c4cf803a89bd3a9bcad89` | 7 layout selectors |
| `crates/arcweft-presentation/src/rich_text/authoring_schema/transform.rs` | `8fbc4fbc8d84f7b5c04e1837fb1b55a37e0c01e1` | 4 transform selectors |
| `crates/arcweft-presentation/src/rich_text/authoring_schema/object.rs` | `5b9365a2de2f1366ce71848fe105e606077c4629` | object selector and metadata properties |
| `crates/arcweft-lang-syntax/src/ast/pattern.rs` | `d610e071d43d378db500181f091528ff6a6a639f` | pattern families |
| `docs/01-language/literals-and-primitives.md` | `220a58fe2898ff674a05583b1395a0f0cd27098f` | numeric/Duration/unit semantics |
| `docs/02-runtime/control-flow-runtime.md` | `04dba4047a5d965eba4b5783323bd5d23cae8a5c` | scopes/control flow |
| `crates/arcweft-core/src/time.rs` | `56de776c264e84689f2a4837d92b23c7121c564e` | exact `LogicalDuration { nanos: u64 }` runtime boundary |
| `docs/implementation/2026-07-27-proof-fragment-detached-payload-deletion.md` | `c6008bb087899f6ba1e1ac0054e69942fc0a84ec` | latest detached expression/item payload deletion |
| `docs/implementation/2026-07-27-proof-unused-public-syntax-facade-deletion.md` | `5bf36b706c9ef0fd469b02973606a8fd308e6ff4` | latest `cst::path`/raw where-clause facade deletion |

## Predecessor archive verification depth

The seven repository-retained predecessor ZIP paths were inspected through the GitHub connector, their repository presence/blob identity was confirmed, and their normative package SHA-256 identities were cross-checked against the primary request and repository intake ledger. For AW-AH-009.4.2, the exact `TYPED_HIR_OWNERSHIP.md` member was reconstructed from the GitHub package blob and mechanically checked at 10,935 bytes with CRC-32 `90541852`; that extraction supplied the exact outer record and coordinate/candidate invariants preserved here. The other predecessor binaries were not all freshly extracted member-by-member in the local runtime. For those, every result-changing clause used here was independently cross-checked against its GitHub-visible request, implementation intake/evidence, current typed owner code, and accepted package identity. The GitHub-only RichText dispatch clause expressly permits the repository evidence route and does not require the external binary.

This verification boundary does not leave an implementation choice: the final schemas, invariants, matrices, and tests in this archive are complete. It records exactly what was and was not mechanically revalidated rather than overstating evidence.

## Request-copy verification

`CORRECTION_REQUEST_COPY.md` was reconstructed from the exact GitHub blob and matched blob identity during construction. `PRIMARY_REQUEST_COPY.md` contains the complete 392-line request body and was line-compared against the GitHub blob; repository blob identity is recorded above. The package manifest records the actual delivered bytes.

## Production state

No production file was edited. The package contains no Rust source file, Cargo manifest, patch, overlay, branch metadata, or PR metadata.
