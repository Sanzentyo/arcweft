# Migration and deletion order

Every cut below is a compiling, reviewable increment. A completed branch may
not expose two successful root readers or two final content inventories.

## Cut 0 — Reconfirm fixed substrate

- Rebase on latest accepted main.
- Re-read all applicable `AGENTS.md` and Rust skill.
- Run focused tests for current strict manifest decoder, binary overlay,
  `ProjectTopologyRevision`, and `CharacterPackage`.
- Do not alter behavior in this cut.

## Cut 1 — Lower typed content authority

- Add final family, target, fact, presence, reference, source-evidence, and
  aggregate types in `arcweft-project::content`.
- Add inherent constructors/accessors and deterministic validation.
- Add exact unit tests for family/presence invariants and ordering.

## Cut 2 — Complete Source elimination substrate

- Land the Lang-01.3.1 final Stream model if not already complete.
- Delete `EntityKind::Source`, `TypeKind::Source`, and Source declaration/runtime
  inventory across original owners.
- Do not add a content-root migration alias.

## Cut 3 — Manifest source evidence

- Extend `ManifestTokenPath` and its inherent conversion.
- Add strict decode/source-map tests for every content unit and profile field,
  including array ordinals and revision mismatch.

## Cut 4 — Typed semantic resolution

- Add one content root resolver using current project symbol/resource/
  Character/Activity authorities.
- Add selected-profile typed reference collection.
- Prove Stream callables and all wrong families are rejected through ordinary
  resolution.

## Cut 5 — Loader acquisition and presence

- Add `ProfileTopologyOverlaySet` and binary capture.
- Replace Character prefix preloading with resolved acquisition plans.
- Build every present `CharacterPackage` from exact manifest-named bytes.
- Produce exact optional absence records and watch targets.
- Compute `ProjectTopologyRevision` from one canonical inventory.

## Cut 6 — Final semantic index and commit carrier

- Construct `AcceptedProjectContent`.
- Require it in `ProjectSemanticIndex` construction.
- Add `AcceptedProfileProject` and make it the sole publication object.
- Delete `ProjectGraphRelationKind::ContentRoot` and its source-HIR producer in
  this same cut.

## Cut 7 — Delete source `content`

- Remove source grammar/CST/AST/HIR/sema/formatter/LSP ownership for `content`.
- Remove old test builders and examples.
- Ordinary current grammar rejects the spelling; no historical recognizer is
  retained.

## Cut 8 — Consumer migration

- Bundle consumes exact present package inventory.
- Watch consumes exact present/absence entries.
- LSP captures text+binary overlays and publishes one accepted carrier.
- Compiler, Agent, CLI, and caches query the final index/revision.

## Cut 9 — Fixtures and complete validation

- Migrate maintained schema-1 projects directly.
- Run every focused suite, workspace check/clippy, `just verify`, mandatory
  `just verify-full`, structural audit, diff checks, and deterministic bundle
  fixtures.

## Direct deletion inventory

| Provisional/obsolete owner | Final action | Acceptance evidence |
| --- | --- | --- |
| `SourceContentRootFamily::Source` or equivalent | delete at original enum | exhaustive family tests and compile-fail API evidence |
| `AcceptedContentRootTarget::Source` or equivalent | delete | target exhaustiveness and no serialized tag |
| `EntityKind::Source` | delete at original enum and inherent tables | typed current-family tests |
| `TypeKind::Source` | delete at original enum and all displays/codecs | compile/type tests |
| authored `source` declaration | delete parser/HIR/sema/tooling | current grammar/HIR behavior tests |
| runtime `Source<T, E>` path | replace directly by final Stream model | runtime/wire/save tests owned by Lang-01.3.1 |
| authored source `content` declaration | delete parser/HIR/sema/tooling | manifest fact tests + parser rejection |
| `ProjectGraphRelationKind::ContentRoot` | delete | final index API tests |
| `index_content_root_relations` | delete | build and content query tests |
| loader `@character.` prefix branch | delete | alias/qualified typed resolution tests |
| directory scan/inference | delete or never add | instrumented no-enumeration tests |
| text-only overlay aggregate | replace by one text+binary set | overlay parity tests |
| separate topology/index LSP publication | replace by `AcceptedProfileProject` | stale/rollback tests |
| any root-fact side hash | never add | revision transcript tests |
