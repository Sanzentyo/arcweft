# Production reconciliation

## 1. Reconciled production seam

At current `main` `8984661d5679efccf7a16255f921530cd0b7cacc`, profile construction already creates a document-bound `HirProject`, borrows it for `CharacterRegistrar`, and then drops it. Accepted publication separately retains the registered world, accepted documents, overlays, generation, and caches. The session begins a signature request from a URI and live editor snapshot, while the symbol table only provides canonical module-to-source lookup. The current server also records cancellation IDs in a set but executes requests synchronously, so an in-progress query cannot observe `$/cancelRequest`.

This contract closes those seams without moving editor policy into sema:

```text
bounded load + overlay binding
        |
        v
parse/lower all selected modules once
        |
        v
Arc<HirProject> ----------------------+
        |                              |
        | borrowed                     | retained
        v                              v
CharacterRegistrar          AcceptedProjectSnapshot
        |                              |
        v                              v
Arc<RegisteredSemanticWorld> + accepted sources/reverse index
        \______________________________/
                       |
               one candidate swap
                       |
                       v
          Arc<AcceptedProfileEnvironment>
                       |
                       v
              AcceptedDocumentHirLease
```

The registered world remains semantic data. The accepted snapshot remains LSP generation data. Their IDs/revisions/module inventories are cross-validated before publication.

## 2. Exact production deltas

| Current owner | Current behavior | Required final behavior |
| --- | --- | --- |
| `arcweft-lang-hir::HirProjectModule` | panicking `new` asserts source binding | inherent `try_new` with typed missing/mismatch errors; `new` deleted |
| `arcweft-lang-hir::HirModule` | exposes source identity but not retained document | inherent `source_document()` accessor |
| `arcweft-project-loader` | LSP path can read/parse without a complete aggregate source budget | typed `ProjectLoadLimits`, bounded enumeration/read, 4,096 documents and 8,388,608 bytes supplied by LSP |
| `profiles/environment.rs` | builds `HirProject` by value, registers by borrow, then drops it | wraps the one project in `Arc`, registers by borrow, retains the same Arc in the candidate |
| `profiles/cache.rs` | source registry/world/candidate/state/cache responsibilities are co-located; no HIR | split accepted project, publication state, and caches; no compatibility module |
| accepted source maps | URI authority uses `String` | one private `LspUriKey` used end-to-end |
| `AcceptedProfileEnvironment` | world + independently published source registry + overlays + caches | world + one `Arc<AcceptedProjectSnapshot>` + overlays + caches; callers use `project().sources()` and old `sources()` is deleted |
| `DocumentStore`/session maps | URI maps use `String` | `BTreeMap<LspUriKey, ...>` |
| overlay publication | complete rebuild for changed bytes; no selected identical-version fast path | changed bytes remain transactional; identical bytes/new version publish metadata-only generation |
| signature handler | live snapshot + word fallback | prepared accepted lease; fallback deleted only after 009.3.1 + original sema query land |
| session cancellation | `BTreeSet<RequestId>`, pre-dispatch only | active `RequestControl` registry whose exact `AtomicBool` reaches sema |
| server dispatch | synchronous request handling blocks cancel notifications | signature work goes through fixed bounded executor; message intake remains available for cancellation |
| cache publication | no final accepted-HIR stamp route | shared validator before cache return and after computation; publication gate serializes cancel/insert |

## 3. Candidate transaction

Candidate construction and publication are separate phases.

### 3.1 Construction phase

Construction may perform I/O, parsing, lowering, symbol linking, registration, character indexing, source indexing, and accepted snapshot validation. It owns only local values. Any error drops the local project/world/snapshot and returns a typed profile diagnostic. No current environment, generation counter, or cache is mutated.

Pseudocode:

```rust
let loaded = load_with_limits(manifest, lsp_project_limits())?;
let modules = lower_selected_modules(&loaded, overlays)?;
let project = Arc::new(HirProject::new(package, modules)?);
let registration = load_project_registration(...)?;
let world = Arc::new(register_semantic_world(project.as_ref(), &registration)?);
let project = Arc::new(AcceptedProjectSnapshot::try_new(
    Arc::clone(&project),
    world.as_ref(),
    accepted_source_seeds(...),
)?);
let candidate = AcceptedProfileCandidate::try_new(profile, world, project, overlays)?;
```

