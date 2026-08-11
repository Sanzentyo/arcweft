# Test matrix

All tests are behavioral Rust tests. No test passes or fails by searching checked-in source, docs, filenames, module paths, or symbol spellings. Concurrency tests use barriers, channels, and explicit test hooks; elapsed-time sleeps are not the correctness oracle.

## 1. HIR and accepted snapshot construction

| ID | Test | Required observation |
| --- | --- | --- |
| AH32-001 | accepted URI exact resolution | One URI resolves to the expected `SourceDocumentIdentity`, canonical module, identical HIR source identity, exact text, and accepted world/revision. |
| AH32-002 | declaration-free root module | A root module with no semantic declarations is present through `HirProject::modules()` and is queryable. |
| AH32-003 | declaration-free non-root module | A non-root module whose source has only its required module declaration resolves through the reverse index. |
| AH32-004 | dependency module | A read-only dependency source with explicit URI and HIR module resolves; access mode does not block read-only signature acquisition. |
| AH32-005 | generated HIR source | A generated source with explicit URI and typed HIR module resolves without deriving a module from URI/path. |
| AH32-006 | generated source without module | An accepted generated/registration source with no HIR module returns `SourceHasNoHirModule` and performs no parse/lower. |
| AH32-007 | generated source without URI | A generated source with unavailable locator is absent from URI lookup and no URI is fabricated. |
| AH32-008 | duplicate source identity | Two source seeds carrying the same `SourceDocumentIdentity` reject snapshot construction; current accepted Arc/generation/cache remain unchanged. |
| AH32-009 | conflicting source ID | Same logical ID with distinct revisions/lengths rejects construction atomically. |
| AH32-010 | duplicate URI | Different source identities with the same `LspUriKey` reject construction atomically. |
| AH32-011 | conflicting module mapping | Two canonical modules carrying one source identity reject `ConflictingModuleMapping` atomically. |
| AH32-012 | module inventory mismatch | HIR/symbol module set difference rejects candidate publication. |
| AH32-013 | module source mismatch | Project, HIR, and symbol source identities disagree; exact typed error, no panic/publication. |
| AH32-014 | HIR text mismatch/collision defense | Equal supplied identity with unequal text is rejected explicitly. |
| AH32-015 | panic-free project module | `HirProjectModule::try_new` reports missing/mismatched source; no panic constructor remains in callers. |

## 2. Limits and footprint

| ID | Test | Required observation |
| --- | --- | --- |
| AH32-016 | document limit exact | 4,096 unique accepted documents pass when all other invariants hold. |
| AH32-017 | document limit one over | 4,097 rejects before parsing the over-limit module. |
| AH32-018 | source bytes exact | Aggregate 8,388,608 unique UTF-8 bytes pass. |
| AH32-019 | source bytes one over | 8,388,609 rejects through bounded read/preflight; no unbounded `read_to_string`. |
| AH32-020 | counter overflow | Injected checked-sum overflow returns `ArithmeticOverflow` and publishes nothing. |
| AH32-021 | symbol work exact/one over | Existing 262,144 work boundary succeeds exactly and rejects one over with existing typed symbol limit evidence. |
| AH32-022 | footprint exactness | Stored document/module/source-byte footprint equals independently constructed typed fixture counts. |

## 3. Overlay acceptance

