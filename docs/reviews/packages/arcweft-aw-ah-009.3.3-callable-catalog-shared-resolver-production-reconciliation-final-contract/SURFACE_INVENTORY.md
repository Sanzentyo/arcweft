# Surface inventory

This inventory is normative and exhaustive for the production call dispatch
observed at `9fd6ee8fb2814ff04dc7a3e4ef413b86b7f4ac4d`. “Validator” means
family-specific checking after the shared resolver has selected typed
candidates. No row authorizes a second name/method lookup.

## 1. Free-call dispatch order

| Priority | Current family | Target candidate | Schema source | Applicability and retained behavior |
|---:|---|---|---|---|
| side observation | user `#[fx]` definition validation | selected ordinary project candidate plus FX validation facts | project catalog + current `FxCatalog` | `FxCatalog::call_errors` remains a validation attachment and never independently resolves the call |
| 1 | closed `Fx.<member>` namespace | `Fx(FxCallableSignatureId)` | inherent `signature_schema` | unknown direct member is poisoned FX, checks args once, returns `Fx`, and cannot fall through |
| 2 | expected project enum variant | `EnumVariant(EnumVariantSignatureId)` | project enum facts + expected `TypeKind` | only exact expected nominal enum owner qualifies; payload checked structurally |
| 2 | `Ok`, `Err` | `Result(ResultConstructorKind)` | inherent schema instantiated from expected `Result` | without expected type, preserve placeholder side and poison; exact one positional payload diagnostics |
| 2 | `Some` | `Option(Some)` | inherent schema instantiated from expected `Option` | one positional payload; expected inner type when available |
| 3 | builtins/capability | `Builtin(BuiltinCallableId)` | inherent schemas | language-reserved before lexical/project/environment |
| 4 | Agent intrinsics | `Agent(AgentIntrinsicSignatureId)` | inherent schemas | language-reserved; exact effects/entity/resource/value-shape behavior retained |
| 5 | presentation | `Presentation(PresentationCallableId)` | inherent schema + typed character owner facts | language-reserved; dynamic look expectation and current presentation state mutations retained |
| 6 | lexical callable/function value/non-callable | `Local`, `FunctionValue`, or non-callable target | lexical scope snapshot | exact local binding stops project/environment fallback; function values carry current group/effects |
| 7 | project callable/non-callable | `Project(CallableDeclarationId)` or non-callable target | immutable project catalog + `ProjectSymbolTable` | exact canonical scope/import resolution; non-callable stops environment lookup |
| 8 | accepted Standard/Adapter function/Rust function | `Environment(EnvironmentCallableId)` | immutable environment catalog | exact duplicates coalesce; differing schemas overload; Standard wins equal tie |
| 9 | well-known virtual runtime path | catalog/environment candidate with virtual-path validator | normalized standard publication | prohibited absolute path is diagnostic/poison, not resolver fallback |
| 10 | speaker/speaker-preset callable value | `Speaker(SpeakerCallableId)` | inherent schema | checks current untyped args once and returns speaker-preset type |
| 10 | evaluated function value/curried value | `FunctionValue` or `Curried` | exact function `TypeKind` and source schema | fixed spread, current group, partial-call context, return/effects retained |
| 11 | unknown/evaluated non-callable | missing or non-callable facts | checker expression judgment | args checked once for recovery; no fabricated signature |

## 2. FX constructors

| Typed path | Candidate ID | Parameters/validation | Result |
|---|---|---|---|
| `Fx.style` | `Fx::Style` | named FX properties; short variants accepted as current closed values | `Fx` |
| `Fx.text` | `Fx::Text` | named FX properties | `Fx` |
| `Fx.color` | `Fx::Color` | named FX properties; color/tint expectations preserved | `Fx` |
| `Fx.transform` | `Fx::Transform` | named FX properties | `Fx` |
| `Fx.mask` | `Fx::Mask` | named FX properties | `Fx` |
| `Fx.filter` | `Fx::Filter` | named FX properties | `Fx` |
| `Fx.shader` | `Fx::Shader` | first resource may be positional; remaining properties named; other positional/spread diagnosed | `Fx` |
| `Fx.transition` | `Fx::Transition` | named FX properties | `Fx` |
| `Fx.conditional` | `Fx::Conditional` | required named `condition: Bool`, `then: Fx`, `else: Fx`; no positional/spread | `Fx` |
| `Fx.stack` | `Fx::Stack` | exactly one positional ordered FX list; each child expected `Fx` | `Fx` |
| unknown direct `Fx.x` | poisoned `Fx` family | existing unknown constructor diagnostic; all args recovery-checked | `Fx` |

