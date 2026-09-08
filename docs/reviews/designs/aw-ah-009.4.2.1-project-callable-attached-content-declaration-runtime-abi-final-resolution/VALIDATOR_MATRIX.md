# Validator matrix

## Declaration owners

| Owner | Required | Optional | Defaulted | Additional rule |
|---|---:|---:|---:|---|
| Ordinary item `fn` | admit | admit | admit | terminal result exact standard `DialogueContent` |
| Inherent method | admit | admit | admit | terminal result exact standard `DialogueContent` |
| Trait requirement | admit | admit | reject | interface owns role/presence; no concrete default prologue owner |
| Trait implementation | admit | admit | reject | role/presence must equal requirement; binding name is local |
| Extern-capability function | admit | admit | reject | host ABI receives required value or option |
| `#[fx]` function | reject | reject | reject | compile-time graph declaration |
| Flow / Predicate / Proof / View | reject | reject | reject | not an ordinary content callable signature |
| Any non-callable item | reject | reject | reject | wrong owner family |

## Source shape

| Shape | Result |
|---|---|
| One trailing `[body: InlineContent]` after all `()` groups | required Inline contract |
| One trailing `[body?: RichContent]` | optional Rich contract |
| One trailing `[body: DialogueContent = expr]` | defaulted Dialogue contract |
| Both `?` and `=` | reject |
| More than one trailing bracket parameter | reject |
| Bracket before a remaining `()` group | reject |
| Destructuring, discard, rest, receiver, or missing binding | reject |
| Qualified/aliased/string role | reject |
| Item attribute or reserved ordinary parameter name | no attached contract |
| Missing bracket/colon/role/default expression | typed recovery; non-executable declaration |

## Schema issuance

| Validator/owner | Execution | Policy | Result |
|---|---|---|---|
| Project declaration with accepted HIR attached parameter | RuntimeContent | Declared(exact role) | admit |
| Project declaration without HIR attached parameter | none | none | admit ordinary schema |
| Project RuntimeContent with Preserve/InlineOnly/RichOnly/LiteralOnly | any | non-Declared | reject schema |
| Project Structural | Structural | any | reject schema |
| Presentation/text-proxy exact catalog identity | Structural | exact catalog policy | admit |
| Structural row not equal to catalog identity/dependency | Structural | any | reject schema |
| Attached project row on non-terminal group | RuntimeContent | Declared | reject schema |
| Attached project row whose terminal result is not exact standard `DialogueContent` | RuntimeContent | Declared | reject declaration/schema |

## Call site, presence, and operand

| Site | Body | Selected schema | Prepared/checked result |
|---|---|---|---|
| Ordinary HIR call, terminal group | absent | none | no attached operand |
| Ordinary HIR call, terminal group | absent | Required | reject candidate |
| Ordinary HIR call, terminal group | absent | Optional/Defaulted RuntimeContent | RuntimeOmitted; pass None |
| ContentCall | absent | none | no attached operand; outer result must prove `DialogueContent` |
| ContentCall | present | none | reject candidate |
| ContentCall | absent | Required | reject candidate |
| ContentCall | absent | Optional/Defaulted RuntimeContent | RuntimeOmitted; pass None |
| ContentCall | present | RuntimeContent | RuntimePresent with exact raw/stable source and ABI position |
| ContentCall | present | Structural catalog row | StructuralPresent; no ABI position |
| DialogueLine | line-owned semantic content | not consulted by this channel | validate exact selected family/HIR owner; `Accepted(None)`; dialogue application authority owns the semantic operand |
| Any earlier curried group | absent | terminal attached row | ignore attached contract; produce continuation |
| Any earlier curried group | present | terminal attached row | reject site/group mismatch |

## C1 relation checks

Every successful row requires all of:

| Check | Failure class |
|---|---|
| selected schema digest/group equals prepared mapping | internal invariant |
| HIR site family and owner equal checked call site | internal invariant |
| present `HirDialogueContentId.owner()` equals application expression | internal invariant |
| stable outer coordinate belongs to the same application | internal invariant |
| schema/body presence pair is admitted | candidate rejection for authored mismatch |
| execution family agrees with ABI-position presence | internal invariant |
| receiver + ordinary + attached runtime positions are contiguous and unique | internal invariant |
| raw IDs are absent from stable digest input | digest invariant test |

