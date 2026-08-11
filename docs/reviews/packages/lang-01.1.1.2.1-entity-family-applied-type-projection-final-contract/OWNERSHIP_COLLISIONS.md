# Ownership and collision contract

## Layer ownership

| Layer | Owns | Must not own |
|---|---|---|
| syntax | existing generic `TypeRef`, exact node source map | semantic `Ref` projection or name lookup |
| HIR | source-backed type references; project/external publication; direct reserved-name rejection | `EntityKind` projection or accepted catalog semantics |
| sema `types` | `EntityKind` authored inventory; `EntityType`; `TypeKind::Ref`; inherent `TypeKind::entity_ref` | path/import lookup |
| sema `nominal::model` | corrected closed `BuiltinTypeConstructor`; typed argument expectation; resolution facts | project publication |
| sema recursive resolver | contextual child classification, exact arity/kind checks, diagnostics/poison/work | second resolver or consumer-local conversion |
| accepted catalog | exact/open non-reserved names only | `Ref` ownership or dependent entity projection |
| checker/callable/entry | consume checked `TypeKind` and shared report/index | re-resolve `Ref` spelling |
| project semantic index | retain checked semantic products and valid project identity edges | fabricate project identity for `Ref`/entity-family atoms |
| LSP | typed presentation, completion policy, navigation/edit policy | display-string reverse parsing or virtual definitions |
| runtime-plan/verify | consume existing `TypeKind::Ref(EntityType)` | source-name resolution |
| bytecode/save/replay | only existing explicit schema boundaries | implicit `Named("Ref<...>")` encoding or dual readers |

## Resolution precedence

For a direct implicit-crate name:

1. scoped `Self` / generic parameter;
2. closed `BuiltinTypeConstructor`, including `Ref`;
3. accepted project nominal or external binding;
4. accepted exact catalog;
5. accepted/detached open rule;
6. unknown or detached-unavailable.

Lexical generic parameters keep the existing scoped precedence. A bare `Ref`
may therefore denote an explicitly scoped zero-arity generic binding, while
`Ref<...>` still proceeds to the arity-one builtin because a generic binding
does not accept arguments. This is lexical scope, not a project/external/open
registration collision, and no special binder ban is introduced.

## Collision table

| Attempt | Result | Existing typed error/owner |
|---|---|---|
| `struct Ref { ... }` | reject publication | `ProjectSymbolLinkError::ReservedTypeName`; code `aw.project.symbol.reserved_type_name` |
| `enum Ref { ... }` | reject publication | same |
| `type Ref = ...` | reject publication | same |
| callable whose published type namespace name is `Ref` | reject where current reservation applies | same |
| direct external binding/import alias `Ref` | reject publication | same |
| accepted exact record `Ref` | reject catalog construction | `AcceptedNominalCatalogError::ReservedPath` |
| open exact rule `Ref` | reject rule construction | `InvalidOpenPattern { reason: ReservedPath }` |
| open namespace rooted at `Ref` | reject rule construction | same |
| qualified accepted/external `pkg::Ref` | allow, but only explicit qualified path selects it | its typed qualified owner |
| project nominal `Character` | allow outside contextual slot | normal project identity |
| `Ref<Character>` when project also declares `Character` | entity family wins inside slot | `EntityKind::Character` |
| `Ref<ProjectOnly>` | constructor wins; argument resolves project then fails kind | `Builtin(Ref)` + `WrongArgumentKind` |

## No accidental domain shadowing

The contextual constructor name `Ref` is globally direct-reserved. Entity
family atoms are contextual rather than globally reserved: this preserves
legitimate project names while making the entity-family slot deterministic.
An unqualified fixed family name in that slot never consults project, external,
accepted, or open registries. Qualified names are not family atoms.
