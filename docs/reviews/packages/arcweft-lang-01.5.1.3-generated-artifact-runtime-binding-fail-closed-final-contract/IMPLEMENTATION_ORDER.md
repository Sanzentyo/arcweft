# Required implementation order

The sequence below preserves fail-closed behavior throughout and follows the request's order. A phase is not complete until its focused tests pass and obsolete paths exposed by that phase are removed.

## Phase 0 — land failing acceptance tests first

Before any successful host sentinel:

- add product/key/catalog test scaffolds;
- add E-19 missing function and missing Activity expectations;
- add stale overlay and unselected tests;
- add no-partial-work counters/snapshots;
- add codec rejection fixtures;
- add plan identity tests for direct/reference/partial forms.

The tests may initially fail to compile while the type surface is introduced in the same coherent implementation cut; do not weaken them to string assertions.

## Phase 1 — foundational ID and owner behavior

1. Add `GeneratedArtifactBindingId` to `arcweft-id` with serde and overflow tests.
2. Add inherent exact-marker `as_str()` and `AdapterTarget::{family, abi_str}` to `arcweft-adapter-metadata`.
3. Add `AdapterFunctionOrigin` directly to `AdapterFunction` and update existing constructors/callers.
4. Run focused fmt/check/tests for these crates.

No runtime lookup is introduced yet.

## Phase 2 — exact key, errors, product, and strict codec

1. Add the `arcweft-runtime-binding` crate.
2. Implement validated ABI/transport identities.
3. Implement nested key types, `ActivityImplementationId` retention, Activity/target invariants, and manual invariant-preserving deserialization.
4. Implement fixed structural correlation and typed mismatch errors.
5. Implement canonical anchor, contiguous ID assignment, derived Activity selections, current accepted ABI/transport marker validation, product validation, and strict schema 1 serde.
6. Complete K/C/M/W product tests.

Do not add a key digest or string lookup accelerator.

## Phase 3 — host-owned catalog

1. Implement fixed kind-aware slots from a verified product.
2. Implement exact function/Activity registration with stale/selection/kind/mismatch/duplicate ordering.
3. Implement immutable freeze with allowed missing slots.
4. Implement resolve with active topology and stale-first ordering.
5. Complete R/E/S/F and catalog no-mutation tests, except the positive sentinel remains last in the overall request sequence if necessary.

No filesystem/provider/backend code is added.

## Phase 4 — unified topology projection

1. Arrange topology assembly so the complete `SourceSetRevision` and selected profile ID are available.
2. Replace the split external-module adapter extension and Activity validation with one projection transaction.
3. Stage non-private function facts and selected Activity facts, including each `ResolvedActivityBinding::implementation_id()`.
4. Build exact keys, canonicalize IDs, derive Activity selections, insert generated function origins, and return product atomically.
5. Add product to `LoadedProfileTopology`.
6. Complete T-series tests, including zero metadata re-decodes.

Delete the old separate Activity validation-only function after all call sites move.

## Phase 5 — semantic and first-class callable propagation

1. Copy origin into the existing semantic callable owner at adapter ingestion.
2. Preserve it through overload resolution.
3. Replace string-only generated callable lowering evidence with `RuntimeTypedCallableOrigin`.
4. Cover direct call, function reference, partial call, effect evidence, and apply.
5. Remove any generated path-to-ID lookup or inference introduced during development.

## Phase 6 — runtime-plan/core variants and compiler correlation

1. Add generated variants to `RuntimeCallTarget` and `RuntimeFunctionBody`.
2. Replace dishonest `as_label()` owner behavior.
3. Lower all generated forms directly from typed evidence.
4. Add mandatory product to `AcceptedLaunchProfileInput`; add `Option<Arc<_>>` to `CompiledProject`, copied exactly from the existing optional accepted-launch input. Never synthesize a no-profile product.
5. Implement the `Option<&product>` plan/Activity-selection cross verifier after existing plan verify.
6. Update all codecs, visitors, digests, exhaustive matches, bundle/save consumers.
7. Complete P/W plan tests.

At the end of this phase, generated exports no longer have a successful string-only runtime path.

## Phase 7 — runtime-host and driver fail-closed gates

1. Thread active topology plus catalog access through the existing launch/session context.
2. Resolve generated direct calls and function applies before any host task/request.
3. Project and carry the exact `GeneratedArtifactActivitySelection`; resolve its binding ID before Activity instance/state/registry/event/scheduler mutation.
4. Map shared errors into the current structured runtime/host error hierarchy without losing machine code or typed cause.
5. Complete N-series tests.

Do not implement actual artifact execution. A successfully resolved binding may be a sentinel or be handed to an existing/future execution boundary.

## Phase 8 — LSP generation lease

1. Associate generation, the compiled project's optional product, and an optional catalog lease with one `AcceptedProfileEnvironment` publication; a lease exists only for `Some(product)`.
2. Reject old generation before exposing catalog.
3. Rebuild registrations for every replacement; copy none.
4. Complete same-content and changed-content generation tests.

## Phase 9 — required positive in-memory binding test

Only after the fail-closed rows are green, add the exact in-memory sentinel test:

- obtain one canonical requirement;
- register one matching typed sentinel with ID + full key;
- freeze;
- resolve deterministically;
- assert identity;
- prove no filesystem/provider/execution operation occurred.

This is not an artifact execution test.

## Phase 10 — deletion and full validation

1. Complete every row in `DELETION_MATRIX.md`.
2. Run focused tests, fmt, and Clippy.
3. Run workspace and tier-2 suites.
4. Run structure audit and `git diff --check`.
5. Record exact commands/exits and update implementation intake/matrix evidence.
6. Inspect final Git diff for aliases, wrappers, dual readers, name fallbacks, and old success branches.

Do not call the slice complete while a generated string fallback remains, even if the new tests pass.