The `DialogueLine` dispatch check precedes attached schema/body pairing. A
family or HIR-owner mismatch is an invariant failure; an exact line is not an
attached-body candidate and therefore cannot become `MalformedMapperSeal` or
`RuntimeOmitted`.

## C2 role admission

| Admission | Allowed content tokens |
|---|---|
| Inline | text, escape, interpolation, Ruby, nested admitted content calls |
| Rich | Inline plus line/paragraph hard breaks and the typed hard-break control |
| Dialogue | Rich plus page/wait/state controls, marks, and timeline host calls |
| Literal | exactly the opaque raw-literal body; no parsed child token |

Nested policies resolve as follows:

| Policy | Final child admission |
|---|---|
| PreserveBodyRole | enclosing report's checked role |
| InlineOnly | Inline |
| RichOnly | Rich |
| LiteralOnly | Literal |
| Declared(role) | exact declared role |

The final report stores this admission. No consumer may default an unresolved
policy to Rich.

## Runtime ABI

| Presence/execution | ABI slot | Caller value | Callee binding |
|---|---|---|---|
| Structural present | none | none | none |
| Runtime Required present | final `DialogueContent` slot | content envelope | `DialogueContent` |
| Runtime Optional present | final `Option<DialogueContent>` slot | Some(envelope) | `Option<DialogueContent>` |
| Runtime Optional omitted | same option slot | None | `Option<DialogueContent>` |
| Runtime Defaulted present | final `Option<DialogueContent>` slot | Some(envelope) | unwrapped `DialogueContent` |
| Runtime Defaulted omitted | same option slot | None | checked default expression result |

## Default transcript and final interface seal

| Condition | Result |
|---|---|
| all callable/default effects closed through the final SCC graph | admit resolution draft |
| draft contains checked IDs, schemas, execution, and exposed rows but no interface digest | admit call/default sealing |
| project-callable default leaf commits checked declaration, schema, execution, and exposed row | admit acyclic transcript leaf |
| checked application commits exact generic/type/effect solution and result | admit instantiated default call |
| default transcript reads an interface digest or referenced body | reject seal as a recursive authority |
| any preliminary/published draft interface digest exists | reject catalog state |
| complete checked default batch joins the draft once | atomically issue final catalog and interface digests |
| missing/extra/foreign default row or default on trait/impl/extern | reject entire interface seal |
| trait/impl structural attached rows differ in group/role/presence/ABI | reject implementation |
| trait/impl binding coordinate or spelling differs while structural row is equal | admit; binding identity is declaration-local |
| self/mutual/cross-module call edge resolves to an exact declaration leaf | admit without recursive digest expansion |
| RuntimeProjectCallable descriptor differs from final checked interface | reject projection atomically |
| Defaulted descriptor lacks default source/coordinate/digest or exact binding/option ABI types | reject callable fact |
| Entry duplicates declaration/owner/attached descriptor beside RuntimeProjectCallable | no admitted shape |
| helper derives attached ABI from a call site or first observed call | reject construction |
| Defaulted call loses its checked Option ABI, invokes default for present content, or passes raw Option to the target site | reject ProjectCall plan |

## Ordinary project callable execution

