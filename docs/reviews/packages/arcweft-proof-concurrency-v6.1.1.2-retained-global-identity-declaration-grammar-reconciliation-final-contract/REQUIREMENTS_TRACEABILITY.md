# Requirements traceability

| Request boundary | Binding answer | Primary document(s) |
|---|---|---|
| 1. Final family inventory and heads | eight identity families, seven authored declarations; exact headers and metric ordering | `FINAL_CONTRACT.md`, `FAMILY_GRAMMARS.md` |
| 2. Identity/name/visibility/reference | owned family APIs, absolute declaration IDs, derived IDs, one namespace/table, visibility and reference forms | `IDENTITY_VISIBILITY_REFERENCES.md` |
| 3. Docs/attributes/metadata | contiguous outer attachment, no inner attrs, common registry, exact ranges | `FINAL_CONTRACT.md`, `PRIVATE_GRAMMAR_NODES.md` |
| 4. Packaged asset | no source declaration; exact path-derived catalog ID and ownership | `FAMILY_GRAMMARS.md`, `BODY_OWNERSHIP.md`, `PUBLIC_AST_HIR_MIGRATION.md` |
| 5. Character | exact alias/body/member/default/product boundary | `FAMILY_GRAMMARS.md`, `BODY_OWNERSHIP.md` |
| 6. View | fixed signature, typed defaults, leading exports, typed fragment, one callable owner | `FAMILY_GRAMMARS.md`, `PUBLIC_AST_HIR_MIGRATION.md` |
| 7. Action | bodyless fixed typed channel, no defaults/result/body/overload | `FAMILY_GRAMMARS.md`, `BODY_OWNERSHIP.md` |
| 8. Activity | abstract interface only; closed sections/defaults; manifest-owned origin | `FAMILY_GRAMMARS.md`, `BODY_OWNERSHIP.md` |
| 9. Signal/Metric | separate typed nodes, closed observable/kind schemas, typed labels/buckets | `FAMILY_GRAMMARS.md`, `BODY_OWNERSHIP.md` |
| 10. Layer | closed kind/policies, typed refs, defaults/order/content constraints | `FAMILY_GRAMMARS.md`, `BODY_OWNERSHIP.md` |
| 11. Private nodes/events | exact item/member kinds, roles, identity classes, parser modules/pipeline | `PRIVATE_GRAMMAR_NODES.md` |
| 12. Ambiguity/recovery/limits | deterministic classification, typed missing/error nodes, poison, sync, exact inclusive limits and rollback | `RECOVERY_AMBIGUITY_LIMITS.md` |
| 13. Public AST/attachment | explicit attached Item variants, private constructors, exact access/round-trip, atomic generic deletion | `PUBLIC_AST_HIR_MIGRATION.md` |
| 14. HIR/project/downstream | arena payloads/IDs/source slots, no asset item, one symbol table, no clone/reparse | `PUBLIC_AST_HIR_MIGRATION.md`, `IMPLEMENTATION_PLAN.md` |
| 15. Migration/deletion | current path/symbol/caller inventory and exact deletion cuts | `MIGRATION_AND_DELETION.md` |
| Required implementation order | private gate, dependency gate, atomic syntax, accepted HIR stages, consumers, deletion, full validation | `IMPLEMENTATION_PLAN.md` |
| Mandatory direct matrix | exactly 184 named rows with typed result and transaction evidence | `TEST_MATRIX.md` |
| Verification commands | focused, workspace, Tier 2, metadata, diff, structural audit; no unrun claims | `VERIFICATION_PLAN.md` |
| Structural evidence | exact measurement/decomposition/dependency plan | `STRUCTURE_PLAN.md` |
| Latest production reconciliation | pinned Git, file evidence, historical package, current pending cuts, JJ scope | `REPOSITORY_EVIDENCE.md` |
| Required output/status | 18 files, zero-open result, manifest and external sidecars | `README.md`, `FINAL_STATUS.md`, `MANIFEST.txt` |

## Superseded request premise

The request's historical “eight private grammar gaps” premise is superseded by latest production evidence: seven dedicated private declaration grammars now exist and are tested; source `asset` was adjudicated out of the declaration inventory under the request's own direct-removal rule. The final acceptance meaning is therefore:

- seven canonical source declarations produce typed private/public/HIR ownership; and
- the eighth retained family, Asset, proves its typed catalog identity/reference path and proves the absence of a source AST/HIR declaration.

No acceptance requirement is silently dropped. Rows that originally spoke of an Asset declaration are satisfied by the explicit no-production/no-node contract, catalog identity tests, `res` separation, and ordinary recovery tests.

## No open implementation choices

The implementation agent does not choose among grammar spellings, body fields, ID forms, defaults, policy vocabularies, source/catalog ownership, AST shape, HIR shape, poison, synchronization, limits, migration order, or deletion points. All such choices are fixed in this archive.
