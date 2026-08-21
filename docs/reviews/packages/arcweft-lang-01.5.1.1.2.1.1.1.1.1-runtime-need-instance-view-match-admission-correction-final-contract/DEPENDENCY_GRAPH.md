# Dependency graph

## Compile-time authority flow

```text
arcweft-lang-syntax
        |
        v
arcweft-lang-hir
        |
        v
arcweft-lang-sema
  - CheckedMatch
  - MatchCoverageAnalyzer
  - CheckedMatchSemanticDigest
  - AcceptedNominal evidence/catalog
  - total ownership certificates
        |
        | exact checked facts and private projections
        v
arcweft-compiler -----------------------> arcweft-view
        |                                  - ViewProgramId
        |                                  - AcceptedViewProgramRevision
        |                                  - core-independent View coordinates
        v
arcweft-runtime-plan
        |
        v
arcweft-core
  - RuntimeValue / RuntimeValueDigest
  - GenerationId and task/Need identities
  - TaskSpec/correlation/event/state
  - AWBC program/task-plan semantics
        |
        +---------------------------+
        |                           |
        v                           v
arcweft-runtime-scheduler      arcweft-bundle
        |                           |
        +-------------+-------------+
                      v
            arcweft-runtime-driver
              - journal transaction
              - View evaluation
              - save/replay/restore
              - hot replacement
                      |
                      v
             host/adaptor crates
```

## Dependency constraints

1. `arcweft-view` remains independent of `arcweft-core`; it stores
   `ViewProgramId`, revision, and lightweight View coordinates only.
2. The compiler/bundle join validates View coordinates against core/AWBC
   products and carries private fixed projections. `arcweft-view` never stores
   `RuntimeValue`, `AwbcRegisterId`, or a copied type table.
3. `arcweft-lang-sema` does not depend on `ResourceTypeRegistry` for this
   contract. Current Agent resource types have no exact registry key.
4. `arcweft-core` is Sans I/O. It defines task request/event data and identity
   algorithms; adapters perform I/O only after receiving a fully derived launch
   envelope.
5. runtime-driver depends on the core `GenerationId`; no reverse dependency or
   local duplicate exists.
6. Bundle/save/replay are storage consumers. They verify recomputed identities
   and digests but cannot construct alternate identity.
7. The sink abstraction for canonical RuntimeValue encoding is private to
   `arcweft-core::entry`; it is not a public cross-crate trait.

## No-cycle proof for View identity

```text
CheckedMatchSemanticDigest
    -> CheckedViewMatchAdmissionDigest
        -> CheckedViewMatchCoordinate(program, site, admission)
            -> View task-plan semantic digest
                -> NeedProducerInstanceKey
```

`AcceptedViewProgramRevision` is not on this path. The accepted revision is
computed from the complete typed View catalog, which may contain the coordinate;
keeping revision outside the path removes the predecessor's digest cycle.

## No-duplication proof

- `NeedProducerContractDigest` commits what producer contract is selected.
- `TaskPlanSemanticDigest` commits the complete static executable/task-plan
  meaning and View program/site/admission where applicable.
- `producer_site` selects one row within that plan.
- `RuntimeTypeSemanticDigest` commits the produced payload type.
- `RuntimeValueDigest` commits evaluated source-order arguments.
- `TaskPolicy` is appended only by Need/task correlation derivation.
- `GenerationId` is appended only by `TaskKey`.
- `TaskLaunchOrdinal` is appended only by `NeedId` and `TaskId`.

No field is authoritative in two transcript owners, and no owner reads another
owner's debug/source spelling.
