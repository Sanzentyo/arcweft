# Deterministic linker and catalog publication rules

## 1. Binding identity

A linked project binding is identified by the tuple:

```text
(scope module: CanonicalModulePath,
 local path: ProjectSymbolPath with ImplicitCrate root,
 target: ProjectSymbolTargetId)
```

Visibility, owner module, and source sites are retained as link/provenance facts. Canonical external identity is not part of the local binding key.

The private rendered `String` used by the current HIR scope map is a lookup accelerator only. It is generated from `ProjectSymbolPath` and is never parsed.

## 2. Direct insertion state transitions

| Input | Scope owner | Installed path | Target | Notes |
|---|---|---|---|---|
| project module `crate.cast` | crate root | `['cast']` | `Module(crate.cast)` | existing public module visibility |
| function `render` in `crate.ui` | `crate.ui` | `['render']` | `Callable(id)` | current duplicate declaration checks unchanged |
| external qualified binding | binding's `module` | exact typed segments, e.g. `['character','akane']` | `External(id)` | no opaque leaf reinterpretation |
| compact binding | binding's `module` | exact one or more compact segments, e.g. `['akane']` | same `External(id)` | independent binding row |
| authored alias | binding's `module` | exact alias path, e.g. `['hero']` | same `External(id)` | canonical identity unchanged |

`insert_externals` does not derive a path from `ExternalDeclarationSeed::canonical_path`.

## 3. Path import behavior

### 3.1 Typed reference/binding pair

`LinkedProjectSymbolPath` carries:

- `reference`: the existing `SymbolPath` used by `targets_for_symbol_path`;
- `unaliased_binding`: the exact destination path installed when no `as` alias exists.

Examples:

| Authored source path | Resolution reference | Unaliased destination binding |
|---|---|---|
| `character.akane` | implicit qualifiers `character`, leaf `akane` | `['akane']` |
| `crate.character.akane` | crate qualifiers `character`, leaf `akane` | `['akane']` |
| `character.hero-pack.2d` | existing opaque external-root `SymbolPath` conversion | `['character','hero-pack','2d']` |
| `crate.character.hero-pack` | existing `InvalidPath` because explicit roots cannot collapse external-only qualifiers | none |
| `akane` | simple implicit leaf | `['akane']` |

The destination rule preserves current ordinary import semantics while retaining external-only source evidence where the current resolver already treats the whole implicit spelling as an opaque root binding.

### 3.2 Explicit alias

For `use character.akane as hero`, lookup uses the same typed reference as the unaliased import. The installed path is exactly `['hero']`. The original `['character','akane']` binding remains in its source scope. The alias does not replace the external canonical path.

### 3.3 Grouped import

For `use crate.cast.{akane, hero as lead}`:

- each selected target is resolved with the existing grouped path algorithm;
- `akane` installs `['akane']`;
- `hero as lead` installs `['lead']`;
- typed selected/alias tokens supply the destination segments;
- no `SymbolPath::leaf()` value is parsed or split.

### 3.4 Glob and re-export

A glob copies each visible candidate's exact `ScopeBinding::path`. Re-exporting changes only owner, requested visibility, and site. Fixed-point repeats therefore cannot lose path segmentation.

## 4. Coalescing and fixed point

### 4.1 Monotone insertion

Insertion remains monotone. A new row either:

- adds a distinct `(path, target, visibility, owner)` row; or
- extends the canonical site set of an existing exact row.

No insertion deletes, renames, or normalizes another path. The fixed-point loop stops under the same existing `changed` condition and work limits.

### 4.2 Deterministic vector order

Each per-key vector is sorted by:

```text
path
then target
then visibility
then owner
```

Because exact rows coalesce, site vectors do not need to break ties. Sites retain the existing `(source document id, revision, range)` order.

### 4.3 Duplicate target resolution

`dedup_bindings` compares `(path, target)` rather than target alone. Within one private rendered key, the path must be equal by insertion invariant; including the path makes that invariant explicit and prevents accidental evidence loss in future changes.

## 5. Typed iterator order

`ProjectSymbolTable::scope_bindings()` emits rows in this exact order:

1. `CanonicalModulePath` ascending order from the outer `BTreeMap`;
2. generated private lookup key ascending order from the inner `BTreeMap`;
3. sorted `ScopeBinding` order from section 4.2.