### 3.2 Publication phase

Publication runs under the session write lock and profile accepted write lock. It verifies the expected state/current Arc and exact open-overlay coverage. It cancels requests bound to the old accepted Arc, clears old signature caches, increments generation with checked arithmetic, creates a fresh environment/cache namespace, and performs one pointer swap.

Generation exhaustion rejects publication without changing the current Arc.

A failed candidate or stale `expected` pointer cannot clear or replace the current environment.

## 4. Overlay reconciliation

| Event | Parse/lower | World/project identity | Generation/cache | Queryability |
| --- | --- | --- | --- | --- |
| `didOpen`, bytes equal accepted source | none | exact current world/project Arcs reused | metadata-only new generation, empty cache, overlay added | queryable only after synchronous publication |
| open/changed bytes equal accepted bytes; accepted version equal | none | current exact Arcs | unchanged | queryable |
| identical bytes, newer LSP version | none | exact current world/project Arcs reused | new generation, empty cache | blocked until metadata publication completes, then queryable |
| changed bytes, rebuild succeeds | full selected project transaction | new accepted Arcs | new generation, empty cache | blocked during transaction, then queryable |
| changed bytes, rebuild fails | failed local transaction only | old Arcs remain | old generation/cache preserved for unchanged docs | changed URI returns `ContentModified`; no old-HIR query |
| close | none | current environment may remain for other docs | document cache entries invalidated | closed URI unqueryable |
| workspace removal/shutdown | none | current strong references cleared | all caches cleared | admission closed |

The session cancels already-admitted requests at `didChange` before attempting either fast metadata publication or rebuild. This prevents an old-version request from publishing while the live document is being replaced.

## 5. Request and cancellation reconciliation

### 5.1 Dispatch

`run_connection` owns:

```text
Arc<RwLock<ArcweftLspSession>>
SignatureRequestRuntime
  - Arc<RequestRegistry>
  - SignatureRequestExecutor (4 workers, bounded by registry maximum 32)
  - response sender clone
```

Signature request preparation is a short session-read operation. The prepared immutable request is sent to the executor. The intake thread immediately resumes receiving protocol messages, so it can route `$/cancelRequest`, `didChange`, `didClose`, workspace changes, and shutdown.

No worker mutates session maps directly. For pre-work/cache lookup it acquires session read, profile accepted read, control gate, and cache in the global order, validates, and releases every guard before a cache-miss sema computation. For cache-hit or computed publication it reacquires the same order, validates, enqueues the response while the lifecycle locks/gate are held, performs any exact stamped-cache insertion, marks `Finished`, and then releases the guards.

### 5.2 Exact cancellation visibility

The sema query borrows `RequestControl.cancelled`. Client cancellation, deadline, document change/close, profile remap/close, accepted replacement, workspace removal, and shutdown all set that same atomic. The binding stores typed URI/workspace/document values plus `Weak<LspProfileState>` and `Weak<AcceptedProfileEnvironment>`; registry matching uses typed equality and `Weak::ptr_eq` without retaining old generations. The worker checks the atomic/deadline/stamp before cache access, and the original query work accounting remains responsible for cooperative `Acquire` checks in bounded loops/checkpoints and immediately before result return.

### 5.3 Cache race proof

There are only two legal linearizations:

1. lifecycle/cancel obtains `RequestControl::gate` first: it marks cancellation; later final validation fails and no insert occurs;
2. worker obtains session/profile/gate/cache first: all stamp fields and deadline are current; it enqueues the response, inserts into the exact stamped cache when cacheable, and finishes; a later lifecycle operation clears that cache while changing state.

A worker cannot insert into the new environment because it never receives the new environment Arc. It cannot insert into an old environment after replacement because replacement requires the session/profile write locks that exclude its final read guards and changes the pointer checked by the validator.

## 6. Invalidation orchestration

### 6.1 Document close

