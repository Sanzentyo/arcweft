# Implementation plan

## Baseline

Latest inspected `main`: `3acc9cfec034d00cee173e41cbfb37cd46115c50`. The repository's retained-global-identity implementation record marks private cuts 0 through 7 complete and public cuts 8 through 11 pending. This plan preserves that state and the broader Proof authority-switch dependency order.

## Cut 0 — intake and invariant lock

- Verify this archive manifest and sidecars.
- Re-read current `AGENTS.md`, the Rust skill, the request, accepted implementation note, and public-switch readiness note.
- Confirm `main` and record Git/Jujutsu state available in the implementation checkout.
- Re-run the current private seven-family focused tests without changing public readers.
- Confirm no source `asset` typed item exists and `RetainedIdentityFamily::Asset` remains catalog-owned.

Review point: documentation-only intake record; no production behavior change.

## Cut 1 — owned boundary APIs and private-gate maintenance

- Add any still-missing inherent `RetainedIdentityFamily::from_prefix` behavior in `arcweft-id`; migrate duplicate family string matches.
- Introduce/complete owned `AssetId` plus normalized asset virtual-path identity conversion and migrate CLI/bundle callers without changing observable ID derivation.
- Move Layer kind/default/reference-family behavior onto the owning Arcweft enums/newtypes.
- Keep all public syntax readers unchanged during this cut.
- Pass focused ID, private grammar, bundle asset-ID, and strict Clippy tests.

Review point: small compiling owner cleanup; no public AST/HIR switch.

## Cut 2 — close and freeze the private declaration gate

- Keep the seven existing private grammar modules as the sole target shape.
- Add any missing direct matrix rows for exact grammar, malformed cases, recovery, ambiguity, LF/CRLF/losslessness, and inclusive budgets.
- Keep top-level `asset` and removed forms as ordinary `ErrorItem` recovery.
- Prove the mixed document inventory and 184-row mapping before public migration.

Review point: `arcweft-id` and `arcweft-lang-syntax` focused suite, stable all-target/all-feature strict Clippy, format, diff check, structural audit. No second public reader.

## External sequence gate

Do not begin the public source authority switch until the current Proof readiness ledger's prerequisites are present as compiling, validated commits. In particular, source-owner removals and shared attached syntax/HIR entry dependencies must be released. This is sequencing, not an open grammar decision.

## Cut 3 — atomic attached public syntax switch

One workspace-compiling cut:

1. make bound `ParsedSource` the sole complete-document authority;
2. publish exact attached declaration wrappers and explicit `Item` variants for `res` plus the seven authored retained families;
3. migrate parser facade, formatter, LSP syntax features, CLI/Agent syntax display, test builders, and every source-backed syntax consumer;
4. expose typed unbound-fragment plus explicit attachment boundary;
5. delete generic/detached `EntityDeclItem`, kinds/bodies/raw fields, legacy retained parsing, and source-less constructors;
6. introduce no Asset declaration and no dual reader.

Review point: complete syntax/attachment/API tests, compile-fail deletion tests, workspace check/Clippy, format, structural audit.

## Cut 4 — source-backed HIR entry

Within the accepted Proof Stage 3 boundary:

- replace `lower_to_hir(&TypedSyntaxTree)` and package-late retained lowering with the checked bound request;
- key source-backed HIR entry by exact grammar `SyntaxNodeId` and source/package/module identity;
- allow the current sole Vec/clone HIR representation only for the contract-defined short interval, but remove retained syntax-string reparsing immediately;
- do not publish a second arena HIR early.

Review point: HIR entry/identity tests and all direct source-provenance tests.

## Cut 5 — accepted private arena-HIR retained payloads

After the broader Proof Stage 4 surface is final, extend the accepted private Stage 5 `HirDatabase` with the exact item, parameter, member, reference, and payload inventory in this archive. Implement transaction/liveness/limit tests privately. Do not expose a declaration-only database or duplicate public HIR.

Review point: HIR database focused tests, no downstream public switch.

## Cut 6 — atomic public HIR/project switch

Within accepted Proof Stage 6:

- move all retained declarations to arena IDs/payloads;
- register seven source items and catalog assets through one module-preserving project generation and one `ProjectSymbolTable`;
- migrate sema, verifier, compiler, runtime-plan preparation, and semantic LSP consumers together;
- delete cloned generic entity HIR, linked/flattened retained readers, package-late builders, and duplicate registries.

Review point: HIR/sema/project/compiler/LSP focused suites, workspace check/Clippy, structural audit.

## Cut 7 — domain consumer migration

Migrate in compiling cohorts while the new HIR/project authority remains sole:

1. Character registration, alias, dialogue/runtime projection;
2. View callable/catalog/export/Style/mount projection;
3. Action channel signature, send/receive/callable projection;
4. Activity interface and manifest binding admission;
5. Signal and Metric sema/runtime schema consumers;
6. Layer presentation/input/Agent plan construction;
7. asset catalog and `res` asset references;
8. CLI, Agent, formatter, LSP, docs, examples, and fixtures.

Each cohort deletes its old string/generic reader in the same commit. No compatibility facade remains.

## Cut 8 — obsolete source and caller deletion

- Complete Lang-01.4 configured-resource migrations to `res`.
- Complete Lang-01.5 source-owner removals required by the active base.
- Remove old `content`, concrete Activity-origin, regular-project top-level statement, and configured-family paths through ordinary grammar behavior.
- Remove obsolete docs/examples/fixtures without adding source gates or historical diagnostics.

## Cut 9 — full acceptance

- Execute every command in `VERIFICATION_PLAN.md` using one recorded stable feature combination.
- Complete all 184 rows.
- Run Tier 2 because the migration spans multiple crates and affects runtime/render/Agent paths.
- Reconcile stale tests to the final production contract rather than adding aliases or duplicate paths.
- Check in the implementation structural audit and exact validation record.
- Record failures honestly; do not count a blocked/unrun command as passing.

## Commit discipline

Every cut is coherent, compiling, and validated at its stated level. No new branch/bookmark is required. Production implementation remains on `main` under the repository's Jujutsu/Git workflow. This design task itself produces no repository commit or production patch.
