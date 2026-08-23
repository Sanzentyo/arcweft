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

## Owner matrix

| Authority | Final owner | Construction input | Consumers | Deleted parallel input |
|---|---|---|---|---|
| Entry catalog and Entry reference | sealed `FinalSemanticAnalysis` | private draft + Entry checker | compiler selection, lowering, tooling | compiler-owned catalog field; HIR-only reference |
| project runtime nominal projection | sema `RuntimeNominalProjectionCatalog` | symbols + accepted type map | Entry, field/case rows, compiler | post-seal re-expansion |
| environment record/field | `TypeCheckEnv::AcceptedEnvironmentRecord` | declaration-ordered registration input | selection, patterns | nested field `HashMap` |
| project item ID | sema checked project item | PublicId/declaration digest + family/type | value/entity transcript and coverage | public spelling/raw item as identity |
| variant case | sema owner case table | typed owner/projection/catalog | expression, pattern, coverage | name vector and selected clone |
| record field | shared sema field-ID enum | project projection or environment row | selection, patterns | reader-side name lookup |
| Character look | sema checked StageLook | accepted manifest row | StageLook transcript | open HirName fallback |
| callable method | callable join + sema selection | checked call join | select transcript | method name authority |
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

Project field lowering migrates to the checked nominal projection plus its
accepted runtime field ordinal and deletes semantic use of the diagnostic
name. Environment field rows remain owned by their accepted environment
schema; no compiler reader may reinterpret the diagnostic name as a runtime
coordinate. Runtime-plan lowering rejects that owner through a typed
owner/ordinal error until a separate accepted runtime structural-record
authority exists; defining such an algebra is outside this C2 request.

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
