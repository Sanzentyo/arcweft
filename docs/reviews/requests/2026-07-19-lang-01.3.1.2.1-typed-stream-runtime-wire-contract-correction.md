# Request: Typed Stream runtime and wire contract correction

## Sequence position

This is Lang-01.3.1.2.1. It is a narrow contract-correction request after
Lang-01.3.1.2 and before any production implementation of that package's core
Stream state, RuntimePlan, AWBC ABI 2 / codec 8, host wire, bundle, or save
schema 2 cuts.

Lang-01.3.1.2 correctly selects one typed Stream runtime and removes the
duplicate Source runtime path, but its returned final contract contains
conflicting or missing exact ownership and wire decisions. Those conflicts
cannot be resolved by implementation judgment without changing the promised
public and serialized model.

This correction must consume:

- the ordinary-function suspension and generator substrate selected by
  Lang-01.1.1;
- the final external Stream callable evidence selected by Lang-01.3.1.1; and
- the identity, lifecycle, policy, host-boundary, Source-deletion, and
  no-compatibility decisions already fixed by Lang-01.3.1.2.

It must not introduce a new function spelling, `stream fn`, role attribute,
`source`, Source compatibility layer, or endpoint-local Stream model.

## Why this correction is required

The returned Lang-01.3.1.2 package passed archive, manifest, and request
integrity checks, but static contract reconciliation found the following
implementation-stopping defects:

1. `RuntimeParameterPassing::{Positional, Named, RestPositional, RestNamed}`
   cannot losslessly represent the shared callable catalog's
   `PositionalOnly`, `PositionalOrNamed`, and `NamedOnly` distinctions.
   `RuntimeParameterPresence` also omits the catalog's `Optional` case without
   specifying a checked rejection or a lossless projection. This conflicts
   with the Lang-01.3.1.1 requirement to consume the shared resolver result.
2. `StreamInstanceState` has no replay/history field or other sole owner for
   replay records, although `Full`, `HashOnly`, `Summary`, `EventOnly`,
   redaction, `DrainAndRetainReplay`, replay limits, tombstone retention, and
   save/restore behavior are normative.
3. live instances are specified both as a global
   `BTreeMap<StreamInstanceKey, StreamInstanceState>` and as owned
   `FiberState.stream_instances: Vec<StreamInstanceState>`. The contract also
   forbids duplicate/sidecar state and says each instance has exactly one
   owner. External event lookup, affine handle movement, producer-fiber
   ownership, snapshot layout, and restore validation therefore lack one
   coherent authority.
4. `RuntimeExternalStreamOrigin.effects` stores `RuntimeEffectSetId`, but the
   exact RuntimePlan additions do not define the table or accepted semantic
   inventory that owns and bounds that ID.
5. the purportedly exact RuntimePlan model refers to support types such as
   `RuntimeSourceMapRef`, `RuntimeStreamFrameLayout`, and Stream-program
   binding/arm types without defining their owning modules, exact fields, or
   whether an existing typed substrate must be reused.
6. policy resolution depends on profile maxima and support flags for blocking,
   replay retention, privacy, permissions, restart, and provider replacement,
   but no exact typed profile input or owning boundary is defined.
7. item/counter exhaustion is both an atomic event rejection that commits no
   mutation and a terminal `RuntimeLimit` transition that closes the producer.
   Both cannot be the same transition under the stated rejection atomicity
   rule.
8. dropping the sole consumer preserves the queue, while payload release
   requires the queue to be drained and the consumer to be dropped. With no
   remaining consumer, queued payloads can have no drain owner and can remain
   permanently retained.
9. Lang-01.1.1 and Lang-01.3.1.2 both claim the sole ABI 2 / codec 8 / save
   schema 2 transition, while their proposed Stream handle, instance,
   StreamPlan/definition, producer, and fiber shapes differ. They cannot be
   implemented as separate atomic migrations.

These are contract defects, not reasons to redesign the already verified
callable resolver, direct-style suspension substrate, current FiberState VM
exchange, or Lang-01.3.1.2's decision to delete the Source runtime path.

