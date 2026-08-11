# REQUIREMENTS TRACEABILITY

## 1. Required decisions

| Request decision | Final decision | Normative section(s) | Direct test coverage |
|---:|---|---|---|
| 1. Authoritative owner and exact types | HIR unified table owns project IDs/records/lookup; sema owns one recursive checked resolver and accepted/open catalog | `FINAL_CONTRACT` §§2–7; `OWNER_INVENTORY` | `ID-*`, `DEP-HIR-NO-SEMA`, `DEP-SEMA-HIR`, `DEP-INHERENT` |
| 2. Identity tuple | Exact world/package/root/profile/revision/module/family/owner/name tuple; aliases have distinct IDs | §3.2 | `ID-*`, especially `ID-ALIAS-IDENTITY-DISTINCT`, `ID-WORLD-DIFFERENT`, `ID-REVISION-DIFFERENT`, `ID-OWNER-PATH` |
| 3. Atomic publication | Nominals collected from module-preserving HIR and inserted in existing link transaction; no publication on diagnostics | §4.3 | `PUB-*`, `LIM-DECL-*`, `ORDER-DIAGNOSTICS` |
| 4. Resolution input and product | Accepted/detached typed inputs, exact world checks, one report/outcome/node/alias/poison product | §7 | `RES-*`, `EXT-*`, `OPEN-*`, `UNK-*`, `SRC-DETACHED-*`, `STALE-*` |
| 5. Open-nominal policy | Typed exact/namespace rules with owner, scope, bounded arity, no wildcard, disjoint-overlap validation | §5.3 | `OPEN-*` |
| 6. Recursive checking | One structural walk over all TypeRef forms and every authored owner; sibling accumulation | §§2.3, 8 | `UNK-*`, `POI-*`, `SRC-NESTED`, `GEN-*` |
| 7. Alias/generic normalization | Exact arity, typed-ID substitution, declaration-module target context, chains, ID cycles, normalized choice check | §9 | `ALS-*`, `TM-072`, `RD-084`, `TM-074`, `TM-083*` |
| 8. Source and detached evidence | Complete type-node TextRange map; SourceSpan only from real documents; detached local-only/unavailable facts | §§2, 3.3, 7.2 | `SRC-*` |
| 9. Structured diagnostics | Stable project/sema codes, typed payloads, exact labels, deterministic ordering/dedup/caps | §10 | `PUB-*`, `RES-*`, `UNK-*`, `ALS-*`, `LIM-DIAG-*`, `SRC-IMPORT-RELATED` |
| 10. Checker integration | Resolve signatures before bodies; side evidence creates Unresolved only from prior authoritative annotation poison; Try/Await no cascades | §11 | `TM-080-*`, `POI-*`, `TM-072`, `TM-083*` |
| 11. Entry resolver reconciliation | Delete entry project/import/alias resolution and payload reparsing; retain schema/role logic | §12; `OWNER_INVENTORY` | `ENT-*`, `DEP-NO-ENTRY-RESOLVER` |
| 12. External/environment integration | Exact accepted catalog, existing external IDs/owners, character identity, explicit open rules; no fake project IDs | §5 | `EXT-*`, `OPEN-*`, `RES-WRONG-EXTERNAL` |
| 13. Limits, revision, caching | Exact collection/resolution/cap/work limits; stale input error; complete cache key; no cross-revision reuse | §§13–14 | `LIM-*`, `STALE-*`, `CACHE-*`, `ORDER-DIAGNOSTICS` |
| 14. Consumers | Narrow sema/entry/compiler/index/LSP/test APIs; typed facts for navigation/refactoring | §15 | `TOOL-*`, `DEP-ROOT-EXPORT`, `DEP-NO-DISPLAY-PARSE`, `DEP-NO-LSP-RESOLVER` |

## 2. Required implementation order

