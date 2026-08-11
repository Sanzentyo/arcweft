# Typed Stream policy-profile contract

## 1. Owner and target selection

The authored tightening vocabulary is
`arcweft_manifest_model::stream_profile::StreamRuntimeProfileSpec`. The existing private
`arcweft_launch::manifest::ProfileSpec` owns one `stream` field of that type, and the
existing `SourceBackedManifest`/`ManifestSourceMap` owns the exact `SourceSpan` for every
authored Stream-profile field. This extends the accepted single-decode manifest substrate;
it does not add a second manifest reader or source-text recovery path.

`arcweft_core::stream::StreamRuntimeTarget::{Native, Web, Agent}` is an explicit typed
field of the compiler build context. The selected launch profile and target must satisfy:

- `LaunchKind::Agent` pairs only with `StreamRuntimeTarget::Agent`;
- every non-Agent `LaunchKind` pairs with exactly the explicitly supplied `Native` or
  `Web` target;
- every other pair is `STREAM_PROFILE_TARGET_KIND_MISMATCH` at the accepted profile
  `kind` span.

The existing `arcweft_compiler::ProjectCompilationContext` is the sole cross-crate
projection boundary. It exhaustively combines the selected source-backed profile and the
typed target into `arcweft_runtime_plan::stream_profile::AuthoredStreamRuntimeProfileInput`.
`arcweft-runtime-plan` applies the built-in baseline and monotonic rules and owns the
resulting `AcceptedStreamRuntimeProfile`. RuntimePlan receives only resolved per-definition
policy plus `RuntimeStreamProfileEvidence { target, canonical_hash }` owned by
`arcweft-core`; it never depends on `arcweft-launch` or reads manifest text.

This preserves the current sibling dependency direction: `arcweft-compiler` already
orchestrates launch and runtime-plan products; no `arcweft-runtime-plan -> arcweft-launch`,
`arcweft-launch -> arcweft-core`, or host-adapter dependency is introduced. No string
profile lookup, Cargo feature/source gate, adapter-name heuristic, or silent fallback
selects the target. The target and accepted profile hash are carried through compilation
and bundle identity. Every absent authored field has explicit `TargetBaseline`
provenance; every present field has its exact accepted `SourceSpan`.

## 2. Exact current target baselines

All byte values are bytes. Every integer is a Rust fixed-width value; its host/save JSON
form is a canonical decimal string.

| Field | Native | Web | Agent |
| --- | --- | --- | --- |
| max_live_instances | 4096 | 1024 | 256 |
| max_tombstones | 4096 | 1024 | 256 |
| max_events_per_step | 4096 | 1024 | 256 |
| max_queue_items_per_instance | 65536 | 16384 | 4096 |
| max_queue_bytes_per_instance | 67108864 | 16777216 | 8388608 |
| max_total_queue_bytes | 268435456 | 67108864 | 33554432 |
| max_item_bytes | 8388608 | 4194304 | 1048576 |
| max_replay_records_per_instance | 4096 | 1024 | 256 |
| max_replay_payload_bytes_per_instance | 67108864 | 16777216 | 4194304 |
| max_replay_total_bytes_per_instance | 75497472 | 20971520 | 6291456 |
| max_total_replay_bytes | 536870912 | 134217728 | 33554432 |
| max_open_arguments | 1024 | 256 | 128 |
| max_open_argument_bytes | 8388608 | 4194304 | 1048576 |
| max_lifetime_events | 18446744073709551614 | 18446744073709551614 | 18446744073709551614 |
| max_lifetime_items | 9223372036854775807 | 9223372036854775807 | 9223372036854775807 |
| max_recoverable_errors | 9223372036854775807 | 9223372036854775807 | 9223372036854775807 |
| max_progress_events | 9223372036854775807 | 9223372036854775807 | 9223372036854775807 |
| max_deliveries | 18446744073709551614 | 18446744073709551614 | 18446744073709551614 |
| max_restart_attempts | 8 | 0 | 0 |
| max_provider_replacements | 8 | 0 | 0 |
| supports_provider_blocking | false | false | false |
| supports_full_replay | true | true | false |
| supports_hash_replay | true | true | true |
| supports_summary_replay | true | true | true |
| supports_event_replay | true | true | true |
| supports_coalesce | true | true | true |
| supports_restart | true | false | false |
| supports_provider_replacement | true | false | false |
| minimum_privacy | Recordable | Recordable | Redacted |
| minimum_permission | AtOpen | AtOpen | EachEvent |
| maximum_terminal_error_replay | Payload | Digest | EventOnly |

