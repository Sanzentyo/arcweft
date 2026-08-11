# Family classification

## 1. Evidence axes

Two independent typed ledgers are mandatory.

### Current-observation axis

Answers whether the current production authority exposes a family through the
shared resolver and supports its two truthful observation cases. This axis may
observe transitional Speaker behavior.

### Final-completion axis

Answers whether the row is owned by the accepted final architecture. Legacy
Capacity dispatch, drifted Capacity schema, frozen Dialogue carriers, and
Speaker never receive final credit.

## 2. Phase totals

| Phase | Inventory | R | U | P | Removed | Current executable | Current cases | Final credited | Final cases |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Pre-capacity | 23 | 18 | 3 | 2 | 0 | 21 | 42 | 20 | 40 |
| Post-capacity / pre-Dialogue | 23 | 18 | 4 | 1 | 0 | 22 | 44 | 21 | 42 |
| Final | 22 | 19 | 3 | 0 | 1 historical | 22 | 44 | 22 | 44 |

`R` is `RejectingSchema`, `U` is `IntentionallyUnchecked`, and `P` is
`PendingAuthority`. Speaker's current `U` is always `PendingRemoval` on the
final-completion axis before its deletion.

## 3. Exhaustive family table

| Family | Pre-capacity current | Pre-capacity final | Post-capacity/pre-Dialogue current | Post-capacity/pre-Dialogue final | Final current/final | Evidence rule |
| --- | --- | --- | --- | --- | --- | --- |
| Fx | R | Credited | R | Credited | R / Credited | preserve returned accepted + schema-negative pair |
| EnumConstructor | R | Credited | R | Credited | R / Credited | preserve |
| ResultConstructor | R | Credited | R | Credited | R / Credited | preserve |
| OptionConstructor | R | Credited | R | Credited | R / Credited | preserve |
| Builtin | R | Credited | R | Credited | R / Credited | preserve |
| Agent | R | Credited | R | Credited | R / Credited | preserve |
| Presentation | R | Credited | R | Credited | R / Credited | preserve |
| Dialogue | P | PendingAuthority | P | PendingAuthority | R / Credited | final CharacterReconfigure pair plus typed ContentApplication activation |
| Project | R | Credited | R | Credited | R / Credited | preserve |
| Environment | R | Credited | R | Credited | R / Credited | preserve |
| Lexical | R | Credited | R | Credited | R / Credited | preserve |
| FunctionValue | R | Credited | R | Credited | R / Credited | preserve |
| CollectionMethod | R | Credited | R | Credited | R / Credited | preserve |
| PresentationHandleMethod | R | Credited | R | Credited | R / Credited | preserve |
| IntegerMethod | R | Credited | R | Credited | R / Credited | preserve |
| DomainMethod | R | Credited | R | Credited | R / Credited | preserve |
| TraitMethod | R | Credited | R | Credited | R / Credited | preserve |
| DataLast | R | Credited | R | Credited | R / Credited | preserve |
| CapacityMethod | P | PendingAuthority | U | Credited | U / Credited | accepted typed associated callee; accepted + clean recovery; no spread rejection |
| StageMethod | R | Credited | R | Credited | R / Credited | preserve |
| Drop | U | Credited | U | Credited | U / Credited | preserve accepted + clean recovery |
| Promotion | U | Credited | U | Credited | U / Credited | preserve accepted + clean recovery |
| Speaker | U | PendingRemoval | U | PendingRemoval | Removed / Removed | current regression observation only; no final credit |

No wildcard/default classification is permitted. Every member of
`CallableFamily::ALL` must have an explicit typed entry in the current-phase
ledger and final-disposition ledger.

## 4. Case taxonomy

### RejectingSchema

A family receives two cases only when the current final owner proves:

1. `Accepted` — resolver candidate/schema observed, checker target selected, and
   primary signature candidate equal to the checker candidate; and
2. `RejectedOrPoisoned` — the same final owner and candidate are observed, a
   family-owned schema rule rejects or poisons the call, and recovery argument
   expressions are still checked through the ordinary typed path.

A missing target, unsupported surface, old dispatcher, provisional carrier, or
source-text assertion is not a negative family case.

### IntentionallyUnchecked

A family receives two cases only when the current final owner proves:

