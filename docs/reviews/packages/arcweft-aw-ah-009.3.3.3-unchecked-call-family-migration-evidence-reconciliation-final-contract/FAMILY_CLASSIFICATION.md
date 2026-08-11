# Family classification

## 1. Classification rule

The table classifies a **family's section-19 argument evidence**, not every
individual member in the family.

- `RejectingSchema`: at least one current production candidate in the family
  has a reachable argument-mapping, exact-type, spread, group, or
  family-validator rejection. Its second evidence case must exercise that
  genuine path while retaining the family candidate.
- `IntentionallyUnchecked`: all section-19 representative production candidates
  deliberately use the exact variadic-unchecked argument contract. Its second
  case must be clean recovery, not a fabricated rejection.

A family does not become `IntentionallyUnchecked` merely because it contains an
unchecked parameter or one unchecked member. This is why `Builtin`,
`Presentation`, `DomainMethod`, and similar mixed families remain
`RejectingSchema`.

## 2. Closed 23-family table

| # | `CallableFamily` | Class | Representative production candidate | Required evidence | Rationale |
|---:|---|---|---|---|---|
| 1 | `Fx` | `RejectingSchema` | `FxCallableSignatureId::Conditional` | accepted + wrong `condition` type | required named `Bool` and closed mapping can reject |
| 2 | `EnumConstructor` | `RejectingSchema` | expected nominal tuple variant with `I32` payload | accepted + wrong payload type | expected enum payload schema is exact |
| 3 | `ResultConstructor` | `RejectingSchema` | `ResultConstructorKind::Ok` under expected `Result<I32, String>` | accepted + wrong payload type | expected result payload is exact |
| 4 | `OptionConstructor` | `RejectingSchema` | `OptionConstructorKind::Some` under expected `Option<I32>` | accepted + wrong payload type | expected option payload is exact |
| 5 | `Builtin` | `RejectingSchema` | `BuiltinCallableId::Sin` | accepted `F32` + wrong argument type | typed builtin members are reachable even though panic/fail/bail are unchecked |
| 6 | `Agent` | `RejectingSchema` | `AgentIntrinsicSignatureId::Expect` | accepted `Bool` + wrong argument type | exact intrinsic schema rejects |
| 7 | `Presentation` | `RejectingSchema` | `PresentationCallableId::Background` | accepted Asset ref + wrong argument type | typed presentation members reject even though open/unchecked properties exist |
| 8 | `Dialogue` | `RejectingSchema` | `DialogueCallableId::SpeakerLine` on the current typed owner | accepted options + wrong typed `view` value | current schema has exact typed fields and spread rejection; no old carrier is restored |
| 9 | `Project` | `RejectingSchema` | typed project function with one `I32` parameter | accepted + wrong argument type | project schema is exact and uses typed project binding publication |
| 10 | `Environment` | `RejectingSchema` | accepted Standard/Adapter function with one `I32` parameter | accepted + wrong argument type | published environment schema is exact |
| 11 | `Lexical` | `RejectingSchema` | `LocalCallableId` with one `I32` parameter | accepted + wrong argument type | lexical callable signature is exact |
| 12 | `FunctionValue` | `RejectingSchema` | fixed `fn(I32) -> I32` value | accepted + wrong argument type | function-value schema is strict positional exact |
| 13 | `CollectionMethod` | `RejectingSchema` | `CollectionMethodId::Contains` on `Vec<I32>` | accepted `I32` + wrong item type | collection item expectation is exact |
| 14 | `PresentationHandleMethod` | `RejectingSchema` | `PresentationHandleMethodId::Hide` | accepted no-arg + extra positional | lifecycle schema is closed and zero-arity |
| 15 | `IntegerMethod` | `RejectingSchema` | `IntegerMethodId::Min` on `I32` | accepted `I32` + wrong argument type | receiver-width input is exact |
| 16 | `DomainMethod` | `RejectingSchema` | `DomainMethodId::MapGet { key: I32, value: String }` | accepted key + wrong key type | typed domain members reject even though context/say/face members are unchecked |
| 17 | `TraitMethod` | `RejectingSchema` | one visible typed trait method with one `I32` parameter | accepted + wrong argument type | selected trait schema is exact |
| 18 | `DataLast` | `RejectingSchema` | data-last callable with receiver injected and one remaining `I32` parameter | accepted + wrong authored type | base callable schema validates remaining authored arguments |
| 19 | `CapacityMethod` | `RejectingSchema` | one-arity `CapacityMethodId` such as `reserve` | accepted positional + one authored spread argument | current capacity schema is positional and rejects spread; rejection is reachable without changing arity identity |
| 20 | `StageMethod` | `RejectingSchema` | `StageMethodId::Acquire` | accepted `PresentationLifetime` + wrong argument type | current 23rd family has an exact typed parameter |
| 21 | `Drop` | `IntentionallyUnchecked` | `DropCallableId::Drop` | accepted + clean recovery | exact variadic-unchecked schema, `Unit` result, no argument rejection |
| 22 | `Promotion` | `IntentionallyUnchecked` | all `PromotionCallableId` variants; representative `Promote` | accepted + clean recovery | Promote/PromoteUnchecked/Assume preserve unchecked arguments and their current results |
| 23 | `Speaker` | `IntentionallyUnchecked` | character speaker and speaker-preset `SpeakerCallableId` | accepted + clean recovery | arguments remain untyped and result remains `SpeakerPreset(Character)` |

