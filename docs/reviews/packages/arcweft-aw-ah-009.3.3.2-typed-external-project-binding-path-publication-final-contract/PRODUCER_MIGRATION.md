# Current-producer migration

## 1. Migration rule

The old string-only construction path is deleted first at the API boundary and every current caller is migrated in the same coherent cut. There is no period in which both constructors compile, no compatibility feature, no deprecated wrapper, and no dual reader.

Every producer must supply a validated `ProjectSymbolPath` whose root is `ModulePathRoot::ImplicitCrate`. Formatting may be used for generated source or opaque canonical IDs only after the typed path exists; formatting is never used to recover segments.

## 2. Current `ProjectDirectBinding::try_new` caller inventory

Code search at inspected `main` found exactly these files calling `ProjectDirectBinding::try_new`:

| Current file | Current role | Required target migration |
|---|---|---|
| `crates/arcweft-project-loader/src/environment.rs` | production character registration facts | construct qualified and compact `ProjectSymbolPath` values from `CharacterId::compact_segments()`; keep `canonical_path` opaque |
| `crates/arcweft-adapter-context/src/manifest.rs` | production adapter environment facts | consume typed `AdapterSymbolPath`, convert each segment to `ProjectSymbolSegment`, and publish the resulting path |
| `crates/arcweft-compiler/src/project.rs` | compiler registration rollback fixture | replace the `environment.missing` string with a fixture-built two-segment path |
| `crates/arcweft-lang-hir/src/symbol/tests.rs` | HIR linker fixtures | change fixture inputs from strings to explicit segment vectors; add path-retention assertions |
| `crates/arcweft-lang-sema/src/test_support/character_project.rs` | shared registration fixtures | change `external_fact` to accept typed paths, not `&[&str]`; construct character paths from `compact_segments()` |
| `crates/arcweft-lang-sema/tests/character_manifest_types.rs` | integration fixtures | replace direct string bindings with typed path values |

The compiler enforces completeness after the old signature is deleted. Any additional branch introduced after the inspected commit must be migrated by the same typed rule before the cut can land.

## 3. Character producer

### 3.1 Exact construction

`character_registration_source` performs the following typed construction once:

```rust
let compact_segments = owner
    .compact_segments()
    .map(|segment| ProjectSymbolSegment::try_new(segment.to_owned()))
    .collect::<Result<Vec<_>, ProjectSymbolPathError>>()?;

let qualified_path = ProjectSymbolPath::new(
    ModulePathRoot::ImplicitCrate,
    std::iter::once(ProjectSymbolSegment::try_new("character")?)
        .chain(compact_segments.iter().cloned()),
)?;

let compact_path = ProjectSymbolPath::new(
    ModulePathRoot::ImplicitCrate,
    compact_segments,
)?;

let direct_bindings = [
    ProjectDirectBinding::try_new(
        CanonicalModulePath::crate_root(),
        qualified_path,
        Some(Visibility::Public),
        declaration.clone(),
        false,
    )?,
    ProjectDirectBinding::try_new(
        CanonicalModulePath::crate_root(),
        compact_path,
        Some(Visibility::Public),
        declaration.clone(),
        false,
    )?,
];
```

The seed's canonical path remains constructed from `owner.as_str()` as one opaque `SymbolPath` leaf. This is a write-only projection from a validated `CharacterId`, not a path parser.

### 3.2 Authored aliases

A producer that owns an authored alias constructs it directly:

```rust
let hero = ProjectSymbolPath::new(
    ModulePathRoot::ImplicitCrate,
    [ProjectSymbolSegment::try_new("hero")?],
)?;

let binding = ProjectDirectBinding::try_new(
    module,
    hero,
    visibility,
    alias_source,
    true,
)?;
```

The target seed and external declaration ID remain the same as the qualified and compact bindings. `authored_alias` is retained for direct-binding provenance but does not participate in scope identity or canonical identity.

### 3.3 Registrar audit

`audit_character_spellings` and related collision diagnostics must stop doing either of the following:

- `character.as_str().strip_prefix("character.")`;
- reconstructing a `SymbolPath` from `collision.spelling()`.

They construct expected qualified/compact `ProjectSymbolPath` values from `compact_segments()` and compare them with `ProjectSymbolBindingCollision::path()` or typed scope iterator rows. Diagnostic text may use `Display` after the identity comparison is complete.

## 4. Adapter manifest producer

### 4.1 Public model migration

Every programmatic adapter symbol changes from:

```rust
manifest.with_symbol("adapter.viewport", ty)
```

to:

```rust
let path = AdapterSymbolPath::try_new([
    AdapterSymbolSegment::try_new("adapter")?,
    AdapterSymbolSegment::try_new("viewport")?,
])?;

manifest.with_symbol(AdapterSymbol::new(path, ty))
```

Standard manifests, LSP profile fixtures, verifier fixtures, and adapter-context tests must import the new typed symbol API. No shared string helper or extension trait is introduced. Repeated static fixtures may use a local test-only constructor that calls the public validating constructors and `expect`s literal validity; production APIs remain typed.