1. `Accepted`; and
2. `CleanRecovery` — authored/recovered arguments are checked without an
   expected type, the family schema does not manufacture a rejection, and the
   selected/recovery facts are retained through the normal checker transaction.

### PendingAuthority

A pending row receives no accepted or negative final case. Current legacy
behavior may be regression-tested separately but may not be relabelled as the
final owner.

### PendingRemoval

A current executable row may be observed, but its two cases are excluded from
final cardinality and the final switch must remove its family and ID.

## 5. Capacity evidence transition

### Pre-switch

The following produce zero family credit:

- `well_known_static_capacity_method_type(&str)`;
- a source/display string reconstructed generic receiver;
- bare `Vec` represented as `Named("_")`;
- `CapacityMethodId::signature_schema` using a closed homogeneous `_` parameter;
- spread rejection caused by that drifted schema;
- any path that bypasses checker-owned target facts or native signature parity.

### Post-switch accepted case

Use a typed associated receiver such as `Vec<I32>.with_capacity(8)` and assert:

- exact authored type/generic identity survives syntax/HIR/nominal resolution;
- one `CallCallee::AssociatedType` request reaches the shared resolver;
- the candidate is `CallableCandidateId::CapacityMethod` with the existing
  `CapacityMethodId` receiver/member/arity identity;
- the schema is the accepted `variadic_unchecked` form;
- the result is the selected receiver type;
- checker and signature primary candidates match;
- old dispatch count is zero.

### Post-switch clean-recovery case

One matrix fixture must include zero, multiple, named, spread, and recovered
entries across the Capacity set. Every contained expression is checked with
`CandidateExpectedType::Unchecked`. The family contributes no
Capacity-owned `UnsupportedSpread`, `UnknownNamedArgument`, or arity rejection.
Nested expression diagnostics, if any, remain ordinary expression diagnostics
and do not convert the family to `RejectingSchema`.

## 6. Dialogue evidence transition

### Pre-switch

The following produce zero final Dialogue credit:

- `DialogueCallableId::SpeakerLine`;
- `DialogueCalleeIdentity::Speaker` or `SpeakerPreset`;
- string content-call or `.say` reconstruction;
- frozen `HirDialogue` or speaker-derived line identity;
- a fabricated ordinary call labelled CharacterFactory/Reconfigure without the
  final candidate owner;
- ContentApplication encoded as `Expr::Call`.

### Final representative accepted case

Use final `CharacterReconfigure` through an ordinary `Expr::Call`:

- target: accepted first-class `CharacterDialogue` retaining an exact
  `CharacterId`;
- arguments: valid named `look` resolved against that character plus one valid
  OpenChecked custom field;
- candidate: `DialogueCallableId::CharacterReconfigure`;
- target fact: selected;
- result: same nominal CharacterDialogue identity/config lineage;
- signature primary candidate: identical to checker candidate.

### Final representative negative case

Use the same target/candidate with one spread argument:

- schema remains the final CharacterReconfigure schema;
- spread policy is `Reject`;
- diagnostic is `UnsupportedSpread` at the spread source;
- target fact is rejected and identifies CharacterReconfigure;
- physical evaluation and retained recovery facts follow
  `OVERLOAD_ACCOUNTING.md`.

### Final ContentApplication activation case

Both bracket and colon forms must:

- exist in the Proof typed HIR expression arena;
- own source/range identity selected by AW-AH-009.4.2;
- receive project line identity selected by AW-AH-009.4.3;
- publish `DialogueCallableId::ContentApplication` and a `DialogueLine` result;
- lower to final runtime-plan behavior;
- contain no ordinary-call or string-callee substitute.

## 7. Speaker deletion transition

The final switch must make these typed assertions simultaneously true:

1. `CallableFamily::ALL.len() == 22`;
2. no Speaker family entry exists;
3. no `SpeakerCallableId` can be named through the crate-owned API;
4. final Dialogue IDs are present;
5. final Dialogue accepted/negative cases pass;
6. all frozen readers are unreachable through typed compilation;
7. final current and completion counts are both 22/44.

A commit in which final Dialogue is credited while Speaker remains in the
family inventory is invalid.

## 8. Section-19 gate

The phase table and classification mapping are design-complete. The final
AW-AH-009.3 section-19 acceptance row remains open until the final phase. A
pending row may appear in the staged ledger but cannot be counted as an
accepted/rejected or accepted/recovery pair.
