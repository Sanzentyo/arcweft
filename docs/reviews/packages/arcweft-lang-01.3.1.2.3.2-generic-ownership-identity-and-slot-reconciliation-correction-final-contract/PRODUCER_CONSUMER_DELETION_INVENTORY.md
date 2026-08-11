# Producer, consumer, and deletion inventory

## 1. Inventory rule

This is an owner inventory, not a file-placement mandate. Implementation may
split modules under current `AGENTS.md`, but it must preserve the sole owner and
dependency direction shown. New behavior on an existing enum/newtype belongs in
that owner's inherent implementation.

“Delete/replace” means direct replacement in the same compile-clean cut. It does
not permit a deprecated alias, adapter trait, parallel model, feature gate, or
dual reader.

## 2. Core identity/value owners

| Current owner/surface | Current evidence | Target producer/owner | Consumers | Required action |
|---|---|---|---|---|
| `arcweft-core::runtime_id` | typed runtime lookup IDs, no execution/slot IDs | same module owns scalar execution/runtime wrappers and strict codecs | value ownership, engine, AWBC, driver snapshot | extend existing module; private raw construction |
| `value::ownership::RuntimeValueOwnership` | shipped two-point lattice and exhaustive traversal | same enum/inherent impl | all execution/snapshot paths | preserve classifier result; extract one internal path-aware visitor |
| `RuntimeValue` | ordered aggregate variants; currently Clone/Serde | same enum, parent affine cut | structured, AWBC, snapshot | no parallel value enum; remove unconditional Clone/live Serde per parent |
| `RuntimeFieldValue` | public `{ name, value }`, no field ID | same struct with private ID/name/value | record construction, traversal, codecs | replace constructor/fields directly |
| `RuntimeNominalRecordValue` | schema-ordered values, unchecked construction surface | same owner + inherent `field_id`/checked layout constructor | field access, traversal, snapshot | no ID side vector |
| `RecordSeqField` | name + values, stored field order | same owner + field ID | column traversal and codecs | replace direct literal construction |
| `RuntimeBinding` | public name/value, Clone/Serde | same struct with slot/declaration/revision/mutability/cell | env, captures, save projection | replace; fields private |
| `RuntimeEnv` | nested name-indexed scopes and spare-scope reuse | same environment with typed slots/scope instances and two-phase `RuntimeScopeExitView`/recycle API | structured engine, suspension | retain names for lookup only; identity never reused; env does not accept a prepared transaction |
| `RuntimeFunctionValue.captures` | `Vec<RuntimeBinding>` and clone-based partial apply | same function owner with closure ID and `RuntimeCaptureBinding` | calls, closure capture, snapshot | replace exact-capture path; no whole-env fallback |
| `RuntimeIterator::Values` | current index + vector; classifier sees remainder | same enum/path-aware visitor | ownership, snapshot, diagnostics | absolute remainder index paths |
| sequence constructors/impls | repeat/get/slice/column storage can clone | parent checked Copy/Move rules + same visitor | structured/AWBC | replace clone paths; no sequence side model |
| `RuntimeCheckedType` | existing typed runtime check owner | retained type compatibility authority | slot endpoints and verifier | consume directly; no string type label |

## 3. HIR, sema, and runtime-plan projection

| Layer | Existing owner | Target output | Lifetime | Forbidden |
|---|---|---|---|---|
| HIR | typed `LocalId`, `CaptureId`, `LocalGeneration`, `HirScope` publication order | unchanged accepted HIR facts | accepted HIR world | exporting HIR IDs into core |
| sema | scope/capture resolution and typed access | checked one-to-one local/capture facts | semantic transaction | recovering free variables from source text |
| runtime-plan | lowering currently collapses some locals/captures to names | `RuntimeLocalDeclarationId`, `RuntimeCaptureSlotId`, typed capture/pattern destinations | immutable plan | second local registry, name hash identity |
| lowering map | new transient map in runtime-plan | `(LocalGeneration, LocalId) -> RuntimeLocalDeclarationId`; `CaptureId -> RuntimeCaptureSlotId` | build only, then discarded | serialize map or add core dependency on HIR |
| constants | parent checked unrestricted plan-value carrier | unchanged plus record-field IDs where materialized | plan/artifact | live affine value in cloneable plan |
| patterns | parent typed pattern/capture plan | typed destination declaration/capture IDs | plan | runtime name-only destination |