`sample` property retains expected `fn(FxSampleContext) -> Transform2D`;
`color`, `tint`, and `outline_color` retain expected `Color`. No property name
is parsed into identity; the FX validator receives typed authored argument names.

## 3. Builtin free calls

| Path(s) | Candidate ID | Exact input rule | Result |
|---|---|---|---|
| `fallback`, `InlineFailure.fallback` | `InlineFailureFallback` | current unchecked/recovery behavior | `InlineFailure` |
| `panic` | `Panic` | all args checked without imposed type | `Never` |
| `fail` | `Fail` | all args checked without imposed type | `Never` |
| `bail` | `Bail` | all args checked without imposed type | `Never` |
| `ensure` | `Ensure` | first arg required `Bool`; remaining checked | `Unit` |
| `assert` | `Assert` | first arg required `Bool`; remaining checked | `Unit` |
| `debug_assert` | `DebugAssert` | first arg required `Bool`; remaining checked | `Unit` |
| `rgb` | `Rgb` | exactly one positional `String` | `Color` |
| `sin` | `Sin` | exactly one positional `F32` | `F32` |
| `cos` | `Cos` | exactly one positional `F32` | `F32` |
| `vec2` | `Vector::Two` | exactly two positional `F32` | `Vec2` |
| `vec3` | `Vector::Three` | exactly three positional `F32` | `Vec3` |
| `vec4` | `Vector::Four` | exactly four positional `F32` | `Vec4` |
| `math.matmul_f32` | `Math::MatMulF32` | two positional `MatrixF32` | `MatrixF32` |
| `math.matrix_add_f32` | `Math::MatrixAddF32` | two positional `MatrixF32` | `MatrixF32` |
| `math.matmul_f64` | `Math::MatMulF64` | two positional `MatrixF64` | `MatrixF64` |
| `math.matrix_add_f64` | `Math::MatrixAddF64` | two positional `MatrixF64` | `MatrixF64` |
| `math.tensor_add_f32` | `Math::TensorAddF32` | two positional `TensorF32` | `TensorF32` |
| `math.tensor_add_f64` | `Math::TensorAddF64` | two positional `TensorF64` | `TensorF64` |
| `event.emit` | `Capability::EventEmit` | first capability prefix argument unchecked; remaining current signature behavior | current environment result |

### 3.1 `std.f32` and `std.f64`

| Operations | Input/output rule |
|---|---|
| `abs`, `floor`, `ceil`, `round`, `trunc`, `fract`, `sqrt`, `sin`, `cos`, `tan`, `exp`, `exp2`, `ln`, `log2`, `log10` | one width value, same-width result |
| `powf`, `atan2` | two width values, same-width result |
| `mul_add` | three width values, same-width result |
| `is_nan`, `is_infinite`, `is_finite`, `is_sign_positive`, `is_sign_negative` | one width value, `Bool` result |
| `to_bits` | one width value, `U32` for F32 or `U64` for F64 |
| `from_bits` | one `U32`/`U64`, width result |
| `std.f32.to_f64` | one `F32`, `F64` result |
| `std.f64.to_f32` | one `F64`, `F32` result |

All arguments are positional; named and spread forms retain existing diagnostics.
Unsupported conversion ID pairs cannot be constructed.

## 4. Agent intrinsic free calls

