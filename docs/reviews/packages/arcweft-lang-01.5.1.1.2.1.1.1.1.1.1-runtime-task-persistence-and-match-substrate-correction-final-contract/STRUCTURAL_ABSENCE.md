# Structural absence contract

This file turns every prohibited compatibility or duplicate authority into an
observable absence gate. A package or implementation is invalid when any row is
present, even when the old route is unreachable in ordinary tests.

## Required absences

| Crossing | Must be absent | Positive replacement | Package validator gate | Production proof in cut 5 |
|---|---|---|---|---|
| unreachable adapter failure | `TaskEnsureError::AdapterCommit` and any rebind equivalent | fallible `prepare_*`, infallible `commit_* -> ()`, infallible `rollback_* -> ()` | `contract.adapter.forbidden_error_variants`, prose token scan in normative files, negative self-test `adapter_commit` | exhaustive error-enum compile test and rollback tests |
| unconditional host execution | `TaskSpec.request: HostTaskRequest`, `Option<HostTaskRequest>`, or a parallel host/runtime request pair | one `TaskSpec.execution: TaskExecution` | exact `task_spec.fields`, forbidden-field set, negative self-test `unconditional_host_request` | Rust struct field compile-fail and all-family route tests |
| runtime task through adapter | adapter API accepting `RuntimeTaskRequest` or scheduler dispatching Timeout/AwaitMany aggregate to host | runtime rows staged and stepped inside `RuntimeTaskScheduler` | producer truth-table and `adapter.runtime_rows_reach_adapter=false` | mock adapter asserts zero runtime-row calls |
| duplicate task owner | driver-owned journal, ordinal counter, Need map, observer map, or rollback protocol | scheduler-owned `RuntimeTaskJournal` and `RuntimeTaskState` | sole-owner declaration and owner/API map | type/API deletion plus integration ownership test |
| post-commit failure | any fallible operation after `commit_launch` or `commit_rebind` | complete validation/staging before commit | contract invariant and borrow-flow specification | failpoint coverage at every precommit edge |
| structural NeedHandle identity | derived `PartialEq`, `Hash`, or `Ord` over correlation/spec/debug fields | manual semantics from `NeedId` only | `need_handle.manual_eq_hash_ord=true` and identity-field set | semantic-equality differential tests |
| generation in value identity | generation/spec/correlation bytes in RuntimeValue tag 20 transcript | exactly `20 || NeedId` | canonical identity exact-field check | canonical-byte golden test |
| second value digest grammar | producer-only serializer, snapshot-only digest, constant-only digest used as identity | one sink-parametric exhaustive canonical visitor | sole-owner declaration and source deletion row | differential byte/hash sink tests |
| incidental constant policy | canonical identity rejecting `Plain + SnapshotOnly` | explicit constant-admission fence | paired-evidence state machine | same-value five-way paired test |
| affine value digest | diagnostic/snapshot producer computing `RuntimeValueDigest` for affine opaque handle | typed nonidentity diagnostic projection or rejection | canonical policy check | affine canonical-byte/digest negative tests |
| undefined persisted row | reference from a snapshot schema to an absent schema | all references resolve among 72 closed rows | graph closure check, negative self-test `undefined_snapshot` | codec round-trip per row |
| permissive persistence | generic Serde, unknown/duplicate field acceptance, trailing bytes, legacy reader, String fallback, zero sentinel | purpose-built strict v1 codec | per-schema strict flags and global contract | mutation corpus |
| prepared adapter token on disk | host reservation token in snapshot/replay/replacement bytes | quiescent snapshot or pre-prepare replacement mapping only | prepared-token invariant | snapshot barrier tests |
| compiler-local ID in bundle | `CheckedMatchRef`, `ExprId`, `HirSnapshotId`, `SourceSpan`, or certificate object in `AcceptedViewMatchBundleRowV1` | digest/projection-only 13-field row | exact bundle fields, negative self-test `compiler_local_bundle_id` | generated schema inspection |
| invented semantic generation | `AcceptedSemanticGeneration` or another parallel lookup owner | current `HirSnapshotId + ExprId` lease | Match substrate declaration | API absence/search plus generation tests |
| HIR allocation in transcript | arena index, raw `ExprId`/`PatternId`, source span/spelling/debug name/hash-map order | declaration-rooted child-role path and stable pattern coordinates | transcript forbidden-input set | allocation/span differential corpus |
| `Shared<T>` false admission | `SnapshotClone` without an actual shared live carrier and codec | `Reject(MissingRuntimeSnapshotOwner)` | exact ownership row, negative self-test `shared_without_carrier` | classifier test |
| Predicate child recursion | recursion into a nonexistent Predicate child | leaf row | exact ownership row, negative self-test `predicate_recursion` | classifier edge test |
| unsupported SnapshotClone | successful row missing runtime projection, live carrier, canonical identity, or snapshot codec | all four evidence cells nonempty | ownership matrix validation | generated classifier/matrix parity |
| private enum-variant claim | publishing `RuntimeValue::NeedHandle` in preparation cut or calling a public enum variant private | atomic cut 5 switch | cut-4 publish/forbidden checks, negative self-test `private_runtime_value_variant` | cut feature compilation |
| cut-3 reverse dependency | View admission depending on task/Need digest/schema types introduced in cut 4 | compiler-local View row depends only on cuts 1–2 plus existing View identities | exact cut-3 dependency check, negative self-test `cut3_depends_on_cut4` | per-cut crate check |
| old String task route | String/suffix IDs, debug-label routing, operation-spelling family inference, legacy snapshot reader | fixed IDs and family enum | deletion/source matrix checks | API absence and old-byte rejection |
| copied View runtime row | persistent bundle embedding compiler-local catalog row | projection-only bundle row | exact bundle shape | bundle codec structural test |
| dual Await carrier | direct-Await surrogate beside `RuntimeNeedHandle` | one NeedHandle route | deletion matrix | exhaustive compiler/runtime build |
| plan self-digest | task plan storing its own digest as mutable authority | digest derived by owner | deletion matrix | recomputation/tamper test |
| numeric reopening | changed AWBC opcode/function/flag/varint allocation | retained parent numeric authority | no numeric allocation file in package; decision register | parent golden vectors remain unchanged |

## Validator interpretation

`tools/validate_package.py` treats the machine files as normative equivalents,
not as a substitute for prose. It checks their closed sets and then checks that
the prose/table artifacts required to explain them exist. Its negative
self-tests copy only the package machine data to a temporary directory, inject
one forbidden crossing at a time, and require the same validation routine to
reject the mutation.

The validator does **not** claim that production source has already been
changed. Production absence proofs are implementation gates listed above and in
`TEST_MATRIX.md`; package validation proves that this design return itself does
not authorize those routes.
