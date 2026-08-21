# Compile-clean implementation sequence

Each cut introduces only final owners and must compile independently. Product/runtime publication remains fail-closed until cut 5; no second published authority exists.

## Cut 1 — generic CheckedMatch authority only

- Add `CheckedMatch`, exact arm/local ordinals, ownership disposition, one coverage row, `CheckedExpressionResolution::Match`, digest/accessors, bidirectional completeness validation, and focused tests.
- Read all type/effect facts from existing `CheckedExpression`/`CheckedPattern`/`CheckedBinding`; do not copy them into CheckedMatch.
- Replace Structural only for Match. No View/runtime/product schema changes.

Compile state: final semantic analysis publishes complete generic Match facts; existing product behavior otherwise remains unchanged.

## Cut 2 — resource input, complete checked View catalog, lightweight coordinates

- Add `arcweft-lang-sema -> arcweft-resource-model` and fallible `FinalSemanticCatalogs::production(world, resource_types)`; update compiler/sema-test/LSP call sites.
- Add `CheckedMatchRef` to the complete checked View catalog and remove copied Match arms/bindings/types/effects.
- Add core-independent View Match coordinate rows and compiler generation of dense site/arm/output/local/body coordinates.
- Keep product construction fail-closed with one final `ReactiveViewAbiNotPublished` error; emit no empty section or placeholder selector.

Compile state: semantic/View owners are final and complete; no new runtime consumer is published.

## Cut 3 — runtime-plan projection plus selector/typed-Need core ABI

- Add compiler construction of `RuntimeViewMatchSelectorSeed`; finalize through existing runtime semantic facts/type table. Add runtime-plan selector builder without sema/View/bundle dependency.
- Add `RuntimeCheckedType::Need`, typed `NeedHandle { payload }`, dedicated RuntimeValue, MakeNeedHandle `0x1e`, producer flag bit 4, producer/selector verifier, VM/AOT/parity execution, exact codecs/digests, and snapshot DTO.
- Delete `AwbcTaskPlan.need_id` from core/runtime-plan construction and update all core consumers/focused tests.
- New owners are not reachable from published View product.

Compile state: final core/runtime-plan APIs and focused exact tests are green; no View dual authority exists.

## Cut 4 — checked unary-Need product/journal staging

- Add complete `ViewReactiveBindingSectionV1`, exact codec/merge/digest/source-role validation, and compiler section staging.
- Add private runtime-driver verified binding indexes, selector decode/install, typed Need extraction, journal/start projection, save/replay/replacement staging.
- Keep final bundle publication fail-closed at the same gate; private consumers are exercised only through verified fixtures.

Compile state: final path is constructed end-to-end in staging without production dual authority.

## Cut 5 — atomic consumer switch and deletion

In one commit/merge unit:

- enable complete reactive View section publication/runtime consumption;
- route View Match/unary Need through new authorities;
- delete old View Await rows/readers/evaluator/save branches;
- delete payloadless NeedHandle, NeedHandle-as-String, await_target String conversion, static task-plan need strings, obsolete bundle fields/readers, copied checked View Match rows, and old fixtures/generated schemas;
- run structural absence and full workspace/Tier-2 gates.

Compile state: only final version-1 authority remains. No alias, compatibility reader, feature flag, fallback resolver, or unreachable scaffolding exists.

## Required gate per cut

At minimum: format, workspace check, focused nextest, all-target Clippy with warnings denied, docs, generated-schema checks, package validator, and structural searches. Cut 5 additionally requires full nextest, VM/AOT differential parity, save/replay/replacement, and Linux/Windows/macOS Tier-2 rows in TEST_MATRIX.csv.