| Path | Candidate | Retained expected values and behavior |
|---|---|---|
| `expect` | `Expect` | required condition `Bool`, optional message `String`; Unit |
| `deny` | `Deny` | required condition `Bool`, optional message `String`; Unit |
| `checkpoint` | `Checkpoint` | one text/name `String`; existing effect; Unit |
| `note` | `Note` | one `DisplayText`; existing effect; Unit |
| `attach` | `Attach` | one closed attach-resource union; existing effect; Unit |
| `choice_action` | `ChoiceAction` | one `ChoiceOption` entity; `ActionTarget` |
| `viewport` | `Viewport` | no args; `CaptureTarget` |
| `layer` | `Layer` | one `Layer` entity; `CaptureTarget` |
| `object` | `Object` | one `ObservedObjectId`; `CaptureTarget` |
| `capture` | `Capture` | required `CaptureTarget`; optional named `name`, `format`/`kind`; existing effect; `AgentResult<CaptureRef>` |
| `read_resource` | `ReadResource` | current resource path and result typing/effect |
| `entity_meta` | `EntityMeta` | current entity metadata probe typing |
| `project_neighbors` | `ProjectNeighbors` | current project-neighbor query typing |
| `signal` | `Signal` | one Signal entity probe; current probe type |
| `metric` | `Metric` | one Metric entity probe; current probe type |
| `state_path` | `StatePath` | current path constructor rule; `DebugStatePath` |
| `observation_path` | `ObservationPath` | current path constructor rule; `ObservationFieldPath` |
| `state` | `State` | `DebugStatePath`; current state probe |
| `observation` | `Observation` | `ObservationFieldPath`; current observation probe |
| `diagnostics` | `Diagnostics` | no args; `Diagnostics` |
| `exists` | `Exists` | current probe existence rule; `Predicate` |
| `action_enabled` | `ActionEnabled` | current action target rule; `Predicate` |
| `all` | `All` | predicate sequence/list rule; `Predicate` |
| `any` | `Any` | predicate sequence/list rule; `Predicate` |
| `not` | `Not` | one predicate; `Predicate` |
| `wait` | `Wait` | current Agent wait arguments/effect/result |
| `advance_text` | `AdvanceText` | no args; existing effect; `AgentResult<ActionResult>` |
| `viewport_point` | `ViewportPoint` | required `x: U32`, `y: U32`, positional or named; `ViewportPoint` |
| `pointer.click` | `PointerClick` | required positional `ViewportPoint`; existing effect/result |
| `invoke` | `Invoke` | current named/positional Agent invoke mapping and environment parameter checks |
| `rag.query` | `RagQuery` | current RAG query/context mapping and effect/result |

The shared schema records ordinary parameter names and types. The Agent
validator retains resource/entity/path-specific checks, spread rejection,
missing/duplicate diagnostics, effects, and result construction.

## 5. Presentation free calls

| Call | Required positional/owner | Known named behavior | Open names | Result |
|---|---|---|---|---|
| `view` | View entity | `lifetime`, `target`, `layer`, `id`, `handle`, `key`, `mount`, `depth: I32`, `visible: Bool`, `enabled: Bool` | yes, unchecked | `Handle<View>` |
| `menu` | View entity | same as `view` | yes, unchecked | `Handle<Menu>` |
| `overlay` | View entity | same as `view` | yes, unchecked | `Handle<Overlay>` |
| `bg` | Asset entity | background slot/target/scope and current image-common fields | no | `Handle<BackgroundSurface>` |
| `image` | Image entity or Asset source | asset, lifetime, target/layer IDs, lifecycle, geometry, playback, proxy, `param.*`, `proxy.param.*` | only typed custom prefixes | `Handle<ImageSurface>` |
| `player_viewport` | current viewport positional forms | width/height dimensions and `fit` | no | `Handle<Viewport>` |
| `show` | Character entity | `look` structural owner, target, character slot, scope | yes, unchecked | `Handle<CharacterSurface>` |
| `ref.bg` | none required | background target/slot/scope family | yes, unchecked | `SlotRef<BackgroundSurface>` |
| `ref.show` | Character entity | target, character slot, scope | yes, unchecked | `SlotRef<CharacterSurface>` |
| `clear.bg` | none required | background target/slot/scope; removes active background default after commit | yes, unchecked | `Option<BackgroundSurface>` |
| `hide` | Character entity | target, character slot, scope; removes active character default after commit | yes, unchecked | `Option<CharacterSurface>` |