## Required decisions

1. Define a lossless runtime projection of the accepted shared callable
   parameter model.
   - Preserve positional-only, positional-or-named, named-only, rest
     positional, and rest named behavior.
   - Decide whether `Optional` is representable for an external Stream
     operation or is rejected before RuntimePlan lowering.
   - If it is rejected, define the typed checked error and its source range.
   - Do not create a second argument resolver or recover parameter behavior
     from source text.
2. Select exactly one owner for each live and tombstoned
   `StreamInstanceState`.
   - Define the exact container and lookup operation used by normalized host
     events.
   - Define how a fiber owns or references an instance, how an affine handle
     move across fibers behaves, and how a producer child fiber refers to its
     instance.
   - Define snapshot and restore shapes with no duplicated mutable authority,
     shadow registry, or state-rebuilding facade.
3. Define the exact replay record and replay-store model.
   - Give closed typed variants for payload, digest, summary, event-only, and
     no-record cases.
   - Define stable ordering, record identity, limit enforcement, redaction,
     hash domains, summary contents, counter updates, and payload erasure.
   - Place replay state in the sole instance owner or another explicitly
     justified single owner and include it in tombstone and save rules.
4. Define the exact owner and canonical table for every
   `RuntimeEffectSetId`.
   - Prefer the existing accepted semantic effect inventory when dependency
     direction permits.
   - Specify canonical ordering, deduplication, bounds validation, RuntimePlan
     lookup, AWBC projection, fingerprint participation, and tampered-ID
     rejection.
5. Complete every referenced RuntimePlan support type.
   - Define or identify the owning typed substrate for source-map references,
     Stream frame layouts, bindings, expressions, match/error arms, and
     program control flow.
   - Source maps remain debug evidence only and must not become semantic
     identity or a source-text recovery path.
   - Do not duplicate an existing Lang-01.1.1 frame, CFG, callable, or source
     range type under a Stream-local name without a concrete ownership reason.
6. Define the exact typed policy-profile input and resolution boundary.
   - Include every maximum, support flag, minimum privacy/permission rule,
     terminal-error restriction, restart rule, and provider-replacement rule
     used by validation.
   - State which project/build profile owns the values and how native, web,
     and Agent profiles represent unsupported provider-side blocking.
   - Define monotonic tightening and the first-error order without stringly
     profile lookups or silent fallback.
7. Choose one exact exhaustion transition.
   - Distinguish event-envelope rejection from accepted runtime-limit
     terminalization if both concepts remain.
   - Specify staging, sequence/counter advancement, queue mutation, terminal
     state, close request emission, observation/result emission, and repeated
     input behavior.
   - Preserve checked arithmetic and prohibit wrapping or partial commits.
8. Close the dropped-consumer queue lifecycle.
   - Define who may consume, discard, redact, or retain already queued items
     after the sole affine consumer is dropped.
   - Bound retained payload/replay data and define the exact release/tombstone
     condition.
   - Preserve the rule that terminal transition cannot overtake items that
     remain observable to a live consumer, without creating an unreachable
     drain requirement after that consumer is gone.
9. Reconcile Lang-01.1.1 with Lang-01.3.1.2 as one version migration.
   - Identify the Lang-01.1.1 direct-call, suspension, generator
     classification, CFG, safe-point, and producer-fiber substrate that remains
     authoritative.
   - Explicitly supersede or translate, at design time only, Lang-01.1.1's
     provisional StreamPlan, handle, state, event, and wire shapes with the
     corrected Lang-01.3.1.2 model.
   - Assign exactly one implementation cut ownership of
     `AWBC_ABI_VERSION = 2`, `AWBC_CODEC_VERSION = 8`, and
     `BUNDLE_SESSION_SAVE_SCHEMA_VERSION = 2`.
   - Product code must never contain two readers, writers, tables, opcodes, or
     migration paths.
