# Deletion-driven compile-clean sequence

## 1. Governing rule

The public `RuntimeTaskPlan` row/table is published only in the Cut 5 atomic
switch. Before that switch, all new encoder/candidate work stays private and no
consumer can depend on a provisional public schema. Every Arcweft-owned marker
is version one; there is no dual reader or migration alias.

## 2. Retained cuts

### Cut 1 — generic Match semantic substrate

Retain the accepted generic checked Match transcript/coverage owners. Do not add
task-plan or View resource dependencies.

### Cut 2 — ownership and producer admission

Retain only carrier-backed accepted ownership/producer-admission facts. Do not
publish task-plan rows.

### Cut 3 — compiler-local View admission

Publish actual `ViewProgramId`, accepted revision, stable
`ViewMatchSiteId`, and exact `CheckedViewMatchAdmissionDigest` in the compiler-
local catalog. The row keeps compiler lookup evidence and remains nonserialized.
No task-plan digest type is required to complete this cut.

### Cut 4 — private preparation only

Retain the standalone opaque `TaskPlanSemanticDigest` role and the completed
reference in `NeedProducerSpec`. Add, behind private modules or unused private
staging only:

1. the four opaque child digest types;
2. explicit inherent semantic tags on existing enums;
3. private row semantic visitors and work meter;
4. private `UnsealedRuntimePlanImage`/decoded expected-key shapes;
5. private construction-token plumbing; and
6. tests for child transcript bytes that do not publish a final table.

Do not expose `RuntimeTaskPlan`, a task-plan table, a raw digest constructor,
View projection, or new public codec row in Cut 4.

## 3. Cut 5 atomic public switch

The following sequence is one protected merge/commit boundary. Intermediate
local commits may exist for development but are not accepted public cuts.

1. **Core static owner:** add final `RuntimeTaskPlan` and closed binding enum to
   the legitimate core plan module. Add inherent family/binding and tag matches.
2. **Coordinates:** make builder push return owner-bound build coordinates;
   migrate all runtime-plan lowering call sites while the rows remain private.
3. **Child encoders:** wire producer function, request template, and
   control/effect inherent encoders into one private semantic context.
4. **Executable encoder:** add the fixed fifteen-table transcript, task
   coordinate references, memoization/cycle checks, and exact limits.
5. **Opaque base/protocol:** evolve `ViewTaskPlanAuthority` and add the field-
   private non-Clone base/request plus one-use authority finalizer.
6. **Upper View owner:** evolve `ValidatedViewProgramResource` with actual
   `ValidatedViewTaskPlanBinding` rows and implement the sole protocol.
7. **Common sealer:** wire both ordinary and View rows, expected comparison,
   global duplicate collector, and final table construction.
8. **Builder publication:** replace the current final plan literal path with
   private-image seal followed by one final `RuntimePlan` construction.
9. **Codec publication:** replace direct/generic task-plan decode with private
   images, actual View validation, the same common sealer, and outer atomic
   bundle publication.
10. **Consumer cutover:** migrate Need producer templates/instances, structured
    task validation, line/AwaitMany owners, snapshot verification, runtime
    resource lookup, and all focused tests to the one sealed table.
11. **Generated outputs:** regenerate version-one schemas, fixtures, deterministic
    artifacts, and maintained docs from the final owner.
12. **Deletion pass:** use compile failures to find every stale route listed in
    section 4; delete them before restoring green.
13. **Structural gates:** run fmt, focused tests, full tests as required by the
    implementation task, Clippy, Cargo metadata dependency proof, trybuild
    negatives, artifact comparison, and source structural audit.
14. **Publish:** expose the final row/table only after all prior steps pass in
    the same atomic switch.

## 4. Mandatory deletions

| Deleted construct/path | Reason | No replacement alias |
|---|---|---|
| provisional `RuntimeTaskPlan.semantic_digest` | self authority and cycle | table association only |
| provisional `producer_contract`, `producer_site`, `payload_type` fields | explicitly excluded roles | legitimate producer instance owners |
| caller `plan_digest` arguments in lowering/builders | caller authority | owner recomputation |
| public `TaskPlanSemanticDigest::from_bytes`/raw conversion | forgeable digest | owner encoder or sealed-table resolution |
| raw core View projection structs/newtypes | reverse semantic copy | actual upper binding |
| general byte sink/callback passed to authority | caller transcript control | upper local hasher + one-use request finalizer |
| extension traits/free task tag/hash helpers | split enum/owner behavior | inherent implementations |
| parallel `TaskContractCatalogV1`/family tables | duplicate lookup authority | one RuntimePlan table |
| public expected digest field | trusted decode key | private expected bytes + recomputation |
| generic Serde task semantic transcript | unstable/noncanonical | purpose-built codec and exact hash visitor |
| old task-plan codec reader | dual model | one version-one strict reader |
| compatibility/fallback/legacy aliases | forbidden dual route | none |
| stale generated schema/fixtures/docs | contradict final field set | regenerated exact outputs |

## 5. Compile-error-driven migration order

After introducing the final private row but before making the final table
public, intentionally delete caller digest and self fields. Resolve resulting
compiler errors in this order:

1. core constructors/verification;
2. runtime-plan lowering and final flow;
3. sema/compiler projections;
4. line/AwaitMany/AWBC joins;
5. bundle validated resources;
6. resource codecs/private decoded images;
7. Need producer instance construction;
8. snapshot/restore verification;
9. runtime hosts/runners and tests;
10. generated schemas/fixtures/docs.

Do not reintroduce a temporary compatibility field to silence errors. A consumer
must move directly to the final typed owner.

## 6. Validation record required from implementation

The implementing return records exact command, full Git SHA, and outcome for:

```text
cargo fmt --all -- --check
focused core task-plan tests
focused runtime-plan lowering tests
focused compiler/Cut 3 tests
focused bundle/View validation tests
focused codec/tamper tests
trybuild negative API tests
cargo metadata dependency gate
cargo clippy --all-targets --all-features -- -D warnings
repository-selected full test suites
repeated deterministic artifact generation and byte comparison
structural absence gate
```

This design-only archive does not claim those production commands were run.