### 5.1 Structural look behavior

For `show`, the owner comes from the typed first positional or named character
argument. The second positional or named `look` argument has exact expected
`CharacterNominalType::Look { character }`. Canonical, compact, qualified, and
aliased owner spellings all resolve through typed symbol facts to the same
`CharacterId`. Identical local look spelling in another character, part, or
variant family cannot satisfy the expectation.

Missing owner, non-character owner, unknown external owner, and unknown-part
facts retain the presentation target but make only the affected expected type
unchecked, emit a typed diagnostic, and poison the target facts. No display
label or alias is parsed and no other character is searched by local spelling.

## 6. Dialogue calls

| Surface | Candidate | Owner source | Schema/retained behavior |
|---|---|---|---|
| speaker colon line | `Dialogue::SpeakerLine` | typed speaker entity/value | `LineOptions` schema; structural `look`; content tokens checked separately |
| speaker-preset line | `Dialogue::SpeakerLine` | typed speaker-preset value | same character identity and option mapping |
| accepted content call head | `Dialogue::ContentCall` | typed callee judgment | reserved `LineOptions` plus open authored `LineArg` names |
| content call without character owner | `Dialogue::ContentCall` poisoned | typed non-character/missing result | `look` unchecked, typed owner diagnostic, all option expressions checked once |

Reserved option order is `id`, `text_key`, `voice`, `look`, `stage`, `portrait`,
`focus`, `cleanup`, `view`, `source_locale`, `hooks`, `style`, `rich_text`, then
open authored line arguments. A repeated reserved/open name is diagnosed as a
duplicate. Dialogue content tags, FX spans, marks, waits, speed, rich text,
inline failure, and line-plan checks stay in their existing typed validators and
do not become callable identity.

## 7. Project and environment callables

| Family | Candidate | Publication source | Required retention |
|---|---|---|---|
| source function | `Project(CallableDeclarationId)` | `HirCallableSignatureSource` | canonical package/module/path, groups, names, defaults, rest, types, docs, spans, effects |
| other currently callable source declaration owner | `Project(CallableDeclarationId)` | same HIR publication | only owners already callable in production; no new syntax |
| module with no callable | no candidate, module row | `HirProject` module/source | retained for complete transaction and source identity |
| project non-callable binding | `NonCallable` target | `ProjectSymbolTable` | shadows environment callables after language-reserved families |
| core standard function | `Environment` owner `Standard(Core)` | sema core publication | typed schema instead of old map entry |
| standard adapter function/method | `Environment` owner known standard ID | accepted typed manifest | standard authority and exact metadata |
| selected adapter function/method | `Environment` owner `AdapterPackageId` | accepted typed manifest | adapter-only availability, overloads, effects/docs |
| typed Rust function | `Environment` kind `RustFunction` | accepted typed Rust metadata | manifest ID owner, exact Rust item path provenance, typed signature/docs |
| legacy untyped environment callable | `Environment` with `Untyped` validator | one-time normalized current inventory | args checked once without imposed type; removed old direct map read |

No source `impl` method is published as a project method in this cut. Existing
source trait/inherent method semantics remain under the trait catalog row below.

## 8. Path-call special forms and values

| Current behavior | Target candidate/fact | Retained result/validation |
|---|---|---|
| `promote` | `Promotion::Promote` | current promotion type/evidence and argument rule |
| `promote_unchecked` | `Promotion::PromoteUnchecked` | current unchecked promotion behavior and diagnostics |
| `assume` | `Promotion::Assume` | current assumption type/effect/evidence |
| character entity as speaker | `Speaker { preset: false }` | typed character speaker result |
| speaker preset callable | `Speaker { preset: true }` | untyped current args, preset result |
| local named function | `Local(LocalCallableId)` | exact local schema/effects/current scope |
| curried local/project/environment function | `Curried(CurriedCallableId)` | next group only; base candidate retained |
| known function-valued expression | `FunctionValue(FunctionValueSignatureId)` | exact `TypeKind::Function`, group, inferred args/result, higher-order effect callable |
| non-callable local/project expression | `NonCallable` facts | arguments checked once for recovery; no environment fallback |
| unknown path | `Missing` facts | existing unknown function diagnostic and recovery |
| virtual path | ordinary candidate + virtual validator | existing OS-absolute rejection and effect behavior |

