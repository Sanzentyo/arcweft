# OWNER INVENTORY

## Reading rule

“Keep” means preserve the current responsibility. “Extend” means add the final
typed behavior in the existing owner and its inherent implementations.
“Replace/delete” means remove the old field or successful path in the same
compile-clean cut; it is not retained behind a compatibility layer.

| Current owner/path | Current responsibility/evidence | Final owner/action | Required deletion or invariant |
|---|---|---|---|
| `arcweft-lang-syntax::types::TypeRef` | Tree shape; paths and generic heads are strings; most nodes lack ranges | **Extend/replace in owner** with `TypePath`, `AuthoredTypeRef`, `TypeRefSourceMap`, and `Recovery` | `parse_type_ref` returns the source-backed form; no public unspanned reader |
| `arcweft-lang-syntax::ast::items::TypeAliasItem` | Alias name/target but generic parameters and exact target range are lost | **Replace fields in owner** with name range, type parameters, typed target, typed predicates | Remove old target-only constructor and `Vec<Expr>` type-predicate path |
| `EnumVariant.payload: Option<String>` | Payload is reparsed downstream | **Replace in owner** with `Option<AuthoredTypeRef>` and exact source | Delete all payload reparsing |
| `StructField` / function/impl/trait/entry type fields | Store unspanned `TypeRef` or strings/coarse ranges | **Replace in each owning AST type** with `AuthoredTypeRef` | No owner may recover a nested range by scanning source |
| `arcweft-lang-hir::model::HirModule` / `HirProject` | Module-preserving HIR and source binding | **Keep** | Nominal collection uses `HirProject::modules`, never `linked_module()` |
| `HirProject::linked_module()` | Transitional flattened HIR | **Keep for unrelated consumers** | It is forbidden as nominal publication or resolution input |
| `arcweft-lang-hir::symbol::identity` | World/revision, callable/external IDs | **Extend in `symbol::nominal`** with `ProjectNominalDeclarationId` | ID includes exact world/revision/module/family/owner/name |
| `ProjectDeclarationId` | Callable/external declaration identity | **Extend enum in owner** with `Nominal` | Update all inherent matches; no parallel ID enum |
| `ProjectSymbol` | Callable/external declaration record | **Extend enum in owner** with `Nominal` | No adjacent nominal table |
| `ProjectSymbolTargetId` | Callable/external/module lookup target | **Extend enum in owner** with `Nominal` | Imports/re-exports carry original nominal ID |
| `ResolvedProjectSymbol` | Borrowed target result | **Extend enum in owner** with `Nominal` | No helper wrapper that bypasses the owner enum |
| `ProjectSymbolTable` | Unified declaration scopes, visibility, imports, deterministic fixed point | **Keep and extend** with nominal collection, `resolve_type_target`, completion projection | Remains the sole project/import authority |
| `ProjectSymbolTable::link` unknown-import branch | Silently omits unresolved imports | **Correct concrete defect** | Classify `unknown_import` vs unanchored `cyclic_import`; atomic failure |
| `ProjectSymbolLimits` | Existing alias/import/diagnostic/work bounds | **Extend in owner** with nominal collection bounds | Existing values unchanged; no unbounded side catalog |
| `arcweft-lang-sema::types::TypeKind` | Semantic type enum; `Named(String)` fallback; string generic IDs | **Extend/replace in owner** with typed project/accepted/open/error carriers and typed generic IDs | Authored resolution never creates `Named` fallback |
| `impl From<&TypeRef> for TypeKind` | Context-free successful conversion including `ArcResult` branch | **Delete** | No production reader accepts unresolved `TypeRef` |
| `checker::helpers::type_ref_kind*` | Context-free conversion helpers | **Delete successful paths** after caller migration | No renamed helper or extension trait |
| `checker::signature` | Converts function signatures with context-free helper | **Migrate** to `resolve_type_ref` reports | Return-boundary poison evidence is stored before body checking |
| `checker::module::global_type_aliases` | Spelling-keyed alias map | **Delete** | Alias selection comes from `ProjectSymbolTable` |
| `checker::module::erase_aliases` | Spelling-based normalization | **Delete** | Use typed alias expansion facts and normalized semantic types |
| `checker::module` nominal fields/enum payload maps keyed by `String` | Local structural facts | **Replace** with declaration-ID keyed project records | No unqualified-name structural inventory |
| `checker::module::check_type_ref_shape` | Recursive shape walk and choice duplicate check | **Narrow** to post-resolution shape policy | Alias-normalized choice comparison consumes resolved types |
| `TypeCheckEnv.symbols` | Values plus some type-name acceptance | **Keep for value duties; stop type reads** | Source type resolver reads only accepted nominal catalog |
| `TypeCheckEnv.nominal_records` | Legitimate structural records keyed by string | **Project into `AcceptedNominalCatalog`** | Delete type-name acceptance from legacy map after projection |
| `TypeCheckEnv` enum inventories | Enum variants/payloads keyed by semantic type | **Keep structural inventory; add exact accepted record** | Inventory presence is typed accepted evidence, not a spelling fallback |
| `TypeCheckEnv.rust_packages` | Rust exports and signatures | **Keep package duties; publish type exports to accepted catalog/external seeds** | No direct source-string lookup in checker |
| `EnvironmentBindingId` in registration internals | Environment owner identity | **Move destructively** to `env::identity` | Update imports; no compatibility re-export |
| `RegisteredExternalOwner` / owner registry | Maps external table IDs to character/environment owners | **Keep** | Exact world/revision check precedes external type projection |
| `RegisteredTypeCheckEnv` | Accepted environment/world/revision | **Extend** with immutable accepted nominal catalog/digest | Must match symbol table exactly |
| `CharacterNominalType` | Structural character nominal identity | **Keep** | External character resolution returns existing typed identity, never fake project ID |
| `entry::checker::nominal::NominalSchemaResolver` declaration maps | Re-invent project nominal inventory | **Delete** | Shared project records replace it |
| `NominalSchemaResolver::visible_type_keys` and import reconstruction | Entry-only import/glob/re-export resolver | **Delete** | No second successful resolver |
| `NominalSchemaResolver::resolve_nominal` | Name-to-struct/enum resolution | **Delete** | Consume `ResolvedTypeRef` and declaration ID |
| `NominalSchemaResolver::resolve_alias_target` / alias stack | Entry alias lookup/cycle handling | **Delete** | Consume shared alias facts/diagnostics |
| Entry enum payload parsing | Reparse payload strings | **Delete** | Payload is source-backed typed syntax/HIR |
| Entry schema expansion | Struct/enum schema shape and entry role policy | **Keep, narrow** | Accepts shared declaration records only |
| `EntryContractBuilder::canonical_type_ref` | Duplicate project/alias lookup and canonicalization | **Narrow** to resolved semantic-type canonicalization | Delete project lookup and alias stack |
| `canonical_constructor("ArcResult")` | Alias-name special case | **Delete** | Generic result aliases normalize by declaration ID/substitution |
| `CheckedReturnTarget` | Known/inferred/unresolved boundary selection | **Keep unchanged** | Add side evidence, not enum redesign |
| Try/Await checker | Nearest boundary, operand-success recovery, propagation diagnostics | **Keep and consume poison gate** | Never scan names or source; no cascade from unresolved boundary |
| `ProjectSemanticIndex.types: BTreeMap<TypeName, TypeKind>` | String-keyed Agent-facing type projection | **Narrow/replace for project noms** with ID-keyed records/edges | Environment-only exact records may remain typed; no project display parsing |
| LSP accepted snapshot | Immutable HIR/source/world/revision carrier | **Extend** with nominal resolution index and reference edges | No LSP-only lookup or stale reuse |
| LSP hover/definition/completion/rename | Existing feature-specific projections | **Migrate** to typed IDs, visible bindings, and source facts | No display-string inverse parsing |
| Compiler diagnostic projection | Converts checker errors | **Extend** for structured nominal diagnostics | `TypeKind::Error` blocks final compilation/lowering |
| `arcweft-core` | Core runtime/domain substrate | **No change** | Nominal resolution dependency is forbidden |
| CSS/Takumi/rendering owners | Presentation paths | **No change** | No dependency or code path is introduced |

## Final successful-resolution graph

```text
AuthoredTypeRef + exact source
        |
        v
ProjectSymbolTable::resolve_type_target
  (project/module/import/re-export only)
        |
        +---- project struct/enum/alias ID
        |
        +---- external declaration ID
        |
        +---- typed project error
        v
arcweft_lang_sema::nominal::resolve_type_ref
  + generic scope
  + built-in owner enum
  + Self scope
  + projection policy
  + RegisteredTypeCheckEnv accepted catalog
  + explicit open rules
        |
        v
ResolvedTypeRefOutcome + diagnostics + poison + alias trace
        |
        +---- normal checker / Try / Await
        +---- entry schema consumer
        +---- compiler / project index
        +---- LSP diagnostics and navigation
```

Any additional successful arrow from source `TypeRef` or a rendered name to a
project declaration violates the contract.