The iterator yields `(&CanonicalModulePath, &ProjectSymbolPath, &ProjectSymbolTargetId)`. It exposes no mutable row, site vector, visibility, owner, or rendered spelling.

A reversed external seed/direct-binding insertion order must emit byte-for-byte equivalent path/target sequences.

## 6. Callable catalog publication

For every iterator row:

1. charge one existing project-binding work unit;
2. charge one existing path-segment work unit per `ProjectSymbolSegment`;
3. convert each segment directly with `CallableName::try_new(segment.as_str())`;
4. enforce `CallableLimits::max_path_segments` with `CallablePath::try_new_with_limits`;
5. construct `ProjectCallablePath` from current package, scope module, and segmented callable path;
6. map the target to the current `ProjectNameBinding` variant;
7. append the pair to the existing `project_bindings` vector;
8. let current `finish_project` build the immutable map or return the current typed collision error.

The builder does not consult `SymbolPath`, external canonical paths, labels, aliases, comments, source text, Rust paths, or adapter types.

## 7. Non-callable type projection

The current sema-owned closure remains exact:

| HIR target | `TypeKind` publication |
|---|---|
| registered character external | `Ref(Character)` with the current entity type construction |
| registered environment external | exact type from `TypeCheckEnv::environment_binding` |
| module | current `Named("Module")` representation |
| callable | not a non-callable type; publish `ProjectNameBinding::Callable` |
| unowned external | existing `MissingProjectBindingType` failure |

No `TypeKind` is stored in HIR. No adapter type crosses into sema.

## 8. Catalog duplicate and collision rules

The current `finish_project` behavior remains normative:

- first occurrence of a path inserts its binding;
- a later identical binding at the same path is accepted;
- a later unequal binding at the same path returns `ProjectBindingCollision`;
- the complete candidate catalog is discarded on error.

Because iterator order is fixed before `HashMap` insertion, the reported `first` and `second` evidence is deterministic. The `HashMap`'s iteration order is not part of any API or test.

## 9. Resolver precedence

The existing successful resolver path remains single and unchanged:

1. language-reserved/special families retain their existing precedence;
2. lexical/project lookup uses the accepted typed project binding map;
3. a project callable resolves through its declaration record;
4. a project non-callable returns a non-callable result and stops;
5. environment free-call fallback is attempted only when no project binding owns the exact segmented path.

The correction makes qualified external shadows visible to step 4; it does not add another resolver.

## 10. Collision/ambiguity outcome table

| Condition | HIR result | Catalog/transaction result |
|---|---|---|
| same exact path and target from multiple sites | coalesced sites | one equivalent binding |
| qualified and compact paths target same external | two distinct rows | two distinct map keys |
| qualified, compact, and authored alias target same external | three distinct rows | three distinct map keys |
| same path, different targets | ambiguity/collision evidence retained | `ProjectBindingCollision`; candidate rejected |
| inaccessible import | existing link diagnostic; no row | catalog not constructed from invalid link |
| visibility escalation | existing link diagnostic; no row | candidate rejected before catalog |
| invalid segment at producer | no HIR seed/row | candidate never starts |
| path exceeds callable segment limit | valid HIR row | existing `PathSegments` catalog limit error; candidate rejected |
| missing external type owner | valid HIR row | existing `MissingProjectBindingType`; candidate rejected |

## 11. Atomic accepted-world rule

No object reachable from the previous accepted world is mutated while constructing the candidate. Symbol linking, external owner validation, complete callable catalog construction, and definition indexing must all succeed before the candidate `RegisteredSemanticWorld` is returned to the existing publication owner.

A malformed typed path and a path collision therefore have the same observable state rule:

```text
accepted_world_after_error is pointer-equal to accepted_world_before_attempt
```

The accepted generation, symbols, environment, callables, character definitions, and caches remain the previous objects.

## 12. Forbidden paths

The following are non-conforming even if tests appear to pass:

- splitting `SymbolPath::leaf()`;
- splitting or stripping `CharacterId::as_str()` outside the owning type;
- splitting an adapter symbol label in sema or the catalog builder;
- constructing one `CallableName` from a complete rendered path;
- omitting a row after a failed scalar conversion;
- keeping both typed and string scope iterators;
- reparsing `ProjectSymbolPath::to_string()`;
- using external canonical identity as an alias key;
- adding a catalog-side or adapter-side project resolver;
- gating behavior on source text, CSS, Takumi, a feature-selected old API, or repository source scans.
