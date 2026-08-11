# AW-AH-009.3.3.2 typed external project-binding path publication

Status: `READY_FOR_IMPLEMENTATION`

Open questions: `0`

Production changes included: `no`

## Purpose

This archive freezes the correction required for external project-symbol producers to retain source-visible binding paths as typed ordered segments through `ProjectDirectBinding`, `ProjectSymbolTable`, and `ProjectCallableCatalog`.

The selected correction closes one concrete production defect on current `main`: `RegisteredCallableCatalogBuilder::add_project_bindings` attempts to convert each complete scope spelling into one `CallableName` and silently skips qualified external bindings such as `character.akane`. The compact alias remains indexed, but the complete non-callable shadow map required by AW-AH-009.3.3 is not built.

The contract changes only the missing path evidence and its direct producer/publication route. It preserves the already implemented callable IDs, callable schemas, catalog records, shared resolver, project symbol resolver, accepted source/world identity, and atomic accepted-world transaction.

## Repository basis

- Repository: `Sanzentyo/arcweft`
- Branch inspected: `main`
- Exact inspected commit: `9a63ac5512cd75947ba70195681e43ab968f9f12`
- Commit subject: `Implement native physical box geometry reconciliation`
- Latest-main pointer rechecked immediately before artifact construction: unchanged
- Jujutsu change: unavailable through the connector
- Governing request SHA-256: `598aa6d354214d4ea486b52aa2ecaf1e31d016f6fbd53668d7ea8ec19bb7a1bb`
- Root `AGENTS.md` blob SHA: `c41ff4d2b3baadda3e9f975c7de3e5a6678f8758`
- Supplied Rust skill SHA-256: `1a28f552adf5efde95205bee8d56590aeb82346c48ebdf3fdbbaff5deca33665`

The private-repository production sources were inspected through the GitHub connector at the immutable commit above. No repository file was edited.

## Frozen result

1. Existing `arcweft-lang-syntax::ProjectSymbolPath` and `ProjectSymbolSegment` remain the sole project-binding path authority.
2. `ProjectDirectBinding` directly stores a validated `ProjectSymbolPath`; its string-only constructor and `name()` accessor are deleted.
3. Private HIR `ScopeBinding` retains that exact path through direct insertion, imports, re-exports, explicit aliases, globs, and fixed-point coalescing.
4. `ProjectSymbolTable::scope_bindings` is directly replaced by one deterministic typed iterator.
5. Character producers construct qualified and compact paths from `CharacterId::compact_segments()`; they never split `CharacterId::as_str()`.
6. Adapter manifests gain a language-free segmented `AdapterSymbolPath` before sema publication; the manifest codec is the only dotted source-field parse boundary.
7. The callable catalog converts each already typed project segment to `CallableName`, charges the existing path-segment work budget, publishes every callable and non-callable binding, and deletes the invalid-name `continue` branch.
8. Existing `ProjectNameBinding`, `ProjectCallablePath`, `TypeKind` mapping, resolver precedence, catalog collision errors, and accepted-world transaction remain unchanged.
9. There is no compatibility constructor, deprecated wrapper, dual reader, source gate, extension trait, CSS/Takumi route, display-string parser, or second project-symbol resolver.

## Read order

1. `FINAL_CONTRACT.md` — normative ownership, exact Rust declarations, errors, visibility, and invariants.
2. `PRODUCER_MIGRATION.md` — current producer inventory and exact direct migration.
3. `LINKER_AND_CATALOG_RULES.md` — typed path propagation, deterministic order, collisions, and publication.
4. `TEST_MATRIX.md` — mandatory direct tests and acceptance assertions.
5. `IMPLEMENTATION_ORDER.md` — the required seven-stage order, expanded into compiling cuts without reordering.
6. `DELETION_CHECKLIST.md` — exact obsolete APIs and branches that must disappear in the same cut.
7. `REJECTED_ALTERNATIVES.md` — rejected models and why they violate the request or current architecture.
8. `REQUIREMENTS_TRACEABILITY.md` — every request decision, test, constraint, and output mapped to this package.
9. `REPOSITORY_EVIDENCE.md` — inspected files, current implementation facts, and verification honesty.
10. `VALIDATION_PLAN.md` — focused, workspace, metadata, and canonical structural validation.
11. `FINAL_STATUS.md` — readiness and non-redesign declaration.
12. `OPEN_QUESTIONS.md` — exact zero-open-question marker.
13. `MANIFEST.txt` — per-member hashes and byte sizes; its own digest is the fixed all-zero sentinel.

## Normative conventions

`must`, `must not`, `only`, `exactly`, and `directly replace` are normative. Rust declarations are exact target declarations unless explicitly identified as unchanged existing substrate. Private fields and visibility are intentional. Any implementation that introduces an additional successful path, fallback reader, compatibility interval, or string-derived identity is non-conforming.

## Verification boundary

The archive itself is mechanically verified for its exact sorted member set, UTF-8/LF Markdown, `OPEN_QUESTIONS.md == "none\n"`, per-member SHA-256 values and sizes, manifest self-entry, ZIP CRCs, deterministic metadata, deterministic rebuild, and agreement with the outside SHA-256 sidecar.

Because the request prohibits production changes, no future Rust patch exists in this archive and no post-change Cargo command is represented as already executed. The exact implementation and validation commands are frozen in `IMPLEMENTATION_ORDER.md` and `VALIDATION_PLAN.md`.
