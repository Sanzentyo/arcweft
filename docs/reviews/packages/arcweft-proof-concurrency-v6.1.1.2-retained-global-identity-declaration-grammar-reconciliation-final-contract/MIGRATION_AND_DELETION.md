# Migration and deletion inventory

## 1. Syntax and parser

| Current path/symbol | Final owner | Migration | Deletion point |
|---|---|---|---|
| `crates/arcweft-lang-syntax/src/parser/{character,view,action,activity,signal,metric,layer}_grammar.rs` | same private one-pass grammar, later public attached projection | retain exact grammar and direct tests | not deleted |
| `parser/declaration.rs` | common retained header | add missing owned enum calls; keep common recovery | not deleted |
| `parser/item.rs`, `parser/document.rs` shadow dispatch | sole full-source grammar dispatch | publish through `ParsedSource` switch | duplicate/legacy dispatch deleted in Cut 3 |
| `ast/items.rs::EntityDeclItem` | seven attached declaration wrappers | migrate all public callers | Cut 3 |
| `EntityDeclKind` | concrete `Item` variants and owned family enums | eliminate broad family matches | Cut 3 |
| `EntityDeclBody` | concrete family body nodes | migrate body access | Cut 3 |
| `signature_tail: String` | Action/View/Signal/Metric typed children | direct typed access/lowering | Cut 3 |
| raw `body`, `structured_body`, `body_range` dual fields | attached body/member handles | direct source ranges | Cut 3 |
| source-less public constructors/test builders | parse/attach builders | compile-fail proof of absence | Cut 3 |
| legacy `parser/items.rs` retained branches | one-pass grammar | remove after all syntax callers switch | Cut 3 |
| source `asset` recognition, if any remains | no grammar | ordinary `ErrorItem` | Cut 2/3 before public completion |

## 2. Attachment and CST

| Current path/symbol | Final owner | Deletion point |
|---|---|---|
| detached `TypedSyntaxTree` source authority | bound `ParsedSource` and attached handles | broader Proof Cut 3 |
| line-only/range-derived source-backed identity | grammar `SyntaxNodeId` | broader Proof Cut 3 |
| duplicate source-backed fragment parsers | unbound fragment plus explicit attachment | broader Proof Cut 3 |
| coarse tag construction authority | exact concrete-kind predicates | as public attachment is exposed |

## 3. HIR

| Current path/symbol | Final owner | Deletion point |
|---|---|---|
| `HirTopLevelDecl::EntityDecl(EntityDeclItem)` | `HirItemKind::{Character,View,Action,Activity,Signal,Metric,Layer}` | accepted Stage 6 switch |
| `lower.rs` clone of generic syntax value | bound attached lowering transaction | source-backed HIR entry/Stage 6 |
| syntax wrappers stored in HIR | typed arena payload/IDs | Stage 6 |
| View callable clone/projection | callable facet on View `ItemId` | prerequisite already removed; no reintroduction |
| linked/flattened retained module readers | module-preserving HIR project snapshot | Stage 6 |
| payload source ranges copied from syntax | HIR source-slot table | Stage 6 |
| downstream signature/body parsing | typed parameter/type/expression/member IDs | each consumer cohort, completed before Stage 6 close |

## 4. ID, asset, and project ownership

| Current path/symbol | Final owner | Deletion point |
|---|---|---|
| repeated family-prefix string matches | inherent `RetainedIdentityFamily` behavior | Cut 1 |
| CLI `bundle_asset_id_from_virtual_path` / component helper | `AssetId` + `AssetVirtualPath` owner | Cut 1 after bundle tests |
| asset symbols inferred only in format-specific bundle code | project asset catalog, with format-specific projections | Cut 7 asset cohort |
| per-family symbol maps/registries used as authority | one `ProjectSymbolTable`; registries are projections | Cut 6/7 |
| family-by-family local conversion helpers | original enum/newtype or named lowering context | when migrated caller compiles |
| Layer `reference_family` free string match | inherent member-kind/family behavior | Cut 1 |

## 5. Sema and project index

Current generic consumers include `crates/arcweft-lang-sema/src/resolve.rs`, `checker/helpers.rs`, and `project_index/entities.rs`. They migrate to typed HIR item/member matches and one project generation. Remove:

- `EntityDeclKind` switches;
- source-string signature parsing;
- raw View/Action/Signal/Metric/Layer tail inspection;
- duplicate Character/View/action lookup tables acting as authorities;
- LSP-local project identity reconstruction.

Deletion point: Cut 6 for authority, Cut 7 for remaining domain facets.

## 6. Compiler, runtime-plan, bundle, presentation

- Compiler image/configured-resource code that still consumes `EntityDeclKind::Image` migrates under Lang-01.4 to typed `res`; it is not converted into a retained `asset` declaration.
- View compiler uses `HirViewDeclaration` and the one accepted View product.
- Action consumers use the typed ordered parameter schema.
- Activity compiler uses abstract interface plus typed manifest binding.
- Signal/Metric consumers use checked semantic schemas.
- Layer plan construction uses typed kind/policy/reference products and existing presentation owners.
- Bundle asset collection uses owned asset IDs and catalog records; `res` and other declarations reference catalog symbols.

Every old reader is deleted in the cohort that introduces its typed replacement.

## 7. Tooling

| Consumer | Required target |
|---|---|
| Formatter | attached nodes and source-preserving spelling; no raw body formatter |
| LSP symbols/hover/definition/references/rename | exact syntax/HIR/project identities; asset path provenance for catalog definitions |
| CLI check/build/bundle | typed HIR/project/catalog; no syntax string matching |
| Agent REPL/observation | typed declaration and runtime products; no duplicate entity index |
| docs/examples/fixtures | only final seven declarations, catalog asset references, and `res` |
| test builders | parse/attach or typed internal HIR builders with no public source-less AST constructor |

## 8. Removed source forms

Final parser state retains no dedicated AST/CST kind, compatibility alias, deprecated variant, source gate, or spelling-specific diagnostic for:

```text
asset declaration
content declaration
source declaration
extern mod
dialogue defaults
old configured-resource family declaration heads
concrete activity `from rust` / `from wasm` / process origin
regular-project top-level statements
```

Behavioral tests prove ordinary current-grammar recovery and absence of executable typed output. No test scans checked-in source text for those names.
