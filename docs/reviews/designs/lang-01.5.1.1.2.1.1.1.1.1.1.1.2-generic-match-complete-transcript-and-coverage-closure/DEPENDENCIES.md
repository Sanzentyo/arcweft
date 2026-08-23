# Owners, consumers, and dependencies

## Dependency direction

| Producer/owner | Same-cut responsibility | Consumer | Direction |
|---|---|---|---|
| `arcweft-lang-hir` | declaration/body root roles; exhaustive child topology; ViewValue and expression-owned non-expression paths | `arcweft-lang-sema` | existing HIR -> sema dependency |
| `arcweft-core` project nominal types | `RuntimeSemanticTypeId`, `TypeLayoutHash`, `RuntimeTypeSchema`, `RuntimeRecordFieldId` | sema checked nominal/field joins | existing core -> sema dependency |
| `arcweft-character` accepted identities | Character identity/registry inputs | sema checked look/case rows | existing character -> sema dependency |
| `arcweft-view` closed View/Style types | owner-defined exhaustive semantic tags/digest behavior where required | sema expression atoms | existing view -> sema dependency |
| sema callable/entry/registration/nominal authorities | exact existing joins/digests | final-analysis owner rows | within sema |
| sema final checker | project item, case, field, look, modifier, typed-binding, rich-text and minimal statement payload rows | transcript and coverage | within sema |
| sema transcript builder | expression/pattern/statement/body/coverage/Match digests | `FinalSemanticAnalysis` query/report and future compiler cut | within sema; no runtime dependency |
| sema private coverage analyzer | exhaustive flag, unreachable coordinates, structured witness/error | transcript publication and diagnostics | within sema |

No edge points from HIR/core/character/view to sema. No compiler or runtime
crate becomes an identity owner. Sans-I/O boundaries remain Sans-I/O.

## Existing owner joins

| Meaning | Existing authority preserved | Same-cut addition |
|---|---|---|
| declaration | `CallableDeclarationKey`, accepted declaration ID, checked callable ID/interface/join digest | `ViewValue { ordinal }` path only |
| project nominal | `CheckedProjectNominal` joined to canonical runtime nominal projection/schema/layout | none; field/case IDs bind these existing atoms |
| project callable | current checked callable catalog | none |
| registered value | `RegisteredSemanticValueId` | none |
| entry | current checked entry catalog/binding digest | retain binding digest in `CheckedEntryReference` |
| dialogue view | `DialogueProjectionCoordinate` | exclude diagnostic name |
| dialogue line/text key | accepted `DialogueLineId`/`DialogueTextKey` | purpose-built rich-text digest |
| effect | current typed `EffectId` | owner-defined semantic digest, not display spelling |

## Same-cut owners and consumers

| New owner | Constructor input | Consumers | Supersedes |
|---|---|---|---|
| `AcceptedProjectItemSemanticId` | accepted public/entity or Flow declaration digest, family, value type | Value transcript, Entity pattern, coverage singleton/witness | public spelling/raw `ItemId` |
| `AcceptedVariantCaseSemanticId` | owner type/layout, source ordinal, payload type | expression/pattern transcript, closed coverage | Character/Builtin case name and selected name |
| `AcceptedRecordFieldSemanticId` | project nominal semantic type/layout/runtime field ID/ordinal/type | record expression/pattern transcript and product coverage | authored field lookup during transcript |
| `AcceptedEnvironmentFieldSemanticId` | environment type identity/ordinal/field type | Field selection, record transcript/coverage | name-only open record selection |
| `AcceptedCharacterLookSemanticId` | accepted character registry row | StageLook transcript | open `HirName` fallback |
| `AcceptedViewModifierSemanticId` | View declaration + accepted modifier row | ViewCall transcript | modifier `HirName` |
| `CheckedRichTextSemanticDigest` | checked rich-text semantic rows + child digests | dialogue expression transcript | HIR IDs/spans/debug representation |
| `CheckedStatementSemanticDigest`/`CheckedBodySemanticDigest` | checked payload + HIR typed roles | Await/Choice/dialogue/nested Match transcript | omitted statement/body meaning |

## Producer and current consumer inventory

All checked expression/pattern/value/select facts are produced only by
`arcweft-lang-sema::final_analysis::analyzer`. Checked edge/callable joins are
published by `final_analysis::match_edges`. Stable declaration paths are
produced by `arcweft-lang-hir::final_project::semantic_paths`. Canonical project
nominal projections are produced by sema's nominal/final-analysis join over the
core runtime schema types.

At the pinned SHA, direct `CheckedMatch` consumers are the sema public query,
re-exports, final-analysis tests, and compile-fail API tests for private/non-
Serde `CheckedMatchRef`. There is no compiler or runtime reader and no persisted
Match consumer. Future compiler/runtime-plan work is downstream only and does
not authorize a wire DTO, task seal, or whole-catalog digest here.

## Forbidden dependency substitutes

- sema scanning source/HIR names after final checking;
- HIR importing sema identities;
- copying project runtime schemas/layouts into a Match-owned table;
- runtime/compiler constructing coverage or accepted IDs;
- hashing all callable/nominal/registration rows to compensate for a missing
  exact owner;
- serializing private matrix/deconstruction/transcript state.
