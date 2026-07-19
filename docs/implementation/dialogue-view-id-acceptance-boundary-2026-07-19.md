# Dialogue View ID acceptance boundary — 2026-07-19

## Outcome

Dialogue display ownership now remains a typed `ViewId` from runtime-plan
lowering through bundle decoding, accepted View-catalog admission, dialogue
presentation state, View evaluation, save restore, and live View-program
replacement.

The previous provisional shapes were removed directly:

- `LineDisplaySpec.view: Option<String>` became required `ViewId`;
- `DialogueViewDefinition(String)` became `DialogueViewDefinition(ViewId)`;
- string and `Into<DialogueViewDefinition>` convenience conversions were
  removed;
- the implicit runtime-driver default was removed. Runtime-plan lowering now
  selects the standard dialogue View explicitly when no authored selection
  exists.

There is no optional compatibility field, dual reader, alias, or migration
shim. Missing, `null`, and malformed serialized View identities are rejected by
the required typed field.

## Acceptance boundaries

The raw bundle validator checks every display line against its bundled View
program and rejects:

- a valid but unregistered `ViewId`;
- a registered View without a `ViewParameterRole::Dialogue` parameter.

The accepted runtime boundary repeats those semantic checks against
`ViewProgramCatalog`, which is the immutable runtime generation actually used
for dense definition lookup. Successful admission records the exact declared
set of dialogue View owners selected by the display catalog. The runtime
separately tracks the union of those declared owners and the owners of
currently retained dialogue occurrences.

That required-owner set is also:

- supplied atomically at standalone runtime construction through
  `BundleViewRuntime::try_new_with_dialogue_display`;
- checked before every public `evaluate_with_dialogue` call; unknown,
  non-dialogue-role, and catalog-valid-but-unauthorized owners return an empty
  frame with `VIEW019_INVALID_DIALOGUE_VIEW_OWNER` before bindings, mounts,
  allocator state, or the required-owner set can mutate;
- extended atomically by catalog-validated live dialogue inputs during a
  content-only hot swap and pruned back to declared owners as occurrences
  retire;
- represented by separate declared and required owner sets, both included in
  prepared replacement stale-state evidence;
- checked against every replacement catalog so a required owner cannot be
  removed or lose its dialogue role;
- exposed as deterministic typed transient-owner evidence. Session snapshots
  and encoded saves are rejected with
  `BundleSessionPendingBlocker::TransientDialogueViewOwners` while a retained
  occurrence still depends on an owner not declared by the active bundle. This
  prevents writing a save that a fresh session could not authorize without
  weakening tamper rejection;
- used during session restore so an internally coherent save cannot switch to
  another dialogue-capable catalog View that the bundle did not authorize.

Raw bundle validation and accepted-catalog validation are intentionally
different trust boundaries. The first rejects malformed product relationships
with source line identity; the second protects the catalog generation and
dense runtime indexes that are actually published.

## Dependency boundary

Two direct dependencies are intentional:

- `arcweft-render-text -> arcweft-view`, because the serialized
  `LineDisplaySpec` now owns the stable semantic `ViewId`;
- `arcweft-runtime-plan -> arcweft-view`, because HIR dialogue references are
  converted to a checked `ViewId` while the display plan is lowered.

Neither edge introduces runtime-driver, renderer-backend, filesystem, network,
or tooling dependencies into the lower crates.

## Preserved behavior

The `ViewId` acceptance boundary itself does not alter dialogue
stage/reveal/advance ownership, text layout, prepared-text rendering, or the
horizontal speaker-flow policy. The same integrated cut does move the standard
dialogue placement to canonical typed Style: panel
`left = 57.6`, `top = 460.8`, `width = 1164.8`, and
`height = 201.6`, with speaker/content offsets owned by the authored View.
Shared outward rounding produces capture origin `(57, 460)` and size
`1166 x 203`; no physical-geometry sidecar fallback is restored.

## Verification

Completed focused validation:

- `cargo test -p arcweft-bundle --test standard_dialogue_view`
  - 13 passed;
  - covers accepted round trip, malformed, missing, `null`, unknown, and
    registered-without-dialogue-role identities, plus duplicate View
    definition rejection before role selection;
  - also covers the standard dialogue `Position`, `Left`, `Top`, width, and
    height Style assignments consumed by physical placement.
- `cargo test -p arcweft-runtime-driver --lib dialogue_acceptance_tests`
  - 8 passed;
  - directly covers accepted-catalog success plus atomic admission/evaluation
    rejection for unknown, non-dialogue-role, and
    catalog-valid-but-unauthorized owners; live-owner pruning and exact
    declared-owner stale detection are also covered.
- `cargo test -p arcweft-runtime-driver --test session
  save_blocks_while_a_transient_dialogue_view_owner_is_active -- --exact`
  - 1 passed;
  - verifies both snapshot and encoded-save entry points report the exact
    typed transient owner instead of emitting an unrestorable save.
- `cargo test -p arcweft-runtime-driver --test view_runtime
  replacement_cannot_remove_or_retype_a_live_dialogue_view_owner`
  - 1 passed.
- `cargo test -p arcweft-runtime-driver --test session
  restore_rejects_catalog_valid_dialogue_owner_not_authorized_by_the_bundle`
  - 1 passed and verifies atomic restore failure.
- `cargo test -p arcweft-bundle -q`
  - all bundle unit and integration tests passed.
- `cargo test -p arcweft-runtime-driver -q`
  - all runtime-driver unit and integration tests passed.
- `cargo clippy -p arcweft-render-text -p arcweft-runtime-plan
  -p arcweft-bundle -p arcweft-runtime-driver --all-targets --all-features
  -- -D warnings`
  - passed.

The repository policy now classifies broad multi-crate public-contract or
runtime/render/Agent changes as Tier 2 risk. The combined cut must run
`just test-tier2` and update stale MCP/Agent expectations to the production
resource URI, semantic identity, content policy, and authored View geometry;
production compatibility paths must not be restored for stale slow tests.

## Tier 2 follow-up: published capture identity

The Tier 2 MCP capture path exposed a separate publication-boundary defect.
The capture tool returned a content-policy-published resource whose image and
selected-capture scopes used the same canonical opaque identity, while session
info projected `latest_capture` and `latest_capture_uri` from the raw capture
cache. Only its resource descriptor came from the published value. This could
therefore report `layer.<opaque-id>` in the capture response and the raw
`dialogue` selector in the same session's latest-capture metadata.

Session info now publishes the latest raw capture once and derives its image
metadata, URI, and descriptor from that single typed `PublishedAgentResource`.
Publication errors propagate instead of being discarded, and no string
fallback, compatibility alias, or second identity projection was introduced.

Focused verification:

- `cargo test -p arcweft-cli --lib
  session_info_reuses_the_published_latest_capture`
  - 1 passed;
  - verifies that policy publication replaces the raw layer selector and that
    session-info image metadata, URI, and descriptor all come from the same
    typed published resource, with matching canonical image and
    selected-capture scopes.
- `cargo test -p arcweft-cli --test check
  agent_mcp_stdio_captures_profile_selected_source_without_prior_observe --
  --ignored --nocapture`
  - 1 passed;
  - verifies the same identity invariant through the Tier 2 MCP stdio capture
    and session-info path.
- `cargo clippy -p arcweft-cli --lib --all-features -- -D warnings`
  - passed.
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`
  - 0 errors and 132 warnings.
