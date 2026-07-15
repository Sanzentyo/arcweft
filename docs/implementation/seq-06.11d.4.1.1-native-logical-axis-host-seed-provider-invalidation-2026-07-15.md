# Seq 06.11d.4.1.1 native logical-axis host seed/provider invalidation

## Status and source

Implementation complete on the working copy based on Git `8140470a0dda25adebc2985a9ea077e853c17666`
and Jujutsu change `nsrnqtmx`. The accepted design source is
`D:/sanze/Downloads/arcweft-seq-06.11d.4.1.1-native-logical-axis-host-seed-provider-invalidation-final-contract.zip`,
SHA-256 `9C3471C1B855C9A00D4570615BD3BBC440FC796774DACCD054B94154181E69B3`.
The package manifest and all nine regular-file entries were checked before implementation. Its stated
baseline was `ec20509c`; the current repository had later documentation/audit work, but no conflicting
logical-axis implementation.

This slice adds no crate and changes no Cargo dependency or feature edge. It does not add a CSS/Takumi/DOM
path, string axis API, compatibility reader, migration alias, second resolver, or public portable trace DTO.

## Ownership and final model

- `arcweft-view::style::axis` owns the four-mode typed seed, checked generation, source, inherited snapshot,
  and canonical FNV-1a host/local revision transcripts.
- `arcweft-view::style::resolver::provider` owns the retained provider records, reverse child index,
  deterministic bounded breadth-first invalidation, barrier stopping, corruption checks, and two-phase
  provider update plan.
- `arcweft-runtime-driver::view_runtime::axis_seed` owns pending next-root reservations, mounted generations,
  checked CAS, lifecycle cleanup, strict snapshot wire, and restore reconciliation against the exact ordinary
  plus dialogue root inventory.
- `BundleSession` is the application-facing facade. A changed live seed updates a visible root snapshot and
  presentation revision exactly once; no-op and rejected CAS operations leave both unchanged.
- `arcweft-player-scene::frame::view_style::axis_seed` adapts only the typed runtime output: top-level roots
  require a host seed, nested roots reject one and inherit from their exact caller, and direct descendants
  inherit the canonical computed snapshot. Primary nodes retain provider state; alternate bindings are
  projections only.

The runtime consumes a pending reservation only after mount allocation, `ViewMountState` construction, and
root occurrence insertion succeed. `prepare_root_mount` is immutable with respect to the registry; dropping
the plan after the owner insertion seam retains the reservation, and a later committed retry consumes it once.

## Contract deviations and clarifications

- The error field is named `seed_source`, not `source`, because `thiserror` reserves a field named `source` as
  an error source. The typed value and diagnostic meaning are unchanged.
- Detached alternate projection uses the explicit deterministic inherited snapshot supplied by its primary
  owner. It never registers a provider record or reverse edge.
- The package's suggested file locations were resolved to the existing responsibility modules rather than
  copied as a parallel implementation.
- The stricter axis-registry restore preflight now rejects orphan dialogue root seeds before the older
  dialogue-store correspondence diagnostic. The existing tamper test was updated to expect the earlier typed
  `UnknownSnapshotMount` message; atomic rejection remains unchanged.

## Direct acceptance evidence

The table names the executable test that directly supplies the evidence. IDs are not credited merely because
they share an implementation path.