| Condition | Result |
|---|---|
| all reached callable/group sites reserved before any definition | admit SCC-safe site graph |
| equal callable instantiation joins at different call sites | one shared instance/site chain |
| distinct generic substitutions share declaration runtime ID | distinct instance/site chains |
| live continuation ABI stores RuntimeSemanticTypeId function/prefix identities | admit shared native/AWBC value |
| RuntimePlanTypeId or AwbcTypeId ordinal enters the live/snapshot continuation ABI | no admitted shape |
| native semantic type is absent/ambiguous in the plan table | reject continuation construction/admission |
| AWBC semantic type is absent/ambiguous or function identity names a non-Function row | reject VM/snapshot admission |
| site-specific application digest or first call selects monomorphization | reject producer |
| open generic body/local type reaches runtime-plan | reject instance publication |
| nonterminal group returns next group site and captures exactly prior supplied locals | admit partial callable |
| attached row appears before descriptor group | reject call fact |
| completed descriptor group lacks attached row or has another ABI position | reject call fact |
| final call drops completed group or attached ABI position | no admitted runtime fact shape |
| DirectFrame + ExpressionCompatible + NonSuspending + closed empty row | Expression site |
| DirectFrame + FlowRequired, MaySuspend, or nonempty closed row | Executable site |
| selected body/default/closure contains an ordinary project call but publishes ExpressionCompatible | reject semantic seal |
| runtime instance/default execution disagrees with checked control/suspension/effect evidence | reject runtime fact |
| source effect-clause spelling selects body kind | reject producer |
| project declaration lowers to RuntimePureHelper/PureCall | reject public switch |
| project call enters RuntimeExpr synchronous evaluator | reject execution |
| ApplyFunction arguments/result disagree with function type/site | reject before frame mutation |
| native call suspends | preserve same fiber and complete call-frame stack |
| ReturnExpr from callee | validate result, unwind cleanup, restore caller, bind atomically |
| AWBC ordinary project call | ProjectCall terminator with exact resume register and identical signature/effect set |
| default None branch executes outside terminal callable frame | reject plan |
| default/target return continuation stores exact ProjectCall caller-function/block site and stage | rejoin verified program and admit |
| return continuation clones ProjectCall or copies outcome/result/pattern/resume authority | no admitted shape |
| restored site does not resolve to the expected default/target function and resume cursor in the artifact-pinned program | reject snapshot before frame mutation |
| return-continuation snapshot serializes raw RuntimeValue instead of AwbcRuntimeValueSnapshot | no admitted snapshot shape |
| source operand row is sorted to ABI order or drops ABI destinations | reject fact/lowering |
| project materialization index addresses an ABI position rather than the source row | reject call fact |
| specialized call reads source expressions directly in ABI/role order | reject lowering; require shared ANF locals |

## Runtime fragment seal

| Condition | Result |
|---|---|
| one binding per template value slot in source order | admit |
| one program/capture row per effect site in source order | admit |
| missing/extra/duplicate/foreign/reordered value or effect row | reject whole fact batch |
| nested fragment ID/path collision | reject whole fact batch |
| duplicate generation-local fragment source or source mapped to another stable fragment ID | reject whole fact batch |
| effect operation retained without captures/program | reject |
| captures/program retained without template effect site | reject |
| capture locals/order/types differ from operation free-local inventory | reject whole fact batch |
| checked effect tail is not Closed, or runtime program set differs from the checked site set | reject whole fact batch |
| effect set inferred from operation/name/integer instead of copied from checked site | no producer API; reject projection |
| core template effect trigger/capture ABI differs from callback function site | reject plan/AWBC |
| content value effect site missing, duplicated, foreign, or reordered | reject value construction/materialization |
| runtime call omits RuntimeContent operand | reject plan |
| structural content receives an ABI operand | reject plan |
| Required ABI type is not exact DialogueContent | reject runtime call fact |
| Optional/Defaulted ABI type is not exact Option<DialogueContent> | reject runtime call fact |
| omitted runtime content lacks its normalized type or present source does not equal the option item | reject runtime call fact |

## Reveal callback executable

| Condition | Result |
|---|---|
| effect site references zero-ordinary-argument, Unit-result executable function site | admit callback binding |
| function captures exactly equal manifest capture ABI and construction values | admit atomically |
| executable body effect set exactly equals checked callback row | admit plan/AWBC signature |
| RuntimeFunctionEffectSet is not canonical typed EffectId order or AWBC table differs | reject construction/lowering |
| synthetic AWBC callback uses empty effect set for a nonempty body | reject lowering/verifier |
| callback function set and emitted effect-plan signature set differ | reject lowering/verifier |
| `intern_evaluated_effect` receives no typed enclosing set or infers one from the variant | no admitted producer API |
| content callback references expression-only function site | reject plan admission |
| callback body or capture row is missing/extra/foreign/reordered | reject before value/fiber publication |
| content construction reaches an effect site | snapshot callback only; do not execute |
| accepted reveal selects exact rebased site once | activate stored runtime function through scheduler |
| callback activation key differs in dialogue activation or rebased site | distinct activation; never alias |
| same callback activation key is replayed | reject before function/fiber allocation |
| structured reveal | enqueue executable function frame; never synchronous pure apply |
| AWBC reveal | use shared ApplyFunction activation validator, bind captures, then VM step |
| callback emits effect | ordinary `VmObservation::Effect` through `EmitEffect` |
| restored AWBC callback | admit through the same function snapshot/activation authority |
| duplicate reveal activation, foreign program, or nonzero remaining arity | reject transaction before frame/observation publication |
| line-task graph retains a ContentEffect action/schedule for a callback-backed site | reject public switch as double execution |
