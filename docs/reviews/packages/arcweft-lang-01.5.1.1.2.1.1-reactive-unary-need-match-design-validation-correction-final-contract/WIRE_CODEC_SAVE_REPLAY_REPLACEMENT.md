# Wire, codec, save, replay, replacement

## ViewProgram transcript v1

The existing AWFB ViewProgram field/envelope remains. The unreleased transcript v1
is replaced directly.

| Row | Canonical key | Validation |
|---|---|---|
| Need subscription | nonzero local subscription ID | dense/unique, same owner program/node |
| producer ref | subscription ID | exact AWBC function/task/cross-section/program digest |
| generic Match | instruction coordinate | selector and source-ordered arm spans/bindings |
| Need-state input | `(value program, register)` | exact subscription and state type |
| binding output | `(arm ordinal, local)` | unique register/type/ownership |
| source ref | existing source coordinate | diagnostics only; excluded from identity |

All public DTOs deny unknown fields, use closed snake-case tags, bounded decode,
canonical ordering, checked indices, and recomputed semantic/contract digests.

## Old bytes

The `await` tag is absent. Old bytes fail closed enum decode before catalog
construction. There is no ViewAwait, ViewAwaitBranchSpan, source_program,
pending/ready/error/denied branch field, integer state discriminant, alias,
renamed tag, compatibility reader, or fallback. Version remains 1.

## Save v1

Canonical tables:

1. producers keyed by producer contract/generation/NeedId;
2. publications with cursor/state/state digest;
3. observers keyed by mount plus stable subscription semantic/contract;
4. retained arms keyed by observer/ordinal/arm digest;
5. sorted invalidations keyed by observer/revision.

Publication payload is stored once; observers use checked indices. NeedId and
TaskKey are existing typed runtime identities, not source/endpoint copies.
Ready payload uses ordinary canonical RuntimeValue snapshot encoding.

## Restore

Validate in scratch: marker; counts; order/uniqueness; program/revision/mount
allocator; semantic/contract joins; producer binding/generation/NeedId; cursor
and terminal invariants; payload type/ownership/digest; observer refs; active and
retained arm contracts; queue revisions; limits. Only then swap. Failure leaves
active session byte-identical.

## Replay

Replay applies the same generation-bound publication API in canonical journal
order. No replay-only selector exists. Host dispatch is suppressed while replay
events remain. A final NotStarted state may emit a start intent only in the first
post-replay committed frame.

## Hot replacement

| Producer | Types | selector/arms | Result |
|---|---|---|---|
| same | same | same | preserve journal/cursor/arms/queue |
| same | same | arm contract changed | preserve publication; drop incompatible arm state; queue one invalidation |
| same | changed | any | reject type/cross-section candidate |
| changed | same/changed | any | fresh generation; no old publication/arm reuse |
| removed | n/a | n/a | retire observer; preserve unrelated/non-View producer observers |
| added | valid | valid | fresh observer; latest exact journal or NotStarted |
| tampered/stale | any | any | reject; active runtime unchanged |

Dense IDs are never compared across revisions. Stable join requires semantic ID
and contract digest. Candidate catalog, mapping, task intents, observer state, and
queues are staged, then swapped once.