| ID | Test | Required observation |
| --- | --- | --- |
| AH32-023 | unchanged accepted overlay | Exact bytes/version acquire the current generation and exact HIR; no rebuild. |
| AH32-023A | `didOpen` equal bytes | A synchronous metadata-only generation adds exact URI/version overlay, reuses exact world/project Arcs, and is visible before notification completion. |
| AH32-023B | `didOpen` changed bytes | URI becomes pending immediately, old URI requests are cancelled, and no signature acquisition occurs until the full candidate publishes. |
| AH32-024 | changed overlay successful rebuild | Old requests are cancelled, new bytes are parsed/lowered transactionally, one new generation publishes, and only new HIR/document/world are acquired. |
| AH32-025 | changed overlay failed rebuild | Old accepted Arc/cache remain for unchanged docs; changed URI returns `DocumentNotAccepted`; no partial project/world/cache publication. |
| AH32-026 | identical bytes, new version | No parse/lower test counters increment; exact world/project Arcs are reused; generation increments; new cache empty; accepted version becomes current. |
| AH32-027 | version not yet accepted | Same bytes but mismatched live/accepted version returns `OverlayVersionNotAccepted`, not a cache hit. |
| AH32-028 | missing overlay coverage | Candidate omitting an open URI is rejected during session publication. |
| AH32-029 | duplicate overlay URI | `AcceptedOverlaySet::try_new` returns `AcceptedOverlaySetError::DuplicateUri` before candidate construction; no map entry is overwritten. |

## 4. Acquisition and stamp changes during flight

Each test pauses the sema query after acquisition, mutates exactly one authority, resumes, and asserts typed stale failure plus no cache insertion.

| ID | Mutated authority | Required failure |
| --- | --- | --- |
| AH32-030 | URI maps to a different `AcceptedProfileKey` or no profile | `ProfileRemapped` with exact expected/actual key. |
| AH32-030A | URI keeps the same profile key but maps to another `Arc<LspProfileState>` | `ProfileStateReplaced`; `Arc::ptr_eq` fails. |
| AH32-031 | accepted environment Arc with otherwise equal values | `AcceptedReplaced`. |
| AH32-032 | generation value | `GenerationChanged`. |
| AH32-033 | registered world Arc | `WorldArcChanged`. |
| AH32-034 | world ID | `WorldIdentityChanged`. |
| AH32-035 | symbol revision | `SymbolRevisionChanged`. |
| AH32-036 | character inventory digest | `CharacterDigestChanged`. |
| AH32-037 | character inventory revision | `CharacterRevisionChanged`. |
| AH32-038 | accepted project snapshot Arc | `ProjectArcChanged`. |
| AH32-039 | value-equal module/HIR carried by another `Arc<HirProject>` | `HirChanged`; project Arc plus module key is the complete HIR identity. |
| AH32-040 | live protocol document identity | `DocumentChanged`. |
| AH32-041 | LSP document version only | `DocumentVersionChanged`. |
| AH32-042 | URI-to-accepted-source mapping | `UriRemapped`. |
| AH32-043 | accepted source Arc/identity | `AcceptedDocumentChanged`. |
| AH32-044 | source-to-module mapping | `ModuleChanged`. |
| AH32-045 | current accepted environment profile key value | `ProfileKeyChanged`; this value check precedes an otherwise-new environment pointer. |

For every case, run both cache-hit and cache-miss/post-compute variants. A stale hit is not returned and a stale miss is not inserted into old or new caches.

## 5. Cancellation, deadline, and publication races

| ID | Test | Required observation |
| --- | --- | --- |
| AH32-046 | cancellation before worker starts | Request is admitted/queued, cancel marks the exact atomic, worker observes cancellation before query work, no cache/result publication. |
| AH32-047 | cancellation during sema work | Query checkpoint observes the exact `RequestControl` atomic; response is `RequestCancelled`; guard removes active/deadline entries. |
| AH32-048 | cancellation immediately before publication | Cancel thread acquires publication gate first; worker cannot insert. |
| AH32-049 | publication wins race | Worker validates, enqueues response, inserts when cacheable, and marks `Finished` under lifecycle/gate locks first; later cancel is too late; subsequent lifecycle invalidation clears as required. |
| AH32-049A | response enqueue failure | Closed response channel causes no cache insertion and no `Finished` transition; guard cleanup removes active/deadline entries. |
| AH32-050 | deadline during queued work | 250 ms deadline includes queue time, sets atomic, cancelled queued job does no sema work. |
| AH32-051 | deadline during running work | Scheduler sets atomic; checkpoint exits; final validator also rejects at/after exact deadline. |
| AH32-052 | scheduler wakeup delayed | Worker final check uses `Instant` and rejects late publication even before scheduler runs. |
| AH32-053 | active limit exact | 32 admitted requests are retained and measured. |
| AH32-054 | active limit one over | 33rd admission returns typed limit error and creates no map/deadline/queue entry. |
| AH32-055 | four-worker bound | Test hook observes no more than four simultaneous query executions. |
| AH32-056 | duplicate request ID | Duplicate active ID rejects without replacing the first control. |
| AH32-057 | unknown cancellation | No tombstone/map/deadline entry is created. |
| AH32-058 | deadline token cleanup | Every normal/error/cancel/queue-failure path removes the exact token and active entry. |
| AH32-058A | weak request binding | Registry/scheduler bindings do not increase strong counts of profile state/accepted environment; prepared requests alone retain them. |
| AH32-058B | cancellation/status mapping | First cancellation reason wins; client maps to `-32800`, content lifecycle reasons to `-32801`, and deadline/closing/removal/shutdown to `-32802`. |
| AH32-058C | worker panic cleanup | Injected panic becomes one `-32603` response when publishable and always removes active/deadline entries without cache insertion. |

