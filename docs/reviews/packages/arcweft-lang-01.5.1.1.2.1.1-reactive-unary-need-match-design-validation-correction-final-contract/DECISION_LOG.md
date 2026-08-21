# Closed decision log

| ID | Selected | Rejected |
|---|---|---|
| D01 | generation-bound CheckedViewCatalog | compiler/source reconstruction |
| D02 | HIR session key + local ID + semantic/contract digests | strings/spans/global handle |
| D03 | journal `(generation,NeedId)`, observer `(mount,subscription)` | per-mount producers |
| D04 | cursor order, idempotent duplicate, conflict rollback, first terminal | arrival/last-terminal |
| D05 | generic Match/AWBC/binding registers | View matcher/Need VM |
| D06 | Result/Option inside Ready | Need error/denied |
| D07 | NotStarted transactional start intent | direct I/O/prestart-only |
| D08 | JoinSameKey + ProducerOwned cancellation | per-observer task/last observer cancel |
| D09 | arm state by observer/ordinal/digest | shared/spelling-keyed state |
| D10 | strict v1 replacement + complete deletion | aliases/V2/dual reader |
| D11 | live subscription is dynamic | certify current Ready |
| D12 | explicit scratch transaction scopes | in-place/global rollback |
| D13 | exact inclusive numeric limits | ambient/unbounded limits |
| D14 | stale/duplicate no-op | errors/hidden normalization |
| D15 | no production gate claim | inferred Cargo success |
