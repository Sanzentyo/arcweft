# Complete test matrix

The IDs below are normative acceptance rows. Tests may be grouped physically, but no row may be omitted or replaced by source-spelling assertions.

## A. Key and product construction

| ID | Scenario | Expected typed result |
|---|---|---|
| K-01 | exact function key from accepted Rust metadata | valid Function requirement |
| K-02 | exact function key from accepted WASM metadata | valid Function requirement |
| K-03 | exact function key from accepted process metadata | valid Function requirement |
| K-04 | exact selected Activity key | valid Activity requirement and selection retaining abstract Activity, implementation, and metadata Activity identities |
| K-05 | Activity abstract ID differs from metadata export | product invalid before publication |
| K-06 | target family/detail mismatch | product/claim construction invalid |
| K-07 | invalid/empty/whitespace ABI newtype | typed identity error |
| K-08 | invalid process transport newtype | typed identity error |
| K-09 | syntactically valid but non-current ABI marker placed in accepted launch product | product invalid accepted-marker error |
| K-10 | syntactically valid but non-current process transport placed in accepted launch product | product invalid accepted-marker error |
| K-11 | duplicate exact canonical anchor | product invalid duplicate |
| K-12 | same anchor with conflicting key | product invalid conflict |
| K-13 | requirement count cannot fit `u32` | product invalid overflow (bounded constructor test, no huge allocation required) |
| K-14 | requirement key topology differs from envelope | product invalid |
| K-15 | selected profile with zero generated requirements | valid `Some(product)` with real topology and empty requirements/selections |
| K-16 | missing/duplicate Activity selection or selection points to Function | product invalid |
| K-17 | Activity selection implementation differs from requirement | product invalid |

## B. Canonicalization

| ID | Scenario | Expected typed result |
|---|---|---|
| C-01 | same keys in every tested permutation | identical canonical requirements, Activity selections, and IDs |
| C-02 | function and Activity under same import | Function sorts before Activity |
| C-03 | imports/mounts/export/implementation/Activity identities differ | ordering follows typed anchor exactly |
| C-04 | metadata/artifact revision changes but anchor is same | ordinal may remain same; topology/key differs and old catalog is stale/mismatch |
| C-05 | map/filesystem iteration order changes | product bytes remain deterministic after canonical serialization |
| C-06 | Activity requirements supplied in arbitrary order | selections canonical by typed `ActivityId` and reference assigned IDs |

## C. Exact registration mismatch matrix

For each row, start from one valid selected requirement, change only the named semantic field while keeping the claimed key internally valid, and assert `runtime-binding-mismatch` plus the exact typed mismatch variant. Where duplicated accepted fields would otherwise conflict, build a complete alternate valid claim rather than constructing an invalid key. Family/detail-inconsistent values are product/claim errors and do not masquerade as runtime mismatch.

| ID | Changed field |
|---|---|
| M-01 | external-module import ID |
| M-02 | mount |
| M-03 | metadata path |
| M-04 | metadata raw digest |
| M-05 | import visibility |
| M-06 | import demand |
| M-07 | metadata document ID |
| M-08 | metadata source revision |
| M-09 | metadata source length |
| M-10 | package ID |
| M-11 | package version |
| M-12 | module ID |
| M-13 | coherent alternate target family |
| M-14 | syntactically valid wrong target ABI in the same family |
| M-15 | Rust target triple |
| M-16 | WASM world |
| M-17 | syntactically valid wrong process transport |
| M-18 | metadata ABI hash |
| M-19 | metadata payload hash |
| M-20 | artifact path |
| M-21 | artifact raw digest |
| M-22 | artifact size |
| M-23 | export kind |
| M-24 | function name |
| M-25 | function visibility |
| M-26 | function parameter name/order/type |
| M-27 | function return type |
| M-28 | function purity |
| M-29 | function effect identity/order |
| M-30 | Activity export ID |
| M-31 | Activity visibility |
| M-32 | Activity identity (valid alternate key changes selected abstract and metadata IDs together) |
| M-33 | selected `ActivityImplementationId` |
| M-34 | Activity interface hash |
| M-35 | Activity state hash |

Additional correlation tests:

| ID | Scenario | Expected typed result |
|---|---|---|
| M-36 | multiple fields differ | first mismatch follows normative order |
| M-37 | failed mismatch registration into empty slot | slot remains empty; later exact registration succeeds |
| M-38 | failed mismatch against occupied slot | mismatch precedes duplicate; existing binding unchanged |

