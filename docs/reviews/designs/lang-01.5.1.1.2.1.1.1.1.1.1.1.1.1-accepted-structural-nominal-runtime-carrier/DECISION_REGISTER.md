# Decision register and traceability

| Request decision | Selected owner/schema | Consumers | Positive proof | Negative mutation | Deletion cut |
|---|---|---|---|---|---|
| D1 source-order fields | ordered sema metadata; core field IDs; record/domain/AWBC rows | classifier, compiler, plan, VM, digest, restore | reorder changes digest and IDs round-trip | duplicate/reordered/missing field | C1-C4 |
| D2 enum payloads | RuntimeValue Variant plus Tuple/Record payloads; one-tuple preserved | constructors, Match, constants, snapshot | unit, tuple0/1/N, record0/1/N goldens | flatten tuple1 or confuse empty forms | C1-C5 |
| D3 recursion/generics/opaque | semantic-ID graph, typed NominalRef, exact opaque leaves | sema, plan, AWBC restore | self/mutual/generic/nested opaque | dangling ref, nonnominal cycle, wrong opaque | C1-C6 |
| D4 schema/layout | core RuntimeNominalSchemaGraph/version-1 encoder | every layer | same layout at sema/plan/AWBC/snapshot | alter any atom or layer digest | C1-C6 |
| D5 checked predicates | checked Record, layout-bearing Variant, graph-aware plan/AWBC walk | Need args, Match, constructors, restore | Result/Option/record payload | wrong owner/layout/ordinal/name/presence | C1-C6 |
| D6 catalog join | RustAdt role plus bijective item/ID/arity metadata join | registrar, final analysis | exact transaction publishes | metadata-only, opaque+metadata, foreign item | C2 |
| D7 visibility/staleness | private projection/stamp and checked constructors | compiler, plan, snapshot | current world compiles/restores | stale analysis/world/catalog/generation | C2-C6 |
| D8 deletion order | six compile-clean cuts, no compatibility path | workspace | fail-closed replaced last | old constructor/tag/model gate | C1-C6 |

## Supporting decisions

- AcceptedRuntimeCarrier is forbidden.
- Runtime nominal IDs derive from typed semantic identity, not source spelling.
- Definitions are reachable-only and sorted by semantic ID; local declaration
  order remains semantic.
- Layout hashing excludes whole-catalog stamps and derived layout hashes.
- New AWBC tag allocations: zero.
- Every Arcweft-owned version marker remains 1.
- Fixed-width version remains only at the existing AWBC envelope; row
  IDs/lengths/ordinals use canonical varint.
- Session JSON remains the strict outer version-1 snapshot envelope.
- No old reader, migration, fallback, source reconstruction, or copied runtime
  catalog is permitted.
