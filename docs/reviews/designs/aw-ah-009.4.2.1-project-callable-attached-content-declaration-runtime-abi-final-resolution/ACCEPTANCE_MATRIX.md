# Acceptance matrix

Every positive row must assert typed products, not only compilation success.
Every negative row must assert the owning diagnostic or invariant class.

## Syntax and HIR

| ID | Case | Required evidence |
|---|---|---|
| S1 | required Inline declaration | exact CST/attachment/HIR role, binding local, terminal group, source roles |
| S2 | optional Rich declaration | exact `?` source and Optional presence |
| S3 | defaulted Dialogue declaration | exact default `ExprId`, scope, expected type, source roles |
| S4 | curried function with trailing body parameter | body group belongs only to last callable group |
| S5 | missing close/colon/role/default | typed recovery; no executable HIR success |
| S6 | attribute and ordinary parameter lookalikes | no attached contract and no compatibility diagnostic |
| S7 | duplicate/misordered/destructured body parameter | closed declaration diagnostic |

## Schema and call sealing

| ID | Case | Required evidence |
|---|---|---|
| C1 | project required/optional/defaulted rows | terminal group + Declared role + RuntimeContent in schema digest |
| C2 | trait requirement/impl match | equal interface contract despite different local binding names |
| C3 | trait/extern defaulted declaration | deterministic rejection |
| C4 | `#plain_call()` returning DialogueContent, no body schema | ContentCall site accepted with no attached operand |
| C5 | `#plain_call()[body]`, no body schema | candidate rejected; no final call publication |
| C6 | declared required body present | exact raw ID + outer stable coordinate + final ABI position |
| C7 | required omitted | candidate rejection in probe and selected replay |
| C8 | optional/defaulted omitted | RuntimeOmitted and fixed option ABI slot |
| C9 | partial curried application | continuation with no premature omission/rank charge |
| C10 | tampered raw owner/stable coordinate/group/execution/position | C1 invariant rejection and atomic rollback |
| C11 | exact `DialogueLine` selected site | exact family/HIR owner admitted as no attached operand; line semantic content remains owned by dialogue application |
| C12 | `DialogueLine` family or HIR-owner mutation | invariant rejection before attached schema/body pairing; never `MalformedMapperSeal` for an exact line |
| C13 | required project row end to end | dedicated binding coordinate, exact `DialogueContent`, RuntimePresent, final ABI operand, callee row |
| C14 | optional project row present/omitted | dedicated binding coordinate, exact `Option<DialogueContent>`, Some/None at one ABI position |
| C15 | attached binding/default semantic roots | dedicated checked path steps; never ordinary parameter pattern/default coordinates |

## Default transcript and interface closure

| ID | Case | Required evidence |
|---|---|---|
| D1 | self-recursive default call | finite default transcript; one final interface digest; no preliminary digest |
| D2 | mutually recursive defaults | deterministic equal result independent of traversal/SCC member order |
| D3 | generic default call | checked application digest commits exact type/effect substitution and result |
| D4 | cross-module default call | exact checked declaration leaf; no public-name or source-string reconstruction |
| D5 | referenced callable exposed effect row changes | default digest and owning final interface digest change |
| D6 | referenced callable body changes behind an equal interface | caller default/interface digest remains equal |
| D7 | ordinary semantic transcript after final seal | project callable leaf commits the one final interface digest |
| D8 | attempted interface/default cycle mutation | final catalog seal rejects atomically; no catalog/digest publication |
| D9 | trait/impl attached join with different binding names | structural contract equality succeeds while local coordinates remain distinct |

## Role sealing and transcript

| ID | Case | Required evidence |
|---|---|---|
| R1 | Preserve nested at Dialogue root | child report admission Dialogue |
| R2 | Preserve nested under Ruby | child report admission Inline |
| R3 | Ruby body containing page/wait/mark/call | structured role diagnostic |
| R4 | Object Rich body containing Page | structured role diagnostic |
| R5 | Raw body containing bracket-looking bytes | one Literal report and opaque raw token |
| R6 | identical tokens sealed under Inline/Rich/Dialogue/Literal | four distinct v1 semantic digests |
| R7 | argument/report admission mismatch mutation | C2 seal rejection |

## Runtime fragments and effects

| ID | Case | Required evidence |
|---|---|---|
| F1 | body interpolation | one fragment value slot and one ordered binding program |
| F2 | nested ContentResult body | independent nested fragment; no discarded child values |
| F3 | nested `[call]` effect | effect site, runtime program, capture schema, and capture bindings retained |
| F4 | delay/content effect triggers | exact trigger preserved through AWBC and VM |
| F5 | missing/extra/reordered binding mutation | runtime-fact/plan rejection before publication |
| F6 | repeated equivalent fragments at distinct stable paths | distinct IDs without raw-ID digest input |
| F7 | zero/one/multiple free-local effect captures | exact deterministic free-local order/type and callback capture values |
| F8 | nested materialization rebases effects | effect sites and callback bindings rebase together without collision or loss |
| F9 | effectful callback plan site | executable function body, exact effect set, Unit result, zero ordinary parameters |
| F10 | expression-only callback-site mutation | plan admission rejects before content value construction |
| F11 | fragment source lookup mutation | duplicate/foreign source rejects; stable ID/digest is unchanged by generation-local IDs |
| F12 | operation preserved but checked effect set changed | sema transcript/runtime program/function set/AWBC signature all change together; no operation inference |