### 4.2 Codec migration

The serialized schema remains:

```text
symbols[].name: String
symbols[].ty: String
```

`AdapterManifestFile::into_manifest` immediately converts `name` with the private `symbol_path_from_file` parser and stores only `AdapterSymbolPath`. There is no second `segments` field, alternate version, old untyped manifest field, or compatibility fallback.

The private parser has these exact outcomes:

| Source field | Result |
|---|---|
| `adapter.viewport` | segments `adapter`, `viewport` |
| `adapter.hero-pack` | segments `adapter`, `hero-pack` |
| empty string | `AdapterSymbolPathError::Empty` |
| `adapter..viewport` | `AdapterSymbolPathError::EmptySegment` |
| `adapter/view` | `AdapterSymbolPathError::InvalidSegment` |
| `adapter:viewport` | `AdapterSymbolPathError::InvalidSegment` |
| `adapter.\u{0007}` | `AdapterSymbolPathError::InvalidSegment` |
| `2d.viewport` | `AdapterSymbolPathError::InvalidImplicitRoot` |

The parser is a source-schema boundary, not a sema or catalog path split.

### 4.3 Registration facts

For each `AdapterSymbol`:

```rust
let project_path = ProjectSymbolPath::new(
    ModulePathRoot::ImplicitCrate,
    symbol
        .path()
        .segments()
        .iter()
        .map(|segment| ProjectSymbolSegment::try_new(segment.as_str().to_owned()))
        .collect::<Result<Vec<_>, ProjectSymbolPathError>>()?,
)?;

let spelling = symbol.path().to_string();
let canonical_path = SymbolPath::try_new(
    ModulePathRoot::ImplicitCrate,
    Vec::new(),
    spelling.clone(),
)?;
let direct_binding = ProjectDirectBinding::try_new(
    CanonicalModulePath::crate_root(),
    project_path,
    Some(Visibility::Public),
    declaration.clone(),
    false,
)?;
let environment_id = EnvironmentBindingId::try_new(spelling)?;
```

The rendered `spelling` serves generated source, the existing opaque canonical external leaf, and the existing environment binding ID. It never feeds `ProjectSymbolPath` construction.

## 5. HIR direct producers

### 5.1 Modules

`insert_module_bindings` creates an implicit one-segment path from the module's validated final `ModuleSegment`. The containing scope remains the module parent.

### 5.2 Source callables

`insert_callables` creates an implicit one-segment path from the already validated declaration name. Callable declaration identity remains `CallableDeclarationId`; the source-visible path is scope evidence only.

### 5.3 Externals

`insert_externals` clones `binding.path()` directly. It no longer receives or copies `binding.name()`.

## 6. Import and re-export producers

| Producer | Destination typed path |
|---|---|
| ordinary unaliased path import | `LinkedProjectSymbolPath::unaliased_binding()` |
| explicit `as hero` | one segment converted from typed `UseAlias::name()` |
| grouped selected item | one segment cloned from typed `UseName::name()` |
| grouped selected item with alias | one segment converted from typed alias |
| glob | exact source `ScopeBinding::path` |
| fixed-point re-export | exact destination path already carried by the rebound row |

The current target lookup, visibility check, and source-site selection are unchanged.

## 7. Sema registration and catalog producer

`CharacterRegistrar::register` retains its current closure for mapping a HIR target to a non-callable `TypeKind`. Only the scope iterator item changes from `&str` to `&ProjectSymbolPath`.

The builder maps every `ProjectSymbolSegment` independently to `CallableName`, uses the existing `CallablePath::try_new_with_limits`, and pushes the same `ProjectNameBinding` variants as today. The current invalid-name skip and its explanatory comment are deleted.

The character registrar's collision audit uses typed paths. No adapter type is imported into sema.

## 8. Error migration

| Owner | New error propagation | Existing error retained for |
|---|---|---|
| HIR | `ProjectDirectBindingError` | direct-binding root invariant |
| project loader | `ProjectSymbolPathError`, `ProjectDirectBindingError` | character source-visible paths |
| adapter codec | `AdapterSymbolPathError` | serialized adapter symbol field |
| adapter registration facts | `ProjectSymbolPathError`, `ProjectDirectBindingError` | adapter-to-project publication |
| sema catalog | existing `CallableBuildLimitError::PathSegments`, `WorkOverflow`, `MissingProjectBindingType`, `ProjectBindingCollision` | catalog conversion/publication |

No new catalog error is needed for segment syntax because the strict project-segment invariant makes `CallableName` conversion infallible. A path-length limit remains a normal existing catalog build failure.

## 9. Migration completion criterion

The producer migration is complete only when the old HIR constructor signature no longer exists and every production/test caller compiles against the typed signature. A call-site compatibility layer, feature-selected old signature, `Into<ProjectSymbolPath>` string bridge, `FromStr` bridge, or deprecated overload fails this contract.
