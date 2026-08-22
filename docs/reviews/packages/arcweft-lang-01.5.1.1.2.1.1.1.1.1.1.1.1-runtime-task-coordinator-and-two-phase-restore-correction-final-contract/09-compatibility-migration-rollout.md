# Compatibility, migration, rollout, and rollback

## 1. Persisted compatibility policy

| Input | Policy |
|---|---|
| current schema v1 | strict canonical decode and full semantic validation |
| explicitly supported older snapshot schema | pure normalize-to-current prepared representation; retain source version in diagnostics |
| older journal without restore records | no restore decision exists; normal snapshot discovery policy applies |
| unknown/newer mandatory schema or feature | fail closed before mutation |
| unknown optional feature | accepted only when the envelope explicitly marks it skippable and digest semantics remain defined |
| same restore token with different bytes/digest | corruption; never migrate by overwrite |

The journal format is append-only and version/domain tagged. A migration never rewrites an existing committed decision in place. It writes a new versioned record whose provenance points to the old record/snapshot and whose digest is independently verified.

## 2. Migration boundaries

- Snapshot schema migration belongs in persistence normalization, not coordinator logic.
- Runtime ABI/catalog migration requires an explicit admitted mapping and new digest; it is not inferred by restore.
- Coordinator rebinding requires a migration operation with a new `RestoreId` and provenance; ordinary restore enforces coordinator identity.
- Task identity/generation is preserved. A migration that changes it produces a new semantic snapshot and explicit mapping table.
- Match transcript/coverage is recomputed and sealed under the target version; incomplete old data cannot be grandfathered in.

## 3. Rollout stages

| Stage | Enablement | Gate |
|---:|---|---|
| 0 | land codecs/reducer and tests, reader disabled | golden/fuzz tests only; no runtime behavior change |
| 1 | shadow prepare on selected snapshots, discard result | equality with normal admission seals/digests; observer-silence metrics |
| 2 | enable startup restore in test/dev coordinators | complete crash/concurrency matrix |
| 3 | canary production restore behind existing feature/config gate | replay/conflict/corruption telemetry; no mixed epoch |
| 4 | broaden enablement | stable SLOs and zero unexplained token/digest mismatches |
| 5 | make coordinator path authoritative; remove old partial path | source audit proves single owner |

Shadow mode must never write `COMMITTED` or publish handles; it is prepare-only validation.

## 4. Rollback

Before a v1 `COMMITTED` record is written, disablement is a normal rollback. After a durable commit exists, binaries used for rollback must still understand/replay that record. Therefore:

- do not deploy a writer until every rollback candidate can read/replay v1, or pin a forward-recovery binary;
- feature disablement stops new restores but cannot ignore committed records;
- optional ACK/compaction can be disabled safely;
- never roll back to a binary that treats the journal tail as unknown and opens scheduling anyway;
- preserve snapshots and journal records until replay success and retention policy allow compaction.

## 5. Observability and alerting

Track rates/latencies for prepare, commit-decision, publish, replay, conflicts, corruption, stale epochs, and committed-to-published delay. Alert on:

- any `COMMITTED` record not published before scheduler-ready;
- same-token/different-digest event;
- task/handle/match cardinality mismatch;
- post-commit publication invariant failure;
- repeated PREPARED records that never commit and exceed retention policy;
- restore latency or decode-budget rejection spikes.

Telemetry identifiers use bounded coordinator/restore IDs or digest prefixes according to project privacy policy; never captured values.

## 6. Security and resource posture

Strict canonical decoding, bounded allocation, no callbacks during prepare, and no untrusted data in panic/log formatting are mandatory. Persistence corruption is not a user-level recoverable task result. Access control for reading snapshots and restoring coordinator state remains with the existing runtime/persistence boundary.
