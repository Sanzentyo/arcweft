# Repository evidence

## 1. Inspected baseline

- repository: `Sanzentyo/arcweft` (private, accessed through the GitHub connector);
- latest inspected `main`: `3acc9cfec034d00cee173e41cbfb37cd46115c50` — `Implement direct Try propagation and entity-family Ref resolution`;
- request blob SHA: `1df6055299771e6af0705d4a1014b5af84cf2821`;
- latest `AGENTS.md` blob SHA: `e91f99213dde67953beda6aa078c370a8dc4541d`;
- local Rust skill SHA-256: `1a28f552adf5efde95205bee8d56590aeb82346c48ebdf3fdbbaff5deca33665`;
- local Arcweft premise SHA-256: `cfa897a0ad93deb92fd454079df0a789edbbd40d85c8377324da703c8aefe0a1`.

The Rust skill was read in full (56 lines). Its applicable rules are narrow/owned APIs, dedicated types/newtypes, careful visibility, no unsafe/unstable/macro expansion without need, and implementation validation with Clippy and formatting. This task writes no Rust or production code.

## 2. Repository policy applied

Current `AGENTS.md` requires:

- layer direction `syntax -> hir -> sema -> runtime-plan/verify -> tooling`;
- typed APIs over stringly APIs;
- inherent behavior on Arcweft-owned enum/boundary types instead of ad hoc matches, extension traits, or string wrappers;
- direct replacement of unreleased provisional parser/compiler models, without deprecated variants, dual readers, aliases, or shims;
- ordinary current-grammar rejection of removed syntax and no permanent removed-spelling diagnostic;
- public-contract changes to receive structural/dependency audit and direct behavioral/compile-fail evidence;
- no source gates that scan checked-in text or file placement.

This contract follows those rules explicitly.

## 3. Current retained-family implementation evidence

At the pinned commit:

- `arcweft-id` owns eight `RetainedIdentityFamily` variants and identifies Asset as catalog-discovered rather than authored.
- The private lossless grammar owns dedicated item/parser modules for Character, View, Action, Activity, Signal, Metric, and Layer.
- Shared retained-header parsing enforces optional absolute family-correct ID, required ordinary name, relative-ID rejection, wrong-family typed recovery, and exact diagnostics.
- `SyntaxKind` already contains the seven item kinds and their typed body/member inventory; it contains no Asset declaration item.
- Direct private tests cover canonical, malformed, recovery, sibling preservation, shared header, mixed inventory, losslessness, and inclusive limits.
- The implementation record marks private cuts 0–7 complete and public AST/HIR/downstream cuts pending.
- The duplicate View callable projection has already been removed; the generic public `EntityDeclItem` and raw signature tail remain as pending deletion inventory.

Primary paths inspected:

```text
crates/arcweft-id/src/lib.rs
crates/arcweft-lang-syntax/src/grammar/kinds.rs
crates/arcweft-lang-syntax/src/grammar/roles.rs
crates/arcweft-lang-syntax/src/incremental/limits.rs
crates/arcweft-lang-syntax/src/parser/declaration.rs
crates/arcweft-lang-syntax/src/parser/character_grammar.rs
crates/arcweft-lang-syntax/src/parser/view_grammar.rs
crates/arcweft-lang-syntax/src/parser/action_grammar.rs
crates/arcweft-lang-syntax/src/parser/activity_grammar.rs
crates/arcweft-lang-syntax/src/parser/signal_grammar.rs
crates/arcweft-lang-syntax/src/parser/metric_grammar.rs
crates/arcweft-lang-syntax/src/parser/layer_grammar.rs
crates/arcweft-lang-syntax/src/attachment/family.rs
crates/arcweft-lang-syntax/src/ast/items.rs
crates/arcweft-lang-hir/src/identity.rs
crates/arcweft-lang-hir/src/model.rs
crates/arcweft-lang-hir/src/lower.rs
crates/arcweft-lang-sema/src/project_index/entities.rs
crates/arcweft-compiler/src/image.rs
crates/arcweft-cli/src/app/bundle.rs
```