Projection allocation is deterministic declaration/capture-plan order. A plan
digest includes projected IDs, not HIR database-local IDs or source spans.

## 4. Structured engine owners/consumers

| Domain | Storage owner | Identity projection | Transaction role |
|---|---|---|---|
| scope/root locals | existing `RuntimeEnv`/`RuntimeScope` | scope instance + local slot | Copy lookup destination, Move binding, Drop scope exit |
| `let`/assignment | existing evaluator and binding owner | declaration ID -> fresh/reused slot as specified | assignment revision; pattern transaction |
| pattern binding | existing typed pattern executor | accepted destination declarations | one transaction; source consumed or copied under parent rule |
| closure creation | existing function evaluator | closure instance + capture slots | unrestricted captures Copy, affine captures Move |
| partial application | existing function value owner | retained closure/capture identity | no capture clone fallback |
| call/return | existing evaluator/frame owners | local/frame slot identities | typed argument/return transfers |
| suspension | existing suspension/snapshot owner | all IDs/revisions persisted | zero active transaction at externally visible safe point |
| unwind | existing cleanup path | cleanup scope/slots | canonical Drop transaction |
| child work | existing scheduler child owner | child instance/packet | Move/Drop through same transaction |
| mailbox | existing mailbox owner | mailbox instance/lane | Move handoff before receiver runnable |
| transfer packet | existing transfer owner | transfer instance/packet | cross-fiber Move |
| cleanup queue | existing cleanup owner | cleanup scope/slot | pre-stage request, then infallible Drop commit |

No structured-engine module may maintain a reduced owner enum or a map from
diagnostic strings to live values.

## 5. AWBC owners/consumers

| Current owner | Target addition | Consumer | Notes |
|---|---|---|---|
| `awbc::schema::AwbcRegisterId` | retained register coordinate | owned-slot evidence | no second register ID |
| AWBC function/frame layout | dynamic `RuntimeFrameInstanceId`; typed frame-local IDs | verifier/VM/snapshot | static register IDs remain artifact coordinates |
| verifier ownership facts | Copy/Move/Drop and join facts from parent | VM and compiled-region exchange | use complete slot enum |
| fiber state | dynamic fiber/frame occurrence IDs; slot cells/revisions | scheduler/snapshot | no ID from vector index |
| VM register storage | integrated `RuntimeSlotCell`/reservation semantics | transaction store | no independent transfer code |
| compiled-region boundary | typed moved/copied evidence and revisions | JIT/accelerator | parity with interpreter |
| AWBC snapshot | occurrence/register/frame-local identity and cursors | save/restore | no wire allocation in this correction |
| codec/verifier | no new opcode/tag/version | existing ABI/codec owner | identity is save/diagnostic state only in G1.2 |

Current production numeric ABI/codec values are evidence, not an allocation
authority for this correction. G1.2 must not create a provisional wire form.

## 6. Runtime-driver owners/consumers

| Current surface | Target owner | Action |
|---|---|---|
| session construction | `RuntimeExecutionDomain::prepare_new` + dormant session | make independently runnable construction private |
| in-place restore | validated dormant candidate + domain reservation | replace; no active mutation during validation |
| session Clone/duplicate image installation | non-Clone active/fresh owners | remove as active-execution authority |
| active session map per driver | one host-shared domain record | replace; no per-driver exclusivity claim |
| save | `RuntimeActiveExecution` only | include identity envelope/cursors |
| replay | preserved-ID fresh candidate | no parallel active replay |
| restart | active execution transition | preserve ID/cursors |
| hot replacement | `RuntimeActiveExecution::replace` | same ID + exact epoch; atomic |
| failed candidate cleanup | reservation `Drop` | exact matching reservation only |
| post-restore owner mint | persisted affine cursor | never recompute/guess |

