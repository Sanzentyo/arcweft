# Complete consumer and deletion inventory

The request baseline is `177ba1e61e43fb2da2149869ce35e165d1e93b66`. The request directly reports 322 `arcweft-core` compile errors when direct value/closure/aggregate `Clone` is removed; this is a real migration inventory, not a reason to preserve cloning. This artifact had no local checkout of that commit. Exact baseline paths come from the request and predecessor evidence; targeted moving raw-main files were additionally inspected and still exhibit the listed clone surfaces. Stage 0 must record the full then-current Git SHA and re-pin renamed/split files without changing the selected result.

Legend:

- **Migrate**: use final generic owner APIs.
- **Delete**: old successful path/derive/helper is absent at final state.
- **Retain**: behavior/owner remains, with typed integration only.
- **Verify path**: owner is required but the exact file may have moved after the evidence snapshot.

## 1. `arcweft-core::value` and generic carriers

| Path/owner | Current/request-evidenced clone or fan-out seam | Final action | Deletion/acceptance evidence |
|---|---|---|---|
| `crates/arcweft-core/src/value.rs::RuntimeValue` | derives `Clone`; generic operations rely on it | Migrate to inherent `ownership`, checked duplication, payload/snapshot/constant eligibility | Delete executable `Clone` at G3; compile-fail public API test |
| `RuntimePayload` in `value.rs` | current tuple wrapper directly owns `RuntimeValue`, so its derived `Clone` duplicates the executable graph | Replace the existing owner/name in place with the exact closed non-runnable enum; retain `Clone`/Serde only on that closed data algebra | Delete the tuple wrapper and every `From<RuntimeValue>`/opaque escape; prove no function/handle/token/iterator/reference/continuation/table variant |
| `RuntimeBinding` | derives `Clone` | store one `RuntimeValueSlot`; explicit copy/move | Delete Clone and binding-copy helpers |
| current `RuntimeFunctionValue` struct | one closure struct; clone-based partial apply | replace in place with accepted two-variant enum | Delete old struct-only constructors/matches |
| `RuntimeClosureValue` | captures cloned bindings/full environment | exact `RuntimeCapturePlan`; owned capture slots | Delete ambient snapshot constructor/fallback |
| `RuntimeExternalStreamPartialFunction` (P4+C1) | parent assumed generic affine owner | private checked ownership cache; no Clone | No Stream-local sidecar/trait |
| tuple/record/variant aggregate carriers | derive/propagate Clone | constructors consume; projection copy checked; destructure consumes | Delete internal operand `.clone()` paths |
| existing `RuntimeSeq` and its tuple/record column carriers | repeat/get/slice/materialization clone values | Add the final ownership/materialization/repeat/index/slice/push/take inherent APIs directly to `RuntimeSeq`; no parallel sequence wrapper | Delete clone-based repeat/get/slice/materialization and compile-fail any invented `RuntimeSequenceValue` surface |
| `value::range` / sequence range (verify exact path) | may materialize/clone sequence/range values | numeric range cursor remains non-owning; sequence range follows exact copy rules | Direct exact-zero/one/one-over/bounds tests |
| `RuntimeIterator::Values` | `items.get(index).cloned()` | own `IntoIter<RuntimeValue>`/consuming storage | Delete index+clone cursor path |
| generic `RuntimeExpr::Value(RuntimeValue)` | live value embedded in cloneable/serializable plan | replace with `RuntimeExpr::Constant(RuntimeConstantId)` backed by checked `RuntimePlanConstant(RuntimePayload)` | Delete variant and all constructors/matches/direct RuntimePlan Serde paths |
| `FlowOp::{Bind, LoopNext, WhileNext, WhileLetNext, ForNext}` and `FlowFiber.pending_ops: VecDeque<FlowOp>` | plan enum and pending queue own live bindings/iterator or cloned continuation bodies | normalize original `RuntimeFlow` to block arena; existing `FlowCursor` addresses plan ops; existing `FlowControlStackEntryKind` owns live continuation/iterator; direct binding-plan commit into `RuntimeEnv` | Delete all five variants, pending op-value queue, recursive runtime body clones, and predecessor plan tags/readers |
| existing `RuntimePattern::Literal(RuntimeValue)` | literal matching and pattern plans retain a live cloneable runtime value | replace the original variant in place with `RuntimePattern::Literal(RuntimeConstantId)` and attach one typed `RuntimePatternBindingPlan` to the pattern/decision owner | Delete live pattern literals, literal-value cloning, and any global/copied pattern-plan registry |
| `RuntimeAffineOwnerToken` | absent | add one opaque generic leaf token owner | No public constructor/Clone/Serde; compile-fail tests |
| generic equality paths | may use derived `PartialEq` on runtime graph | typed borrowed equality evidence | Delete blanket language-equality use on executable values |
| generic debug/digest helpers | may clone to normalize/compare | borrow traversal or snapshot/payload projection | No debug-string identity/side table |