## 9. Selected/method dispatch order

| Priority | Current family | Target candidate | Applicability and retained behavior |
|---:|---|---|---|
| 1 | drop-name special | `Drop::Drop` | all args recovery-checked; `Unit`; no further method lookup |
| 2 | `traverse` | `Domain::Traverse` | `Vec<T>` receiver; one capability-qualified task returning `Need`; connects effects |
| 2 | `parallel` | `Domain::Parallel` | current named `limit: I64`; `Need` receiver/result |
| 3 | accepted environment method | `Environment(EnvironmentCallableId)` | exact receiver key; typed or `Untyped`; shadows following families |
| 4 | collection | `CollectionMethodId` | closed method name consumes invalid receiver with current diagnostics |
| 5 | presentation handle | `PresentationHandleMethodId` | matching `Handle` receiver and lifecycle method |
| 6 | integer | `IntegerMethodId` | integer receiver and closed name |
| 7 | Arcweft domain | `DomainMethodId` | exact receiver-pattern family |
| 8 | well-known capacity | `CapacityMethodId` | current receiver/name/arity table; untyped args |
| 9 | trait | `TraitMethod(TraitCallableId)` or typed ambiguity | current visible predicate/inherent/unique/ambiguous behavior |
| 10 | data-last fallback | `DataLast(DataLastCallableId)` | visible callable whose final/current-next parameter accepts receiver |
| 11 | unknown | missing facts | args checked once and unknown method diagnostic |

Environment, closed inherent/domain, capacity, and trait success record whether
a viable data-last candidate was shadowed; checker commit emits the current
warning once.

## 10. Collection methods

| Method | Receiver | Arguments | Result/recovery |
|---|---|---|---|
| `len` | `String`, `Vec<T>`, `Seq<T>`, `Slice<T>`, `Array<T,N>` | none | `USize`; invalid receiver/extra args retain diagnostics |
| `map` | iterable sequence accepted by current checker | one function value | current item expected type, mapped collection result, higher-order effects |
| `filter` | iterable sequence | one predicate function | current item expected type, same collection-family result/effects |
| `sum` | integer-item iterable sequence | none | `I64`; non-integer item/invalid receiver diagnostic |
| `contains` | iterable sequence | exactly one positional item | `Bool`; item expected exact sequence item type |

The closed name row is selected before receiver validation, matching current
`BuiltinCollectionMethodCallOutcome::Checked`: an invalid `len`/`map`/`filter`/
`sum`/`contains` receiver does not fall through to trait or data-last resolution.

## 11. Presentation-handle methods

| Receiver/name | Candidate | Arguments | Result |
|---|---|---|---|
| any `Handle<name>.show` | `Show` | none | `Unit` |
| any handle `.hide` | `Hide` | none | `Unit` |
| any handle `.unmount` | `Unmount` | none | `Unit` |
| any handle `.release` | `Release` | none | `Unit` |
| any handle `.destroy` | `Destroy` | none | `Unit` |
| `Handle<Overlay>.pop` | `OverlayPop` | none | `Unit` |

Extra arguments retain the current no-arg diagnostic and are checked once.

## 12. Integer methods

| Method | Arity | Expected arguments | Result |
|---|---:|---|---|
| `min` | 1 | same exact integer receiver type, positional | receiver type |
| `max` | 1 | same exact integer receiver type, positional | receiver type |
| `clamp` | 2 | same exact integer receiver type, positional | receiver type |

Wrong arity still checks supplied arguments with receiver expectation. Named or
spread arguments retain the current positional diagnostic.

## 13. Arcweft domain methods

