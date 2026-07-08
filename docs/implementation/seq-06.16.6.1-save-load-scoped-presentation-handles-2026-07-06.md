# seq-06.16.6.1 save/load scoped presentation handles implementation note

## Summary

This cut turns the scoped presentation handle substrate into an end-to-end Product AWBC session save/load contract. The save payload is runtime-driver-owned and stores execution state plus `BundlePresentationSnapshot` together. This prevents presentation handle tombstones and cleanup owner metadata from being restored without the compact fiber state that owns future cleanup execution.

## Schema

- envelope schema id: `arcweft.bundle_session`
- envelope schema version: `1`
- payload codec: typed JSON through `arcweft-save`
- Product AWBC only in v1
- Structured VM/AOT restore: typed rejection
- pending runtime-driver queues/tasks/UI waits: typed rejection unless quiescent

The payload root is `BundleSessionSnapshot`:

- `schema`
- `generation`
- `runtime`
- `executor`
- `presentation`
- `pending`

`generation` carries active generation id, content root, optional container content root, ABI, and adapter requirement fingerprint. `executor` is tagged and only has the `product_awbc` variant in v1. `presentation` is the full portable `BundlePresentationSnapshot` including handle records, operation epoch, and tombstones.

## Product AWBC snapshot/restore

`AwbcProductStepExecutor::snapshot` exports compact main/child fibers, active dialogue/choice state, Product AWBC pending host-call identity, de-duplication sets, stream sequence counters, compact pure stats, and facade observation state.

`AwbcProductStepExecutor::restore_snapshot` validates compact fiber tables against the current `AwbcProgram`, restores internal Product AWBC state, rebuilds facade source/stream runtime state from compact queues, and calls `sync_facade` so status/env/display/observe paths read the restored compact state.

## Rollback behavior

Rollback restores `BundlePresentationSnapshot` and Product AWBC `FiberState` atomically. Existing `PresentationResourceState::is_terminal` handling remains the source of truth: restored terminal handles cannot be shown, hidden, or unmounted by stale operations because those operations return terminal-handle diagnostics rather than changing the resource state.

## Pending save-points

Version 1 does not serialize adapter-owned pending host tasks or UI waits. `BundleSession::snapshot_session` rejects such save-points with `BundleSessionPendingBlocker`. Product AWBC source/stream queues are not adapter-owned and are preserved through compact `FiberState`.

## Verification

- `presentation_handle_save_load_restores_lifecycle_table`
- `presentation_handle_rollback_restores_tombstones`
- `awbc_save_load_preserves_cleanup_stacks`
- `session_restore_rebuilds_facade_fiber`
- `save_decode_rejects_mismatched_bundle_generation`
- `save_decode_rejects_unsupported_executor_tier`
- `save_envelope_strict_decode_rejects_future_or_trailing_payloads`

All of the above are covered by:

```bash
cargo test -p arcweft-runtime-driver --all-features --test awbc_product_session
```

## CLI/player

The native runner now exposes the first host UX slice:

```bash
arcw run --runner native path/to/game.arcw --session-load path/to/session.awfs
arcw run --runner native path/to/game.arcw --session-save-out path/to/session.awfs
```

The native-player save file is a typed `arcweft.native_player_session` envelope
that stores the runtime-driver `BundleSession` save bytes and the
`InputControllerSnapshot` owned by the player scene. That keeps authored
presentation handles, cleanup stacks, rollback tombstones, and player-owned
scroll offsets in one portable native session file.

The UX rejects `--watch`, `--runner auto`, `--runner web`, and
`--runner headless` with session save/load flags. Save still rejects
non-quiescent runtime-driver state rather than serializing pending host calls,
text write-backs, waiting action receives, host tasks, or generation pins.

Additional verification:

```bash
cargo test -p arcweft-player-native --all-features native_player_session_save_pairs_runtime_and_input_snapshots
cargo test -p arcweft-cli --all-features --test native_text_input_trace_cli runtime_run_session_save_flags_are_native_player_only
cargo run -p arcweft-cli --all-features --quiet -- run --runner headless samples\modern-feedback-view\src\main.arcw --session-save-out target\arcweft\session-smoke.awfs
```

The last command is an expected native-only rejection check.