## 4. Asset evidence

Current bundle code already:

- enumerates normalized relative virtual files under the authored asset root;
- derives image asset IDs by removing the final extension, splitting `/`, lowercasing ASCII alphanumerics, mapping `_`/`-` to `_`, rejecting other characters, and prefixing `asset.`;
- keeps virtual-file bytes and format-specific image metadata in bundle ownership; and
- validates referenced image asset IDs against available bundle image assets.

The current derivation is a CLI-local helper. This contract relocates that domain rule to an owned typed identity API without changing the observable algorithm and generalizes project symbol admission to the catalog boundary rather than inventing source `asset`.

## 5. Current generic public/HIR debt

Current public syntax still groups unrelated entities in `EntityDeclItem`/`EntityDeclKind`, with raw signature/body fields and selected structured bodies. Current HIR still contains clone-based generic entity declarations. Sema/project/compiler/tooling call sites still match generic kinds or syntax values. These are migration/deletion inventory, not compatibility requirements.

The current HIR identity module already owns module/revision/snapshot IDs, typed item/expression/statement/type/pattern/scope/local/capture IDs, liveness errors, synthetic roles, and inclusive limits. The retained declaration contract extends that owner with parameter/member identity rather than creating a local identity system.

## 6. Sequence evidence

`docs/implementation/2026-07-20-proof-concurrency-v6-1-1-2-retained-global-identity-implementation.md` records:

- accepted archive filename equal to this request;
- historical archive SHA-256 `7be398ebe2cefa2daefa963c7c8c6efb0b2389bb015edf36e585fb8b770242b1`;
- 18 verified members and the 64-zero self-entry rule;
- 184 normative direct-test rows;
- `asset` catalog/reference family with no authored declaration;
- seven private grammar rows;
- pending atomic public AST, arena HIR/project, downstream, deletion, and full-validation cuts.

`docs/implementation/2026-07-21-proof-public-switch-readiness.md` records that public switching is dependency-sequenced, not design-incomplete: one attached syntax authority, one source-backed HIR entry, later private arena HIR, and one atomic public HIR/project switch. This archive preserves that order.

## 7. Git and Jujutsu identity scope

`GIT_COMMIT=3acc9cfec034d00cee173e41cbfb37cd46115c50` is the exact latest connector-visible `main` used for this reconciliation.

The GitHub connector does not expose the local Jujutsu change ID associated with the current Git tip. The full Jujutsu ID reported by the repository's retained-global-identity implementation intake is:

`xpzvlyvqvtvowssyxlpswsnpkwnspxqr`

It is recorded in sidecars as the exact repository-recorded sequence lineage and is not represented as an independently verified Jujutsu working-copy ID for `3acc9cfec034d00cee173e41cbfb37cd46115c50`. An implementation checkout must record its actual current `jj log -r @ -T change_id` / `jj status` identity in the implementation audit before production work.

This limitation is evidentiary only; it leaves no grammar, AST, HIR, ownership, recovery, or migration decision open.

## 8. Validation scope of this returned archive

Performed for the returned artifact:

- latest `main` commit rechecked through the connector;
- request, `AGENTS.md`, Rust skill, current private grammar, ID/HIR/project/bundle evidence inspected;
- all required documents generated;
- `OPEN_QUESTIONS.md` exact-content check;
- 184 unique test rows counted;
- 18 member names/order checked;
- every member hash and manifest rule checked;
- ZIP extraction/member-byte verification and sidecar ZIP SHA-256 checked.

Not performed in this design-only session:

- Cargo build/test/Clippy/format commands against a local checkout;
- current physical LOC/dependency audit against a local filesystem checkout;
- production code changes or runtime/Tier 2 execution.

Historical repository implementation notes mention previously passing private syntax tests, strict Clippy, and structural audit. This archive treats those as repository evidence, not as commands rerun here.