## 2. Structured engine, environment, pattern, suspension

| Path/owner | Current/request-evidenced seam | Final action | Delete/verify |
|---|---|---|---|
| `RuntimeEnv::bindings_snapshot()` in existing `value/env.rs` | closure construction clones the complete visible environment | Delete the method when its final caller migrates; diagnostics borrow/visit exact bindings instead of preserving a clone API | No executable or diagnostics fallback may keep a full-environment clone surface |
| environment lookup/get APIs | return cloned values | split borrow vs typed copy/move; slot state tracks moved/dropped | Delete clone-returning executable lookup |
| environment scope push/pop | clones bindings/snapshots | move owned slots; explicit cleanup stack | Verify reverse cleanup and failure digest |
| structured closure evaluator | captures ambient env | call `RuntimeEnv::try_capture_closure` with accepted plan | Delete source/name/free-variable reconstruction |
| structured ordinary call evaluator | clones callee/arguments/captures | consumes by-value inputs; explicit Copy for reuse | Delete `partially_apply` clone logic |
| structured external group application frame | cloneable callee/evaluated values | non-Clone owned slots across suspension | Delete clone/restart of prior expressions |
| structured suspension/save | clones runtime frame/env | owned frame cursors + snapshot DTO | Delete runnable snapshot clone |
| existing `crates/arcweft-core/src/pattern.rs::RuntimePattern` | `Literal(RuntimeValue)` embeds live plan data and binding/rest paths clone values | change the original literal variant to `RuntimeConstantId`; perform borrowed selection against the constant table, then run the directly attached transactional `RuntimePatternBindingPlan` | Delete live literal values, member clone/fan-out helpers, and any second/global binding-plan registry |
| `let`/parameter binding | copies RHS into bindings | evaluate once; move/copy by plan; no partial publication | Direct failure non-mutation tests |
| tuple/record/sequence patterns | clone selected/rest members | whole aggregate consume or checked borrowed copies | No public partially moved aggregate |
| match/if-let/while-let arms | may clone scrutinee/bindings | borrow selection, then exact arm transfer | Join/liveness tests |
| assignment/reassignment | clone replacement/old value | prepared atomic replace + old drop | Delete fallible `mem::replace` + later drop route |
| unwind/cancel cleanup | Rust drops cloned owners or incomplete stacks | explicit prepared drop/cleanup exactly once | No Rust destructor as language Drop |
| flow/thread child capture | prior evidence shows parent env clone | exact typed child capture packet | Delete `parent.env.clone()` and fake copy handoff |
| scheduler/fiber child queue integration | copied values/handles possible | atomic sender transfer + child/scope/observation | No ambient environment copy |

## 3. Runtime plan and compiler lowering

