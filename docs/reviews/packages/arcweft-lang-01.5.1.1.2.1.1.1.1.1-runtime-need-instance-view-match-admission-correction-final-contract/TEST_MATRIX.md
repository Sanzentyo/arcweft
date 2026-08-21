# Implementation validation and test matrix

## Status

These are required implementation admission tests. They are specified by this
design archive; they were not executed against production in this design-only
return.

Total normative rows: **231**.

### Coverage by kind

- `boundary`: 1
- `differential`: 5
- `exact_limit`: 15
- `integration`: 6
- `negative`: 47
- `one_over`: 15
- `positive`: 45
- `precedence`: 1
- `property`: 36
- `rollback`: 8
- `structural`: 20
- `tamper`: 14
- `tier2`: 15
- `unit`: 3

### Coverage by concern

- `await`: 4
- `await_many`: 10
- `event`: 20
- `identity`: 36
- `limits`: 26
- `match`: 20
- `ownership`: 22
- `runtime_value_digest`: 12
- `structural`: 20
- `task_policy`: 20
- `tier2`: 15
- `timeout`: 6
- `view`: 20

## Normative rows

| ID | Concern | Kind | Owner | Input | Expected | Gate |
|---|---|---|---|---|---|---|
| `ID-FAM-01A` | identity | property | `NeedProducerInstanceKey` | two separately constructed StructuredTaskPlan inputs with identical contract/plan/site/payload/canonical argument digest | byte-identical nonzero NeedProducerInstanceKey | `cargo test -p arcweft-core task_identity` |
| `ID-FAM-01B` | identity | property | `NeedProducerInstanceKey` | StructuredTaskPlan input varied one field at a time: contract, plan, site, payload, arguments | each variation changes the key; family variation also changes the key | `cargo test -p arcweft-core task_identity` |
| `ID-FAM-02A` | identity | property | `NeedProducerInstanceKey` | two separately constructed AwbcTaskPlan inputs with identical contract/plan/site/payload/canonical argument digest | byte-identical nonzero NeedProducerInstanceKey | `cargo test -p arcweft-core task_identity` |
| `ID-FAM-02B` | identity | property | `NeedProducerInstanceKey` | AwbcTaskPlan input varied one field at a time: contract, plan, site, payload, arguments | each variation changes the key; family variation also changes the key | `cargo test -p arcweft-core task_identity` |
| `ID-FAM-03A` | identity | property | `NeedProducerInstanceKey` | two separately constructed ViewMatchSubscription inputs with identical contract/plan/site/payload/canonical argument digest | byte-identical nonzero NeedProducerInstanceKey | `cargo test -p arcweft-core task_identity` |
| `ID-FAM-03B` | identity | property | `NeedProducerInstanceKey` | ViewMatchSubscription input varied one field at a time: contract, plan, site, payload, arguments | each variation changes the key; family variation also changes the key | `cargo test -p arcweft-core task_identity` |
| `ID-FAM-04A` | identity | property | `NeedProducerInstanceKey` | two separately constructed AwaitManyBase inputs with identical contract/plan/site/payload/canonical argument digest | byte-identical nonzero NeedProducerInstanceKey | `cargo test -p arcweft-core task_identity` |
| `ID-FAM-04B` | identity | property | `NeedProducerInstanceKey` | AwaitManyBase input varied one field at a time: contract, plan, site, payload, arguments | each variation changes the key; family variation also changes the key | `cargo test -p arcweft-core task_identity` |
| `ID-FAM-05A` | identity | property | `NeedProducerInstanceKey` | two separately constructed AwaitManyChild inputs with identical contract/plan/site/payload/canonical argument digest | byte-identical nonzero NeedProducerInstanceKey | `cargo test -p arcweft-core task_identity` |
| `ID-FAM-05B` | identity | property | `NeedProducerInstanceKey` | AwaitManyChild input varied one field at a time: contract, plan, site, payload, arguments | each variation changes the key; family variation also changes the key | `cargo test -p arcweft-core task_identity` |
| `ID-FAM-06A` | identity | property | `NeedProducerInstanceKey` | two separately constructed Timeout inputs with identical contract/plan/site/payload/canonical argument digest | byte-identical nonzero NeedProducerInstanceKey | `cargo test -p arcweft-core task_identity` |
| `ID-FAM-06B` | identity | property | `NeedProducerInstanceKey` | Timeout input varied one field at a time: contract, plan, site, payload, arguments | each variation changes the key; family variation also changes the key | `cargo test -p arcweft-core task_identity` |
| `ID-FAM-07A` | identity | property | `NeedProducerInstanceKey` | two separately constructed LineTask inputs with identical contract/plan/site/payload/canonical argument digest | byte-identical nonzero NeedProducerInstanceKey | `cargo test -p arcweft-core task_identity` |
| `ID-FAM-07B` | identity | property | `NeedProducerInstanceKey` | LineTask input varied one field at a time: contract, plan, site, payload, arguments | each variation changes the key; family variation also changes the key | `cargo test -p arcweft-core task_identity` |
| `ID-FAM-08A` | identity | property | `NeedProducerInstanceKey` | two separately constructed HostAdapterTask inputs with identical contract/plan/site/payload/canonical argument digest | byte-identical nonzero NeedProducerInstanceKey | `cargo test -p arcweft-core task_identity` |
| `ID-FAM-08B` | identity | property | `NeedProducerInstanceKey` | HostAdapterTask input varied one field at a time: contract, plan, site, payload, arguments | each variation changes the key; family variation also changes the key | `cargo test -p arcweft-core task_identity` |
| `ID-FAM-09A` | identity | property | `NeedProducerInstanceKey` | two separately constructed MakeNeedHandle inputs with identical contract/plan/site/payload/canonical argument digest | byte-identical nonzero NeedProducerInstanceKey | `cargo test -p arcweft-core task_identity` |
| `ID-FAM-09B` | identity | property | `NeedProducerInstanceKey` | MakeNeedHandle input varied one field at a time: contract, plan, site, payload, arguments | each variation changes the key; family variation also changes the key | `cargo test -p arcweft-core task_identity` |
| `ID-001` | identity | unit | `NeedId` | same producer, Join policy, ordinal 0 | same nonzero NeedId | `cargo test -p arcweft-core task_identity` |
| `ID-002` | identity | unit | `TaskKey` | same generation+producer+policy across calls | same TaskKey | `cargo test -p arcweft-core task_identity` |
| `ID-003` | identity | unit | `TaskId` | same TaskKey+ordinal | same TaskId | `cargo test -p arcweft-core task_identity` |
| `ID-004` | identity | property | `TaskKey` | vary only launch ordinal in a reference transcript harness | TaskKey bytes remain unchanged; production API has no ordinal parameter | `cargo test -p arcweft-core task_identity` |
| `ID-005` | identity | property | `TaskId` | vary ordinal once while TaskKey constant | TaskId changes exactly once; no double-inclusion path | `cargo test -p arcweft-core task_identity` |
| `ID-006` | identity | property | `NeedId` | vary ordinal once while producer/policy constant | NeedId changes exactly once | `cargo test -p arcweft-core task_identity` |
| `ID-007` | identity | negative | `NeedProducerInstanceKey` | try_from_bytes([0;32]) | typed Zero(NeedProducerInstance), no rehash | `cargo test -p arcweft-core task_identity` |
| `ID-008` | identity | negative | `NeedId` | try_from_bytes([0;32]) | typed Zero(Need), no rehash | `cargo test -p arcweft-core task_identity` |
| `ID-009` | identity | negative | `TaskKey` | try_from_bytes([0;32]) | typed Zero(TaskKey), no rehash | `cargo test -p arcweft-core task_identity` |
| `ID-010` | identity | negative | `TaskId` | try_from_bytes([0;32]) | typed Zero(Task), no rehash | `cargo test -p arcweft-core task_identity` |
| `ID-011` | identity | positive | `GenerationId` | GenerationId::new(0) | accepted and round-trips | `cargo test -p arcweft-core task_identity` |
| `ID-012` | identity | positive | `TaskLaunchOrdinal` | JoinSameKey with 0 | accepted as JOIN | `cargo test -p arcweft-core task_identity` |
| `ID-013` | identity | negative | `TaskLaunchOrdinal` | JoinSameKey with 1 | NonZeroJoinOrdinal | `cargo test -p arcweft-core task_identity` |
| `ID-014` | identity | negative | `TaskLaunchOrdinal` | AlwaysStart with 0 | ZeroAlwaysStartOrdinal | `cargo test -p arcweft-core task_identity` |
| `ID-015` | identity | positive | `TaskLaunchOrdinal` | AlwaysStart with 1 | accepted | `cargo test -p arcweft-core task_identity` |
| `ID-016` | identity | negative | `snapshot Option fields` | missing cursor encoded as zero digest rather than Option::None | strict decode rejects malformed/missing Option representation | `cargo test -p arcweft-bundle task_codec` |
| `ID-017` | identity | property | `correlation` | same producer+ordinal across GenerationId 0 and 1 | same NeedId, different TaskKey and TaskId | `cargo test -p arcweft-core task_identity` |
| `ID-018` | identity | tamper | `NeedProducerInstance` | stored key changed without fields | recompute mismatch and no TaskSpec publication | `cargo test -p arcweft-core task_identity` |
| `TASK-001` | task_policy | integration | `TaskHost::ensure_task` | two identical JoinSameKey starts | one adapter launch; same handle/correlation | `cargo test -p arcweft-runtime-scheduler ensure_task` |
| `TASK-002` | task_policy | integration | `observer table` | two View mounts + two fiber observers on same Join handle | one task/event stream/Need publication; four observer states | `cargo test -p arcweft-runtime-driver need_observers` |
| `TASK-003` | task_policy | integration | `TaskHost::ensure_task` | two equal-input AlwaysStart calls | same TaskKey, ordinals 1/2, distinct NeedId/TaskId | `cargo test -p arcweft-runtime-scheduler ensure_task` |
| `TASK-004` | task_policy | integration | `RuntimeNeedState` | two equal-input AlwaysStart launches publish different Ready values | both accepted; no terminal conflict | `cargo test -p arcweft-runtime-driver need_journal` |
| `TASK-005` | task_policy | rollback | `ensure_task transaction` | AlwaysStart adapter prepare fails before commit | no task/group/Need; ordinal counter absent/unchanged | `cargo test -p arcweft-runtime-scheduler ensure_task_rollback` |
| `TASK-006` | task_policy | rollback | `ensure_task transaction` | staged journal limit fails after adapter prepare | adapter rollback called; ordinal not consumed | `cargo test -p arcweft-runtime-scheduler ensure_task_rollback` |
| `TASK-007` | task_policy | positive | `snapshot replay` | AlwaysStart committed ordinals 1,2; save/restore; launch again | restored next ordinal is 3 | `cargo test -p arcweft-runtime-driver task_restore` |
| `TASK-008` | task_policy | negative | `Join group` | same TaskKey but changed request/outcome | JoinSpecificationConflict, existing task unchanged | `cargo test -p arcweft-runtime-scheduler ensure_task` |
| `TASK-009` | task_policy | positive | `MakeNeedHandle` | JoinSameKey plan | reusable RuntimeNeedHandle with ordinal 0 | `cargo test -p arcweft-core awbc_need_handle` |
| `TASK-010` | task_policy | negative | `AWBC verifier` | MakeNeedHandle plan with AlwaysStart | verification rejection before runtime | `cargo test -p arcweft-core awbc_verifier` |
| `TASK-011` | task_policy | negative | `RuntimeNeedHandle` | try_reusable_join with AlwaysStart spec | ReusableAlwaysStart | `cargo test -p arcweft-core runtime_need_handle` |
| `TASK-012` | task_policy | positive | `RuntimeNeedHandle` | accepted AlwaysStart TaskHandle + exact spec | AcceptedLaunch handle may be awaited | `cargo test -p arcweft-core runtime_need_handle` |
| `TASK-013` | task_policy | negative | `RuntimeNeedHandle` | accepted TaskHandle with different spec generation/producer | CorrelationMismatch | `cargo test -p arcweft-core runtime_need_handle` |
| `TASK-014` | task_policy | property | `TaskHost` | debug label changes on second Join submission | identity unchanged; stored semantic task not replaced | `cargo test -p arcweft-runtime-scheduler ensure_task` |
| `TASK-015` | task_policy | exact_limit | `RuntimeTaskJournal` | exact max task groups/tasks/Need cells | last admissible transaction succeeds | `cargo test -p arcweft-runtime-driver journal_limits` |
| `TASK-016` | task_policy | one_over | `RuntimeTaskJournal` | one task group over limit | typed JournalLimit; no ordinal/task leak | `cargo test -p arcweft-runtime-driver journal_limits` |
| `TASK-017` | task_policy | negative | `TaskHost` | caller attempts to supply NeedId/TaskKey/TaskId fields | compile-fail because fields/API do not exist | `cargo test -p arcweft-core --test api_absence` |
| `TASK-018` | task_policy | differential | `scheduler` | single-thread and multithread adapters process same TaskSpecs/events | identical normalized correlations/state/metrics semantic subset | `cargo test -p arcweft-runtime-scheduler scheduler_differential` |
| `TASK-019` | task_policy | rollback | `observer registration` | same observer key attempts different correlation | conflict; original observer unchanged | `cargo test -p arcweft-runtime-driver need_observers` |
| `TASK-020` | task_policy | positive | `observer registration` | duplicate same observer/correlation | idempotent no new launch/publication | `cargo test -p arcweft-runtime-driver need_observers` |
| `DIG-001` | runtime_value_digest | positive | `RuntimeValueDigest` | RuntimeValue::Tuple([]) | stable canonical non-ZERO digest | `cargo test -p arcweft-core runtime_value_digest` |
| `DIG-002` | runtime_value_digest | negative | `producer arguments` | RuntimeValueDigest::ZERO supplied as empty arguments | producer construction rejects mismatch with canonical Tuple([]) digest | `cargo test -p arcweft-core task_identity` |
| `DIG-003` | runtime_value_digest | differential | `canonical sinks` | all admitted RuntimeValue variants through byte sink and BLAKE3 sink | hash(bytes)==direct digest and identical errors/budgets | `cargo test -p arcweft-core runtime_value_digest_differential` |
| `DIG-004` | runtime_value_digest | property | `canonical sinks` | random bounded nested RuntimeValue corpus | direct digest equals canonical-bytes digest | `cargo test -p arcweft-core runtime_value_digest_proptest` |
| `DIG-005` | runtime_value_digest | positive | `RuntimeValue::NeedHandle` | valid Join handle | canonical bytes are tag 20 + exact NeedId | `cargo test -p arcweft-core runtime_value_digest` |
| `DIG-006` | runtime_value_digest | property | `RuntimeValue::NeedHandle` | same NeedId with differing nonidentity debug labels in otherwise valid equivalent spec | same canonical value digest | `cargo test -p arcweft-core runtime_value_digest` |
| `DIG-007` | runtime_value_digest | tamper | `NeedHandle snapshot` | NeedId changed but spec/correlation unchanged | restore rederivation rejects | `cargo test -p arcweft-runtime-driver task_restore` |
| `DIG-008` | runtime_value_digest | tamper | `NeedHandle snapshot` | origin changed ReusableJoin->AcceptedLaunch or reverse without invariant | restore rejects | `cargo test -p arcweft-runtime-driver task_restore` |
| `DIG-009` | runtime_value_digest | exact_limit | `canonical value encoder` | value exactly max nodes/depth/encoded bytes | byte and hash sinks both succeed | `cargo test -p arcweft-core runtime_value_digest_limits` |
| `DIG-010` | runtime_value_digest | one_over | `canonical value encoder` | one node/depth/byte over each limit | both sinks return same first error; no partial digest | `cargo test -p arcweft-core runtime_value_digest_limits` |
| `DIG-011` | runtime_value_digest | negative | `canonical value encoder` | map iteration order differs for record source | canonical field identity sort yields same digest | `cargo test -p arcweft-core runtime_value_digest` |
| `DIG-012` | runtime_value_digest | negative | `identity construction` | generic Serde output used as arguments digest | test oracle differs/rejected; sole canonical owner used | `cargo test -p arcweft-core runtime_value_digest` |
| `AWAIT-001` | await | positive | `AwaitTarget` | direct Await valid RuntimeNeedHandle | uses exact correlation/NeedId; no rederive | `cargo test -p arcweft-core await_target` |
| `AWAIT-002` | await | negative | `AwaitTarget` | RuntimeValue::String containing hex/nonempty ID | type mismatch; cannot satisfy NeedHandle | `cargo test -p arcweft-core awbc_await` |
| `AWAIT-003` | await | positive | `reusable Join handle` | NotStarted observation | ensure_task once then subscribe | `cargo test -p arcweft-runtime-driver await_runtime` |
| `AWAIT-004` | await | positive | `accepted AlwaysStart handle` | direct Await after accepted launch | subscribes without relaunch | `cargo test -p arcweft-runtime-driver await_runtime` |
| `AWAIT-005` | await_many | property | `AwaitMany base` | same captured/source tuple | same base producer key | `cargo test -p arcweft-core await_many_identity` |
| `AWAIT-006` | await_many | property | `AwaitMany base` | reorder source items | base arguments digest/key change | `cargo test -p arcweft-core await_many_identity` |
| `AWAIT-007` | await_many | positive | `AwaitMany child` | duplicate item values at indexes 0 and 1 | different argument digest/instance key | `cargo test -p arcweft-core await_many_identity` |
| `AWAIT-008` | await_many | property | `AwaitMany child` | same item/index/captures constructed independently | same key | `cargo test -p arcweft-core await_many_identity` |
| `AWAIT-009` | await_many | boundary | `AwaitMany child` | source index u32::MAX | exact index encodes and is accepted if source model admits length | `cargo test -p arcweft-core await_many_identity` |
| `AWAIT-010` | await_many | negative | `AwaitMany target` | source length u32::MAX + 1 | reject before digest/launch/ordinal | `cargo test -p arcweft-runtime-driver await_many` |
| `AWAIT-011` | await_many | differential | `AwaitMany runtime` | children complete reverse/random order | final result remains source order | `cargo test -p arcweft-runtime-driver await_many_differential` |
| `AWAIT-012` | await_many | rollback | `AwaitMany start` | one child adapter prepare fails in staged batch | no partial batch/ordinal/fiber mutation | `cargo test -p arcweft-runtime-driver await_many_rollback` |
| `AWAIT-013` | await_many | positive | `AwaitMany snapshot` | mixed pending/ready children saved/restored | exact handles/cursors/indexes restored | `cargo test -p arcweft-runtime-driver await_many_restore` |
| `AWAIT-014` | await_many | tamper | `AwaitMany snapshot` | child stored under wrong source index | restore rejects cross-reference | `cargo test -p arcweft-runtime-driver await_many_restore` |
| `AWAIT-015` | timeout | positive | `NeedTimeoutTarget` | same source NeedId/site/contract/limit | same timeout producer instance/output NeedId | `cargo test -p arcweft-core need_timeout_identity` |
| `AWAIT-016` | timeout | property | `NeedTimeoutTarget` | change source NeedId only | timeout producer/output NeedId changes; source unchanged | `cargo test -p arcweft-core need_timeout_identity` |
| `AWAIT-017` | timeout | property | `NeedTimeoutTarget` | change limit value only | timeout producer/output changes | `cargo test -p arcweft-core need_timeout_identity` |
| `AWAIT-018` | timeout | integration | `timeout race` | source Ready and limit expires same step | source terminal wins per retained ordering | `cargo test -p arcweft-runtime-driver need_timeout` |
| `AWAIT-019` | timeout | integration | `timeout cancellation` | scope cancellation/source/expiry same step | cancellation precedence; source not mutated by timeout | `cargo test -p arcweft-runtime-driver need_timeout` |
| `AWAIT-020` | timeout | tamper | `timeout restore` | output handle claims different source relationship | restore rejects both-row publication | `cargo test -p arcweft-runtime-driver need_timeout_restore` |
| `EVT-001` | event | positive | `TaskEvent` | first Progress cursor (epoch,0) | Pending and cursor installed | `cargo test -p arcweft-runtime-driver task_events` |
| `EVT-002` | event | positive | `TaskEvent` | duplicate exact cursor/event | idempotent no-op | `cargo test -p arcweft-runtime-driver task_events` |
| `EVT-003` | event | negative | `TaskEvent` | same cursor different Progress/Ready value | conflict rollback | `cargo test -p arcweft-runtime-driver task_events` |
| `EVT-004` | event | positive | `TaskEvent` | lower cursor | stale no-op with bounded audit | `cargo test -p arcweft-runtime-driver task_events` |
| `EVT-005` | event | negative | `TaskEvent` | sequence gap | CursorGap rollback | `cargo test -p arcweft-runtime-driver task_events` |
| `EVT-006` | event | negative | `TaskEvent` | logical epoch regression | EpochRegression rollback | `cargo test -p arcweft-runtime-driver task_events` |
| `EVT-007` | event | positive | `TaskEvent` | same-step Progress seq0 then Ready seq1 | both accepted; final Ready | `cargo test -p arcweft-runtime-driver task_events` |
| `EVT-008` | event | negative | `TaskEvent` | Progress after Ready terminal | PostTerminalPublication | `cargo test -p arcweft-runtime-driver task_events` |
| `EVT-009` | event | tamper | `TaskEvent` | generation changed | correlation error before cursor | `cargo test -p arcweft-runtime-driver task_event_tamper` |
| `EVT-010` | event | tamper | `TaskEvent` | producer key/contract changed | correlation/contract error | `cargo test -p arcweft-runtime-driver task_event_tamper` |
| `EVT-011` | event | tamper | `TaskEvent` | NeedId changed | journal correlation mismatch | `cargo test -p arcweft-runtime-driver task_event_tamper` |
| `EVT-012` | event | tamper | `TaskEvent` | TaskKey changed | journal correlation mismatch | `cargo test -p arcweft-runtime-driver task_event_tamper` |
| `EVT-013` | event | tamper | `TaskEvent` | TaskId changed | task lookup/correlation mismatch | `cargo test -p arcweft-runtime-driver task_event_tamper` |
| `EVT-014` | event | tamper | `TaskEvent` | launch ordinal changed | correlation mismatch | `cargo test -p arcweft-runtime-driver task_event_tamper` |
| `EVT-015` | event | positive | `RuntimeNeedOutcome` | Ready typed Result::Err payload | Ready(Value(Result::Err)); not infrastructure failure | `cargo test -p arcweft-runtime-driver task_events` |
| `EVT-016` | event | positive | `RuntimeNeedOutcome` | adapter worker failure | Ready(InfrastructureFailure) | `cargo test -p arcweft-runtime-driver task_events` |
| `EVT-017` | event | positive | `Need cancellation` | Cancelled event | Need::Cancelled, no payload | `cargo test -p arcweft-runtime-driver task_events` |
| `EVT-018` | event | differential | `live/replay` | same ordered event sequence through live and replay | identical journal/observer state | `cargo test -p arcweft-runtime-driver task_replay_differential` |
| `EVT-019` | event | tamper | `replay envelope` | event bytes changed without digest | digest failure before state | `cargo test -p arcweft-runtime-driver task_replay` |
| `EVT-020` | event | rollback | `event fanout` | observer limit exceeded during event apply | task/Need/observers all unchanged | `cargo test -p arcweft-runtime-driver task_event_rollback` |
| `MATCH-001` | match | positive | `CheckedMatch` | same semantics built in two HIR allocation orders/IDs | same CheckedMatchSemanticDigest | `cargo test -p arcweft-lang-sema checked_match_digest` |
| `MATCH-002` | match | property | `CheckedMatch` | change only SourceSpan/debug spelling | same digest | `cargo test -p arcweft-lang-sema checked_match_digest` |
| `MATCH-003` | match | property | `CheckedMatch` | change pattern/body/binding type | digest changes | `cargo test -p arcweft-lang-sema checked_match_digest` |
| `MATCH-004` | match | positive | `guard classifier` | exact checked Boolean literal true | ConstantTrue and contributes | `cargo test -p arcweft-lang-sema match_coverage` |
| `MATCH-005` | match | positive | `guard classifier` | exact checked Boolean literal false | ConstantFalse; FalseGuard | `cargo test -p arcweft-lang-sema match_coverage` |
| `MATCH-006` | match | positive | `guard classifier` | local initialized true | Dynamic; no constant fold | `cargo test -p arcweft-lang-sema match_coverage` |
| `MATCH-007` | match | positive | `guard classifier` | !false, 1==1, const call | all Dynamic | `cargo test -p arcweft-lang-sema match_coverage` |
| `MATCH-008` | match | precedence | `coverage` | false guard whose pattern already covered | FalseGuard reason retained | `cargo test -p arcweft-lang-sema match_coverage` |
| `MATCH-009` | match | positive | `coverage` | dynamic guarded wildcard + later wildcard | later wildcard reachable and Match exhaustive | `cargo test -p arcweft-lang-sema match_coverage` |
| `MATCH-010` | match | negative | `coverage` | only dynamic guarded wildcard over Bool | non-exhaustive | `cargo test -p arcweft-lang-sema match_coverage` |
| `MATCH-011` | match | positive | `coverage` | Bool false/true arms | exhaustive | `cargo test -p arcweft-lang-sema match_coverage` |
| `MATCH-012` | match | negative | `coverage` | Bool only true arm | witness false | `cargo test -p arcweft-lang-sema match_coverage` |
| `MATCH-013` | match | positive | `coverage` | nested Result<Option<T>,E> patterns | accepted constructor order/exhaustiveness | `cargo test -p arcweft-lang-sema match_coverage` |
| `MATCH-014` | match | positive | `coverage` | Vec exact/rest symbolic partitions | correct usefulness without length enumeration | `cargo test -p arcweft-lang-sema match_coverage` |
| `MATCH-015` | match | negative | `coverage` | generic/inferred array length | hard error before matrix | `cargo test -p arcweft-lang-sema match_coverage` |
| `MATCH-016` | match | positive | `coverage` | open opaque domain + wildcard | exhaustive | `cargo test -p arcweft-lang-sema match_coverage` |
| `MATCH-017` | match | negative | `coverage` | open opaque constructors without wildcard | non-exhaustive/open residual | `cargo test -p arcweft-lang-sema match_coverage` |
| `MATCH-018` | match | positive | `generic/View separation` | ordinary Match moves/destructures affine handle | generic CheckedMatch accepted when ordinary type rules allow | `cargo test -p arcweft-lang-sema checked_match` |
| `MATCH-019` | match | negative | `CheckedMatch API` | caller attempts to pass coverage/exhaustive/unreachable | compile-fail: constructor has no fields/arguments | `cargo test -p arcweft-lang-sema --test api_absence` |
| `MATCH-020` | match | rollback | `CheckedMatch` | non-exhaustive after unreachable candidates | no Match/digest/warnings/View row published | `cargo test -p arcweft-lang-sema checked_match_atomicity` |
| `VIEW-001` | view | negative | `CheckedViewMatchAdmission` | retained affine output from otherwise legal Match | View admission rejects; generic Match remains | `cargo test -p arcweft-compiler checked_view_match` |
| `VIEW-002` | view | negative | `CheckedViewMatchAdmission` | retained Stream/borrow/frame-local | typed rejection at View boundary | `cargo test -p arcweft-compiler checked_view_match` |
| `VIEW-003` | view | negative | `CheckedViewMatchAdmission` | retained ViewValue | MissingViewPersistenceEvidence | `cargo test -p arcweft-compiler checked_view_match` |
| `VIEW-004` | view | positive | `ViewMatchSiteId` | same program/declaration/child-role path across HIR reallocation | same site | `cargo test -p arcweft-lang-sema view_match_site` |
| `VIEW-005` | view | property | `ViewMatchSiteId` | change child-role path or declaration | site changes | `cargo test -p arcweft-lang-sema view_match_site` |
| `VIEW-006` | view | property | `ViewMatchSiteId` | change SourceSpan or accepted revision only | site unchanged | `cargo test -p arcweft-lang-sema view_match_site` |
| `VIEW-007` | view | positive | `TaskPlanSemanticDigest` | same View program/site/admission, different accepted revision | same plan/producer instance/NeedId | `cargo test -p arcweft-compiler view_task_identity` |
| `VIEW-008` | view | positive | `replacement` | valid explicit mapping, revision only changed | live state retained; NeedId stable; TaskKey/TaskId rederived for generation | `cargo test -p arcweft-runtime-driver view_replacement` |
| `VIEW-009` | view | negative | `replacement` | accepted revision bytes invalid or mismatch registry/bundle cross-section | strict bundle/replacement rejection or prescribed cancellation; no identity rehash | `cargo test -p arcweft-runtime-driver view_replacement` |
| `VIEW-010` | view | negative | `replacement` | CheckedMatchSemanticDigest differs | affected state cancelled transactionally | `cargo test -p arcweft-runtime-driver view_replacement` |
| `VIEW-011` | view | negative | `replacement` | CheckedViewMatchAdmissionDigest differs | cancel | `cargo test -p arcweft-runtime-driver view_replacement` |
| `VIEW-012` | view | negative | `replacement` | CheckedNeedProducerAdmissionDigest differs | cancel | `cargo test -p arcweft-runtime-driver view_replacement` |
| `VIEW-013` | view | negative | `replacement` | producer family/contract differs | cancel | `cargo test -p arcweft-runtime-driver view_replacement` |
| `VIEW-014` | view | negative | `replacement` | payload or plan differs | cancel | `cargo test -p arcweft-runtime-driver view_replacement` |
| `VIEW-015` | view | negative | `replacement` | ownership evidence differs | cancel | `cargo test -p arcweft-runtime-driver view_replacement` |
| `VIEW-016` | view | negative | `replacement` | resource dependency or arguments digest differs | cancel | `cargo test -p arcweft-runtime-driver view_replacement` |
| `VIEW-017` | view | negative | `replacement` | explicit mapping missing | cancel; no implicit site search | `cargo test -p arcweft-runtime-driver view_replacement` |
| `VIEW-018` | view | rollback | `replacement` | adapter rebind prepare fails | no partial new correlation; prescribed cancellation transaction | `cargo test -p arcweft-runtime-driver view_replacement_rollback` |
| `VIEW-019` | view | differential | `View evaluation` | product/AWBC selector over NotStarted/Pending/Ready/Cancelled | same arm/bindings; ordinary Match owner | `cargo test -p arcweft-runtime-driver view_match_differential` |
| `VIEW-020` | view | positive | `View observers` | two mounts observe same Join Need | one publication; independent mount state/invalidation | `cargo test -p arcweft-runtime-driver view_need_observers` |
| `OWN-001` | ownership | negative | `AcceptedNominalInventoryInput` | omit value_class | constructor unavailable/strict decode rejects | `cargo test -p arcweft-lang-sema accepted_nominal` |
| `OWN-002` | ownership | negative | `AcceptedNominalInventoryInput` | omit persistence | constructor unavailable/strict decode rejects | `cargo test -p arcweft-lang-sema accepted_nominal` |
| `OWN-003` | ownership | tamper | `AcceptedNominalCatalogDigest` | change value_class without digest | catalog mismatch | `cargo test -p arcweft-lang-sema accepted_nominal` |
| `OWN-004` | ownership | tamper | `AcceptedNominalCatalogDigest` | change persistence without digest | catalog mismatch | `cargo test -p arcweft-lang-sema accepted_nominal` |
| `OWN-005` | ownership | negative | `registrar` | producer/type name suggests default evidence | no inference; missing evidence rejects | `cargo test -p arcweft-lang-sema accepted_nominal` |
| `OWN-006` | ownership | positive | `opaque Plain` | constant-admissible persistence | SnapshotClone with exact evidence | `cargo test -p arcweft-lang-sema checked_ownership` |
| `OWN-007` | ownership | positive | `opaque Plain` | snapshot-only persistence | SnapshotClone with exact evidence | `cargo test -p arcweft-lang-sema checked_ownership` |
| `OWN-008` | ownership | negative | `opaque AffineHandle` | either persistence | AffineValue | `cargo test -p arcweft-lang-sema checked_ownership` |
| `OWN-009` | ownership | positive | `AgentResource` | current core Agent DTO | SnapshotClone without ResourceTypeRegistry | `cargo test -p arcweft-lang-sema checked_ownership` |
| `OWN-010` | ownership | positive | `AgentResourceBody` | current core Agent DTO | SnapshotClone without ResourceTypeRegistry | `cargo test -p arcweft-lang-sema checked_ownership` |
| `OWN-011` | ownership | positive | `Ref` | EntityRef(String) carrier | SnapshotClone | `cargo test -p arcweft-lang-sema checked_ownership` |
| `OWN-012` | ownership | positive | `Need<T>` | valid handle and producer argument certificate | SnapshotClone independent of T retained binding | `cargo test -p arcweft-lang-sema checked_ownership` |
| `OWN-013` | ownership | positive | `Shared<T>` | child SnapshotClone | SnapshotClone | `cargo test -p arcweft-lang-sema checked_ownership` |
| `OWN-014` | ownership | negative | `Shared<T>` | child affine/rejected | first child error | `cargo test -p arcweft-lang-sema checked_ownership` |
| `OWN-015` | ownership | negative | `Function type` | no value certificate | FunctionValueRequiresCertificate | `cargo test -p arcweft-lang-sema checked_ownership` |
| `OWN-016` | ownership | positive | `Function value` | capture-free accepted stable callable certificate | SnapshotClone | `cargo test -p arcweft-lang-sema checked_value_ownership` |
| `OWN-017` | ownership | negative | `Function value` | closure/nonempty capture or missing callable identity | reject | `cargo test -p arcweft-lang-sema checked_value_ownership` |
| `OWN-018` | ownership | negative | `project nominal` | recursive active cycle | RecursiveRetentionCycle with stable first path | `cargo test -p arcweft-lang-sema checked_ownership` |
| `OWN-019` | ownership | property | `OwnershipEvidenceDigest` | add unrelated accepted nominal catalog row | consulted evidence digest unchanged | `cargo test -p arcweft-lang-sema ownership_evidence_digest` |
| `OWN-020` | ownership | property | `OwnershipEvidenceDigest` | change one consulted opaque evidence row | digest changes | `cargo test -p arcweft-lang-sema ownership_evidence_digest` |
| `OWN-021` | ownership | negative | `generic Match` | ownership classifier set to fail for affine scrutinee | generic Match still constructs; classifier not called | `cargo test -p arcweft-lang-sema checked_match` |
| `OWN-022` | ownership | rollback | `producer admission` | later argument fails after earlier accepted | no partial certificate/evidence/digest | `cargo test -p arcweft-lang-sema checked_need_producer_admission` |
| `LIM-01A` | limits | exact_limit | `max_arms` | coverage arms exactly at configured limit | success when all other inputs valid | `cargo test -p arcweft-lang-sema match_coverage` |
| `LIM-01B` | limits | one_over | `max_arms` | coverage arms one over configured limit | typed first limit error and complete rollback | `cargo test -p arcweft-lang-sema match_coverage` |
| `LIM-02A` | limits | exact_limit | `max_matrix_rows` | coverage matrix rows exactly at configured limit | success when all other inputs valid | `cargo test -p arcweft-lang-sema match_coverage` |
| `LIM-02B` | limits | one_over | `max_matrix_rows` | coverage matrix rows one over configured limit | typed first limit error and complete rollback | `cargo test -p arcweft-lang-sema match_coverage` |
| `LIM-03A` | limits | exact_limit | `max_specializations` | coverage specializations exactly at configured limit | success when all other inputs valid | `cargo test -p arcweft-lang-sema match_coverage` |
| `LIM-03B` | limits | one_over | `max_specializations` | coverage specializations one over configured limit | typed first limit error and complete rollback | `cargo test -p arcweft-lang-sema match_coverage` |
| `LIM-04A` | limits | exact_limit | `max_type_nodes` | ownership type nodes exactly at configured limit | success when all other inputs valid | `cargo test -p arcweft-lang-sema checked_ownership` |
| `LIM-04B` | limits | one_over | `max_type_nodes` | ownership type nodes one over configured limit | typed first limit error and complete rollback | `cargo test -p arcweft-lang-sema checked_ownership` |
| `LIM-05A` | limits | exact_limit | `max_evidence_rows` | ownership evidence rows exactly at configured limit | success when all other inputs valid | `cargo test -p arcweft-lang-sema checked_ownership` |
| `LIM-05B` | limits | one_over | `max_evidence_rows` | ownership evidence rows one over configured limit | typed first limit error and complete rollback | `cargo test -p arcweft-lang-sema checked_ownership` |
| `LIM-06A` | limits | exact_limit | `max_observers_per_need` | observers per Need exactly at configured limit | success when all other inputs valid | `cargo test -p arcweft-runtime-driver need_observers` |
| `LIM-06B` | limits | one_over | `max_observers_per_need` | observers per Need one over configured limit | typed first limit error and complete rollback | `cargo test -p arcweft-runtime-driver need_observers` |
| `LIM-07A` | limits | exact_limit | `max_queued_invalidations` | queued invalidations exactly at configured limit | success when all other inputs valid | `cargo test -p arcweft-runtime-driver need_observers` |
| `LIM-07B` | limits | one_over | `max_queued_invalidations` | queued invalidations one over configured limit | typed first limit error and complete rollback | `cargo test -p arcweft-runtime-driver need_observers` |
| `LIM-08A` | limits | exact_limit | `max_tasks_per_generation` | journal tasks exactly at configured limit | success when all other inputs valid | `cargo test -p arcweft-runtime-driver journal_limits` |
| `LIM-08B` | limits | one_over | `max_tasks_per_generation` | journal tasks one over configured limit | typed first limit error and complete rollback | `cargo test -p arcweft-runtime-driver journal_limits` |
| `LIM-09A` | limits | exact_limit | `max_need_cells_per_generation` | journal Need cells exactly at configured limit | success when all other inputs valid | `cargo test -p arcweft-runtime-driver journal_limits` |
| `LIM-09B` | limits | one_over | `max_need_cells_per_generation` | journal Need cells one over configured limit | typed first limit error and complete rollback | `cargo test -p arcweft-runtime-driver journal_limits` |
| `LIM-10A` | limits | exact_limit | `max_restore_tasks` | restore tasks exactly at configured limit | success when all other inputs valid | `cargo test -p arcweft-runtime-driver task_restore_limits` |
| `LIM-10B` | limits | one_over | `max_restore_tasks` | restore tasks one over configured limit | typed first limit error and complete rollback | `cargo test -p arcweft-runtime-driver task_restore_limits` |
| `LIM-11A` | limits | exact_limit | `max_restore_bytes` | restore bytes exactly at configured limit | success when all other inputs valid | `cargo test -p arcweft-runtime-driver task_restore_limits` |
| `LIM-11B` | limits | one_over | `max_restore_bytes` | restore bytes one over configured limit | typed first limit error and complete rollback | `cargo test -p arcweft-runtime-driver task_restore_limits` |
| `LIM-12A` | limits | exact_limit | `max_nodes` | runtime value nodes exactly at configured limit | success when all other inputs valid | `cargo test -p arcweft-core runtime_value_digest_limits` |
| `LIM-12B` | limits | one_over | `max_nodes` | runtime value nodes one over configured limit | typed first limit error and complete rollback | `cargo test -p arcweft-core runtime_value_digest_limits` |
| `LIM-13A` | limits | exact_limit | `max_encoded_bytes` | runtime value encoded bytes exactly at configured limit | success when all other inputs valid | `cargo test -p arcweft-core runtime_value_digest_limits` |
| `LIM-13B` | limits | one_over | `max_encoded_bytes` | runtime value encoded bytes one over configured limit | typed first limit error and complete rollback | `cargo test -p arcweft-core runtime_value_digest_limits` |
| `STR-001` | structural | structural | `workspace API/schema` | ViewProgramSemanticDigest type/name | no production/API/schema/generated occurrence | `cargo test --workspace structural_absence && cargo check --workspace --all-targets` |
| `STR-002` | structural | structural | `workspace API/schema` | u32 View revision | only AcceptedViewProgramRevision([u8;32]) remains | `cargo test --workspace structural_absence && cargo check --workspace --all-targets` |
| `STR-003` | structural | structural | `workspace API/schema` | duplicate RuntimeValueDigest declaration | only arcweft_core::entry owner | `cargo test --workspace structural_absence && cargo check --workspace --all-targets` |
| `STR-004` | structural | structural | `workspace API/schema` | ResourceTypeRegistry in CheckedOwnershipContext | absent | `cargo test --workspace structural_absence && cargo check --workspace --all-targets` |
| `STR-005` | structural | structural | `workspace API/schema` | unkeyed Agent resource registry lookup | absent | `cargo test --workspace structural_absence && cargo check --workspace --all-targets` |
| `STR-006` | structural | structural | `workspace API/schema` | AwbcTaskPlan.plan_digest field | absent | `cargo test --workspace structural_absence && cargo check --workspace --all-targets` |
| `STR-007` | structural | structural | `workspace API/schema` | AwbcTaskPlan.need_id field | absent | `cargo test --workspace structural_absence && cargo check --workspace --all-targets` |
| `STR-008` | structural | structural | `workspace API/schema` | String NeedId/TaskKey/TaskId public carrier | absent | `cargo test --workspace structural_absence && cargo check --workspace --all-targets` |
| `STR-009` | structural | structural | `workspace API/schema` | NeedHandle-as-String verifier/VM route | absent | `cargo test --workspace structural_absence && cargo check --workspace --all-targets` |
| `STR-010` | structural | structural | `workspace API/schema` | await_target String parse | absent | `cargo test --workspace structural_absence && cargo check --workspace --all-targets` |
| `STR-011` | structural | structural | `workspace API/schema` | indexed String/suffix identity | absent | `cargo test --workspace structural_absence && cargo check --workspace --all-targets` |
| `STR-012` | structural | structural | `workspace API/schema` | caller-supplied TaskSpec IDs/ordinal | absent | `cargo test --workspace structural_absence && cargo check --workspace --all-targets` |
| `STR-013` | structural | structural | `workspace API/schema` | copied View coverage/Match arm authority | absent | `cargo test --workspace structural_absence && cargo check --workspace --all-targets` |
| `STR-014` | structural | structural | `workspace API/schema` | generic Match ownership/persistence call | absent | `cargo test --workspace structural_absence && cargo check --workspace --all-targets` |
| `STR-015` | structural | structural | `workspace API/schema` | opaque evidence default/omitting constructor | absent | `cargo test --workspace structural_absence && cargo check --workspace --all-targets` |
| `STR-016` | structural | structural | `workspace API/schema` | identity translation/compatibility reader | absent | `cargo test --workspace structural_absence && cargo check --workspace --all-targets` |
| `STR-017` | structural | structural | `workspace API/schema` | driver-local GenerationId | absent | `cargo test --workspace structural_absence && cargo check --workspace --all-targets` |
| `STR-018` | structural | structural | `workspace API/schema` | extension trait for Arcweft-owned enum behavior | absent | `cargo test --workspace structural_absence && cargo check --workspace --all-targets` |
| `STR-019` | structural | structural | `workspace API/schema` | numeric AWBC reallocation in this feature cut | maintained allocation files unchanged except required consumers | `cargo test --workspace structural_absence && cargo check --workspace --all-targets` |
| `STR-020` | structural | structural | `workspace API/schema` | old View Await types/variants/evaluator/AwaitView spelling | absent from API/schema/generated/runtime | `cargo test --workspace structural_absence && cargo check --workspace --all-targets` |
| `T2-001` | tier2 | tier2 | `workspace` | workspace check | must pass before implementation readiness is reported | `cargo check --workspace --all-targets` |
| `T2-002` | tier2 | tier2 | `workspace` | workspace tests | must pass before implementation readiness is reported | `cargo test --workspace --all-targets` |
| `T2-003` | tier2 | tier2 | `workspace` | Clippy | must pass before implementation readiness is reported | `cargo clippy --workspace --all-targets -- -D warnings` |
| `T2-004` | tier2 | tier2 | `workspace` | rustdoc | must pass before implementation readiness is reported | `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` |
| `T2-005` | tier2 | tier2 | `workspace` | format | must pass before implementation readiness is reported | `cargo fmt --all -- --check` |
| `T2-006` | tier2 | tier2 | `workspace` | native adapter parity | must pass before implementation readiness is reported | `cargo test -p arcweft-cli -p arcweft-native-adapter task_need_parity` |
| `T2-007` | tier2 | tier2 | `workspace` | Web adapter parity | must pass before implementation readiness is reported | `cargo test -p arcweft-web task_need_parity` |
| `T2-008` | tier2 | tier2 | `workspace` | headless parity | must pass before implementation readiness is reported | `cargo test -p arcweft-headless task_need_parity` |
| `T2-009` | tier2 | tier2 | `workspace` | Agent parity | must pass before implementation readiness is reported | `cargo test -p arcweft-agent-runner task_need_parity` |
| `T2-010` | tier2 | tier2 | `workspace` | bundle codecs | must pass before implementation readiness is reported | `cargo test -p arcweft-bundle task_need_view_codec` |
| `T2-011` | tier2 | tier2 | `workspace` | save/replay | must pass before implementation readiness is reported | `cargo test -p arcweft-runtime-driver task_save_replay` |
| `T2-012` | tier2 | tier2 | `workspace` | generated artifacts | must pass before implementation readiness is reported | `repository generation/check command for AWBC/bundle schemas` |
| `T2-013` | tier2 | tier2 | `workspace` | AOT/VM/product differential | must pass before implementation readiness is reported | `cargo test --workspace awbc_product_runtime_step_parity` |
| `T2-014` | tier2 | tier2 | `workspace` | replacement matrix | must pass before implementation readiness is reported | `cargo test -p arcweft-runtime-driver view_replacement_matrix` |
| `T2-015` | tier2 | tier2 | `workspace` | Miri/loom targeted transaction tests | must pass before implementation readiness is reported | `project-supported deterministic concurrency gate where configured` |

## Test implementation rules

1. Property tests use deterministic recorded seeds on failure.
2. Differential tests compare semantic final state, event/correlation order,
   and error category, not debug labels or host timing.
3. Tamper tests change exactly one field unless the row says otherwise.
4. Rollback tests snapshot every affected semantic map/counter before the
   operation and assert byte/structural equality after failure.
5. Exact-limit and one-over tests charge counters before allocation/descent.
6. Structural absence tests use compiler/API/schema/generated model proofs and
   strict legacy decode rejection; a simple grep is supplementary only.
7. Tier-2 rows may be split by CI job, but all must pass on the same final
   production revision before implementation readiness is reported.
8. No test fixture may mint fixed IDs from arbitrary bytes except explicit
   zero/tamper tests; normal fixtures call final inherent transcript owners.
