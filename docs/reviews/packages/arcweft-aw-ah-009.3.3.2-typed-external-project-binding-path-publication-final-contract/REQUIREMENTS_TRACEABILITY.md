# Requirements traceability

## 1. Required decisions

| Request decision | Frozen answer | Normative location |
|---|---|---|
| 1. owner of source-visible root and ordered segments | existing syntax-owned `ProjectSymbolPath`/`ProjectSymbolSegment` | `FINAL_CONTRACT.md` §§2, 6 |
| 2. `ProjectDirectBinding` storage | stores `ProjectSymbolPath` directly; implicit root enforced by `ProjectDirectBindingError` | `FINAL_CONTRACT.md` §3.1 |
| 3. path retention through insertion/import/re-export/alias/coalescing | `ScopeBinding.path`; typed `LinkedProjectSymbolPath`; typed rebind; path-inclusive coalescing | `FINAL_CONTRACT.md` §§4–6; `LINKER_AND_CATALOG_RULES.md` §§2–4 |
| 4. `hero` versus `character.akane` | independent exact path keys targeting the same declaration; aliases never mutate canonical identity | `FINAL_CONTRACT.md` §§3.2, 8, 12 |
| 5. character/adapter producer construction | character uses `compact_segments`; adapter stores `AdapterSymbolPath` and converts segment-by-segment at fact publication | `FINAL_CONTRACT.md` §§8–9; `PRODUCER_MIGRATION.md` §§3–4 |
| 6. canonical versus binding identity | opaque `ExternalDeclarationSeed::canonical_path` retained; each direct/scope path stored separately | `FINAL_CONTRACT.md` §3.2 |
| 7. deterministic iterator | one typed `scope_bindings` iterator ordered by module/private key/typed row | `FINAL_CONTRACT.md` §7; `LINKER_AND_CATALOG_RULES.md` §5 |
| 8. modules and non-callable `TypeKind` without adapter dependency | HIR retains target only; existing sema closure maps target to `TypeKind`; adapter types never enter sema | `FINAL_CONTRACT.md` §10.3 |
| 9. duplicate/ambiguity/inaccessible/invalid behavior | exact duplicate/coalescing, collision, import, and constructor outcomes frozen | `FINAL_CONTRACT.md` §11; `LINKER_AND_CATALOG_RULES.md` §10 |
| 10. replacement/deletion plan | old constructors/accessors/iterator/string model/skip deleted in one cut | `IMPLEMENTATION_ORDER.md`; `DELETION_CHECKLIST.md` |

## 2. Required implementation order

| Required stage | Package coverage |
|---|---|
| 1. typed owner and direct tests | `IMPLEMENTATION_ORDER.md` §1 |
| 2. direct constructor replacement and all producers | §2; `PRODUCER_MIGRATION.md` |
| 3. retain paths through linker/import behavior | §3; `LINKER_AND_CATALOG_RULES.md` §§2–4 |
| 4. deterministic typed iterator | §4; `FINAL_CONTRACT.md` §7 |
| 5. catalog consumes every binding/removes skip | §5; `FINAL_CONTRACT.md` §10 |
| 6. accepted-world malformed/collision atomicity tests | §6; `TEST_MATRIX.md` §10 |
| 7. focused/workspace/clippy/audit validation | §7; `VALIDATION_PLAN.md` |

The order is reproduced without substitution or reordering.

## 3. Mandatory direct tests

| Requested direct test | Matrix IDs |
|---|---|
| `character.akane`, `akane`, `hero` exact paths and same external declaration | H-03, C-01, C-02, C-03, L-01 |
| adapter `adapter.viewport` published without sema split | A-03, A-05, A-06 |
| qualified and alias non-callable terminate environment fallback | R-01, R-02 |
| qualified callable, module, external distinct/deterministic | M-01 |
| import, re-export, glob, explicit alias preserve segments | L-02 through L-06 |
| valid `-` external segment remains valid/not module identifier | S-01, C-04, A-01, L-07 |
| invalid empty/control/separator segments fail owner constructor | S-02, S-03, A-02, A-04 |
| reversed fact insertion identical catalogs | A-07, L-10, P-05 |
| collision rejects candidate and preserves previous accepted pointer | P-06, T-01, T-02, T-04 |
| public/dependency evidence via typed APIs and Cargo metadata, no scans | D-01 through D-05 |

