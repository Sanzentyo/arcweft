# Final decision register

| ID | Decision point | Closed decision | Authority |
|---|---|---|---|
| `D01` | Need handle state | ReusableJoin stores complete `Box<TaskSpec>`; AcceptedLaunch stores none | mandatory 1 |
| `D02` | Reusable constructor | public, pure validation; derives active-generation Join ordinal zero | mandatory 1 |
| `D03` | Accepted constructor | crate-private sealed committed-journal proof; no rederivation | mandatory 1 |
| `D04` | MakeNeedHandle Host+Join | lazy reusable handle; no host launch | mandatory 1 |
| `D05` | AlwaysStart | only AcceptedLaunch; MakeNeedHandle eagerly ensures | mandatory 1 |
| `D06` | AwaitMany evidence | captured + source_items + one typed child template + nonzero limit | mandatory 2 |
| `D07` | Child argument | `Tuple([Tuple(captured), UInt(U32(i)), item])` | mandatory 2 |
| `D08` | Child digest | derived internally; caller digest/spec forbidden | mandatory 2 |
| `D09` | Aggregate transaction | one scheduler batch; no per-child commit | mandatory 3 |
| `D10` | Rollback | reverse prepared-token order; discard all after-images | mandatory 3 |
| `D11` | Observer allocator | per-generation `NonZeroU64`, starts 1, largest issue max-1 | mandatory 4 |
| `D12` | Observer restore | next strictly greater than every stored/referenced ID | mandatory 4 |
| `D13` | Cancellation | one adapter/launch/Need/observer/runtime/scope transaction | mandatory 5 |
| `D14` | Cancel idempotence | correlation-derived command ID; duplicate/repeat rules closed | mandatory 5 |
| `D15` | Adapter owner | trait/envelopes in core; scheduler depends only on core | mandatory 6 |
| `D16` | Prepare timing | reservation only; no worker/I/O; commit/rollback infallible | mandatory 6 |
| `D17` | Host operation identity | typed builtin or exact catalog digest+nonzero ID; no string identity | mandatory 7 |
| `D18` | Snapshot owner | evolve existing AwbcRuntimeValueSnapshot in place; one reader | mandatory 7 |
| `D19` | Structured function | strict projection rejection until executable authority can rebind plan | mandatory 7 |
| `D20` | Dense sequences | all 21 current constructors/cases preserved exactly | mandatory 7 |
| `D21` | Match HIR inventory | 38 current expression families and 13 pattern families | mandatory 8 |
| `D22` | Callable authority | checked callable id+digest catalog join; method resolves HirName through receiver catalog | mandatory 8 |
| `D23` | Integer ownership | signed Int, unsigned UInt, no IntOrUInt | mandatory 9 |
| `D24` | Unsupported nominal rows | reject MissingRuntimeSnapshotOwner rather than fabricate maps | mandatory 9 |
| `D25` | Event order | generation?, logical_epoch, task_id, sequence | mandatory 10 |
| `D26` | Host snapshots | prepared blocks; quiescent blocks; restartable persists/restores | mandatory 10 |
| `D27` | Cuts | five cuts; Need ownership private until atomic Cut 5 | compile-clean |
| `D28` | Status | READY_FOR_IMPLEMENTATION; OPEN_QUESTIONS=0 | package |

All decisions are mirrored by `machine/contract.json` or the specialized machine tables. No row is a placeholder.