## 3. Exact unchecked production contract

Every candidate used to establish an `IntentionallyUnchecked` family must have
this typed schema shape:

```text
groups.len() == 1
group[0].index == 0
group[0].kind == Initial
group[0].parameters.len() == 1
parameter[0].index == 0
parameter[0].name == "args"
parameter[0].type == Unchecked
parameter[0].passing == RestPositional
parameter[0].presence == Optional
argument_policy.unknown_named == OpenUnchecked
argument_policy.spread == Unchecked
```

The validator/result pairs are exact:

| Family/candidate | Validator | Result |
|---|---|---|
| `DropCallableId::Drop` | `CallableValidator::Drop` | `Unit` |
| `PromotionCallableId::Promote` | `CallableValidator::Promotion(Promote)` | `Promoted` |
| `PromotionCallableId::PromoteUnchecked` | `CallableValidator::Promotion(PromoteUnchecked)` | `Promoted` |
| `PromotionCallableId::Assume` | `CallableValidator::Promotion(Assume)` | `Unit` |
| character speaker | `CallableValidator::Speaker` | `SpeakerPreset(Character)` |
| speaker-preset callable | `CallableValidator::Speaker` | `SpeakerPreset(Character)` |

A clean-recovery fixture passes one or more unresolved argument expressions.
Because the parameter is `Unchecked`, each slot has `expected == None`; because
mapping is open unchecked, the slot, argument, and call stay clean. The
expression itself is still checked exactly once and may emit its ordinary
expression-level recovery diagnostic.

## 4. Mixed-family examples

The classification is deliberately not based on “contains any unchecked
parameter”:

- `Builtin` includes unchecked panic/fail/bail/fallback paths, but `Sin`, `Rgb`,
  assertions, vectors, and other members have reachable rejecting schemas.
- `Presentation` has open/unchecked property paths, but Background and other
  typed members reject wrong values.
- `Dialogue` includes unchecked option fields, but also exact `id`, `text_key`,
  `view`, `source_locale`, nominal `look`, and spread rejection.
- `DomainMethod` includes unchecked context/face/say members, but MapGet,
  Parallel, probe comparisons, and other members reject.
- `StageMethod::Look` has one unchecked `look` input, but typed `crossfade` and
  `StageMethod::Acquire` supply reachable rejection.

## 5. Drift closure

The implementation must make drift visible without reading source files:

1. the exhaustive inherent match fails compilation when a new family is added;
2. a typed order/uniqueness test compares the classification sequence with
   `CallableFamily::ALL`;
3. a count test requires exactly 20/3;
4. an exact-set test requires unchecked families to be exactly
   `Drop`, `Promotion`, and `Speaker`;
5. schema-shape tests enumerate every Drop/Promotion/Speaker candidate above;
6. the 20 negative cases prove a reachable current rejection. If a schema or
   validator stops rejecting, its case fails rather than being replaced by an
   unknown target;
7. changing an unchecked family to typed/rejecting requires an intentional
   production-contract change and a same-change matrix update; neither can
   happen silently.

`CallableCandidateId::Curried` continues to classify through `base().family()`;
it is not a 24th row. External project aliases continue to classify as
`Project` through the typed path publication from AW-AH-009.3.3.2.