| Path/owner | Current seam | Final action | Delete/verify |
|---|---|---|---|
| `crates/arcweft-runtime-plan/src/function_values.rs` | nested/cloneable function expressions; no generic owner | consume accepted compiler projection and exact capture/group plans | No sema dependency or name lookup |
| existing `arcweft-core::plan` / runtime-plan expression, pattern, flow, and constant owners | live literals; runtime-only `FlowOp` continuations; derived RuntimePlan Clone/Serde; cache-held live data | expression/pattern IDs into one constant table; original `RuntimeFlow` block arena; `Arc<RuntimePlan>` cache owner; pattern comparisons borrow entries; expression instantiation creates fresh values | Delete both live literal variants, five runtime-only FlowOp variants, direct RuntimePlan Serde/Clone, and every live value/iterator/binding in plan/AOT/JIT cache state |
| existing `arcweft-core::engine::{Engine, FlowFiber, FlowControlStackEntryKind}` | `Engine`/fiber/status clone, pending cloned ops, Arc body clones, iterator embedded in `ForNext` | non-Clone Engine with `Arc<RuntimePlan>`; cursor-only program access; continuation and iterator on original control-frame enum; capture/binding transfers through original owners | Delete executable Clone, pending op-value queue, cloned body scheduling, ambient child env clone, and synthetic `*Next` op manufacture |
| runtime-plan aggregate construction | duplicates literal/operand values | emit consuming operands and explicit Copy operations | No hidden clone helper |
| runtime-plan closure lowering | runtime free-variable inference/ambient env possible | project accepted HIR `CaptureId`/LocalId/order/mode | Delete source scan/name reconstruction |
| runtime-plan ownership facts | absent | project closed type ownership/capture/operand-use facts | Static unknown/maybe-affine is not treated unrestricted |
| runtime-plan sequence repeat/index/slice lowering | runtime decides from actual count/value | emit `RuntimeRepeatPermission`; reject affine dynamic repeat; typed copy requirement | No data-dependent permission fallback |
| `crates/arcweft-compiler` accepted sema projection | compiler is valid sema->core boundary | one bounded projection of capture/group/constant/ownership evidence | No runtime-plan normal dependency on sema |
| compiler cache/AOT plan assembly | clones plans containing values | clone IDs/digests/Arc table only | No live `RuntimeValue` in cache key/value |
| compiler/AWBC lowering | implicit clone-like Move | emit `CopyValue` for reuse, consuming Move/operands | Golden instruction/state tests |
| source/HIR capture owners | accepted capture set/first-use order | retain; add typed ownership mode projection only | Do not redesign syntax/HIR IDs or rebuild from source |

## 4. AWBC schema, codec, verifier, VM, fiber

| Exact/known path | Current/request-evidenced seam | Final action | Delete/verify |
|---|---|---|---|
| `crates/arcweft-core/src/awbc/schema.rs` | ABI1/codec7 current evidence; instruction values not ownership-complete | protected ABI2/codec8 cut; one register state; `CopyValue=0x2a` | `0x2b..` unknown; no provisional enum |
| existing `Move` schema/semantics | VM clone-like move | consuming transition | Direct use-after-move tests |
| existing `Drop=0x1f` | may rely on Rust drop/clone state | table-aware prepared drop | Drop once/table reciprocity tests |
| `crates/arcweft-core/src/awbc/codec/code.rs` | strict codec current tail to 0x26 | add exact 0x2a encode/decode in owner | Golden bytes, unknown/noncanonical/trailing tests |
| `crates/arcweft-core/src/awbc/codec/wire.rs` | canonical primitives | Retain | Reuse exact varu32/u16 rules |
| `crates/arcweft-core/src/awbc/codec.rs` | codec7 strict reader | atomic codec8 replacement per parent | Delete codec7 acceptance in codec8 product; no dual reader |
| AWBC type/signature/frame tables | flat/current ownership incomplete | attach closed ownership facts to final layouts | No Stream-specific register schema |
| verifier structure/type pass (verify split path, predecessor names `verify/structure.rs`) | no complete move/copy/drop joins | propagate exact register/cleanup state | Reject mismatched joins/use-after-move/copy affine |
| verifier safe-point pass | may ignore in-flight ownership | require no transaction/borrow and complete slots | Snapshot/suspension gate tests |
| `crates/arcweft-core/src/awbc/vm.rs` | `Move` and operand reads clone values; flat ApplyFunction | shared slot/transfer/drop APIs; consuming calls/constructors/Stream ops | Delete `.clone()` execution paths |
| `crates/arcweft-core/src/awbc/fiber.rs` | fiber state owns clonable register values | non-Clone frame/register/fiber; owned snapshot DTO | Delete `FiberState::clone`/derived Clone and clone handoff |
| AWBC cleanup instructions | obligations may not track owner moves | transfer/discharge exact cleanup facts | No cancel-cleanup owner leak |
| AWBC branch/match/loop joins | value equality/clone reconciliation | exact liveness/type/ownership equality; explicit Drop on dead live edge | No implicit merge copy |
| AWBC spawn/child transfer | function captures/args cloned | typed capture packet + atomic child creation | Delete parent register/env clone |
| AWBC snapshot | runnable fiber cloned | strict snapshot DTO/dormant owner evidence | No token in wire DTO |
| AWBC tests/fixtures | direct Clone assertions/fixtures | use checked copy/move/test Stream authority | Compile-fail and parity tests |

## 5. Product-step facade and compiled-region exchange

