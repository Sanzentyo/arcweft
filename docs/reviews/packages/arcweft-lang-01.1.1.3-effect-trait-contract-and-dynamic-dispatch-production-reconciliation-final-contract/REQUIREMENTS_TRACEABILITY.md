# Requirements traceability

## 1. Request decisions

| Request requirement | Closed decision | Normative location | Test coverage |
|---|---|---|---|
| sole trait requirement effect owner | `CheckedCallableCatalog` record keyed by `CheckedCallableId` | Final Contract §§1,4; Identity §§1-5 | I001-I016, Z006 |
| impl inferred row owner | `CheckedCallableEffects::Body.inferred` after one fixed point | Final Contract §§1.2,6 | A023, E014-E032 |
| conformance relation owner | catalog-owned `TraitMethodConformance` keyed by impl/requirement IDs | Final Contract §4.2; Identity §5 | T001-T008 |
| exact fields/constructors/visibility/spans | private fields, owner-specific crate constructors, public read accessors; source-backed clause/method records | Final Contract §§2-3; Identity §§1-6 | S001-S008, I001-I016 |
| publication order | link IDs -> shells -> one body pass -> fixed point -> own contract -> conformance -> freeze -> projections | Final Contract §6; Implementation Order | all focused/broad gates |
| no parallel string ID/copied row/second inference | delete `TraitCallableId`, string source IDs, row-copy schema, synthetic requirement impl; existing checker traversal only | Final Contract §§1,5,6,10 | Z001-Z009 |
| compiler/runtime method identity | one-way checked digest into general `RuntimeCallableId`; two-field runtime identity; typed compiler lowering index; no `(usize, String)` inventory | Final Contract §8.3; Identity §11; Implementation §11 | P001-P010, Z010-Z012 |
| inheritance/import/reexport/alias | preserve original typed declaration ID; binding only | Final Contract §§4.4,5.4 | I004-I011, T001-T004 |
| method values | `BoundMethodValue` carries receiver/target/signature/row/groups | Final Contract §5.3 | R004-R011 |
| static witness | requirement ID + witness + substituted exposed row | Final Contract §§5.1-5.2; E017 disposition | R003,R005,E017S |
| omitted bodyless row closed empty | explicit bounded `EffectRow::closed(empty)` with name anchor | Final Contract §3.2 | E015,D006 |
| explicit closed/existing open row | current `EffectRow`; clauses are head, existing typed variable is tail | Final Contract §§3.1,4.3 | E024,E030,T007-T008 |
| exact subset check | inherent `EffectRow::check_subset`, residual-tail algorithm | Final Contract §4.3 | E024-E030,T005-T008 |
| multiple effects/generics/currying/suspend | sorted effects, typed substitutions, final-group latent row, ordinary EffectId | Final Contract §§4.3,5.2-5.3 | E025-E030,T005-T008,R006-R008 |
| inherent method separately | no row -> ordinary body inference; not implicit closed empty | Final Contract §3.3 | T009 |
| E015 variant/code/ranges | `TraitOmittedRowMissing`; `sema.trait.effect.omitted_row_missing`; name primary | Diagnostic §§1-3 | E015,D006-D008 |
| E016 variant/code/ranges | `TraitClosedRowMissing`; `sema.trait.effect.closed_row_missing`; row primary | Diagnostic §§1-3 | E016,D002-D008 |
| E022/E023 variant/code/ranges | `ClosedRowMissing`; `sema.effect.closed_row_missing`; typed direct/shortest trace | Diagnostic §§1-5 | E022,E023,D001-D009 |
| reconcile AWF-EFX | delete `UpperBoundExceeded`/`AWF-EFX-001`, no mapping | Diagnostic §10 | Z003, exact code tests |
| CLI/LSP one diagnostic | enum inherent renderer -> `TypeCheckError` -> CLI/LSP | Diagnostic §9 | D007-D011 |
| E017 precedence | parent row superseded; future feature | Dynamic Disposition §§1-6 | E017,X001-X002 |
| supported replacement | E017S static witness row | Dynamic Disposition §4 | E017S,R003,R005 |
| compile-clean deletion order | twelve-step authority switch and deletion inventory | Implementation Order | Z001-Z009 + broad gates |
| independent of Stream/Proof | no exact dependency; no wire/opcode/syntax switch | Final Contract §11 | dependency review/gates |

## 2. Parent row disposition

| Parent row | Final disposition | Exact acceptance |
|---|---|---|
| A023 | retained and closed | awaiting impl infers `control.suspend`; accepted only through permitting requirement |
| E014 | retained and closed | bodyless requirement explicitly exposes `control.suspend` |
| E015 | retained and corrected | omitted row is closed empty; typed diagnostic/name primary |
| E016 | retained and corrected | substituted subset failure; row primary; shortest typed trace |
| E017 | superseded | future dynamic trait objects; not static evidence |
| E017S | new replacement | current static witness uses requirement checked ID/row |
| E022 | retained and corrected | `sema.effect.closed_row_missing`, direct terminal span |
| E023 | retained and corrected | same code, shortest typed transitive trace |
| E024 | retained as supporting row | existing open tail absorbs full residual without loss |

## 3. Constraints

| Constraint | Compliance |
|---|---|
| no `task fn`/`dialogue fn`/`stream fn`/`FunctionKind` restoration | no such design or carrier appears |
| no compatibility aliases/shims/dual readers | direct replacements and deletions only |
| no source gate | removal tests are typed/API/behavior/dependency evidence |
| no removed-syntax diagnostic | dynamic object source uses ordinary grammar rejection |
| no source reparse | exact source records flow syntax -> HIR -> sema -> diagnostics/tooling |
| no string-ID fallback | all source/effect identities typed and revision-bound |
| no CSS/Takumi | absent |
| preserve accepted resolver | candidate precedence unchanged; identity/effect lookup corrected |
| preserve project nominal identity | no nominal identity change |
| preserve direct suspension/cancellation | no runtime semantic change |
| preserve Stream classification | no classification or wire/opcode change |
| existing dependency direction | syntax -> HIR -> sema -> compiler/index/tooling |

## 4. Required output closure

All requested members are present. `OPEN_QUESTIONS.md` is exactly `none` and
`FINAL_STATUS.md` says `READY_FOR_IMPLEMENTATION`. `MANIFEST.sha256` is sorted
and lists hash/length for every other member. The only external sidecar is the
completed ZIP SHA-256.
