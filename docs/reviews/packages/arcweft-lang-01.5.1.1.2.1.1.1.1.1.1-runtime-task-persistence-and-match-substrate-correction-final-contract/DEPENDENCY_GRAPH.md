# Corrected dependency graph

## Final crate direction

```text
arcweft-lang-hir
       |
       v
arcweft-lang-sema  -- CheckedMatch/coordinates/ownership/admission -->
       |                                                        |
       v                                                        v
arcweft-compiler / arcweft-view                         arcweft-runtime-plan
       |                                                        |
       +---- compiler-local View row                             |
       |                                                        v
       +---------------- accepted digest products ------> arcweft-core
                                                            |
                                                            v
                                              arcweft-runtime-scheduler
                                                   |             |
                                                   |             v
                                                   |      arcweft-host-adapter
                                                   |       /       |       \
                                                   v      v        v        v
                                      arcweft-runtime-driver  desktop  web  runtime-host
                                                   |
                                                   v
                                           View/AWBC/engine consumers

arcweft-bundle consumes projections from compiler/View/runtime-plan/core.
It never imports compiler-local `CheckedMatchRef`.
```

## Owner edges

| From | To | Permitted dependency |
|---|---|---|
| HIR | sema | current arenas, exact `HirSnapshotId`, structural child roles |
| sema | compiler/View | checked Match, stable site/admission facts |
| sema | runtime-plan | closed checked type/ownership/producer-admission facts |
| core | runtime-plan | RuntimeValue/type/task identity owner definitions |
| core | scheduler | TaskSpec/TaskExecution/NeedHandle/events/value snapshots |
| scheduler | host-adapter | Host-only prepared token trait |
| driver | scheduler | step/events/snapshot/replacement API |
| bundle | projections | strict accepted bytes/digests only |
| concrete adapters | host-adapter/core | implement Host prepared token and typed events |

Forbidden reverse edges:

- core or scheduler to compiler-local HIR/sema IDs;
- sema to scheduler;
- bundle to `CheckedMatchRef`/HIR;
- host adapter to scheduler journal internals;
- driver to journal/counter concrete maps;
- Cut 3 View row to Cut 4 task digest types.

## Cut graph

```text
Cut 1 Generic Match
   |
   v
Cut 2 Ownership
   |
   v
Cut 3 Compiler-local View admission
   |
   v
Cut 4 Private core identity/sink preparation
   |
   v
Cut 5 Atomic public switch
```

The chronological edge from Cut 3 to Cut 4 does not mean Cut 3 may name Cut 4
types. Cut 3's type dependencies are only Cut 1/2 and current View identities.

## Cut 5 protected switch set

The following surfaces change together:

```text
RuntimeValue enum + every exhaustive visitor
TaskSpec/TaskExecution/correlation/events
RuntimeTaskScheduler journal/runtime state/adapter
driver consumer
Host adapter implementations
View runtime + Await/AwaitMany/timeout
bundle projection
RuntimeValue/task snapshot and replay codecs
replacement transaction
generated schema/fixtures
old String/dual route deletion
```

There is no intermediate public repository state where `RuntimeValue` has a
NeedHandle arm but an exhaustive consumer is unupdated.
