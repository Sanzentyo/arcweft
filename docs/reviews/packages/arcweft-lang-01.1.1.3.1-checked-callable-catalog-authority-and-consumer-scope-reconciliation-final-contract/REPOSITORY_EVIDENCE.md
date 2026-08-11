# Repository evidence

## 1. Inspection identity and method

Repository access used the private GitHub connector for `Sanzentyo/arcweft` and local byte-level inspection for the supplied request/parent ZIP. No unpushed working-copy/WIP shape was treated as design authority.

Current pushed repository identity:

| Item | Value |
|---|---|
| branch/ref inspected | pushed `main` |
| Git commit | `b305c698b22a01b30f1d7e68be6d925e6e3a2875` |
| subject | `Adjudicate returned Lang and Stream contracts` |
| root `AGENTS.md` blob | `e91f99213dde67953beda6aa078c370a8dc4541d` |
| correction request blob | `24c21af78db99939904f2a08cd4dbf2a52978a1a` |
| uploaded correction request SHA-256 | `77CCD51D4D382A5CF82804E6E9476F9E3D506A81ACF3F0920B94A13EE4B8DF08` |
| supplied Rust skill SHA-256 | `1A28F552ADF5EFDE95205BEE8D56590AEB82346C48EBDF3FDBBAFF5DECA33665` |
| supplied Arcweft premise SHA-256 | `CFA897A0AD93DEB92FD454079DF0A789EDBBD40D85C8377324DA703C8AEFE0A1` |

The GitHub connector exposes the pushed Git commit but not Jujutsu operation/change metadata. Jujutsu change identity is therefore recorded as:

```text
CURRENT_PUSHED_MAIN_JUJUTSU_CHANGE_ID = NOT_EXPORTED_BY_PUSH_SURFACE
```

No value is inferred or fabricated. An implementation checkout resolves the matching local change with:

```bash
jj log -r 'git_commit(b305c698b22a01b30f1d7e68be6d925e6e3a2875)' \
  --no-graph -T 'change_id ++ "\n"'
```

That command is evidence bookkeeping, not an open design choice. The parent package separately recorded its own inspected baseline Git `0b7e095f4193b9f7fbbc95cc350a626a8a63640a` and local Jujutsu change `pxulxlkmwqztnrwykmtowvvlkruusooy`; those are parent evidence, not the current main identity.

## 2. Parent archive verification

Supplied parent:

`arcweft-lang-01.1.1.3-effect-trait-contract-and-dynamic-dispatch-production-reconciliation-final-contract.zip`

| Check | Result |
|---|---|
| byte length | 46,663 |
| SHA-256 | `4FD834564C458639CD4EBE46615E4EC79C54F91D686439AAAACCC7F2B3714B5E` |
| ZIP integrity | pass |
| member count | 13 |
| non-manifest rows verified | 12/12 |
| SHA-256 mismatches | 0 |
| length mismatches | 0 |
| missing members | 0 |
| `OPEN_QUESTIONS.md` | exactly `none` |
| parent final status | `READY_FOR_IMPLEMENTATION` (catalog authority rejected by repository intake) |

All parent documents were read. The mechanically valid parent was not accepted literally because its `CheckedCallableFacts` owns copied signature/source/access and its fixed row is copied into checked facts without naming the accepted `RegisteredCallableCatalog` / `CallableRecord` relationship.

## 3. Repository policy evidence

The latest `AGENTS.md` was read in full. Relevant binding rules:

- preserve syntax -> HIR -> sema -> compiler/runtime/tooling dependency direction;
- represent semantic ownership with typed identities and owned APIs;
- implement family behavior on the original enum/inherent impl rather than helpers/extension traits;
- perform final-model direct replacement rather than compatibility aliases, shims, dual readers, or source gates;
- use deletion-driven migration and repair compile fallout toward the final owner;
- run focused tests, workspace all-target/all-feature check, strict Clippy, `just test-workspace`, applicable Tier 2, and `cargo +nightly -Zscript tools/structure-audit.rs --root .`;
- do not claim semantic removal by source-text searching.

The supplied Rust skill was read in full. Relevant rules incorporated here:

- model identities/invariants with dedicated types and private fields;
- keep public API minimal and deliberate;
- avoid `unsafe`, unstable features, new macros, and lint suppression as design shortcuts;
- prefer standard library/iterators and ownership clarity;
- run Clippy at checkpoints and final formatting.

## 4. Request and intake evidence