## 6. Lifecycle invalidation

| ID | Test | Required observation |
| --- | --- | --- |
| AH32-059 | document change | Existing URI request cancelled and old document cache entry invalidated before rebuild. |
| AH32-060 | document close | Request cancelled, live maps removed, historical overlay unqueryable, rebuild scheduled from disk/remaining overlays, and no later cache insertion. |
| AH32-061 | workspace removal | All unique state-bound requests cancelled, all affected caches/environments cleared, admission closed. |
| AH32-062 | accepted replacement | Old request cancelled; old cache cleared; new cache empty; old computation cannot insert into either. |
| AH32-063 | failed replacement | Expected old environment/cache pointer and contents remain; changed URI still blocked by live mismatch. |
| AH32-064 | shutdown | Admission closes before cancellation/clear; workers/scheduler exit; all maps and deadline entries empty; no stale insertion. |
| AH32-065 | old accepted Arc memory safety | A test retains an old Arc reader across replacement, safely reads immutable project/source/HIR, but publication through its stale stamp is rejected. |
| AH32-066 | repeated replacements with active cap | Many replacements while 32 requests retain distinct generations never exceed 32 old request-held accepted Arcs; releasing guards drops them. |

## 7. Sema integration and no fallback

| ID | Test | Required observation |
| --- | --- | --- |
| AH32-067 | exact query tuple | `SignatureQuery::try_new` receives lease document, lease HIR, lease world, 009.3.1 carrier, and control atomic; all identities match. |
| AH32-068 | cache miss performs no compiler build | Instrumented parse/lower/load counters remain zero during request/cache miss. |
| AH32-069 | acquisition failure performs no fallback | Not-applicable/stale/invariant failures do not call word parser, source substring parser, adapter signature helper, or a second resolver. |
| AH32-070 | exact cache semantics preserved | Existing result ordering, error ordering, key fields, limit behavior, and hit/miss outputs match original AW-AH-009.3 tests. |
| AH32-071 | 009.3.1 carrier boundary | Integration consumes the landed carrier API without a local syntax/range model or reconstructed range. |

## 8. Type/dependency/structure evidence

| ID | Test/audit | Required observation |
| --- | --- | --- |
| AH32-072 | Send/Sync assertions | Compile-time generic assertions cover session shared state, accepted snapshot, request control, and prepared request as required by worker transfer. |
| AH32-073 | Cargo metadata dependency evidence | Deserialized package IDs/edges prove loader/HIR/sema/LSP dependency direction and no new external crate. |
| AH32-074 | typed URI/profile construction | Only `from_uri(&Uri)` creates `LspUriKey`; `AcceptedProfileKey` consumes existing `ProfileId`; behavioral map tests prove exact lookup/remap semantics. |
| AH32-075 | canonical structure audit | Repository structure script reports no new dependency cycle/duplicate public type/error and records changed-file metrics. |
| AH32-076 | workspace validation | fmt, check, Clippy `-D warnings`, and workspace tests pass with required assets present. |