Native/Web/Agent host wrappers receive the same active owner or driver façade.
They do not create a domain, parse rendered IDs, or supply execution identity.

## 7. Bundle/save/digest consumers

| Owner | Required change |
|---|---|
| parent save-schema-2 root | add one required identity envelope in place |
| canonical save decoder | strict identity/record/slot/evidence/cursor validation |
| canonical writer | u64 decimal strings in human-readable form; fixed LE binary |
| digest owner | add domain-separated identity section through inherent method |
| bundle product | carry accepted static declaration/capture/record identities where already part of runtime plan |
| restore candidate | rebuild indexes/capacity/reservations, preserve semantic IDs |
| replay record | include/validate execution identity; never allocate from text |
| hot-reload fingerprint | identity section affects state digest, not immutable program digest |
| old readers/writers | no compatibility path introduced by this correction |

## 8. Direct deletion/replacement inventory

### Core value/environment

- delete successful use of `RuntimeEnv::get_cloned`;
- delete `RuntimeEnv::bindings_snapshot` as live ownership/snapshot authority;
- delete `set_ref`, `bind_all_ref`, root-ref replacement, and clone-based
  rebinding paths;
- delete name-only assignment/mutation owner lookup after typed resolution;
- delete public `RuntimeBinding { name, value }` literals;
- delete public `RuntimeFieldValue { name, value }` literals;
- delete unchecked nominal-record runtime construction;
- delete clone-based whole-environment closure capture;
- delete clone-based partial application of captured/argument values;
- delete any fake moved/dropped value constructed independently of a slot.

### Ownership transaction

- delete prepared Move/Drop methods accepting arbitrary `RuntimeValue`;
- delete fallible destination validation after a source take;
- delete owner/path re-scan after the first committed mutation;
- delete sidecar reservation maps;
- delete transaction ID reuse after error;
- delete reduced owner variants or wildcard “other” domain;
- delete slot revisions inferred from mutation count or vector generation.

### Identity/path

- delete name/span/debug/pointer identity;
- delete record path by field name or raw vector ordinal without typed ID;
- delete iterator remainder paths relative to the remaining suffix;
- delete raw public ID constructors and integer conversions;
- delete `Default` for nonzero identities;
- delete extension traits/free helpers that own enum ordering/rendering.

### Driver/persistence

- delete direct public runnable `BundleSession` construction;
- delete per-driver-only activation exclusivity;
- delete install-alongside-active restore/replay;
- delete restore-time affine cursor recomputation/guessing;
- delete live `RuntimeBinding`/`RuntimeValue` Serde as save authority;
- delete `Eq` from floating snapshots;
- delete optional identity sidecars and dual readers;
- delete activation identity supplied by host/request payload.

## 9. Retained non-identity data

The following remain usable for presentation/diagnostics but never decide
identity:

- binding/field diagnostic names;
- source spans and source maps;
- HIR local/capture IDs inside the accepted compiler world;
- public/debug labels;
- vector indices inside private prepared handles;
- display strings; and
- map/cache iteration order.

## 10. Structural acceptance

Implementation evidence must prove through typed APIs and dependency metadata,
not source-text symbol scans:

- core has no dependency on HIR/sema/runtime-plan/driver;
- one `RuntimeEnv`, one `RuntimeValue`, one `RuntimeOwnedSlotId`, and one
  transaction owner remain;
- all existing owner domains implement the sealed store protocol;
- no public raw/fake constructor exists;
- no dual save reader/writer exists; and
- old successful clone/name-only/active-session paths are unreachable.
