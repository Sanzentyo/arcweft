# Rejected alternatives

## 1. New HIR-owned duplicate of `ProjectSymbolPath`

Rejected because the existing syntax type already owns the required root, ordered external-capable segments, validation, ordering, and source import representation. A second HIR type would require conversion, duplicated invariants, and another place for path identity to diverge without fixing any current flaw.

Selected instead: store the existing `ProjectSymbolPath` directly in `ProjectDirectBinding` and private `ScopeBinding`.

## 2. Keep `ProjectDirectBinding.name: String` and add a parallel typed field

Rejected because it creates dual authorities and a synchronization invariant. Every consumer would have to choose between the old spelling and new segments, producing a de facto dual reader and compatibility interval.

Selected instead: replace the string field and constructor in one cut.

## 3. Split `SymbolPath::leaf()` in sema

Rejected because `SymbolPath` explicitly permits an opaque external leaf. Splitting it would manufacture identity after linking and would make unrelated punctuation/display decisions semantic.

Selected instead: retain segments before conversion to `SymbolPath` and expose them from HIR.

## 4. Split a complete scope spelling in the callable catalog builder

Rejected because the builder does not own source syntax or external producer semantics. It cannot distinguish a true segment boundary from punctuation inside an opaque external identity.

Selected instead: convert each existing `ProjectSymbolSegment` independently to `CallableName`.

## 5. Store `character.akane` as one `CallableName`

Rejected because `CallableName` correctly rejects `.` and other path separators. Relaxing it would redesign implemented callable identity and blur one path with one segment.

Selected instead: `CallablePath(['character', 'akane'])`.

## 6. Continue omitting invalid complete spellings

Rejected because the project binding map must be complete. Omission allows an environment callable to be selected through a project non-callable shadow and violates accepted-world semantic consistency.

Selected instead: every typed row is published or the candidate fails with an existing typed error.

## 7. Reject all qualified character/adapter registrations

Rejected because these registrations are currently valid production facts. The defect is evidence loss, not invalid producer input. Rejecting them would break existing worlds and avoid the required complete-shadow correction.

Selected instead: producers retain typed segmentation.

## 8. Collapse compact/authored aliases into canonical external identity

Rejected because aliases are source-visible bindings, not declaration identity. The same external declaration may have qualified, compact, imported, re-exported, and authored alias paths.

Selected instead: one canonical seed plus multiple exact binding paths.

## 9. Use `AdapterCallablePath` for non-callable symbols

Rejected because it conflates callable and non-callable manifest domains and would make the already implemented adapter callable model own behavior it was not designed to express. It also risks future callable-only constraints leaking into environment symbol identity.

Selected instead: a small language-free `AdapterSymbolPath` in the adapter-context owner, converted once to `ProjectSymbolPath` at fact publication.

## 10. Store `ProjectSymbolPath` directly in base adapter manifests

Rejected because `arcweft-adapter-context` deliberately keeps syntax/HIR/sema dependencies optional under the `sema` feature. Making the base manifest model depend unconditionally on language syntax would change dependency layering for a producer-local concern.

Selected instead: the adapter owns a grammar-equivalent language-free path and converts at its existing sema-enabled publication boundary.

## 11. Retain adapter symbol strings and split only in `source_backed_registration_facts`

Rejected because the programmatic manifest model would still lose typed evidence until a late optional publication phase. It would allow invalid labels to exist in accepted typed manifests and would force a production dotted split outside the source codec.

Selected instead: all programmatic adapter construction is typed; only the file codec parses its authored source field.

## 12. Add a new schema-v2 `segments` field while accepting schema v1

Rejected because it is a dual reader and unnecessary schema churn. The current v1 field already represents authored source data and can be decoded directly to the final typed model.

Selected instead: keep v1, one field, one direct decoder, no retained untyped representation.

## 13. Make `AdapterSymbolPath::from_str` public

Rejected because it would become a dotted-string compatibility constructor and invite callers to render and reparse paths rather than preserve typed segments.

Selected instead: typed segment constructor for programmatic use and one private codec parser for authored files.

## 14. Add an extension trait or free helper around Arcweft-owned types

Rejected because the behavior belongs in the owning constructors/accessors and would create an indirect API beside the final model.

Selected instead: inherent APIs on `ProjectDirectBinding`, `AdapterSymbolPath`, `AdapterSymbol`, and existing owner types.

## 15. Replace the private HIR scope map with a new resolver/index

Rejected because current project resolution, visibility, ambiguity, fixed-point, and limits are implemented and no concrete flaw was found in them. Replacing the map would exceed the correction seam and risk behavior changes.

Selected instead: retain the map and add typed evidence to each row; the string key remains a private generated accelerator.

## 16. Add a second typed iterator while retaining the string iterator

Rejected because sema could continue to consume the wrong one and the two views could diverge. It is a dual reader.

Selected instead: directly replace the only iterator.

## 17. Make HIR store `TypeKind` or adapter types

Rejected because it reverses dependency direction and couples project linking to sema/adapter context. HIR only needs target identity and typed source path.

Selected instead: retain the existing sema-owned target-to-`TypeKind` closure.

## 18. Change `ProjectNameBinding` or catalog record schemas

Rejected because current implementation evidence shows those types correctly model callable/non-callable shadowing. The failure occurs before them when a binding is skipped.

Selected instead: feed the existing types complete segmented input.

## 19. Add a catalog fallback to `ProjectSymbolTable`

Rejected because it would create two successful project-name authorities and make catalog completeness optional.

Selected instead: the catalog receives every HIR binding during the atomic transaction.

## 20. Change the shared resolver

Rejected because current project-before-environment behavior is correct once the binding map is complete. No concrete resolver flaw was found.

Selected instead: no resolver code-path redesign; only direct tests prove newly complete qualified shadows.

## 21. Change accepted-world publication or add rollback

Rejected because current construction is already fail-closed: the world object is returned only after catalog completion. A rollback API would add mutation and complexity without a demonstrated transaction flaw.

Selected instead: preserve transaction ordering and add pointer-identity tests.

## 22. Source scans as correctness gates

Rejected because typed API compilation and Cargo metadata provide direct dependency/public-surface evidence. Source scans are brittle, can miss semantic aliases, and are explicitly prohibited as a gate.

Selected instead: compile-only public API tests, metadata graph assertions, focused tests, workspace clippy/tests, and the canonical structural audit.

## 23. CSS or Takumi routes

Rejected because they are outside this production seam and expressly forbidden. No CSS/Takumi type, parser, feature, fixture, or publication path is introduced.
