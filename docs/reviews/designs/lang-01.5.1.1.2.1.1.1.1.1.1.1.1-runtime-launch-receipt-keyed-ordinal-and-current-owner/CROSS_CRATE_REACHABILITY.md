# Cross-crate create/read/mutate reachability

This inventory is normative for the boundaries among `arcweft-core`,
`arcweft-runtime-scheduler`, `arcweft-host-adapter`, and the HIR-to-sema child
edge. Protocol state fields are private, never `pub` or `pub(crate)`. Public
enum variants remain the typed sum construction API; immutable limit/config
records and persisted codec records are not mutation authorities and retain
their separately specified representation.

| Boundary value | Create authority | Read authority | Mutation authority |
|---|---|---|---|
| `NeedProducerSpec` | infallible `new` from typed digests/value digest | six semantic getters plus `instance_key` | none |
| `TaskCorrelation` | fallible canonical `derive`; no raw ID constructors | generation/producer/policy/ordinal/NeedId/TaskKey/TaskId getters | none |
| `TaskSpec` | validated `try_new(..., TaskValidationAuthority)`; snapshot/template paths call the same validation | all eight fields through typed getters | none |
| timeout/AwaitMany requests and debug/named values | typed constructors | complete typed getters or existing closed enum variants | none |
| `RuntimeNeedHandle` | reusable validated constructor or private-field committed receipt | correlation/producer/outcome/state/NeedId getters | none; journal owns live state |
| scalar/digest newtypes | selected typed constructors in the Rust schema | `get` or `as_bytes` | none |
| fixed producer/Need/task/cancel IDs | canonical derivation only | enclosing typed getter | none; no raw byte constructor |
| Host launch/restore/rebind/cancel input batch and row | complete typed `new`/`try_new` | generation/correlation/request/capability/reason getters | none |
| Host receipt row/receipt and prepared wrapper | adapter creates typed row; receipt/wrapper validates exact input batch | complete receipt getters; wrapper receipt/into-parts | opaque token only in adapter |
| Host operation catalog, row, and request contract | typed validating constructors | digest/rows/resolve and complete row/contract getters | none after construction |
| `RuntimeGenerationJournal` | core `new` or outer-authority restore | generation/revision and typed row lookup | only sealed core apply |
| task/Need/observer/scope/accepted-Host journal rows | core transaction/restore only; no public constructor | read-only typed getters | only inside core after-image |
| `JournalTransaction` | `RuntimeGenerationJournal::begin_transaction` with matching validation authority | typed ensure results and Host adapter batches | private staged after-image only |
| `SealedJournalAfterImage` | `JournalTransaction::seal` only | no raw row access | consumed by journal apply |
| `AppliedJournalBatch` / `AppliedEnsureResult` | successful complete journal swap only | generation/revision/results and live handles | none |
| scheduler runtime after-image | scheduler-private planner only | scheduler-private | one infallible scheduler swap |
| committed launch receipt | journal lookup after committed apply only | consumed by public handle constructor | none; fields remain private |
| `HirExpressionChildEdge` | HIR traversal owner only | public `child()` and `role()` for sema | none |

`TaskLaunchAdapter` exposes all twelve prepare/commit/rollback methods. Prepare
owns the only fallible external reservation step and returns an inspectable
receipt plus an adapter-private token. No core row is constructed by the
scheduler or adapter.

## Atomic coordinator transcript

Each scheduler coordinator owns exactly one
`SealedJournalAfterImage`, one scheduler-private runtime after-image, and the
prepared tokens for its operation family. The operation is:

1. run all core, scheduler, catalog, receipt, and adapter-prepare validation;
2. attempt journal apply, whose generation/revision check occurs before any
   mutation;
3. if that attempt fails, consume tokens through reverse rollback and return;
4. after success, perform the infallible scheduler swap;
5. perform infallible adapter commits in canonical order; and
6. return the core-built applied results.

The ensure, restore, rebind, and cancel coordinators have separate typed token
wrappers but identical ordering. There is no `Result`, validation lookup,
allocation, receipt-to-handle conversion, or callback after journal apply.
Thus a journal apply failure remains rollback-safe, while success has no later
error edge that could expose a half-applied graph.

## Negative closure

The repository-aware validator parses the schema AST. It independently mutates
every required public constructor/getter to private, every protected protocol
type's first field to public, a representative field to `pub(crate)`, every
adapter method and scheduler coordinator out of existence, and adds a raw
`TaskJournalRow` constructor. Each mutation must fail its typed API gate.
Source substring placement is not acceptance evidence.
