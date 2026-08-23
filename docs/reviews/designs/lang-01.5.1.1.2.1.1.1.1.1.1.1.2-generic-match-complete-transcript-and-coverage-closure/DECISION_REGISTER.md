# Decision register

Each row is unique and normative. Test and mutation IDs are defined in
`CUTS_TESTS_AND_DELETION.md` and `machine/negative_corpus.json`.

| Decision | Selected owner | Rust-shaped schema | Consumers | Positive test | Negative mutation | Deletion cut |
|---|---|---|---|---|---|---|
| 1. all resolution transcripts | existing checked joins plus same-cut project-item/entry/method/field/look/View/Style/dialogue/body atoms | `CheckedExpressionResolution` table and exact replacement rows in `SCHEMAS.md` | expression/body transcript, nested Match | `T01_ALL_RESOLUTIONS` | `N01_DROP_RESOLUTION_FAMILY` | C2/C3 delete every `UnsupportedIdentity` arm and name/raw-ID payload |
| 2. Entity and Character/Builtin cases | `AcceptedProjectItemSemanticId`, `CheckedVariantCase`, `AcceptedVariantCaseSemanticId` | project-item and variant schemas | pattern/expression transcript, constructor domain, witness | `T02_ENTITY_CASE_IDENTITY` | `N02_CASE_NAME_AUTHORITY` | C2 delete string case arrays/selected case name and raw Entity authority |
| 3. record-pattern field row | sema `CheckedRecordPatternField` joined to canonical nominal projection | record owner/field schemas | pattern transcript, product specialization/witness | `T03_RECORD_FIELD_JOIN` | `N03_DROP_LAYOUT_OR_FIELD_ID` | C2 delete transcript name lookup and reject missing checked row |
| 4. bounded Maranget coverage | private sema `MatchCoverageAnalyzer`, `Matrix`, `PatternVector`, domains/constructors | private model in `COVERAGE_ALGORITHM.md` | coverage publication/diagnostics | `T04_MATRIX_DIFFERENTIAL` | `N04_ENABLE_BASIC_COVERAGE` | C4 delete `CoverageAtom`, `CoverageShape`, `CoverageDomain`, `basic_coverage` |
| 5. domain/witness transcripts and limits | coverage domain/witness records plus one checked-`u64` transaction | coverage/witness/limit schemas | Match payload, non-exhaustive error | `T05_WITNESS_AND_LIMITS` | `N05_UNCHECKED_OR_U32_LIMIT` | C3/C4 delete saturation, `expect`, mixed counters, scalar witness |
| 6. declaration/body bridge | HIR declaration paths under existing keys, including `ViewValue` and nested body roles | `HirDeclarationBodyRootRole`, `HirExpressionOwnedBodyRole` | sema coordinates/body digests; later path consumers | `T06_ALL_BODY_ROOTS` | `N06_VIEW_MISSING_BODY` | C1 delete View `MissingBody` arm and omission of nested statement/pattern roots |
| 7. compile-clean replacement | HIR then exact checked rows then transcript then coverage then publication | cut graph in `CUTS_TESTS_AND_DELETION.md` | all constructors/readers/tests | `T07_DELETION_CLEAN` | `N07_RETAIN_PARALLEL_OR_VERSION_BUMP` | C1-C5 ordered deletion; no temporary resolver/fallback |

## Decision closure

All seven decisions are closed. No row delegates a result-changing choice to
implementation. Exact tags/atoms are in `TRANSCRIPT_GRAMMAR.md`; matrix
semantics and canonical witness order are in `COVERAGE_ALGORITHM.md`; ownership
and dependency direction are in `DEPENDENCIES.md`; compile-clean sequencing and
executable evidence are in `CUTS_TESTS_AND_DELETION.md`.