10. Reconfirm the exact numeric and codec allocation against the integration
    base immediately before implementation.
    - Preserve the intended next unused runtime type, instruction,
      terminator, safe-point, nested enum, and table tags unless current main
      has consumed one.
    - Include current callable/flow executable tables and unrelated accepted
      RuntimePlan/AWBC fields in the canonical ordering rules.
11. State the exact JSON representation for every Stream-owned integer that
    crosses the shared host or save boundary.
    - This includes primitive `u64`/`usize` fields nested inside progress,
      counters, statistics, replay records, and snapshots, not only newtype
      identifiers.
    - Define canonical decimal-string serde, platform-independent bounds,
      duplicate/unknown-field rejection, and byte parity.
12. Amend the Lang-01.3.1.2 deletion inventory and test matrix wherever the
    corrected sole-owner, replay, effect-table, profile, or version-cut model
    changes an implementation target.

## Required implementation order

The returned correction must prescribe compile-clean cuts in this order:

1. freeze the corrected lossless parameter projection, single instance owner,
   replay state, effect-set owner, complete RuntimePlan support types, typed
   profile input, exhaustion rule, and dropped-consumer cleanup rule;
2. land or consume Lang-01.1.1's codec-stable ordinary-function/direct
   suspension substrate and the minimum generator classification evidence
   required by the corrected model, without landing its provisional Stream
   runtime/wire shape;
3. consume Lang-01.3.1.1's final shared-resolver-backed external Stream
   callable evidence;
4. implement corrected core Stream identities, handle, policy, lifecycle,
   instance/replay state, requests, events, and deterministic tests;
5. replace RuntimePlan Source/old Stream ownership with the sole corrected
   Stream definition and instance-reference model;
6. perform one atomic AWBC ABI 2 / codec 8 migration containing the generator
   requirements from Lang-01.1.1 and the sole Stream definition/runtime model
   from corrected Lang-01.3.1.2;
7. replace RuntimeStep and native/web/Agent host boundaries with one shared
   typed schema;
8. perform one atomic save schema 2, bundle, restore, fingerprint,
   hot-reload, and generation-pin migration;
9. delete the remaining Source and provisional Stream product paths, then run
   workspace validation and the structural audit.

The correction may split implementation commits more finely, but it must not
create any mergeable revision that writes a new version with an incomplete
schema or accepts both old and new product formats.

## Tests to specify

- shared callable projection tests for positional-only,
  positional-or-named, named-only, both rest forms, required, defaulted, and
  the selected Optional behavior;
- negative tests proving external Stream lowering cannot bypass the shared
  resolver or infer parameter behavior from source text;
- single-owner tests covering creation, host-event lookup, affine moves within
  and across fibers, producer-child ownership, drop, close, tombstone
  replacement, snapshot, and tampered duplicate ownership;
- replay tests for Full, HashOnly, Summary, EventOnly, None, Redacted,
  Transient, Recordable, Private, every retention limit boundary, deterministic
  eviction, and payload-erasure guarantees;
- effect-set table canonicalization, bounds, duplicate, fingerprint, AWBC
  round-trip, and tampered-reference rejection;
- typed profile-resolution tests for every maximum/support flag and monotonic
  tightening branch, including native/web/Agent blocking rejection;
- exhaustion tests at `MAX - 1`, `MAX`, and exhausted state for host items,
  local producers, delivery counters, result/observation emission, close
  idempotence, and byte-for-byte rejection atomicity;
- dropped-consumer tests with empty and non-empty queues, pending and terminal
  producers, every retention policy, bounded replay, repeated cleanup, and
  eventual tombstone release;
- RuntimePlan tests proving every support ID resolves through one accepted
  table and source maps have no semantic or fingerprint effect;
- ABI 2 / codec 8 tests combining direct suspension, authored generators,
  external Stream origins, derived Streams, branch/match, `for await`,
  safe-point restore, and producer fibers;
- tests proving only one Stream definition table, one instance authority, and
  one version reader/writer exist through public typed behavior and codec
  rejection, not repository source-text searches;