| Repository file | Blob SHA | Evidence used |
|---|---|---|
| `docs/reviews/requests/2026-07-25-lang-01.1.1.3.1-checked-callable-catalog-authority-and-consumer-scope-reconciliation.md` | `24c21af78db99939904f2a08cd4dbf2a52978a1a` | requires one record authority, exact Arc/order/consumer/deletion decisions, one ZIP plus SHA-256 |
| `docs/reviews/requests/2026-07-24-lang-01.1.1.3-effect-trait-contract-and-dynamic-dispatch-production-reconciliation.md` | `21b2e84dea49cf8886623915aa9fed79b8662164` | parent scope forbids redesign/copy, fixes effect/E017/diagnostic requirements |
| `docs/implementation/2026-07-25-lang-01-1-1-3-effect-trait-intake.md` | `74d005fa5bdadac0aac1aa767e59f89c74699f80` | accepts parent semantics/deletions but blocks copied catalog authority |
| `docs/implementation/2026-07-21-aw-ah-009-3-semantic-selection-and-resource-accounting.md` | `93e2f1e4bf3b89019191c5919c3263e293502db7` | shared resolver/catalog, semantic selection, resource/work accounting must remain authoritative |

The current main intake explicitly identifies accepted `Arc<CallableRecord>` signature lookup/semantic digest as stable substrate and holds revision-bound public identity/project graph/Agent projection until this correction. The final design follows that boundary.

## 5. Accepted callable catalog evidence

| Repository file | Blob SHA | Current evidence | Contract consequence |
|---|---|---|---|
| `crates/arcweft-lang-sema/src/callable/catalog.rs` | `643eca8cc31b474f1ae5b6164a4cecea15cacdb0` | immutable `CallableRecord`, `Arc<CallableSignatureSchema>`, docs/source/provider/publication/order, project/environment indexes, catalog digest | retain and extend this owner; checked facts retain exact Arc |
| `crates/arcweft-lang-sema/src/callable/builder.rs` | `301c8f8cdc877f761097e04bb39bda95fe927a2d` | fail-closed crate-private builder creates project record Arcs, consumes environment publications, finishes once | extend in place for method/standard records; no second builder/catalog authority |
| `crates/arcweft-lang-sema/src/callable/schema.rs` | `776142ad69e2de6a47bb5c180739d8c679bd20d7` | shared signature, source, documentation, effect schema owners | delete checked/project-index copies; change project effect schema to ID-only |
| `crates/arcweft-lang-sema/src/callable/publication.rs` | `6fa7c0eeb1e6267d84c64ac839fb11d36d374b75` | environment publication owns validated schema/docs/source/Rust records and digest before catalog freeze | move once into exact final record; no publication DTO retained by checked layer |
| `crates/arcweft-lang-sema/src/callable/identity.rs` | `6a97f5e0ac850c4035a2c13d1a8d43c95aff89a7` | existing project/environment/trait candidate IDs and lookup bindings | delete `TraitCallableId`, add typed project/detached/environment/standard checked mapping |
| `crates/arcweft-lang-hir/src/symbol/identity.rs` | `6ac5696f8f5c1296dd64f7fcdac7d048b3c7227f` | `CallableDeclarationId`, `CallableDeclarationOwner`, project world/revision, project symbol visibility | add structural trait/impl keys and owner behavior in original module |


## 6. Registration/world evidence

| Repository file | Blob SHA | Current evidence | Contract consequence |
|---|---|---|---|
| `crates/arcweft-lang-sema/src/registration/model.rs` | `099a247d79ad07549bde3e44b5840b132d7e27c8` | `RegisteredSemanticWorld` / `RegisteredTypeCheckEnv` retain Arcs and revision/digest facts | expose/validate exact accepted catalog Arc |
| `crates/arcweft-lang-sema/src/registration/registrar.rs` | `09830a1526cb185ba634af51e865b9aff0bdc562` | registration validates project/nominal inputs, builds catalog once, then publishes world | accepted catalog freeze must precede checked shells |
| `crates/arcweft-compiler/src/project.rs` | `eeef9b100b506deccac5f1732fbfdb3d4ef07a6e` | compiler builds `Arc<HirProject>`, registers world, typechecks, lowers, and stores report/world | report checked catalog becomes the only compiler input; no second catalog argument |

## 7. Checker and trait evidence

| Repository file | Blob SHA | Current evidence | Contract consequence |
|---|---|---|---|
| `crates/arcweft-lang-sema/src/checker.rs` | `a12920c66dd5115f2d05c6b567e3704753a047d2` | report currently exposes separate effect/callable execution data; checker has string/local effect maps | replace with one checked catalog Arc and typed IDs |
| `crates/arcweft-lang-sema/src/checker/module.rs` | `ce46dac14021a2c8d08dcade7898dd7a15f6fe1f` | trait collection/function binding/body check/effect finish/report construction order | insert private pending shells and freeze before report |
| `crates/arcweft-lang-sema/src/traits.rs` | `056e66e39d76f2c5739777177e841e72ca90d77a` | `TraitMethodRequirement`/`TraitMethodImpl` copy signatures/param groups/return types; resolution clones methods; local handles/string indexes | reduce to checked IDs/body/witness metadata and query exact record |

