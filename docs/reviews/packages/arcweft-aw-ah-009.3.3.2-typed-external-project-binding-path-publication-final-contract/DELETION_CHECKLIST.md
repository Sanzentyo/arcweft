# Direct replacement and deletion checklist

Every checked item is required in the implementation diff. Deletion is compile-enforced and review-enforced; no production source-text gate is added.

## 1. HIR direct-binding API

- [ ] Delete `ProjectDirectBinding { name: String, ... }`.
- [ ] Add `path: ProjectSymbolPath` in its place.
- [ ] Delete the string-taking `ProjectDirectBinding::try_new(..., name: impl Into<String>, ...)`.
- [ ] Add only the typed `try_new(..., path: ProjectSymbolPath, ...)`.
- [ ] Delete `ProjectDirectBinding::name()`.
- [ ] Add only `ProjectDirectBinding::path()`.
- [ ] Do not add `try_new_dotted`, `from_name`, `from_spelling`, `Into<ProjectSymbolPath>`, `From<&str>`, `From<String>`, a deprecated overload, or a feature-selected old signature.

## 2. HIR scope storage and iterator

- [ ] Add `ScopeBinding::path`.
- [ ] Delete the separate string/name argument from `ScopeBinding::new`, `rebound`, and `insert_scope_binding`.
- [ ] Change `import_bindings` from `Vec<(String, ScopeBinding)>` to `Vec<ScopeBinding>`.
- [ ] Delete every call that constructs a scope row without a typed path.
- [ ] Delete the old `scope_bindings` item containing `&str`.
- [ ] Expose only the typed iterator item containing `&ProjectSymbolPath`.
- [ ] Do not add a second spelling iterator or extension trait.
- [ ] Keep the private rendered map key only as a generated lookup accelerator.

## 3. Collision evidence

- [ ] Delete `ProjectSymbolBindingCollision.spelling: String`.
- [ ] Delete `ProjectSymbolBindingCollision::spelling()`.
- [ ] Add `path: ProjectSymbolPath` and `path()`.
- [ ] Migrate every diagnostic/registrar caller to typed path evidence.
- [ ] Do not preserve a deprecated spelling accessor.

## 4. Character producer and registrar

- [ ] Delete direct-binding construction from `[owner.as_str(), owner.compact_str()]` strings.
- [ ] Construct both paths from `CharacterId::compact_segments()` and the typed literal `character` segment.
- [ ] Keep only the canonical external leaf projection from `owner.as_str()`.
- [ ] Delete sema-side `strip_prefix("character.")` reconstruction.
- [ ] Delete any sema-side `split('.')` of character IDs or bindings.
- [ ] Build expected audit paths from typed segments.

## 5. Adapter manifest model

- [ ] Delete `AdapterSymbol.name: String`.
- [ ] Add `AdapterSymbol.path: AdapterSymbolPath`.
- [ ] Delete `AdapterSymbol::new(name, ty)`.
- [ ] Add only `AdapterSymbol::new(path, ty)`.
- [ ] Delete `AdapterSymbol::name()`.
- [ ] Add only `AdapterSymbol::path()`.
- [ ] Delete `AdapterManifest::with_symbol(name, ty)`.
- [ ] Add only `AdapterManifest::with_symbol(AdapterSymbol)`.
- [ ] Do not reuse `AdapterCallablePath` for non-callable symbol identity.
- [ ] Do not make the adapter base model depend unconditionally on syntax/HIR/sema.

## 6. Adapter codec

- [ ] Keep schema version 1 and the existing single `symbols[].name` source field.
- [ ] Decode that field immediately into `AdapterSymbolPath`.
- [ ] Do not retain a string in the typed manifest.
- [ ] Do not add a `segments` alternative field.
- [ ] Do not accept both old and new manifest object shapes.
- [ ] Keep the dotted split private to `codec.rs`.
- [ ] Do not expose `FromStr` or a dotted public constructor for `AdapterSymbolPath`.

## 7. Adapter registration facts

- [ ] Construct `ProjectSymbolPath` from individual `AdapterSymbolSegment` values.
- [ ] Render only after typed construction for generated source/canonical ID/environment ID.
- [ ] Delete direct binding construction from the rendered adapter path string.
- [ ] Sort symbols by typed path, not a reparsed label.

## 8. Callable catalog builder

- [ ] Delete `CallableName::try_new(spelling)` where `spelling` is a complete project binding.
- [ ] Delete the `let Ok(name) = ... else { continue; }` branch.
- [ ] Delete the temporary comment explaining that qualified external leaves are omitted.
- [ ] Construct one `CallableName` per `ProjectSymbolSegment`.
- [ ] Use the existing path-segment limit and work accounting.
- [ ] Publish one catalog binding per typed iterator row or return a typed failure.
- [ ] Do not add a fallback to `SymbolPath`, `leaf()`, labels, aliases, source text, or formatted paths.

## 9. Resolver and transaction

- [ ] Do not add or retain a second project-binding resolver.
- [ ] Do not add a catalog-only lookup fallback to `ProjectSymbolTable` for omitted rows.
- [ ] Do not change project-before-environment shadow precedence.
- [ ] Do not add rollback mutation, dual accepted objects, or partial catalog publication.
- [ ] Keep candidate construction fail-closed before accepted pointer publication.

## 10. Test and fixture migration

- [ ] Migrate all six current files found by `ProjectDirectBinding::try_new` code search.
- [ ] Migrate standard adapter manifests and every adapter symbol fixture to typed paths.
- [ ] Change shared sema fixture helpers to accept typed paths, not `&[&str]`.
- [ ] Add mandatory direct tests from `TEST_MATRIX.md`.
- [ ] Use compilation and Cargo metadata for public/dependency evidence.
- [ ] Do not add a repository source scan as a test or gate.

## 11. Explicit forbidden additions

- [ ] No compatibility shim.
- [ ] No deprecated wrapper.
- [ ] No dual reader.
- [ ] No extension trait around Arcweft-owned types.
- [ ] No source gate.
- [ ] No source-text identity parser.
- [ ] No CSS path.
- [ ] No Takumi path.
- [ ] No second project-symbol resolver.
- [ ] No redesign of callable IDs, schemas, catalog records, adapter callable model, accepted source/world identity, call ranges, or request lifecycle.

## 12. Completion assertion

The deletion pass is complete only when all current production/test callers compile against the final typed APIs and all old APIs are absent from the public documentation generated by `cargo doc`/compile metadata. Absence is proven by the deleted declarations and successful compilation of the final call graph, not by a persistent text-search test.