- save schema 2 tests for closed external queues, generator safe points,
  replay records, tombstones, global affine uniqueness, tampered ownership,
  old schema rejection, and atomic restore failure;
- shared host/save JSON tests for minimum/maximum decimal strings, leading
  zeroes, plus signs, whitespace, overflow, numbers supplied as JSON numbers,
  duplicate fields, unknown fields, invalid UTF-8/BOM, and native/web/Agent
  byte parity;
- direct rejection evidence for codec 7, ABI 1, save schema 1, old Source
  tables/tags/events, and provisional Lang-01.1.1 Stream wire shapes, with no
  legacy decoder dispatch;
- focused crate tests, workspace check/clippy at reviewable cuts, and
  `cargo +nightly -Zscript tools/structure-audit.rs --root .` after the final
  ownership change.

## Constraints

- Preserve the already implemented shared callable catalog/resolver,
  definition-source index, accepted-HIR lifecycle, external binding
  publication, and query-budget substrate unless a concrete defect is
  demonstrated.
- Preserve Lang-01.1.1's ordinary-function and direct-style suspension
  direction. This correction must not restore `stream fn`, `source`, `task fn`
  as a workaround, or invent a Stream role attribute.
- Preserve Lang-01.3.1.2's typed identity, affine handle, generation-aware
  instance, terminal queue drain for a live consumer, one shared host codec,
  Source deletion, hard old-version rejection, and Sans-I/O boundaries unless
  one of the contradictions above requires a narrowly stated correction.
- Keep `arcweft-core` Sans I/O and keep syntax, HIR, sema, RuntimePlan,
  verifier, bundle/save codecs, and host adapters in their existing dependency
  direction.
- Do not add dual readers, dual writers, deprecated fields, aliases,
  compatibility modules, migration shims, endpoint DTOs, sidecar state maps,
  extension-trait adapters, source-name recovery, stringly protocol identity,
  feature/source gates, or spelling-specific removed-syntax diagnostics.
- Do not use source-text searches as acceptance evidence. Test ownership,
  visibility, behavior, codecs, tampering, and dependency direction through
  typed APIs and structured metadata.
- Do not redesign proof/concurrency, style/environment, view, rich text,
  character identity, Need/task scheduling, or unrelated callable behavior.
- Do not touch CSS or Takumi dependencies.

## Non-goals

- choosing or changing Arcweft author syntax;
- reintroducing a `source` declaration or a dedicated Stream function kind;
- designing provider-specific I/O, transport framing, resume tokens, or
  endpoint-local schemas;
- preserving unreleased Source, provisional StreamPlan, ABI 1/codec 7, or save
  schema 1 compatibility;
- changing non-Stream save, bundle, presentation, proof, or environment
  contracts except where an existing typed field must carry the corrected
  Stream state;
- broad refactoring of verified substrate without a concrete defect required
  to resolve this request.

## Expected output

Return an independently usable final-contract package containing:

- `FINAL_CONTRACT.md` with all decisions above closed and
  `OPEN_QUESTIONS=0`;
- a normative delta table showing every corrected Lang-01.3.1.2 field,
  owner, enum variant, invariant, wire tag, and superseded Lang-01.1.1
  provisional Stream shape;
- exact Rust-shaped ownership and wire schemas with all referenced support
  types defined or linked to one existing owner;
- a single version/tag allocation table against the stated current-main
  revision;
- an updated ordered implementation plan with one named owner for the ABI,
  codec, and save cuts;
- an updated exhaustive positive/negative/tampering test matrix with unique
  stable IDs;
- an amended Source/provisional-Stream deletion inventory;
- worked canonical host JSON, AWBC, bundle fingerprint, save, restore, replay,
  exhaustion, and dropped-consumer examples;
- repository evidence distinguishing already implemented substrate from
  proposed changes;
- a manifest with byte counts and SHA-256 hashes for every artifact.

The package must state any remaining unresolved decision explicitly instead of
claiming implementation readiness. Production implementation may resume only
when the corrected exact shapes are mutually consistent and no implementer
choice remains at the public/runtime/wire boundaries.