| Owner/family | Current/request-evidenced seam | Final action | Delete/verify |
|---|---|---|---|
| product-step facade synchronization (`crates/arcweft-core/src/step.rs` and related) | compact/facade state rebuilt/synchronized by clone | one core owner; owned update/transfer packet | Delete duplicate runnable facade representation/rebuild clones |
| runtime accelerator/JIT (verify exact crate/modules) | may clone plan/fiber/value for execution/rollback | immutable plan sharing; non-Clone owned exchange | No cloned baseline `FiberState` |
| AOT compiled plans | plans contain cloned runtime constants/values | IDs + Arc checked constant table | Delete live value embedding |
| compiled region entry | copies registers into region | borrow unrestricted scalars or move owned values into tracked locations | Exact owner map at boundary |
| compiled region exit/deopt | clones state back to core | consume `RuntimeCompiledRegionExchange` | Validate complete candidate before atomic replace |
| trap rollback | relies on cloned pre-state | leave core pre-state untouched until exchange commit | Failure digest equality tests |
| product executable/snapshot facade | duplicate state surfaces | project from sole state without ownership | Delete synchronization authority |

## 6. Stream parent owners

| Path/owner | Parent authority | Final integration | Delete/verify |
|---|---|---|---|
| `arcweft-core::stream::StreamInstanceKey` | exact definition/generation/ordinal | Retain | No debug-string key |
| `StreamHandle` | unique key/layout/consumer lease; no Clone | add private generic token/accessors | No struct-literal/fake token public path |
| `StreamInstanceTable` | sole lifecycle/lease table | non-Clone live table; private token mint/drop/activation hooks | Snapshot DTO is separate |
| `RuntimeFunctionValue::ExternalStreamPartial` | sole partial variant | generic recursive ownership/no Clone | No side enum/extension trait |
| `RuntimeExternalStreamArgumentProduct` | sole grouped canonical product | owns cells by move; snapshot traversal exact | No flatten/rebuild/copy product |
| `.2` group application plan/frame | exact evaluation/default order | owned failure/slots and no Clone | Delete plain error losing owners |
| `RuntimeStreamOpenTransaction` | atomic Open table/request/handle | include private generic owner-ID allocation and non-fallible token/table/lease/handle/request commit after every fallible preparation succeeds; the token itself has no reservation state | No early token/lease/table mutation and no `ReservedForTransfer` authority state |
| Stream request/event/replay owners | parent exact behavior | payload-only host/replay, no live owner leakage | No endpoint DTO/handle codec |
| Stream snapshots/tombstones/pins | parent schema 2 | exact owner evidence/occurrence/pin traversal | No repair/lease rotation |

## 7. Runtime driver, save, swap, host

| Exact/known path | Current/request-evidenced seam | Final action | Delete/verify |
|---|---|---|---|
| `crates/arcweft-runtime-driver/src/session_save.rs` | current save recursively clones/validates runtime values; parent moves schema to 2 | whole-execution frozen projection, dormant evidence, exact tamper order | Delete live RuntimeValue clone/Serde route; no schema1 migration |
| `crates/arcweft-runtime-driver/src/swap.rs` | hot reload can stage/synchronize cloned state/facades | exclusive candidate/atomic replace, exact generation pins | Delete clone-based candidate/current facade sync |
| runtime-driver execution/session owner (verify path) | cloneable executor snapshots | non-Clone live execution; explicit snapshot guard | No alongside restore API |
| runtime-driver save blocker owner | lacks generic transaction/borrow/reciprocity blockers | add variants in original enum | No external helper enum |
| runtime host core request bridge | might convert generic runtime values | accept only core typed request + RuntimePayload | No live partial/handle/token |
| native adapter (verify exact crate/path) | may clone request/value DTO | serialize core data directly | Byte parity and no endpoint DTO |
| Web adapter | JS/Serde clone temptation | same core request/payload bytes | Structured-clone not language copy |
| Agent adapter | JSON DTO/flatten temptation | same core request/payload bytes | No Agent ownership model |
| headless/test host | fake handle/token fixtures | use Stream test authority and core request | No raw token constructor |
| event/replay bridge | may clone generic payload/runtime values | clone closed RuntimePayload only | No generic value tag |

## 8. Bundle, save crate, and codecs

