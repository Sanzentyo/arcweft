# Dependencies and consumers

## Layer direction

```text
arcweft-id -------------------------------> arcweft-lang-sema
arcweft-character ------------------------> arcweft-lang-sema
arcweft-presentation -> arcweft-view -----> arcweft-lang-sema
arcweft-core ------------------------------> arcweft-lang-sema
arcweft-lang-hir --------------------------> arcweft-lang-sema
arcweft-lang-sema -------------------------> arcweft-verify
arcweft-lang-sema + arcweft-verify --------> arcweft-compiler
arcweft-core <------------------------------ arcweft-compiler lowering
```

No lower crate depends on sema. Lower owners expose only their own exhaustive
tags/digests. Sema composes those values into C2 facts. Compiler and verifier
consume only sealed `FinalSemanticAnalysis`.

## Compile-clean construction dependency

```text
C2.1 lower owners
  -> C2.2a projection context/expander foundation
  -> C2.2b accepted environment Record authority
  -> C2.3 exact final row types + projection-independent rows + prepared seeds
  -> sealed call application authority + inference-free joins + Method enrichment
  -> C2.4 exhaustive visitor + digest-order projection + seed/Entry seal
  -> C2.5 deletions
  -> C2.6 gates and one reviewable commit/push
```

The exhaustive visitor has a real type dependency on every C2.3 final row and
prepared seed family and therefore belongs to C2.4, not C2.2a. A C2.3 producer
must not project during source traversal because that would change aggregate
budget and first-error precedence from semantic-digest order to source order.
The C2.2a final-analysis wrappers are
temporary delegates used only to keep development compiling while the context
is extracted. They confer no ownership, are not an independently acceptable
cut, and must be deleted before C2.4 publishes the single catalog. No
intermediate C2 state is committed or pushed.

## Owner matrix

| Authority | Final owner | Construction input | Consumers | Deleted parallel input |
|---|---|---|---|---|
| Entry catalog and Entry reference | sealed `FinalSemanticAnalysis` | private draft + Entry checker | compiler selection, lowering, tooling | compiler-owned catalog field; HIR-only reference |
| project runtime nominal projection | sema `RuntimeNominalProjectionCatalog` | complete prepared inventory + symbols + moved accepted type map | Entry, field/case rows, compiler | source-order/per-producer projection; post-seal re-expansion |
| environment record/field | `AcceptedNominalRecord` + `AcceptedNominalSemantics::Record` | accepted ID/path/type + ordered fields | type lookup, selection, patterns, catalog/world digest | all `TypeCheckEnv::nominal_records` maps |
| project item ID | sema checked project item | PublicId/declaration digest + family/type | value/entity transcript and coverage | public spelling/raw item as identity |
| variant case | sema owner case table | projection-independent owner or consumed project seed + cached row | expression, pattern, coverage | name vector and selected clone |
| record field | shared sema field-ID enum | consumed project seed + cached row or exact environment row | selection, edge facts, patterns, lowering | reader-side name lookup |
| Character look | sema checked StageLook | accepted manifest row | StageLook transcript | open HirName fallback |
| callable method | callable join + sema selection | one post-finalization join map, moved to edge facts | select transcript | method name authority; edge-time rejoin |
| selected call application | sealed `CheckedCallApplication` over an unpublished acyclic core | one private candidate transaction + checked callable/effect catalogs + sole sema-root coordinate owner | compiler, runtime-plan projection, Need admission, verifier, Entry, LSP, project index, signature help, call join | provisional fact, common callee fact, `PendingCallAnalysis`, final rebuild, join/compiler inference |
| call continuation | `CheckedCallResult::Continuation` | exact stable callable/schema + cumulative frozen solution + prefix application core | prepared function-value callee and next group solver seed | raw base clone and all duplicated `next_group` fields |
| Effect | `EffectId` | canonical parsed identity | expression transcript | display/source spelling |
| Agent field | core `RuntimeAgentField` | closed owner enum | select transcript | scattered match helper |
| Progress field | sema `ProgressField` | closed owner enum | select transcript | name mapping at transcript |
| View element/value | `arcweft-view` | closed owner enum/value | sema View/Style fact | sema copied inventory |
| RichText | existing checked report, C3 digest | accepted token/action rows | dialogue compiler/C3 | raw ID in digest |
| Postfix selection | sema generation-local resolution | selected ExprId | compiler lowering/reachability | raw ID in digest |

## Entry and compiler consumer migration

`crates/arcweft-lang-sema/src/entry/checker.rs` and
`entry/checker/contract.rs` change their concrete analysis field to
`PreparedEntrySemanticAuthority`. Their observed operations remain exactly:
generation validation, `checked_callables`, `ty`, `item`, `calls`, and nominal
projection.

`crates/arcweft-compiler/src/project.rs` obtains one final analysis, runs
`verify_project`, validates selection against `analysis.checked_entries()`,
and passes that borrowed catalog to reachability/lowering. The
`CompiledProject.checked_entries` storage field is deleted; its accessor
delegates to the analysis so downstream APIs do not gain a second catalog.

Project field lowering consumes the sealed selection/record row's runtime
semantic owner, runtime field ID, and field type identity and deletes semantic
use of the diagnostic name. Project record-expression fields move into checked
child-edge facts; project record-pattern fields retain their typed runtime
coordinate. Runtime-plan expression/pattern readers no longer call a
name-to-field resolver. Environment field rows remain owned by their accepted
environment schema and reject executable lowering through
`UnrepresentableEnvironmentRecordField { owner, ordinal }`; defining a runtime
structural-record algebra is outside this C2 request.

Environment field lookup goes only through
`TypeCheckEnv.nominal_catalog().exact(path)` and the borrowed Record semantics
row. The catalog digest already participates in registered-world identity, so
field changes cannot leave a stale world stamp with new field semantics.

In the final C2 state, nominal projection consumes a typed request inventory
spanning all prepared and published sema fact families. The context owns both
per-root and aggregate budgets during construction; the sealed final analysis
owns only the completed projection catalog, never an expander, mutable cache,
or the temporary C2.2a delegate wrappers.

## C1 predecessor

C1's declaration roots, View nonbinding callable publication, 19 root
families, expression/pattern/statement child roles, source order, and semantic
paths are consumed as-is. C2 uses those paths to order Entry-reference seals
and later C3 recursion. No C1 enum, coordinate, or traversal is redesigned.

## Structural review triggers

The implementation crosses these cohesive responsibilities:

- `final_analysis/analyzer.rs`: orchestration only; owner row constructors stay
  in their existing analyzer submodules/model owners;
- `final_analysis/nominal_schema.rs`: one cohesive cross-layer projection;
- `env/base.rs`: one accepted environment record owner;
- `view/style/value.rs`: one cohesive exhaustive value encoder; and
- `compiler/project.rs`: sequencing and sealed-product consumption only.

If file-size gates trigger, split implementation helpers below the same module
owner; do not split state authority.