| Receiver and method | Candidate | Arguments | Result/effects |
|---|---|---|---|
| `FxSampleContext.ordinal_phase` | `FxSampleOrdinalPhase` | none | `F32` |
| `Vec<ObservedObject>.require_role` | `ObservedObjectRequireRole` | one positional `String` | `AgentResult<ObservedObject>` |
| `Map<K,V>.get` | `MapGet { K, V }` | one positional `K` | current production value result `V` |
| `Probe<T>.eq` | `ProbeCompare::Eq` | one positional `T` | `Predicate` |
| `Probe<T>.ne` | `ProbeCompare::Ne` | one positional `T` | `Predicate` |
| `Probe<T>.not_eq` | `ProbeCompare::NotEq` | one positional `T` | `Predicate` |
| `Probe<T>.gt` | `ProbeCompare::Gt` | one positional `T` | `Predicate` |
| `Probe<T>.greater` | `ProbeCompare::Greater` | one positional `T` | `Predicate` |
| `Probe<T>.ge` | `ProbeCompare::Ge` | one positional `T` | `Predicate` |
| `Probe<T>.greater_or_equal` | `ProbeCompare::GreaterOrEqual` | one positional `T` | `Predicate` |
| `Probe<T>.lt` | `ProbeCompare::Lt` | one positional `T` | `Predicate` |
| `Probe<T>.less` | `ProbeCompare::Less` | one positional `T` | `Predicate` |
| `Probe<T>.le` | `ProbeCompare::Le` | one positional `T` | `Predicate` |
| `Probe<T>.less_or_equal` | `ProbeCompare::LessOrEqual` | one positional `T` | `Predicate` |
| `Diagnostics.has_error` | `DiagnosticsHasError` | none | `Predicate` |
| `RagContextPack.summary` | `RagContextPackSummary` | none | `DisplayText` |
| `Need<...>.context` | `Context` | current untyped args | same Need shape |
| `Need<...>.with_context` | `WithContext` | current untyped args | same Need shape |
| `Option<T>.context` | `Context` | current untyped args | `Result<T, ArcError>` |
| `Option<T>.with_context` | `WithContext` | current untyped args | `Result<T, ArcError>` |
| `Result<T,E>.context` | `Context` | current untyped args | `Result<T, ArcError>` |
| `Result<T,E>.with_context` | `WithContext` | current untyped args | `Result<T, ArcError>` |
| character speaker `.face` | `CharacterFace` | current untyped args | `CharacterPatch(Character)` |
| character speaker `.say` | `CharacterSay` | current untyped args | `SpeakerPreset(Character)` |
| `Vec<T>.traverse` | `Traverse` | one capability-qualified task | `Need<Vec<R>, E>` and task effects |
| `Need<Vec<T>,E>.parallel` | `Parallel` | exactly named `limit: I64` | same Need shape |

For `Map.get`, this contract intentionally preserves current checker result
behavior rather than changing it to `Option<V>`. Result changes belong to a
separate language design.

## 14. Trait methods

| Existing trait outcome | Target behavior |
|---|---|
| missing | continue to data-last |
| inherent trait-catalog method | one `TraitCallableId { source: Inherent }`; resolve projections; check common signature |
| one visible trait method | one `TraitCallableId { source: Predicate }`; resolve projections; check common signature |
| multiple visible methods | `AmbiguousTraitMethod`; arguments checked once in untyped poisoned recovery; stop |

The candidate ID owns typed trait path, method name, deterministic implementation
index, and source kind. Display trait name is not parsed. Active trait predicates
are borrowed by `CallResolverRequest`. A trait ambiguity cannot be broken by
insertion order, adapter authority, or data-last fallback.

## 15. Data-last fallback

Visibility and shadowing are exact:

1. lexical callable;
2. project callable;
3. Standard environment callable;
4. Adapter environment callable.

A candidate is applicable when the receiver is compatible with either:

- the final non-rest parameter of the current call group; or
- the sole parameter of the next curried group.

The injected receiver is represented by `DataLastCallableId` and
`CallableInstantiation::DataLast`. Authored arguments map to the remaining
parameters. Direct/inherent/capacity/trait success takes precedence and emits a
single shadow warning when a viable data-last candidate exists. Exact duplicate
sources coalesce. Multiple equally viable same-authority callables are
`DataLastAmbiguity`. Effects, fixed literal spread, partial groups, and
higher-order function arguments use the same ordinary candidate checker.

