# Focused, property, differential, tamper, rollback, and restore test matrix

Total normative rows: **100**.

The rows are implementation acceptance tests. A row succeeds only when the production owner performs the assertion; package validation checks that the row exists and is traceable but does not pretend to execute future production code.

## Coverage by category

| Category | Rows |
|---|---:|
| `cut` | 3 |
| `differential` | 5 |
| `focused` | 36 |
| `inventory` | 4 |
| `layering` | 1 |
| `migration` | 1 |
| `negative` | 20 |
| `property` | 8 |
| `restore` | 3 |
| `rollback` | 8 |
| `structural absence` | 1 |
| `tamper` | 10 |

## Exact rows

| ID | Category | Owner | Setup / mutation | Required assertion |
|---|---|---|---|---|
| `H01` | `focused` | `RuntimeNeedHandle::try_reusable_join` | complete Host JoinSameKey TaskSpec; active generation | constructs ReusableJoin with exact boxed spec; no scheduler/adapter mutation |
| `H02` | `focused` | `RuntimeNeedHandle::try_reusable_join` | complete Runtime JoinSameKey TaskSpec | constructs ReusableJoin and derives ordinal-zero correlation |
| `H03` | `negative` | `RuntimeNeedHandle::try_reusable_join` | AlwaysStart TaskSpec | rejects ReusableRequiresJoinSameKey before mutation |
| `H04` | `negative` | `RuntimeNeedHandle::try_reusable_join` | producer or outcome differs from TaskSpec | rejects exact relationship mismatch |
| `H05` | `focused` | `RuntimeNeedHandle::try_from_accepted_launch` | sealed committed Join launch proof | constructs AcceptedLaunch without retaining TaskSpec |
| `H06` | `focused` | `RuntimeNeedHandle::try_from_accepted_launch` | sealed committed AlwaysStart proof with positive ordinal | constructs AcceptedLaunch and retains committed correlation |
| `H07` | `tamper` | `RuntimeNeedHandle restore` | ReusableJoin snapshot with missing/changed TaskSpec field | rejects before publishing RuntimeValue |
| `H08` | `tamper` | `RuntimeNeedHandle restore` | AcceptedLaunch snapshot whose correlation is absent from journal | rejects MissingAcceptedLaunch |
| `H09` | `property` | `RuntimeNeedHandle Eq/Hash/Ord` | same NeedId with different structural metadata and different NeedId with same metadata | semantic Eq/Hash/Ord depends only on NeedId |
| `H10` | `focused` | `await ReusableJoin` | stored spec joins existing row | scheduler returns exact correlation then observer is registered |
| `H11` | `tamper` | `await ReusableJoin` | scheduler returns a structurally different correlation | rejects CorrelationMismatch; no observer ID consumed |
| `H12` | `focused` | `await AcceptedLaunch` | active generation and committed row | registers directly without ensure/relaunch/rederivation |
| `H13` | `focused` | `AWBC MakeNeedHandle Host+Join` | verified MakeNeedHandle instruction | constructs ReusableJoin and adapter prepare count remains zero |
| `H14` | `focused` | `AWBC MakeNeedHandle AlwaysStart` | Host or Runtime AlwaysStart plan | routes through ensure and returns AcceptedLaunch only |
| `H15` | `differential` | `structured/AWBC Need handle` | same accepted plan/site/arguments | same NeedId, state choice, correlation and launch behavior |
| `A01` | `focused` | `RuntimeAwaitManyAggregateRequest::try_new` | captured tuple, source items, typed child template, nonzero limit | retains exact evidence and computes source-order aggregate base transcript |
| `A02` | `property` | `RuntimeAwaitManyAggregateRequest::child_spec` | arbitrary accepted captured/source arrays and every valid source index | argument is Tuple([Tuple(captured), UInt(U32(i)), source_items[i]]) and template owns final execution |
| `A03` | `negative` | `RuntimeAwaitManyAggregateRequest::child_spec` | index outside source_items or index not representable as u32 | rejects before digest/spec construction |
| `A04` | `tamper` | `AwaitMany restore` | persisted derived child digest/spec differs in one field | regeneration detects mismatch and rejects whole aggregate |
| `A05` | `differential` | `AwaitMany construction` | caller-supplied debug labels or source spelling vary | derived child semantic digest is unchanged |
| `A06` | `differential` | `AwaitMany construction` | captured value, source item, index, or accepted template changes | derived child semantic digest changes |
| `B01` | `focused` | `RuntimeTaskScheduler::ensure_task_batch` | all new Host child rows succeed | one plan stages all rows, prepares all, atomically applies, commits, then publishes child state |
| `B02` | `focused` | `RuntimeTaskScheduler::ensure_task_batch` | mix of existing Join, new Host, and new Runtime children | existing Join remains nonmutating; new rows share one transaction |
| `B03` | `rollback` | `RuntimeTaskScheduler::ensure_task_batch` | nth Host prepare refuses | prepared tokens 0..n-1 rollback in reverse order; all deltas discarded |
| `B04` | `rollback` | `RuntimeTaskScheduler::ensure_task_batch` | cross-reference validation fails after all prepares | all prepared tokens rollback in reverse order; no IDs/counters consumed |
| `B05` | `rollback` | `RuntimeTaskScheduler::ensure_task_batch` | work limit fails during staging | no scheduler state or adapter-visible command changes |
| `B06` | `property` | `RuntimeTaskScheduler::ensure_task_batch` | arbitrary failure point across derive/inspect/ordinal/observer/prepare/validate stages | failed batch consumes no task ordinal or observer ID and exposes no worker work |
| `B07` | `negative` | `aggregate implementation` | instrument every internal call | per-child public ensure_task commit count is zero |
| `B08` | `property` | `batch determinism` | same generation/journal/input with adapter reservations returning same route facts | results and staged IDs are byte-for-byte equal in source-index order |
| `O01` | `focused` | `RuntimeGenerationJournal observer allocator` | fresh generation | next_observer_id starts at NonZeroU64(1) |
| `O02` | `focused` | `observer registration` | successful single registration | issues current ID and advances next only with atomic commit |
| `O03` | `rollback` | `observer registration` | validation or adapter preparation fails | next_observer_id remains unchanged |
| `O04` | `rollback` | `batch observer registration` | any child fails | none of the staged observer IDs are consumed |
| `O05` | `restore` | `generation snapshot` | next id strictly greater than all stored/referenced IDs | restore succeeds and next registration is monotonic |
| `O06` | `tamper` | `generation snapshot` | next id equals or is below a persisted/reference ID | restore rejects ObserverAllocatorNotStrictlyGreater |
| `O07` | `negative` | `observer allocator` | next_observer_id == u64::MAX | ObserverIdOverflow before mutation; u64::MAX is never issued |
| `O08` | `property` | `observer removal` | remove any subset in any order | allocator never rewinds and no ID is reused |
| `C01` | `focused` | `TaskLaunchAdapter prepare/commit` | valid Host launch batch | prepare reserves unpublished slots only; commit makes commands visible and cannot fail |
| `C02` | `negative` | `TaskLaunchAdapter prepare` | instrument filesystem/network/audio/worker-start calls | all counts remain zero during prepare |
| `C03` | `rollback` | `TaskLaunchAdapter rollback` | prepared launch/restore/rebind/cancel token | drops reservation only and is infallible |
| `C04` | `focused` | `post-commit worker failure` | worker reports I/O failure after committed launch | publishes TaskEvent::InfrastructureFailure, never domain Result/Option payload |
| `C05` | `focused` | `RuntimeTaskScheduler::cancel_tasks` | active cancellable Host rows | stages adapter/launch/Need/observer/runtime/scope/events then atomically applies and commits |
| `C06` | `rollback` | `RuntimeTaskScheduler::cancel_tasks` | adapter prepare_cancel refuses one route group | all prepared cancel tokens rollback; every scheduler owner remains unchanged |
| `C07` | `focused` | `Host cancellation idempotence` | same committed correlation cancelled twice | second call returns AlreadyRequested and does not call adapter |
| `C08` | `negative` | `HostTaskCancelBatch` | duplicate correlation in one input batch | rejects before prepare and changes nothing |
| `C09` | `focused` | `Host cancellation terminal/absent` | terminal or unknown correlation | returns AlreadyTerminal or NotFound without adapter call |
| `C10` | `property` | `HostCancelCommandId` | same canonical correlation and altered correlation | same correlation gives same ID; any semantic correlation change gives different ID |
| `C11` | `layering` | `Cargo dependency graph` | scan scheduler direct/transitive product dependencies | scheduler depends on core and never arcweft-host-adapter/host/native/web/headless |
| `C12` | `migration` | `legacy adapter routes` | source inventory after Cut 5 | HostAdapter::submit and cancel(&TaskId)->bool symbols/call sites are absent |
| `S01` | `property` | `AwbcRuntimeValueSnapshot round trip` | generated values for every accepted final RuntimeValue variant and nested combinations | restore(snapshot(value)) is structurally equal and revalidates identities |
| `S02` | `focused` | `RuntimeIterator snapshot` | Values, exact Range iterator, Witness state+next | all fields survive round trip |
| `S03` | `focused` | `RuntimeSeq snapshot` | Values, every DenseSeq case, TupleColumns, RecordColumns | variant and all shape/field/length data survive round trip |
| `S04` | `tamper` | `RuntimeSeq RecordColumns` | wrong field ordinal, duplicate name, or column length mismatch | restore rejects before constructing live sequence |
| `S05` | `focused` | `RuntimeOpaqueValue snapshot` | producer, semantic identity, class, persistence, recursive payload | owner is reconstructed and payload acceptance rechecked |
| `S06` | `focused` | `RuntimeReductionValue snapshot` | owner/state and ordered commands | constructor/target/payload order is preserved exactly |
| `S07` | `focused` | `RuntimeAgentValue snapshot` | all eight current variants and recursive predicate variants | exact operands and nested predicate order survive |
| `S08` | `negative` | `RuntimeFunctionBody::Structured snapshot` | live structured closure | returns UnrebindableStructuredFunction before bytes are emitted |
| `S09` | `restore` | `RuntimeFunctionBody::Awbc snapshot` | exact program generation/digest/function authority | restores only when authority join succeeds |
| `S10` | `tamper` | `AWBC function snapshot` | changed generation/program digest/function | restore rejects authority mismatch |
| `S11` | `focused` | `RuntimeNeedHandle snapshot` | both ReusableJoin and AcceptedLaunch states | closed state projects isomorphically and revalidates state-specific invariants |
| `S12` | `inventory` | `live/snapshot validator` | add/remove/change a live enum variant or field shape without matching snapshot row | package/source inventory validator rejects |
| `S13` | `tamper` | `snapshot codec` | unknown enum tag, nonminimal length, or trailing bytes | version-1 decoder rejects; no compatibility reader runs |
| `S14` | `negative` | `snapshot source scan` | generic {kind,items}, {source,cursor}, opaque bytes, or callable summary form | forbidden lossy form check fails |
| `M01` | `inventory` | `Match expression tags` | all current HirExprKind variants | exactly 38 rows; unique stable tags; no wildcard success |
| `M02` | `inventory` | `Match pattern tags` | all current HirPatternKind variants | exactly 13 rows; unique stable tags |
| `M03` | `property` | `CheckedExpressionChildRole path` | arbitrary accepted HIR expression tree within limits | role sequence matches semantic owner direct-child order exactly |
| `M04` | `negative` | `role-path construction` | depth/node/child/byte limit exceeded | first limit breach is returned; no partial transcript |
| `M05` | `differential` | `expression transcript` | source spelling and arena IDs differ, accepted semantic facts/catalog equal | digest is equal |
| `M06` | `differential` | `callable transcript` | accepted CheckedCallableDigest changes with other facts constant | digest changes |
| `M07` | `negative` | `Call resolution` | missing checked callable catalog row | rejects MissingCheckedCallableJoin before transcript emission |
| `M08` | `negative` | `selected Method resolution` | HirName cannot join receiver+checked callable catalog | rejects MissingSelectedMethodJoin before transcript emission |
| `M09` | `focused` | `unit Call` | separate current call-target fact plus callable catalog row | emits callable identity/digest payload in defined order |
| `M10` | `focused` | `Choice traversal` | nested If/For/Match/Option/OptionFor/CompactArm and plan items | depth-first/source-order child roles exactly match table |
| `M11` | `focused` | `Dialogue traversal` | target, coordinates, interpolation, tag payload, line plan | ordered roles exactly match table |
| `M12` | `tamper` | `constructor table` | duplicate tag, missing role enum row, or payload order drift | validator rejects before accepting Match digest grammar |
| `P01` | `inventory` | `ownership classifier` | current TypeKind algebra | exactly 85 rows and every TypeKind appears once |
| `P02` | `focused` | `signed numeric ownership` | I8/I16/I32/I64/I128/ISize | exact RuntimeValue::Int width and snapshot Int |
| `P03` | `focused` | `unsigned numeric ownership` | U8/U16/U32/U64/U128/USize | exact RuntimeValue::UInt width and snapshot UInt |
| `P04` | `negative` | `ownership matrix` | any IntOrUInt carrier | validator rejects ambiguous carrier |
| `P05` | `focused` | `Result/Option ownership` | accepted checked projection | exact RuntimeValue::Variant owner/ordinal/payload and snapshot Variant |
| `P06` | `focused` | `Tuple/sequence ownership` | accepted tuple and Vec/Array/Slice/Seq projections | outer carriers are exactly Tuple and Seq |
| `P07` | `negative` | `Choice ownership` | current projection lacks one closed accepted carrier | rejects MissingRuntimeSnapshotOwner rather than selecting Tuple/Variant ad hoc |
| `P08` | `negative` | `Agent/dialogue/character ownership` | exact nominal/case/field catalog owner missing | rejects MissingRuntimeSnapshotOwner |
| `P09` | `focused` | `TextCluster/DisplayText` | exact accepted nominal semantic identity and UTF-8 projection | uses RuntimeValue::String and snapshot String after projection validation |
| `P10` | `focused` | `Predicate/Shared` | Predicate and Shared rows | Predicate has no TypeKind child edge; Shared rejects before child recursion |
| `P11` | `cut` | `Need ownership certificate` | Cut 2 before Cut 5 owner publication | certificate remains private; becomes public only in atomic Cut 5 |
| `P12` | `tamper` | `ownership restore` | nominal/case/field identity differs from catalog | restore rejects and never reconstructs from source spelling |
| `E01` | `focused` | `event normalization` | same logical epoch and varied TaskId/sequence | orders by logical_epoch, TaskId, then sequence |
| `E02` | `focused` | `retained generation event normalization` | events from retained generations | orders generation, logical_epoch, TaskId, sequence |
| `E03` | `negative` | `event order inventory` | sequence precedes TaskId | validator rejects E_EVENT_ORDER |
| `R01` | `negative` | `snapshot admission` | any prepared adapter transaction | snapshot blocks |
| `R02` | `negative` | `snapshot admission` | active MustBeQuiescent Host row | snapshot blocks |
| `R03` | `focused` | `snapshot admission` | active Restartable Host row with complete request/correlation/capability | snapshot succeeds and persists exact row |
| `R04` | `restore` | `restartable Host task` | valid persisted row and adapter capacity | prepare_restore, atomic state install, infallible commit_restore |
| `R05` | `rollback` | `restartable Host restore` | prepare_restore refusal or state validation failure | state remains unpublished; all tokens rollback |
| `R06` | `negative` | `snapshot policy` | implementation rejects all active/nonterminal Host rows | validator rejects blanket policy |
| `K01` | `cut` | `compile sequence` | public owner dependency graph | every public reference targets same/earlier cut |
| `K02` | `cut` | `Cut 5 publication` | handle/batch/allocator/adapter/snapshot/final ownership/event rows | all publish atomically and superseded routes delete in same cut |
| `K03` | `negative` | `compile sequence` | Cut 2 public certificate cites Cut 5 owner | validator rejects forward reference unless row is private until Cut 5 |
| `K04` | `structural absence` | `whole package` | scan prose/machine/Rust schema | no compatibility reader, identity alias, dual carrier, second digest grammar, source-string reconstruction, AdapterCommit error, or blanket Host rejection |

## Required execution layers

The focused/property rows must be implemented at the lowest owning crate, not as prose-only integration tests. Adapter timing tests use an instrumented reservation adapter that separately counts prepare allocation, worker-visible commit, rollback, and post-commit failure publication. Batch failure tests inject a fault at every stage and compare complete before/after scheduler, journal, observer, runtime-task, scope, pending-event, and adapter state.

Snapshot property generators are depth/byte bounded and cover every accepted live variant plus recursively nested values. Rejected variants are generated separately and must fail before any bytes are appended. Match differential generators randomize source spelling, source spans, module-local arena indices, and allocation order while pinning accepted semantic catalogs.