| Contract IDs | Direct evidence |
|---|---|
| T-HOST-001..007, 013, 018, 021..022 | `host_seed_mode_matrix_and_known_handle_rejections_are_exact_and_atomic`; `reservation_last_write_cancel_and_mount_identity_are_deterministic` |
| T-HOST-008..012, 015..017 | `live_update_is_checked_noop_or_generation_advancing_without_wrap` |
| T-HOST-014 | `nested_mount_host_mutation_is_rejected_without_changing_runtime_state` |
| T-HOST-019 | `hidden_unmounted_terminal_and_remount_lifecycle_retains_or_replaces_exact_state` |
| T-HOST-020 | `same_evaluation_non_view_resolution_discards_pending_seed_and_emits_one_diagnostic` |
| T-HOST-023..024 | `pending_seed_prepare_is_transactional_and_later_commit_consumes_it_once`, at the registry prepare/owner-insert/commit transaction seam used by the evaluator |
| T-REV-001..005, 012 | `host_seed_revisions_match_the_canonical_transcript`; `host_seed_distinguishes_default_from_explicit_horizontal_ltr`; runtime host mode matrix |
| T-REV-006..009, 012 | `style::axis::tests::local_provider_revisions_match_the_canonical_transcript` |
| T-REV-010 | `barriers_stop_ancestor_walk_but_their_own_provider_change_reaches_descendants` |
| T-REV-011 | `cloned_resolvers_replay_the_same_axis_revision_sequence`; registry replay comparison in `restore_cross_table_matrix_is_strict_and_replay_is_exact` |
| T-LIFE-001..008 | session axis seed round trip; `restore_cross_table_matrix_is_strict_and_replay_is_exact`; lifecycle/remount test |
| T-LIFE-009..023 | `restore_rejects_duplicates_tampering_and_non_view_lifecycle_atomically`; `restore_cross_table_matrix_is_strict_and_replay_is_exact`; nested restore tamper test |
| T-LIFE-024, 026; T-WIRE-006..008 | `runtime_snapshot_requires_the_strict_axis_seed_registry_field`; session round trip; strict snapshot-wire test |
| T-LIFE-025 | `ordinary_and_dialogue_restore_roots_cannot_share_a_handle_identity` |
| T-PROP-001..005, 012..014, 016..017 | logical-axis cascade/provider integration tests, including both parent-shape permutations and ProjectionOnly registration rejection |
| T-PROP-006..009; T-PARITY-004 | `inherited_style_resolves_across_a_live_call_view_mount_boundary`, with nested root, direct child, exported no-winner, exported local barrier, and its descendant in three independent player states |
| T-PROP-010..011, 015 | top-level missing-seed and nested missing-caller/unexpected-seed player tests |
| T-PROP-018 | `ViewStyleResolveContext` rustdoc `compile_fail` test omitting `inherited_axes` |
| T-BAR-001..003, 007 | retained barrier integration test with direct barrier eviction, stopped grandchild eviction, changed local provider, and stable re-resolve revision |
| T-BAR-004..006; T-INV-003..005 | `local_barrier_transitions_invalidate_only_their_own_descendants` |
| T-INV-001..002, 013..016; T-BUD-001..003, 005..006 | provider unit tests for sorted BFS, exact/over/zero budget, barrier stopping, marker suppression, and corrupt reverse indices |
| T-INV-006 | `reparenting_replaces_the_old_edge_and_only_the_new_parent_invalidates` |
| T-INV-007..012, 017; T-BUD-004 | mount cleanup, self/long cycle, revision mismatch, atomic budget failure, and failed-child retry integration tests |
| T-CACHE-001..007, 009 | provider/cache integration tests across repeated identity, mode/provider/parent/application/environment changes and targeted node-wide eviction |
| T-CACHE-008 | `targeted_node_eviction_preserves_survivor_fifo_order_at_capacity` |
| T-CACHE-010..011 | revision-only winner-stability and retained-plus-projection eviction assertions in `ancestor_change_evicts_descendant_projection_entries_and_mount_cleanup_is_idempotent` |
| T-REVSET-001..008 | `every_revision_set_recomputes_and_provider_identity_follows_the_actual_winner` |
| T-PARITY-001..002 | `native_web_and_headless_style_states_match_for_default_and_every_explicit_seed` |
| T-PARITY-003 | three restored independent session facades in `session_axis_seed_api_is_shared_typed_cas_state_and_round_trips_pending_and_live_roots` compare exact CAS outcomes and canonical serialized snapshots |
| T-NEG-013..015 | `locale_rich_text_direction_and_theme_never_infer_a_different_axis_provider`, including direct retained cache identity after typed rich-text direction/content, locale, and palette-only mutations |
| T-WIRE-001..005 | strict host seed wire test; snapshot duplicate/tampered derived tests |
| T-ARCH-001..003 | unchanged Cargo manifests, shared runtime/player types compiled for native/web/headless crates, and canonical structural audit |

No known test-matrix ID remains without direct behavioral, codec, compile-fail, metadata, or structural evidence. The
T-HOST-023 failure is exercised at the stable registry transaction seam rather than by adding a test-only evaluator
fault injector; production ordering is separately compiled and reviewed in `prepare_occurrence`.

## Structural measurements

Canonical command:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . --write target/structure-audit-d411
```

The final audit reports 0 errors. Existing workspace-wide warnings remain ownership-review warnings rather than
acceptance errors. Exact final metrics for the main changed files are recorded below; tests are split from the runtime
axis registry so the production owner remains compact.

| Path | Owner/class | Bytes | Physical LOC | Embedded tests |
|---|---|---:|---:|---|
| `crates/arcweft-view/src/style/axis.rs` | view production | 19,828 | 659 | yes, small golden unit module |
| `crates/arcweft-view/src/style/resolver.rs` | view production | 43,428 | 1,200 | no |
| `crates/arcweft-view/src/style/resolver/provider.rs` | view production | 20,768 | 569 | yes, bounded index white-box tests |
| `crates/arcweft-view/tests/logical_axis_provider.rs` | view integration test | 60,556 | 2,077 | no |
| `crates/arcweft-runtime-driver/src/view_runtime/axis_seed.rs` | runtime production | 17,448 | 468 | no |
| `crates/arcweft-runtime-driver/src/view_runtime/axis_seed_tests.rs` | runtime unit test | 30,851 | 856 | no |
| `crates/arcweft-player-scene/src/frame/view_style.rs` | player production | 24,838 | 755 | no |
| `crates/arcweft-player-scene/src/frame/view_style/axis_seed.rs` | player production | 1,081 | 32 | no |
| `crates/arcweft-player-scene/src/frame/view_style/tests.rs` | player unit test | 40,437 | 1,159 | no |

The largest changed integration test remains below the 2,500-LOC warning threshold. No dependency fan-in/fan-out
changed. `resolver.rs` is kept at the 1,200-LOC review boundary with axis and provider responsibilities already split
into their own modules; it has no embedded test module.

## Validation

Final commands and results:

```bash
cargo fmt --all --check
cargo test -p arcweft-view --all-targets --all-features
cargo test -p arcweft-runtime-driver --all-targets --all-features
cargo test -p arcweft-player-scene --all-targets --all-features
cargo test -p arcweft-view --doc
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root . --write target/structure-audit-d411
git diff --check
```

All listed tests/checks pass. The structural audit has 0 errors. A transient stale Rust metadata failure occurred during
an earlier tight loop; `cargo clean -p arcweft-view` removed approximately 2.0 GiB of stale incremental artifacts and
the unchanged command then passed. It was a local build-cache failure, not a product failure.

Tier-2 MCP stdio and exact visual goldens were not run because this slice changes neither MCP transport nor visual
pixel/golden behavior. No remaining implementation TODO is known for seq 06.11d.4.1.1; seq 06.11d.4.2.1 and later
design packages remain independent follow-up work.