## ABI, AWBC, and VM

| ID | Case | Required evidence |
|---|---|---|
| A1 | required attached body | final caller operand and callee parameter are `DialogueContent` |
| A2 | optional present/absent | Some/None at the same final `Option<DialogueContent>` position |
| A3 | defaulted present | default expression is not evaluated |
| A4 | defaulted absent | default expression evaluates once in callee prologue; effects/captures exact |
| A5 | receiver plus ordinary args plus body | contiguous receiver/arg/body ABI order |
| A6 | structural presentation body | no runtime call operand |
| A7 | AWBC codec roundtrip | evolved v1 manifest/instruction/value/capture shape is isomorphic |
| A8 | verifier mutations | wrong slot/type/fragment/effect/capture rejected |
| A9 | VM reveal-time effect | stored effect executes at trigger, not construction; captures resolve exactly |
| A10 | AWBC session save/restore with effectful content | callback function and capture snapshot roundtrip through session-save authority |
| A11 | structured reveal-time effect | native scheduler starts executable function frame and emits exactly once |
| A12 | AWBC callback signature effect set | nonempty checked set becomes nonempty exact `AwbcEffectSetId`, never synthetic id 0 fallback |
| A12a | AWBC emitted effect-plan signature | same typed set as enclosing callback function; id-zero mismatch mutation rejects |
| A13 | AWBC reveal activation | shares ApplyFunction validation/frame binding and yields ordinary `VmObservation::Effect` |
| A14 | reveal activation mutation | foreign program, nonzero arity, wrong captures, duplicate site/event reject atomically |
| A15 | optional/defaulted omitted ABI | typed `None` uses retained exact `Option<DialogueContent>` without schema lookup |
| A16 | root/nested reveal switch | one callback observation per site and no line-task EvaluatedEffect action/delayed duplicate |
| A17 | activation identity | same rebased site in different dialogue activations is distinct; exact pair replay rejects before allocation |
| A18 | Defaulted ProjectCall materialization | checked Option ABI remains on call fact; present projects Content and omission invokes the default FunctionSite once; target receives only DialogueContent binding |
| A19 | entry/direct/value callable projection | all own the same RuntimeProjectCallable attached descriptor; no duplicate entry fields |
| A20 | self/mutually recursive project calls | all callable/group sites reserve before definition; no pure-helper recursion side path |
| A21 | curried project callable | each completed group selects the exact site; attached body exists only at descriptor terminal group/position |
| A22 | effectful default omitted | one terminal executable frame runs None prologue effect then authored body with the final closed effect set |
| A23 | effectful default present | Some bypasses default; authored body executes once in the same frame |
| A24 | native project call suspension | same fiber preserves call-frame continuation and binds exact result on resume |
| A25 | AWBC project call suspension/effect | ProjectCall terminator resumes once and verifier joins target/default function and effect-plan sets |
| A26 | pure project call in a body/default/closure | caller is FlowRequired and uses Executable FunctionSite even when effects are empty and suspension is NonSuspending |
| A27 | raw effect-clause selector mutation | no effect; body family follows final checked execution/control/suspension/effect row |
| A28 | equal generic instantiation at two sites | one callable-instance digest and one group-site chain; application coordinates remain distinct |
| A29 | two substitutions of one declaration | distinct instantiated ABI/body facts and distinct group-site chains |
| A30 | named arguments in reverse ABI order | source row and observable evaluation remain authored order; derived ABI destinations bind the correct parameters |
| A31 | rest/spread project call | every physical source evaluates once; materialization indexes the source row and packs one logical Vec binding |
| A32 | specialized named call | shared ANF locals preserve source evaluation order while target payload reads ABI/role order |
| A33 | native/AWBC continuation ABI equivalence | both retain identical RuntimeSemanticTypeId function/prefix rows despite unrelated local table ordinals |
| A34 | AWBC continuation save/restore | snapshot stores semantic type identities and rejects a program missing or changing any exact row |
| A35 | default suspends then returns | frame snapshot retains site+stage+once-evaluated values; restore rejoins verified terminator and invokes target once |
| A36 | ProjectCall continuation mutation | copied/foreign site, wrong stage/function/resume, or altered artifact rejects before caller/frame mutation |

## Compiler and tooling

| ID | Case | Required evidence |
|---|---|---|
| T1 | compiler nested ContentResult | no `_child_values`, dropped `child_effects`, or separate effect-site scan |
| T2 | signature help/hover | trailing binding, role, and presence displayed after ordinary groups |
| T3 | formatter | canonical required/optional/defaulted bracket spelling roundtrips |
| T4 | LSP invalid role/presence/default | exact attached parameter/body source diagnostic |
| T5 | full current compiler/LSP fixtures | no regression in presentation, Object, CharacterDialogue, evaluated effects, or nominal inventory |

## Required test tiers

Run and record, without an explicit Cargo job count unless intentionally
coordinating independent commands:

```text
targeted syntax/attachment/HIR tests
targeted sema callable/content/evaluated-effect tests
targeted runtime-plan and core/AWBC tests
targeted compiler and LSP tests
cargo check --workspace
cargo test --workspace
lint and deterministic generated-artifact checks required by the repository
```