`supports_provider_blocking=false` is non-overridable for all three targets. A
`ProviderBlocking` definition produces `StreamPolicyError::ProviderBlockingUnsupported`
before RuntimePlan emission. No accepted plan or host adapter emulates blocking.

## 3. Authored default

Absence of authored Stream policy yields exactly:

```text
backpressure             = LatestOnly (one queued delivery)
replay                    = EventOnly
privacy                   = Transient
permission                = AtOpen
consumer_drop             = DiscardQueued
replay_retention          = UntilConsumerDrop
terminal_error_replay     = EventOnly
restart                   = Deny
provider_replacement      = Deny
requested maxima          = none (use definition-derived capacity plus target ceiling)
```

This preserves the existing accepted Source-policy default semantics while deleting all
Source types and paths. `LatestOnly` resolves to item capacity 1 and byte capacity equal
to the target/definition item-byte limit.

## 4. Explicit privacy and permission ranks

Derived Rust enum ordering is not policy ordering. Resolution uses these explicit total
ranks:

```text
privacy:   Recordable(0) < Redacted(1) < Transient(2) < Private(3)
permission: AtOpen(0) < OnRestart(1) < EachEvent(2)
replay data class: EventOnly(0) < Summary(1) < Digest(2) < Payload(3)
```

A higher privacy/permission rank is tighter. A lower replay-data-class rank is tighter.
The project profile resolves privacy/permission with `max(rank)` and terminal replay with
`min(rank)`. The accepted result and provenance are embedded in build evidence and the
profile hash; this is explicit monotonic tightening, not an unreported fallback.

## 5. Project-profile monotonic tightening

For every optional project-profile field:

- maxima resolve with `min(target, authored)`; a zero value is allowed only for replay
  records/bytes and restart/replacement attempts;
- support flags resolve with `target && authored`; `true` cannot enable a target-disabled
  feature;
- minimum privacy/permission can only increase the rank;
- maximum terminal replay can only decrease the rank;
- provider blocking has no authored switch and remains false.

An authored attempt to loosen a field is a typed profile error at that field's manifest
range. `None` means use the typed target baseline and is represented with explicit
`TargetBaseline` provenance; no text lookup or guessed default occurs.

## 6. Definition-policy resolution

Resolution occurs once after accepted callable/effect/type evidence and before RuntimePlan
emission. It returns either one `ResolvedStreamPolicy` or one typed first error.

1. Validate item/error payload eligibility and canonical maximum item size.
2. Resolve target/project profile as section 5.
3. Reject `ProviderBlocking` because all current targets disable it.
4. Validate backpressure capacity and byte limits against the resolved profile.
5. Validate the overflow branch:
   - `DropOldest`, `DropNewest`, and `TerminalError` require no helper;
   - `Coalesce` requires target support and an accepted pure, deterministic,
     non-suspending reducer with signature `(T, T) -> T` and no effects.
6. Validate requested replay mode support. Privacy does not bypass a disabled requested
   mode; for example Agent rejects requested `Full` even when Redacted would otherwise
   store a summary.
7. Apply the privacy/replay projection matrix from `FINAL_CONTRACT.md`.
8. Tighten permission to the profile floor.
9. Tighten terminal-error replay to the profile cap.
10. Validate drop/replay retention: `DrainAndRetainReplay` requires replay other than
    `None`, privacy `Recordable` or `Redacted`, nonzero record/total limits, and
    `ThroughTombstone`.
11. Validate restart/replacement requests. A definition requesting an allowed behavior
    when target support is false is rejected rather than changed to `Deny`.
12. Resolve every requested maximum. A per-definition request over the accepted profile
    ceiling is rejected rather than clamped.
13. Validate cross-field arithmetic/invariants.

## 7. Cross-field invariants

- queue item and byte capacities are nonzero;
- `max_item_bytes <= max_queue_bytes`;
- `LatestOnly.max_queue_items == 1`;
- bounded/blocking capacity is no larger than both per-definition and target limits;
- replay payload bytes are no larger than replay total bytes;
- per-instance queue/replay maxima are no larger than global maxima;
- `max_deliveries >= checked_add(max_lifetime_items, max_recoverable_errors)`;
- every lifetime maximum is at most `u64::MAX - 1`, preserving a final checked terminal
  transition/observation slot;
- restart/replacement maxima are zero when the support flag is false and nonzero when an
  authored policy requests the corresponding allowed rule;
- `Private` and `Transient` produce no replay records and block saves while their queue
  contains payload;