## 4. Constraints

| Constraint | Enforcement |
|---|---|
| no split of `SymbolPath::leaf()` | source path segments retained before conversion; catalog uses iterator segments; forbidden in `LINKER_AND_CATALOG_RULES.md` §12 |
| no split of `CharacterId::as_str()` | producer uses existing `compact_segments`; canonical leaf is write-only | `FINAL_CONTRACT.md` §8 |
| no split of adapter labels in sema/catalog | typed `AdapterSymbolPath` before sema; only private codec source parser | `FINAL_CONTRACT.md` §9 |
| no dotted compatibility constructor | no public `FromStr`/string overload; deletion checklist §§1, 5–6 |
| no deprecated wrapper | `DELETION_CHECKLIST.md` §§1, 3, 11 |
| no dual reader | only typed iterator/model; v1 codec decodes directly; no alternate field | `FINAL_CONTRACT.md` §§7, 9.3 |
| no extension trait | inherent owner APIs only | `REJECTED_ALTERNATIVES.md` §14 |
| no source gate | compilation/Cargo metadata/structural audit only | `TEST_MATRIX.md` §11; `VALIDATION_PLAN.md` §5 |
| no second project-symbol resolver | existing HIR resolver retained; carrier is evidence only | `FINAL_CONTRACT.md` §§5, 12 |
| source-visible alias not canonical identity | explicit separation | `FINAL_CONTRACT.md` §3.2 |
| import behavior unchanged | same resolution/visibility/ambiguity/limits/fixed point | `FINAL_CONTRACT.md` §6 |
| HIR not depend on sema/adapter-context | target-only HIR; metadata validation | `FINAL_CONTRACT.md` §10.3; D-04 |
| no redesign of accepted identity, call ranges, request lifecycle | frozen substrate | `FINAL_CONTRACT.md` §1 |
| direct final model and delete string path in same cut | coherent-cut rule | `IMPLEMENTATION_ORDER.md` final section |
| no CSS/Takumi | explicit forbidden addition | `DELETION_CHECKLIST.md` §11 |

## 5. Expected output elements

| Expected element | Package member/section |
|---|---|
| selected ownership and rejected alternatives | `FINAL_CONTRACT.md` §2; `REJECTED_ALTERNATIVES.md` |
| exact Rust declarations/errors/visibility | `FINAL_CONTRACT.md` §§2–10, 14 |
| current-producer migration table | `PRODUCER_MIGRATION.md` §2 |
| deterministic linker/catalog rules | `LINKER_AND_CATALOG_RULES.md` |
| direct tests | `TEST_MATRIX.md` |
| implementation order | `IMPLEMENTATION_ORDER.md` |
| deletion checklist | `DELETION_CHECKLIST.md` |
| no display-string split/compatibility/second resolver confirmation | `FINAL_STATUS.md`; `FINAL_CONTRACT.md` §15 |
| OPEN_QUESTIONS=0 | `OPEN_QUESTIONS.md`; `FINAL_STATUS.md` |
| implementation-ready ZIP | archive plus verified `MANIFEST.txt`, status, summary, and SHA sidecars |

## 6. Non-redesign proof

| Existing substrate | Contract action |
|---|---|
| `ProjectSymbolPath` | reused unchanged as owner |
| `SymbolPath` opaque external leaf | retained for resolution/canonical identity |
| `ProjectSymbolTable` resolver | retained; rows gain path evidence |
| import visibility/ambiguity/fixed point | retained |
| callable scalar/path IDs | retained |
| `ProjectCallablePath` | retained |
| `ProjectNameBinding` | retained |
| callable catalog records/maps/errors | retained |
| resolver precedence | retained |
| `TypeKind` mapping closure | retained |
| accepted-world transaction | retained and tested |
| adapter callable model | retained; separate symbol path added |
| source/world identities, call ranges, request lifecycle | untouched |

The only redesigned APIs are the concretely defective string-only project/adapter symbol binding constructors and the incomplete string iterator/publication seam.