## 16. Capacity methods

Every existing `well_known_capacity_method_type(receiver, name, arity)` row is
normalized at compile time into an inherent sema table that returns:

```text
CapacityMethodId { receiver, method, arity }
CallableSignatureSchema {
    parameters: arity unchecked positional parameters,
    result: current table result,
    validator: Capacity,
}
```

The current table is the sole row source; the migration adds inherent typed
record construction to its owning enum/table rather than duplicating spellings
in a helper. Capacity methods remain before trait and data-last, use exact
argument count in identity, check arguments once without imposed types, and
emit the current data-last shadow warning.

## 17. Environment-method precedence cases

The following collisions are mandatory direct tests because the current order
is result-changing:

| Collision | Selected target |
|---|---|
| environment method vs collection `len` | environment |
| environment method vs presentation-handle lifecycle | environment |
| environment method vs integer `min`/`max`/`clamp` | environment |
| environment method vs domain `get`/probe/context/etc. | environment |
| environment method vs capacity | environment |
| environment method vs trait | environment |
| environment method vs data-last | environment + one shadow warning |
| `traverse`/`parallel` vs environment method | domain `traverse`/`parallel` |
| collection/domain/capacity vs trait | earlier inherent family |
| trait unique vs data-last | trait + one shadow warning |
| trait ambiguous vs data-last | trait ambiguity; no fallback |

## 18. Function values, curried groups, and effects

| Case | Candidate/facts | Retained behavior |
|---|---|---|
| direct fixed function value | `FunctionValue` | positional checks against exact function input types; exact result |
| named argument to ordinary function value | same candidate, poisoned | current rejection; value checked once |
| fixed literal spread | same candidate | map expanded fixed items under existing rule |
| non-fixed spread | same candidate, poisoned | current unsupported spread diagnostic |
| first group of curried source function | base candidate + `Curried` instantiation | return function type for remaining groups |
| subsequent curried invocation | `Curried(CurriedCallableId)` | current group only; base identity retained |
| partial call in permitted context | selected candidate, `next_group` facts | existing partial result/effects |
| higher-order argument | ordinary selected candidate | exact function expectation and effect callable connection after commit |
| function returning effectful function | target facts carry function-value type/effect callable | existing delayed effect propagation |

Rejected overload checkpoints do not leak effects, warnings, borrow judgments,
typed-lowering evidence, presentation state, or target facts.

## 19. Ambiguity and recovery outcomes

| Condition | Resolver/checker outcome |
|---|---|
| exact Standard/Adapter semantic duplicate | one primary Standard candidate; Adapter ID/provenance equivalent |
| differing Standard/Adapter overloads | both candidates ordered; viability/specificity then Standard tie-break |
| same key from two Standard providers | accepted-world `SameRankCollision` |
| same key from two Adapter providers | accepted-world `SameRankCollision` |
| duplicate typed environment ID | accepted-world `DuplicateTypedId` |
| duplicate project declaration ID/path binding | accepted-world typed rejection |
| equal viable overloads in one provider | query `AmbiguousOverload` |
| trait ambiguity | query `AmbiguousTraitMethod`; stop |
| data-last ambiguity | query `DataLastAmbiguity` |
| corrupt non-empty set/key/order/by-ID relation | query `CorruptCatalog`; fail closed |
| missing character owner for `look` | family selected, unchecked affected arg, typed diagnostic, recovered poison |
| non-callable lexical/project shadow | non-callable facts; no environment fallback |
| unknown callable/method | missing facts and existing recovery diagnostics |
| cancellation/work/limit | typed query error; no partial fact/help/cache publication |

## 20. Explicitly excluded surfaces

- Source `impl` methods are not synthesized into the project catalog; existing
  trait catalog behavior is the only current authority.
- No CSS/Takumi, removed syntax, compatibility reader, deprecated API,
  signature-only resolver, source-text search, display-label parser, or Rust
  display-path parser is introduced.
- AW-AH-009.3.1 decides exact source/cursor ranges. AW-AH-009.3.2 decides
  accepted-HIR leases and stale request lifecycle. This inventory consumes their
  results but selects neither carrier.