- `Redacted` never stores a payload record;
- terminal error replay never exceeds both privacy projection and profile cap;
- provider replacement is only `SameOrigin`: definition key, module ABI, capability,
  operation, callable signature, effects, and item/error layouts must match.

## 8. Stable typed errors and first-error order

The validator returns the first error in this exact order. Within a group, fields are
checked in declaration order shown in `RUST_SCHEMAS.md`.

| Order | Stable error code | Condition/range |
| --- | --- | --- |
| 1 | `STREAM_PROFILE_TARGET_KIND_MISMATCH` | Typed build target and selected `LaunchKind` violate the Agent/non-Agent matrix; accepted profile `kind` span. |
| 2 | `STREAM_PROFILE_INTERNAL_BASELINE` | Built-in target constants violate a cross-field invariant; no source range. |
| 3 | `STREAM_PROFILE_LOOSEN_MAXIMUM` | Project profile raises a target maximum; authored manifest field. |
| 4 | `STREAM_PROFILE_ENABLE_UNSUPPORTED` | Project profile enables a target-disabled support flag; authored field. |
| 5 | `STREAM_PROFILE_LOWER_PRIVACY` | Project profile lowers the privacy floor; authored field. |
| 6 | `STREAM_PROFILE_LOWER_PERMISSION` | Project profile lowers the permission floor; authored field. |
| 7 | `STREAM_PROFILE_RAISE_TERMINAL_REPLAY` | Project profile raises terminal replay cap; authored field. |
| 8 | `STREAM_POLICY_PAYLOAD_INELIGIBLE` | Item/error schema is not host/save payload eligible; return type span. |
| 9 | `STREAM_POLICY_ITEM_TOO_LARGE` | Type's maximum canonical size exceeds target item limit; return type span. |
| 10 | `STREAM_POLICY_BLOCKING_UNSUPPORTED` | ProviderBlocking requested; backpressure span. |
| 11 | `STREAM_POLICY_ZERO_QUEUE` | Queue item/byte capacity is zero; capacity span. |
| 12 | `STREAM_POLICY_QUEUE_LIMIT` | Per-definition queue request exceeds profile; offending limit span. |
| 13 | `STREAM_POLICY_ITEM_QUEUE_RELATION` | Item bytes exceed queue bytes; first offending field span. |
| 14 | `STREAM_POLICY_COALESCE_UNSUPPORTED` | Target disables coalescing; overflow span. |
| 15 | `STREAM_POLICY_COALESCE_CALLABLE` | Reducer evidence/signature/effects/suspension invalid; reducer span. |
| 16 | `STREAM_POLICY_REPLAY_UNSUPPORTED` | Requested replay mode support is false; replay span. |
| 17 | `STREAM_POLICY_REPLAY_LIMIT` | Replay request exceeds profile or has inconsistent byte limits; offending field. |
| 18 | `STREAM_POLICY_DROP_REPLAY` | DrainAndRetainReplay preconditions fail; drop-policy span. |
| 19 | `STREAM_POLICY_PERMISSION` | Permission rule cannot satisfy accepted semantic/provider requirement; permission span. |
| 20 | `STREAM_POLICY_TERMINAL_REPLAY` | Requested terminal policy cannot be represented under privacy/profile cap; terminal span. |
| 21 | `STREAM_POLICY_RESTART_UNSUPPORTED` | SameProvider requested without target support/nonzero attempts; restart span. |
| 22 | `STREAM_POLICY_REPLACEMENT_UNSUPPORTED` | SameOrigin requested without target support/nonzero replacements; replacement span. |
| 23 | `STREAM_POLICY_LIFETIME_LIMIT` | Lifetime request exceeds profile; offending maximum span. |
| 24 | `STREAM_POLICY_DELIVERY_RELATION` | Delivery maximum cannot cover item+recoverable-error maxima; first maximum span. |
| 25 | `STREAM_POLICY_PROVIDER_ABI` | External origin/provider ABI cannot satisfy policy; external operation span. |

Errors retain typed source/manifest ranges from accepted evidence. They do not search
source text or fall back to another profile.

## 9. Hot-reload policy monotonicity

For a live/pinned definition, a replacement policy is compatible only when it is equal or
stricter by the ranks above, all maxima are equal or lower, no support is added, queue and
replay already fit the new maxima, and the consumer/producer lifecycle can continue
without data loss. A loosening or any state that exceeds a tighter maximum is
`Incompatible`; there is no state truncation or policy shim. Same-provider restart and
same-origin replacement are used only when both old and new resolved policies permit the
operation.
