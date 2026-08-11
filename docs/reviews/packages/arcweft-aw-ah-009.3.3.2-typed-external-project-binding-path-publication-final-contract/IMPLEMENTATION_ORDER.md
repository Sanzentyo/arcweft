# Required implementation order

The seven requested stages are normative and must not be reordered. The implementation may use intermediate local commits, but the branch must move directly to the final typed model; no intermediate compatibility API may be merged or left in the final diff.

## 1. Add the typed binding-path owner and direct constructor/accessor tests

### Production work

1. Reuse the existing `ProjectSymbolPath` and `ProjectSymbolSegment`; do not create a second HIR project path type.
2. Add `ProjectDirectBindingError` to HIR and re-export it from `arcweft_lang_hir::symbol`.
3. Replace `ProjectDirectBinding.name: String` with `path: ProjectSymbolPath`.
4. Replace its constructor signature and `name()` accessor with the exact typed constructor and `path()` accessor.
5. Add the new language-free `AdapterSymbolSegment`, `AdapterSymbolPath`, and `AdapterSymbolPathError` in adapter-context.
6. Add typed `AdapterSymbol` construction/access and the direct `AdapterManifest::with_symbol(AdapterSymbol)` API.

### Tests before proceeding

- syntax segment/path validation;
- direct binding retains path and rejects explicit roots;
- adapter path validates exact segments without `sema`;
- canonical external path versus direct binding path distinction.

### Stage gate

The old HIR/adapter string constructors are removed, so downstream callers may temporarily fail to compile inside the local branch. Do not reintroduce overloads to make the stage compile in isolation; proceed immediately to stage 2 within the same coherent change.

## 2. Replace string-only construction and migrate every producer in one cut

### Production migrations

1. `arcweft-project-loader`: construct character qualified/compact paths from `CharacterId::compact_segments()`.
2. `arcweft-adapter-context::codec`: parse the existing schema-v1 symbol source field directly into `AdapterSymbolPath`.
3. `arcweft-adapter-context::standard`: construct every standard symbol through typed segments.
4. `arcweft-adapter-context::manifest::source_backed_registration_facts`: convert typed adapter segments to typed project segments before rendering canonical/generated strings.
5. `arcweft-compiler` registration fixtures: replace string direct bindings.
6. HIR linker tests and all sema registration fixtures/integration tests: accept/build typed paths.
7. Any current-main caller found by the compiler after constructor deletion: apply the same rule; do not add a bridge.

### Error propagation

- project loader adds `ProjectSymbolPathError` and `ProjectDirectBindingError` variants;
- adapter codec adds `AdapterSymbolPathError`;
- adapter fact publication adds `ProjectSymbolPathError` and `ProjectDirectBindingError`.

### Stage gate

All production and test call sites compile against the typed constructor. No `ProjectDirectBinding::try_new(..., &str/String, ...)`, `AdapterSymbol::new(String, ...)`, or `with_symbol(String, ...)` remains.

## 3. Retain typed paths through `ProjectSymbolTable`

### Production work

1. Add `path: ProjectSymbolPath` to private `ScopeBinding`.
2. Change `ScopeBinding::new` and `rebound` to require the exact destination path.
3. Change `insert_scope_binding` to accept only a `ScopeBinding`; derive the private rendered key after the typed row exists.
4. Construct one-segment typed paths for modules and source callables.
5. Clone exact direct external paths.
6. Add private `LinkedProjectSymbolPath` for path imports.
7. Change `import_bindings` to return `Vec<ScopeBinding>`.
8. Preserve exact paths through explicit aliases, grouped imports, globs, re-exports, and fixed-point iterations.
9. Include path in coalescing and deterministic row ordering.
10. Replace collision `spelling` with typed `path` and migrate registrar diagnostics/audits.
11. Delete character `strip_prefix` reconstruction in sema.

### Behavioral constraints

Do not change:

- target lookup;
- visibility/accessibility checks;
- unknown import omission;
- ambiguity and visibility escalation errors;
- alias/import/work/diagnostic limits;
- fixed-point loop and termination.

### Stage tests

Run HIR direct/import/glob/re-export/alias/hyphen/coalescing/determinism tests and registration collision audits.

## 4. Expose one deterministic typed scope-binding iterator

### Production work

1. Directly replace the old iterator item `(&CanonicalModulePath, &str, &ProjectSymbolTargetId)` with `(&CanonicalModulePath, &ProjectSymbolPath, &ProjectSymbolTargetId)`.
2. Ensure per-key vectors are sorted by `(path, target, visibility, owner)` before iteration.
3. Keep private rendered keys private.
4. Do not add a second/legacy iterator.

### Stage tests

- exact iterator item paths;
- mixed callable/module/external rows;
- reversed insertion order equality;
- same-target different-path distinction.

## 5. Publish every callable and non-callable binding

### Production work

1. Change `RegisteredCallableCatalogBuilder::add_project_bindings` to consume the typed iterator.
2. Charge one row plus one work unit per typed segment.
3. Convert each `ProjectSymbolSegment` directly to `CallableName`.
4. Use existing `CallablePath::try_new_with_limits`; map path-length failure to existing `CallableBuildLimitError::PathSegments`.
5. Preserve existing `ProjectNameBinding` mapping and `TypeKind` closure.
6. Delete the complete-spelling `CallableName::try_new`, invalid-name `continue`, and explanatory temporary comment.
7. Keep `finish_project`, catalog record types, resolver, and shadow precedence unchanged.
8. Migrate registrar collision diagnostics to typed paths; do not import adapter types.

### Stage tests

- qualified character/adapter publication;
- complete callable/module/external row publication;
- non-callable qualified/alias environment fallback termination;
- path limit/work accounting;
- reversed catalog equality;
- deterministic collision.

## 6. Add accepted-world atomicity tests

### Required tests

1. A malformed character/adapter typed path fails at the producer constructor or codec before registration and leaves the prior accepted pointer/generation unchanged.
2. Unequal project bindings at one typed path produce the existing catalog collision and no candidate world.
3. The LSP/profile accepted-world owner proves `Arc::ptr_eq` with the previous accepted world after the collision.
4. Symbols, registered environment, callable catalog, character definitions, and generation remain the previous accepted objects.
5. A subsequent valid update succeeds, proving no residue from the rejected candidate.

### Constraint

Do not add a rollback API. The test must validate the existing construct-then-publish transaction.

## 7. Run focused, workspace, metadata, and structural validation

Run the exact sequence in `VALIDATION_PLAN.md`:

1. formatting;
2. focused syntax/HIR/character/adapter/loader/sema/compiler/LSP tests;
3. Cargo metadata dependency evidence;
4. workspace check;
5. workspace clippy with `-D warnings`;
6. workspace all-target/all-feature tests;
7. canonical nightly structural audit.

Record the exact commit, command, exit status, and any pre-existing warning separately. A command not run is not reported as passed.

## Coherent-cut rule

The implementation is one direct replacement. Before merge, all of these must be true simultaneously:

- typed producer models exist;
- all producers use them;
- HIR retains paths;
- the only scope iterator is typed;
- the catalog publishes every row;
- atomicity tests pass;
- old string APIs/branches are deleted.

A branch that stops after any partial stage is not a conforming deliverable.
