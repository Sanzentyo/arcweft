# Decision register

| ID | Decision | Selected result | Rejected alternatives |
|---|---|---|---|
| D1 | publication | `FinalSemanticAnalysis` owns sealed entries and nominal projections | public builder, wrapper publication, compiler side table |
| D2 | phases | private draft -> Entry check -> EntryRef seal -> final validation/publish | post-publication patch, second analysis |
| D2a | prepared expressions | one private `PreparedExpressionFact` map with Complete/Entry rows and shared candidate journaling | pending side map, Structural placeholder, public pending variant |
| D3 | verification precedence | Entry binding precedes verification; Entry selection remains after | exposing draft to verifier, reverse dependency |
| D4 | Entry checker input | concrete narrow prepared authority | `&FinalSemanticAnalysis`, public trait |
| D5 | nominal projection | one exhaustive typed request visitor/context over symbols + accepted type map, sealed complete catalog retaining `TypeShape` | demand-only seeds, duplicate projector, reader recomputation |
| D5a | nominal construction order | C2.2a context foundation -> C2.2b Record authority -> C2.3 exact rows -> C2.4 exhaustive prepared/final visitor and catalog seal | visitor over nonexistent/placeholder row families, partial inventory |
| D5b | compile scaffold | existing final-analysis projection wrappers temporarily delegate in C2.2a, remain uncommitted/unpublished, and are deleted in C2.4 | separate accepted cut, retained wrapper authority, intermediate commit/push |
| D6 | nominal limits | fresh `NominalResolutionLimits` budget per root plus non-resetting `NominalAggregationLimits` project budget | one global root budget, new constants, saturation |
| D7 | environment fields | ordered typed Record semantics inside existing accepted nominal record/catalog/world digest | TypeCheckEnv map/index, public raw record/field mint, scope removal |
| D8 | environment patterns | admit exact accepted named record using same rows | reader name reconstruction |
| D9 | View modifier | delete success and fail closed | invented registry, name hash |
| D10 | shared types | reuse `DeclarationIdentityFamily`, `CallableReceiverMode`, one field-ID enum | parallel enums |
| D11 | variants | one owner table plus selected ordinal/borrowed accessor | cloned selected row |
| D12 | Character look | hash exact accepted manifest Character/look/selection row | HirName fallback, ID-only unvalidated row |
| D13 | Style | owner-defined exhaustive 26-variant encoder | literal-count gate, Serde/debug |
| D14 | Postfix | selected ExprId remains private lookup-only; C3 hashes child digest | deleting live lookup, hashing ID |
| D15 | RichText | C2 retains typed report; C3 derives token/open ordinals and digest | C2 partial digest, raw tag ID hash |
| D16 | dead Select | delete TupleElement and RecordElement; reserve `0x0405/0x0406` | fabricate producers, tag reuse |
| D17 | versions | every new domain remains version 1 | V2, compatibility path |
| D18 | C1 | consume unchanged | topology/path redesign |

All decisions are closed.
