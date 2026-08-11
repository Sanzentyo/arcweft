# PRODUCTION RECONCILIATION

## 1. Audited state

The repository was audited at `main` commit:

```text
23ed5d93824630d8ead9092d32f7fc70f0a8f314
Move View and image products into compiler ownership
```

The request was prepared at:

```text
c56c82240dacc0d254c7d32e17359d4be0f04b41
```

The audited head is 41 commits ahead and 0 commits behind that baseline. The
intervening changes are primarily compiler-owned View/image products, Proof
switch audits, dialogue/profile admission, launch/bundle, and related
tooling. No intervening commit supplies the missing project nominal resolution
authority.

The head was rechecked immediately before contract generation and remained
`23ed5d93824630d8ead9092d32f7fc70f0a8f314`.

## 2. Evidence-backed seams

| Repository path | Current evidence | Contract consequence |
|---|---|---|
| `crates/arcweft-lang-syntax/src/types.rs` | `TypeRef::Path(String)` and `Generic { base: String }`; no complete type-node source map | Add typed `TypePath` and `AuthoredTypeRef`; no downstream display/source parsing |
| `crates/arcweft-lang-syntax/src/ast/items.rs` | enum payload is `Option<String>`; type alias lacks generic parameters/name/target source; several owners hold unspanned types/strings | Directly migrate authored owners and remove payload reparsing |
| `crates/arcweft-lang-syntax/src/parser/type_declaration_grammar.rs` | lossless grammar already recognizes declaration generic parameters and type roles | Preserve grammar substrate; correct typed projection rather than redesign grammar |
| `crates/arcweft-lang-hir/src/model.rs` | HIR preserves top-level declarations and source-bound module HIR | Collect from module-preserving HIR |
| `crates/arcweft-lang-hir/src/project.rs` | `HirProject` preserves module/source identity; `linked_module()` flattens | Do not use flattened HIR for nominal identity/publication |
| `crates/arcweft-lang-hir/src/symbol/identity.rs` | world/revision and callable/external IDs exist; declaration/target enums lack nominal types | Extend the owner enums and add nominal IDs/records |
| `crates/arcweft-lang-hir/src/symbol/table.rs` | one bounded fixed-point project scope/import table with deterministic BTree ordering | Keep it as the sole project authority and publish nominals in the same transaction |
| same | unresolved `ImportResolutionError::Unknown` is omitted in final link reporting | Concrete defect: classify unknown vs unanchored cycles |
| `crates/arcweft-lang-hir/src/symbol/error.rs` | structured link errors/codes and deterministic report cap exist | Extend in owner with unknown/cyclic/reserved/invalid-nominal codes |
| `crates/arcweft-lang-sema/src/types.rs` | unrecognized paths become `Named`; `ArcResult<T>` is a production spelling branch | Delete context-free success and the `ArcResult` branch |
| `crates/arcweft-lang-sema/src/env/base.rs` | legitimate standard/domain/nominal/enum/Rust facts live in `TypeCheckEnv` | Convert them to exact accepted records/open policies; absence from project is not automatically unknown |
| `crates/arcweft-lang-sema/src/registration/model.rs` | registered environment and project symbols already share exact world/revision; external owner registry distinguishes character/environment | Preserve transaction and use it for external nominal projection |
| `crates/arcweft-lang-sema/src/checker/helpers.rs` | `type_ref_kind*` funnels to context-free conversion | Migrate every caller, then delete helpers |
| `crates/arcweft-lang-sema/src/checker/signature.rs` | normal signatures use context-free conversion | Resolve annotations before checked return-boundary/body checking |
| `crates/arcweft-lang-sema/src/checker/module.rs` | spelling-keyed global aliases and erasure; string-keyed nominal inventories | Replace with ID-keyed shared facts and alias traces |
| `crates/arcweft-lang-sema/src/entry/checker/nominal.rs` | private declaration inventory, import visibility, alias expansion/cycles, enum payload reparsing | Delete shared responsibilities; retain entry schema-shape logic only |
| `crates/arcweft-lang-sema/src/entry/checker/contract.rs` | independent canonical type conversion and `ArcResult` constructor | Consume resolved facts and delete project/alias/name branches |
| `crates/arcweft-lang-sema/src/project_index.rs` | context-free conversion import; project struct/enum/alias declarations ignored | Add typed nominal records/reference edges; remove string project projection |
| `crates/arcweft-lsp/src/profiles/accepted_project.rs` | accepted snapshot already validates HIR/source/symbol world/revision atomically | Publish nominal resolution index in this same snapshot |
| `crates/arcweft-source` | exact UTF-8 `SourceSpan` is revision-bound and validates against real documents | Use it for accepted evidence; detached HIR retains local ranges only |