| Owner/family | Current/parent seam | Final action | Delete/verify |
|---|---|---|---|
| `arcweft-bundle` schema 6 | immutable executable data | constants as checked table/IDs; no live state | No RuntimeValue/handle/token in bundle |
| bundle AWBC wrapper | canonical executable/verification | include `CopyValue=0x2a` under codec8 | No codec7/8 dual reader |
| bundle runtime summary | parent Stream definitions/profile | Retain | No owner occurrence in summary |
| canonical value codec | may serialize generic RuntimeValue | restrict to RuntimePayload/checked constants as owning boundary | Delete opaque generic value variant/tag |
| `arcweft-save` strict schema decoder | parent save2 | strict snapshot DTO only | No schema1 migration/repair |
| save canonical digest | old cloned state bytes | bind exact snapshot/evidence/pins | Tamper/canonical round-trip tests |
| replay codec | payload/lifecycle records | retain closed data only | No handle/partial/token snapshot tag |
| host JSON codec | parent wide integer/string rules | retain; grouped product/payload direct | No flattened endpoint arguments |

## 9. Test and public API consumers

| Consumer | Migration | Required proof |
|---|---|---|
| core unit tests constructing/cloning values | builders + checked duplication/owned moves | unrestricted positive; affine compile/runtime rejection |
| sequence/property tests | consuming iterator and exact repeat/get/slice | zero/one/one-over/bounds/empty affine slice |
| structured engine tests | exact capture plans/env digests | no ambient capture; failure non-mutation |
| AWBC golden/VM tests | CopyValue/consuming Move/Drop/joins | bytes + verifier + runtime parity |
| compiled/JIT tests | owned exchange | interpreter/compiled state digest parity |
| save/restore tamper tests | strict owner evidence/order | no duplicate runnable owner; failed restore atomic |
| host/native/Web/Agent tests | core request bytes | exact cross-target parity/no owner DTO |
| parent Lang-01.3 matrices | unchanged plus integration | all 803 predecessor rows retained |
| public API/trybuild tests | removed traits/constructors/variants | `RuntimeValue`/`RuntimeSeq`/`RuntimePattern` executable graph is not cloneable, handle/token not constructible, no live expression or pattern literal variant |
| Cargo metadata/structure tests | dependency/visibility | core/data Sans I/O; runtime-plan no sema; no new cycles |

## 10. Final deletion checklist

The implementation is incomplete while any of these remains reachable on a successful production path:

1. `Clone`/`Copy` on executable `RuntimeValue`, binding, function/closure/partial/handle, aggregate/sequence/iterator, env/register/frame/fiber/execution, ownership transaction, restore candidate, or live Stream table.
2. `RuntimeFunctionValue::partially_apply` or equivalent that clones captures/arguments.
3. executable closure construction through `RuntimeEnv::bindings_snapshot()` or full-environment clone.
4. `RuntimeIterator::Values` index plus `.cloned()`.
5. clone-based repeat/get/slice/pattern/rest/projection/call/return/assignment/cross-fiber paths.
6. AWBC `Move`/operand reads that clone, or a verifier that does not distinguish Live/Moved/Dropped and exact ownership.
7. product-step/compiled bridge that clones/rebuilds a second runnable fiber/environment/facade.
8. runnable snapshot/candidate creation by cloning live RuntimeValue/table/frame/fiber state.
9. direct generic RuntimeValue Serde through payload/host/replay/bundle/canonical data codecs.
10. `RuntimeExpr::Value(RuntimeValue)`, `RuntimePattern::Literal(RuntimeValue)`, direct derived RuntimePlan Serde/Clone, or AOT/JIT cache ownership of live values.
11. `FlowOp::Bind`, `LoopNext`, `WhileNext`, `WhileLetNext`, `ForNext`, `FlowFiber.pending_ops: VecDeque<FlowOp>`, cloned flow bodies, or executable `Engine`/fiber/status Clone.
11. raw/fake token/handle constructors or tests that make detached authority.
12. Stream-only value/register/environment/ownership sidecar, extension trait, debug-string registry, copied capture registry, source-text free-variable reconstruction.
13. compatibility aliases, dual readers, migration shims, endpoint DTOs, source gates, removed-syntax diagnostics, CSS, or Takumi paths.

Typed IDs, immutable plan/snapshot DTOs, `RuntimePlanConstant` closed data, and the closed `RuntimePayload` algebra may remain cloneable exactly as listed in `RUST_OWNERS_AND_APIS.md`.