## D. Registration selection/kind/duplicate

| ID | Scenario | Expected typed result |
|---|---|---|
| R-01 | exact function registration | succeeds |
| R-02 | exact Activity registration | succeeds |
| R-03 | function API for Activity ID | `runtime-binding-kind-mismatch` |
| R-04 | Activity API for function ID | `runtime-binding-kind-mismatch` |
| R-05 | ID outside product | `runtime-binding-unselected` |
| R-06 | second exact registration into same slot | `runtime-binding-duplicate`; first binding retained |
| R-07 | fabricated binding for unselected module/export | no requirement ID exists; unselected |
| R-08 | private generated function claim | no requirement; unselected |
| R-09 | unselected Activity export claim | no requirement/selection; unselected |
| R-10 | attempt to construct catalog from no-profile `None` | API/assembly rejects; no empty fallback catalog |

## E. Missing and successful resolution

| ID | Scenario | Expected typed result |
|---|---|---|
| E-01 | selected generated function, no host binding | `runtime-binding-missing` with exact requirement |
| E-02 | selected generated Activity, no host binding | `runtime-binding-missing` with exact requirement |
| E-03 | one exact in-memory function sentinel registered | deterministic borrowed sentinel selected |
| E-04 | one exact in-memory Activity sentinel registered | deterministic borrowed sentinel selected |
| E-05 | one slot bound, another selected slot missing | bound ID succeeds; missing ID fails; no cross-slot fallback |
| E-06 | wrong requested kind at resolve | kind mismatch before missing |
| E-07 | out-of-range ID at resolve | unselected before missing |

E-03 is the required successful host-binding test. It must not read a path, load a library, instantiate WASM, spawn a process, discover a provider, parse metadata, or execute an artifact.

## F. Stale and generation correlation

| ID | Scenario | Expected typed result |
|---|---|---|
| S-01 | claimed key profile differs from product | stale before structural mismatch |
| S-02 | claimed key source-set revision differs | stale before structural mismatch/duplicate |
| S-03 | catalog resolve with newer topology revision | stale before missing |
| S-04 | metadata overlay changes exact bytes | new document/source-set revision; old binding stale |
| S-05 | manifest overlay changes import/mount/profile facts | old catalog stale |
| S-06 | old and new products assign same numeric ID | old catalog still stale under new topology |
| S-07 | LSP environment replacement with changed bytes | old generation lease stale |
| S-08 | LSP environment replacement with identical bytes | old generation lease still stale; no catalog carry-forward |
| S-09 | stale claim presented to occupied new slot | stale precedes duplicate |

## G. No fallback

Each test creates an otherwise plausible alternative and proves no resolution API or success path exists/occurs through it.

| ID | Forbidden discriminator |
|---|---|
| F-01 | callable spelling |
| F-02 | Activity spelling/ID alone at host catalog resolution |
| F-03 | mounted callable path |
| F-04 | artifact basename |
| F-05 | adapter profile/adapter ID |
| F-06 | metadata path alone |
| F-07 | artifact filesystem path alone |
| F-08 | artifact digest alone |
| F-09 | package/module name alone |
| F-10 | last-known-good/parent catalog |
| F-11 | Activity implementation spelling/export spelling reconstructed after projection |

Compile-time/API tests may assert prohibited public methods are absent only as supplemental evidence. The primary tests exercise typed runtime behavior and failure.

## H. Runtime-plan and launch identity preservation

| ID | Scenario | Expected typed result |
|---|---|---|
| P-01 | direct generated full call | plan contains exact `RuntimeCallTarget::GeneratedArtifact(id)` |
| P-02 | generated top-level function reference | runtime function body retains ID |
| P-03 | generated partial call | remaining function body retains same ID and captures passed args |
| P-04 | apply generated function value | dispatch uses retained ID, no name reconstruction |
| P-05 | ordinary intrinsic call | current intrinsic behavior unchanged |
| P-06 | ordinary non-generated named call | named behavior unchanged where still intentional |
| P-07 | plan ID absent from `Some(product)` | compiler product-invalid |
| P-08 | plan generated ID points to Activity requirement | compiler product-invalid |
| P-09 | plan/product topology mismatch | compiler product-invalid |
| P-10 | nested generated function value in serializable runtime value | verifier visits and validates ID |
| P-11 | no accepted launch profile and no generated IDs/selections | compile succeeds with `CompiledProject` product `None` |
| P-12 | no accepted launch profile but plan contains generated ID | compiler product-invalid |
| P-13 | selected profile with zero requirements | compile succeeds with `Some(empty selected product)` |
| P-14 | inspect no-profile output | no synthetic ProfileId/topology/product/catalog exists |
| P-15 | selected Activity runtime launch record | carries exact `GeneratedArtifactActivitySelection`, including implementation and binding ID |
| P-16 | Activity launch selection missing/mismatched against product | compiler/runtime assembly product-invalid before host work |