## 3. Dependency proof

Current Cargo direction is suitable:

```text
arcweft-lang-syntax
        |
        v
arcweft-lang-hir ----> arcweft-source
        |
        v
arcweft-lang-sema ----> accepted domain/character/environment crates
        |
        +----> compiler / project index / LSP consumers
```

`arcweft-lang-hir` does not depend on sema and SHALL continue not to. HIR owns
project identity/table/records because those are independent of `TypeKind`.
Sema consumes the table and environment because only sema can produce checked
types, generic substitution, `Self`, projection, diagnostics, and poison.

No dependency on `arcweft-core`, rendering, CSS, or Takumi is introduced.

## 4. Concrete preservation decisions

### Try and Await

No syntax or semantic shape is redesigned. The nominal implementation consumes
the existing typed operator source and selected checked-return-boundary
contract. The sole addition is side evidence connecting an unresolved boundary
to authoritative type poison.

### Callable catalog and identities

`CallableDeclarationId`, AW-AH-009.3 publication/resolution, callable
effect/fixed-point behavior, and current callable target semantics remain.
Nominal declarations join the same project table but do not replace callable
records or introduce callable aliases.

### Anonymous choices

The existing choice duplicate diagnostic remains. Only its input changes from
string/spelling erasure to fully resolved alias-normalized semantic types.

### Character registration

The existing source-backed character identity and external-owner integrity
remain. Character types participate as accepted/external outcomes and are not
project declaration IDs.

### Accepted project snapshot

Existing exact source, module, HIR, symbol-world, symbol-revision, and
environment checks remain. Nominal facts are added to that transaction rather
than published later.

## 5. Direct deletions required

The implementation is not production-reconciled while any of these successful
paths remain:

```text
TypeKind::from(&TypeRef) used by authored checking
type_ref_kind / type_ref_kind_with_generics authored fallback
checker-local string alias maps or erasure
entry-local project/import/alias resolution
entry enum-payload reparsing
ArcResult canonical constructor or TypeKind branch
Unknown spelling special case
LSP/project-index display-string project lookup
arbitrary TypeKind::Named acceptance for source paths
silent unknown import omission
```

A temporary compile failure inside a migration branch is resolved within the
same implementation cut; it is not addressed with a compatibility adapter.

## 6. Production transaction after reconciliation

```text
parse exact typed syntax
    -> lower module-preserving, source-bound HIR
    -> collect module/callable/nominal/external declarations
    -> resolve imports/re-exports and validate collisions/limits
    -> publish ProjectSymbolTable only on success
    -> register exact environment/character/external catalog for same world
    -> resolve all authored type references recursively
    -> publish diagnostics, poison evidence, alias traces, reference edges
    -> normal/entry/body/propagation checking
    -> compiler/project-index/LSP projections
```

There is no point at which a successful consumer sees nominal declarations
without the corresponding import world, source revision, or accepted
environment.

## 7. Reconciliation status

```text
DESIGN_STATUS=READY
IMPLEMENTATION_PERFORMED=false
PRODUCTION_CHANGES=false
OPEN_QUESTIONS=0
AUDITED_MAIN=23ed5d93824630d8ead9092d32f7fc70f0a8f314
```