| Request step | Contract cut | Proof |
|---:|---|---|
| 1. IDs, records, outcomes, errors, source evidence, invariant tests | Cut 1 | `IMPLEMENTATION_ORDER` Cut 1; `ID-*`, `SRC-*` |
| 2. Publish declarations/import/re-export bindings with collisions/limits/revision | Cut 2 | `PUB-*`, publication limit rows |
| 3. Migrate entry project/import/alias lookup | Cut 3 | `ENT-*`, owner deletion inventory |
| 4. Bounded recursive resolution, policies, aliases, poison | Cut 4 | `EXT-*`, `OPEN-*`, `UNK-*`, `GEN-*`, `ALS-*`, `POI-*` |
| 5. Migrate normal consumers and delete disagreeing paths | Cut 5 | `TM-*`, `RD-084`, `DEP-NO-ARCR`, `DEP-NO-UNKNOWN` |
| 6. Diagnostics and compiler/LSP consumers | Cut 6 | `TOOL-*`, source/diagnostic rows |
| 7. Focused/workspace/Tier 2/metadata/diff/structural audit | Cut 7 | `DEP-*`; `VERIFICATION_MANIFEST` |

## 3. Mandatory direct tests

| Mandatory family | Test IDs |
|---|---|
| Local/child/parent/qualified/imported/aliased/globbed/re-exported structs/enums/aliases to one identity | `ID-LOCAL-*`, `ID-CHILD-*`, `ID-PARENT-*`, `ID-QUAL-*`, `ID-IMPORT-*`, `ID-AS-*`, `ID-GLOB-*`, `ID-REEXPORT-*` |
| Duplicate/cross-family/ambiguity/private/visibility/unknown/insertion independence | `PUB-*`, `RES-INACCESS-*`, `RES-AMBIG-GLOB`, `ORDER-DIAGNOSTICS` |
| Rust/domain/records/enums/characters/adapters/open validity | `EXT-*`, `OPEN-EXACT`, `OPEN-NAMESPACE`, `OPEN-MODULE`, `OPEN-SUBTREE`, `OPEN-DETACHED` |
| Unknown paths in all requested positions | every `UNK-*` row |
| Generic collisions/nesting/Self/projections | every `GEN-*`, `SELF-*`, `PROJ-*` row |
| Alias arity/chains/cycles/imports/substitution | every `ALS-*` row |
| TM-074 | `TM-074`, `POI-CHOICE-EXCLUDE` |
| TM-080 | `TM-080-POSTFIX`, `TM-080-PREFIX`, `TM-080-AWAIT`, `POI-UNRELATED-BODY`, `POI-OPERAND-ERROR` |
| TM-083 | `TM-083`, `TM-083-RENAME`, `TM-083-IMPORT` |
| TM-072/RD-084 | `TM-072`, `RD-084` |
| Prefix Try and propagating Await | `TM-080-PREFIX`, `TM-080-AWAIT`; TM-072/TM-083 fixtures repeated for each operator in implementation parameterization |
| UTF-8/repeated/multi-document/related labels/detached/stale/limits/caps/order | `SRC-*`, `STALE-*`, `LIM-*`, `ORDER-DIAGNOSTICS` |
| Entry/normal fact equality | `ENT-*` |
| Compiler/LSP diagnostic/hover/definition/completion/rename | `TOOL-*` |
| Dependency/visibility/Cargo metadata/no source gates | `DEP-*` |

## 4. Constraints and non-goals

| Constraint | Contract enforcement | Test/audit |
|---|---|---|
| No `Unknown` or `ArcResult` special case | precedence and alias-ID rules; direct deletion list | `DEP-NO-ARCR`, `DEP-NO-UNKNOWN`, `TM-083-RENAME` |
| Do not treat every `Named` as source declaration | authored resolver never creates `Named` fallback | `OPEN-NEAR-MISS`, every `UNK-*` |
| No additional project/import/alias resolver | unified owner and entry deletion plan | `DEP-NO-ENTRY-RESOLVER`, `DEP-NO-LSP-RESOLVER` |
| No compatibility shim/dual reader/source gate | direct migration rules | `DEP-NO-SHIM`, `DEP-NO-SOURCE-GATE` |
| No display/source parsing into identity | `TypePath`, source map, typed IDs | `DEP-NO-DISPLAY-PARSE`, `TOOL-REPEATED` |
| Syntax parser-only; HIR/data Sans I/O; core uninvolved | dependency section and owner inventory | `DEP-HIR-NO-SEMA`, `DEP-CORE` |
| No CSS/Takumi path | explicit non-goal and dependency audit | `DEP-NO-CSS-TAKUMI` |

## 5. Matrix completeness rule

`TEST_MATRIX.csv` is normative, not illustrative. Every row must become an
automated test or a deterministic parameter of an automated table-driven
test. A row may be covered by a parameterized test only when the test output
reports the row's `test_id` on failure.