## I. Codec and round trip

| ID | Scenario | Expected typed result |
|---|---|---|
| W-01 | launch product round trip | every key field, Activity selection, and canonical ID equal |
| W-02 | runtime call-target generated variant round trip | ID equal |
| W-03 | runtime function-body generated variant round trip | ID equal |
| W-04 | wrong format | decode failure; no fallback reader |
| W-05 | schema 0/2 | decode failure |
| W-06 | unknown top-level field | decode failure |
| W-07 | unknown nested key/target/export/selection field | decode failure |
| W-08 | non-canonical requirement order | decode failure, not repaired |
| W-09 | non-contiguous/duplicate IDs | decode failure |
| W-10 | duplicate anchor | decode failure |
| W-11 | key/envelope topology mismatch | decode failure |
| W-12 | invalid nested typed path/digest/identity | decode failure |
| W-13 | Activity abstract/metadata identity mismatch | decode failure |
| W-14 | family/detail mismatch | decode failure |
| W-15 | deterministic serialization from permuted source candidates | byte-identical canonical product |
| W-16 | no legacy aliases/defaults accepted | decode failure for every removed/alternate spelling fixture |
| W-17 | non-current but syntactically valid ABI marker in product | decode failure |
| W-18 | non-current process transport in product | decode failure |
| W-19 | Activity selection missing/duplicate/non-canonical | decode failure |
| W-20 | Activity selection implementation/binding mismatch | decode failure |
| W-21 | compiled launch envelope `None` round trip, where such codec exists | remains absent; no fabricated empty product |
| W-22 | compiled launch envelope `Some(empty selected product)` round trip | remains present with exact real topology |
| W-23 | old envelope omitting newly required presence field | no compatibility/default reader |

## J. No partial host work

Use counters plus exact before/after snapshots.

| ID | Failure | State required unchanged |
|---|---|---|
| N-01 | missing generated full call | callback count, task/request queues, scheduler work |
| N-02 | stale generated full call | same |
| N-03 | unselected/wrong-kind generated full call | same |
| N-04 | missing generated function apply | callback count, captures/value state except returned error, queues |
| N-05 | stale generated function apply | same |
| N-06 | missing generated Activity start | Activity state allocation count, registry, targets, events, scheduler/tasks |
| N-07 | stale generated Activity start | same |
| N-08 | wrong-kind/unselected/mismatched Activity selection start | same |
| N-09 | failed registration mismatch | catalog slot state unchanged |
| N-10 | failed duplicate registration | original binding identity unchanged |
| N-11 | no-profile generated dispatch attempt | no catalog creation and no host work |

## K. Projection transaction

| ID | Scenario | Expected typed result |
|---|---|---|
| T-01 | accepted non-private function | callable origin and requirement share same ID |
| T-02 | private function | neither callable nor requirement |
| T-03 | selected Activity binding | exactly one Activity requirement and one matching selection retaining `ActivityImplementationId` |
| T-04 | unselected Activity export | no requirement or selection |
| T-05 | missing selected Activity module/export | complete topology transaction fails |
| T-06 | Activity identity/implementation reconciliation mismatch | complete topology transaction fails |
| T-07 | duplicate mounted identity | neither adapter nor product is published |
| T-08 | invalid function signature/purity/effects | neither adapter nor product is published |
| T-09 | metadata decoder invocation counter | projection performs zero additional decodes |
| T-10 | same accepted inputs | adapter origins, requirements, and Activity selections deterministic |
| T-11 | selected profile has no generated requirements | topology still publishes real empty selected product |

## L. Validation commands at implementation completion

At minimum, record actual exits for:

```text
cargo fmt --all -- --check
cargo clippy --all-targets
focused tests for arcweft-id, adapter-metadata, adapter-context,
  project-loader, lang-sema, runtime-plan, core, compiler,
  runtime-binding, runtime-host, runtime-driver, and lsp
just test-workspace
just test-tier2
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

Use the latest applicable repository commands if names have moved, and record the substitution. Source grep alone is not acceptance evidence.