## 8. Project index and relation evidence

| Repository file | Blob SHA | Current evidence | Contract consequence |
|---|---|---|---|
| `crates/arcweft-lang-sema/src/project_index.rs` | `ad1e4445d51975fc38c2bfa99beb88ac26d60ef8` | `ProjectCallableSymbol` copies `FunctionSignature`/source/hash; callable graph uses `QualifiedName`; index scans names; Agent callables copy signature/effects | retain checked catalog Arc, structural map key + checked ID, remove copies/name lookup |
| `crates/arcweft-lang-sema/src/project_index/entities.rs` | `afb39085ad470534d538cf1d652a8b0ddfe98f70` | functions/views are reprojected from HIR/typecheck facts into copied symbols | construct ID-only symbols from catalog/report |
| `crates/arcweft-lang-sema/src/project_index/relations.rs` | `7d49a7e82f5ac54deb8fdf6ad25b4736979ab365` | callable relations are created from raw HIR qualified names | consume typed checked call-target facts |

## 9. Compiler, Agent, persistent, and LSP evidence

| Repository file | Blob SHA | Current evidence | Contract consequence |
|---|---|---|---|
| `crates/arcweft-compiler/src/agent_project.rs` | `376553e3c4994f425b886ba65758197f118772d0` | callable symbol IDs are formatted from owner/name and graph callable refs fall back through names | structural digest IDs, no partial/name fallback |
| `crates/arcweft-compiler/src/persistent.rs` | `b1c5a3d55c3b8346108578a6b9ba4a47cfaffb52` | `InterfaceSummary` rebuilds callable signatures from HIR and fabricates declaration strings | use project index/catalog structural identity and record digests; delete HIR fallback |
| `crates/arcweft-project/src/persistent_object/payload.rs` | `c967837e74eaf7e99aa8b70bd70edcf0c7dd7e4e` | `PublicSymbolObject { name, kind, signature_digest }`; private unreleased object schema | direct typed schema replacement/version bump; no compatibility reader |
| `crates/arcweft-lsp/src/profiles/state.rs` | `892ca8450bd494379b9a8c1532808a2ae0c0f296` | accepted candidate validates exact HIR Arc, world/revision/source/overlay identities and publishes generation atomically | add checked/registered catalog Arc and generation validation |


## 10. Key current-code observations

### 10.1 Existing accepted catalog already has the right ownership shape

`RegisteredCallableCatalogBuilder` creates `Arc<CallableRecord>` values, indexes them by project/environment identity, and freezes once. The final record already owns schema, documentation, source, provider, Rust/publication provenance, and declaration order. This directly supports retention model 1.

### 10.2 Parent literal shape would duplicate authority

The parent checked facts add owned signature/source/access and an owned fixed row. No exact reference to the accepted record is specified. Literal implementation would permit accepted and checked values to diverge and leave consumers with two valid-looking readers.

### 10.3 Current project/Agent/persistent consumers already demonstrate the required correction scope

- project symbols copy signature/source;
- project relations use qualified names;
- Agent IDs use owner/name strings;
- persistent interfaces reconstruct callable signatures from HIR.

A correction limited only to `CheckedCallableFacts` would therefore be incomplete. The consumer migration in this package is required to make the selected owner true end to end.

### 10.4 Existing LSP publication is a compatible generation boundary

The accepted LSP profile already validates exact compiled HIR Arc, registered world, project symbol revision, source revision, and overlays, then publishes a monotonic generation. Adding checked catalog Arc/generation validation extends that existing owner; it does not create another scheduler/cache authority.

## 11. Validation boundary

Performed:

- full supplied request/Rust skill reading;
- full latest `AGENTS.md` reading;
- exact pushed main/commit identification;
- requested repository file inspection plus responsibility siblings/consumers;
- parent ZIP byte/hash/member/manifest/open-question verification;
- design consistency and requirement traceability review;
- output ZIP manifest/hash/integrity verification.

Not performed:

- production code edits;
- local Cargo/Clippy/test/Tier 2/structure-audit execution against the repository;
- Jujutsu metadata query in a local checkout.

Those are implementation validation gates, not design decisions. Their exact commands and expected behavior are mandatory in `IMPLEMENTATION_ORDER.md` and `TEST_MATRIX.md`.