Under session write lock:

1. capture current URI/profile/accepted source identity;
2. cancel matching controls as `DocumentClosed`;
3. invalidate original signature-cache entries for that accepted identity;
4. remove document, profile mapping, and document analysis;
5. when the workspace/profile still exists, synchronously mark the URI closed and schedule one disk/remaining-overlay rebuild so the next accepted overlay set omits it;
6. only when no URI or workspace owner retains the profile state, close it and clear its current environment/caches instead of scheduling a rebuild.

### 6.2 Workspace removal

The workspace URI is the typed value in `AcceptedProfileKey`. The session collects unique state Arcs by `Arc::ptr_eq`, closes their admission, cancels workspace-bound controls, clears all caches/current accepted Arcs, then removes workspace mappings and analyses. Iteration order is `BTreeMap` order and results are deterministic.

### 6.3 Accepted replacement

The session verifies expected profile-state pointer and old environment pointer. Cancellation and old-cache clearing occur before the swap while workers cannot finalize. New cache state is empty.

### 6.4 Failed replacement

`record_failed_replacement` verifies that the expected old Arc remains current and returns without mutation. It is intentionally an observable no-op: the accepted generation represents the last successful transaction, while changed live bytes/version make the affected URI temporally stale.

### 6.5 Shutdown

Under the session write lock, `begin_shutdown` closes admission before any cache/environment is cleared, cancels active controls, closes profile states, and clears maps/caches. After releasing that lock, `SignatureRequestRuntime::shutdown` closes/drains queued jobs, joins finite cooperative workers, then closes/joins the deadline scheduler and verifies no active entry remains. Unknown future cancellation IDs are not retained.

## 7. Limits and memory reconciliation

The profile path must not parse a source before the bounded reader has charged it. `ProjectLoadLimits { documents, source_bytes }` belongs to project-loader, exposes only `new` and value getters, and contains caller-supplied typed values; project-loader does not depend upward on sema/LSP. LSP passes the existing 4,096-document and 8,388,608-byte authorities. One connection/profile rebuild transaction owns at most one unpublished candidate and there is no background candidate queue.

The accepted footprint counts every unique identity once. It is stored with the snapshot and asserted in tests. HIR module count cannot exceed accepted document count because every module source must exist and be unique.

Permanent HIR ownership is one `Arc<HirProject>` per current environment. Registration does not retain another copy. Old accepted environments are retained only by at most 32 admitted contexts; only four execute. Registry/scheduler bindings use `Weak` for state/environment matching. Replacement clears old caches immediately. This closes both count growth and input-size growth.

## 8. Structural placement

The implementation shall split the current large `profiles/cache.rs` rather than append more responsibilities:

```text
crates/arcweft-lsp/src/uri_key.rs
crates/arcweft-lsp/src/profiles/accepted_project.rs
crates/arcweft-lsp/src/profiles/caches.rs
crates/arcweft-lsp/src/profiles/state.rs
crates/arcweft-lsp/src/requests.rs
crates/arcweft-lsp/src/requests/control.rs
crates/arcweft-lsp/src/requests/registry.rs
crates/arcweft-lsp/src/requests/executor.rs
crates/arcweft-lsp/src/requests/signature.rs
crates/arcweft-lsp/src/session/signature.rs
crates/arcweft-lsp/src/session/lifecycle.rs
```

`profiles/cache.rs` is deleted after callers move; it is not left as a re-export or compatibility module. Existing public exports are rehomed directly in `profiles.rs`. No new file may combine accepted project validation, worker scheduling, and cache implementation.

## 9. Dependency and non-goal reconciliation

No external crate dependency is added. `std::sync`, existing `lsp-server`, `lsp-types`, HIR, sema, source, and project-loader APIs are sufficient.

This cut does not select the AW-AH-009.3.1 call syntax/range carrier, redesign character nominal types, define AW-AH-009.3.3 resolver records, forge syntax snapshot IDs, add serialization, alter result ordering/labels/cache keys, or restore removed syntax. Construction/dependency checks use typed APIs and Cargo metadata, never source spelling/path gates.
