# AW-AH-009.4.4.1 line-plan semantic type and callable cut

Date: 2026-08-21
Implementation base: `cec30b57fa734efb059d7b846b397ac7d2b0701a`
Working tree before implementation: clean; `main` matched `origin/main`

## Scope established

This cut replaces the temporary semantic spellings for the line-plan
capability and handle families with direct checked authority:

- `TypeKind::StageApi(CharacterId)` retains the exact Character selected by a
  checked Character value;
- `TypeKind::LineContext` is the non-value line activation capability;
- `TypeKind::StageActorHandle(StageActorHandleType::{Exact, Any})`,
  `CueHandle`, and `VoiceHandle` are direct handle types;
- exact stage-actor handles inject into the erased `Any` storage type, while
  erased or differently owned handles do not satisfy an exact expectation;
- `StageMethodId::Acquire` projects the exact Character into its result and
  `StageMethodId::Look` requires the same Character-owned look type;
- `LineContextMethodId::VoiceHandle` replaces the unrelated
  `CapacityMethodId` route; and
- `LineScheduleCallableId::At` replaces the environment-catalog `at` function
  while preserving the accepted curried `at(Duration): callback` shape.

The standard nominal catalog no longer publishes `VoiceHandle` as a second
accepted-nominal type. Authored `StageActorHandle`, `CueHandle`, and
`VoiceHandle` resolve through the language-owned builtin constructor table.
`StageApi` and `LineContext` are contextual capabilities and are not authored
runtime-value constructors.

Compiler type projection deliberately rejects all five new families in this
cut. That is the fail-closed boundary until the next opaque-affine cut adds the
sole runtime handle owner. No `Named` fallback, accepted-nominal alias, dynamic
projection, or temporary RuntimeValue representation was added.

## Exact Character propagation

`CheckedProjectItem` already retained validated Character identity. The
Character `stage` field now copies that typed identity into
`CheckedValueResolution::CharacterField`, so local result inference and shared
call resolution do not reconstruct it from a path or public string.

Declared Character look variants continue to resolve through their ordinary
manifest-backed variant owner. A symbolic stage look that is not in that closed
inventory retains the same exact `CharacterNominal::Look` type with the
existing `CheckedExpressionResolution::StageLook` runtime spelling; it no
longer falls back to `Named("StageLook")`.

## Structural review

The touched owner is `arcweft-lang-sema::types`, responsible for the closed
semantic vocabulary, compatibility, identity digest, deterministic ordering,
generic traversal, and openness checks. Dependency direction remains unchanged:
sema depends only on existing lower language/data crates, and compiler remains
the first runtime projection boundary.

The two recursive openness queries previously duplicated the complete atomic
type inventory. This cut replaces that copied list with one local exhaustive
pattern macro, preserving compiler-checked enum coverage while reducing both
query bodies below the structural line trigger. No public duplicate type,
extension trait, new crate dependency, or source-string resolver was added.

## Validation

Passed:

- `cargo fmt --all -- --check`
- `cargo check -p arcweft-lang-sema -p arcweft-compiler -p arcweft-runtime-plan --all-targets --all-features`
- `cargo test -p arcweft-lang-sema --lib --all-features` (209 passed)
- focused exact stage/line/schedule fixture test
  `dialogue_line_plan_bindings_are_inferred_in_source_order`
- focused direct builtin inventory and standard nominal authority tests
- `git diff --check`

The strict sema-library Clippy command was performed and failed on 18 existing
warnings outside the new direct-type logic: callable builder argument count,
pre-existing resolver/analyzer/validation line counts, one existing
single-match shape, existing redundant closures/unused-self, and existing
semicolon style findings. The new compatibility and openness changes were
decomposed until they added no Clippy finding. A separate compiler/runtime-plan
library Clippy run then stopped on the existing
`compiler/src/persistent.rs:761` missing-semicolon style finding; no changed
compiler or runtime-plan line produced a Clippy diagnostic.

The unchanged RUN-037 fixture check was performed and intentionally remains
failed at this intermediate gate. It now passes semantic analysis and stops at
`compiler.runtime_semantic_projection` because the newly selected
`StageMethod` has no typed runtime intrinsic. This is the expected fail-closed
handoff to the RuntimePlan operation cut; the former semantic type failure and
all `Named` fallback routes are gone.

## Remaining package work

- extend the original opaque RuntimeValue/type owner and ownership/save/AWBC
  projections for exact snapshot-only line handles;
- add the typed RuntimePlan line operations, result target, handle sites, and
  admission limits before enabling runtime projection;
- place Stage/LineContext/schedule-to-runtime operation construction in the
  legitimate runtime-plan lowering context. An inherent sema method cannot
  return the higher-layer `RuntimeLineOperation` without reversing the crate
  dependency, so the package's Rust-shaped cross-layer method sketch is not
  implemented literally; and
- continue structured/AWBC/host/persistence migration before deleting the old
  string handle/result route atomically.
