# Ordered deletion-driven implementation plan

Every cut is a compile-clean review point. No cut publishes an accepted project that
contains a mixture of old and final View execution authority. A branch may stage
non-compiling edits privately, but each pushed review point satisfies its exit gate.

## C1 — freeze the contract and direct-replacement allocation

Owners: reviewers plus `arcweft-lang-sema`, `arcweft-compiler`, `arcweft-view`,
`arcweft-bundle`, `arcweft-runtime-driver`, and save owners.

Freeze the catalog, execution variants, RuntimeValue/AWBC owner, native constant /
projected-program binding, resource/image identity, proof/digest rules, unchanged
AWVP/AWBC/save allocations, limits, diagnostics, and deletion table. No production
schema is changed in C1.

Exit: `OPEN_QUESTIONS=0`; no unresolved public/wire/save choice; no production diff.

## C2 — publish complete final semantic facts while compiler still fails closed

Owners: `arcweft-lang-sema::final_analysis::view` and existing final-analysis builder.

1. Add the complete `CheckedViewCatalog` and static disposition to
   `FinalSemanticAnalysis`.
2. Build it transactionally from the already accepted HIR/project/type/effect/
   callable/resource facts.
3. Publish read-only generation-checked accessors and exact source roles.
4. Keep the current compiler fail-closed: it may report that the final execution
   cut has not landed, but it may not read source, old AST, or a copied catalog.
5. Add catalog completeness, deterministic ordering, dependency, proof, and budget
   tests.

Exit: sema focused tests, workspace format/check/strict Clippy; current product
writer is unchanged; no second semantic catalog is visible.

Forbidden interim state: a compiler-side semantic mirror, source reparse, a View
endpoint DTO, or an accepted semantic world whose catalog is incomplete.

## C3 — connect the ordinary dynamic-value owner

Owners: existing ordinary function/AWBC lowerer, `arcweft-resource-model`, and
runtime checked-type owners.

1. Add the contextual inherent `ResourceRefValue` runtime conversion on the owner.
2. Generate one ordinary `AwbcFunctionKind::Synthetic` function for each distinct
   nonconstant View value program, using the existing final-HIR expression lowerer.
3. Reuse existing direct-await and handler function semantics; do not add a View VM.
4. Make projection contracts exhaustive on their owning enums/types.
5. Add generic value, resource nominal, handler input, key, default, and direct-await
   tests before switching the View product.

Exit: ordinary AWBC/runtime tests pass for all View value families; no generic value
is routed through `FxRuntimeValue`; no dependency reversal or new AWBC tag exists.

## C4 — one atomic compiler/product switch and stale-test replacement

Owners: `arcweft-compiler::view`, `arcweft-view`, and the bundle ViewProgram,
ViewText, Input, Style, image/resource, merge, digest, and source-map owners.

This is one protected merge group. Review commits may be staged together, but no
commit in the group is releasable or cherry-pickable independently.

1. Extend `ViewInstruction` with `Match` at its owning enum and move any required
   inherent validation/visit behavior there.
2. Directly replace the unreleased AWVP field-1 transcript and every dynamic-capable
   static field with the final constant/program shape.
3. Require exact AWBC cross-section binding for every dynamic program.
4. Switch `ViewProjectLowerer` from ad hoc call classification to the generation-
   matched `CheckedViewCatalog` for every accepted current View shape.
5. Build ViewProgram, ViewText, Input, Style, image/resource bindings, source maps,
   AWBC functions, fragments, and certificates in one scratch transaction.
6. Delete `MissingCheckedViewProjection`, the literal/dialogue-only branches,
   ordinal identities, hard-coded authored defaults, executable
   `ViewValueProgramInventory`, stringly ViewText local/projection variants, old
   image expectations, and stale `view_product` tests in the same group.
7. Replace the seven stale tests with the complete final-HIR product matrix.

Exit: strict codec roundtrip/tamper/limit tests, compiler `view_product`,
`dialogue_profile_admission` 5/5 regression, workspace format/check/strict Clippy.
There is no format revision that accepts old and final transcripts.

Forbidden interim state: new writer plus old reader; old static field plus dynamic
shadow field; missing function-binding acceptance; or dynamic View rejected merely
because the product cannot encode it.

## C5 — migrate all runtime, save, replacement, host, and tooling consumers

Owners: runtime catalog/evaluator/replacement, runtime-plan/host, save/replay,
native/Web/headless/Agent/MCP, generated artifacts, and tooling/LSP.

1. Validate AWVP, AWBC cross-references, resource/type joins, fragments, and
   certificates before catalog publication.
2. Evaluate constants/programs through one runtime path and exact projection.
3. Retain mount/input/focus/handler/resource/animation/observation lifecycle for
   certified fragments.
4. Bind generated artifacts and hot-replacement candidates to full semantic,
   program, dependency, resource, and certificate digests.
5. Keep session save schema 2; restore semantic bindings only after exact artifact
   match, with no persisted certificate path or cache.
6. Remove endpoint resolver tables, debug stringification, placeholder coercions,
   static-path branches, and any string resource/part lookup.

Exit: bundle/runtime/save/hot-reload tests plus native/Web/headless/Agent/MCP parity;
old active generation survives every candidate failure; no partial mount/handler/
resource publication.

## C6 — close dynamic/certified parity and tamper matrices

Owners: all compiler/product/runtime/backend test owners.

Run every `PAR-*`, `TAM-*`, `SAV-*`, `HOT-*`, and `IMG-*` row. Compare canonical
frames, input outcomes, source identities, resource/animation logical state,
observations, and save bytes. A test-only execution selector may force dynamic or
certified evaluation of an otherwise certifiable subject; it is not a production
source/config surface.

Exit: exact parity for every supported backend and consumer; all tampering fails
before candidate publication; exact-limit and one-over evidence is green.

## C7 — add authored `#[static]` after automatic proof is established

Owners: current attribute syntax/HIR source-role owner and sema static requirement.

1. Add the current canonical attribute surface without a new View parser.
2. Resolve its subject to definition or subtree identity.
3. Reuse the automatic static result and diagnostic path exactly.
4. A dynamic result fails after ordinary semantic validity with the attribute as
   primary and the first contaminating node as related evidence.
5. Delete any temporary unchecked hint or compiler-only assertion route.

Exit: success/failure/source/format/LSP tests; no admission bypass; no separate proof.

## C8 — final deletions and repository gates

Delete any remaining compatibility alias, temporary bridge, old test fixture, local
extension trait/helper mirror, source gate, endpoint catalog, or obsolete module.
Run all focused, workspace, Tier-2, metadata/dependency, and structural gates in
`GATE_COMMANDS.md`. Check in the repository-required structure audit and
implementation note with actual command logs.

Exit: zero structural errors, strict Clippy, complete Tier-2 matrix, no partial
publication, and every `DEL-*` typed/API evidence row green.
