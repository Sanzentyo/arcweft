# CharacterDialogue voice, Variant, Option, Result, and Choice grammar

## 1. Voice decision

Tuple index 5 is a nested Option. It is not a flat three-case variant.

Logical states and exact RuntimeValue forms:

| Logical state | RuntimeValue |
|---|---|
| absent | `Option::None` |
| automatic | `Option::Some(CharacterDialogueVoice::Auto)` |
| explicit ID | `Option::Some(CharacterDialogueVoice::Id(EntityRef))` |

## 2. Outer Option

The exact owner is `RuntimeVariantIdentity::Option`.

| Ordinal | Name | Payload |
|---:|---|---|
| 0 | `Some` | required inner voice variant |
| 1 | `None` | absent |

Any other owner, ordinal, name, or payload presence fails.

## 3. Inner voice variant

The exact nominal variant owner is the accepted semantic identity for
`arcweft.dialogue.CharacterDialogueVoice`.

| Ordinal | Name | Payload |
|---:|---|---|
| 0 | `Auto` | absent |
| 1 | `Id` | required `RuntimeCheckedType::EntityRef` |

The owner identity is projected once from accepted semantic facts and stored in
the closed checked type. It is never recovered from the case name or display
label.

## 4. Voice encode branch selection

Encoding follows domain state, not value-shape guessing:

1. `None` -> outer None;
2. `Some(Auto)` -> outer Some, inner Auto;
3. `Some(Id(id))` -> outer Some, inner Id with canonical EntityRef.

There is exactly one branch for every valid domain state. Invalid entity IDs
fail before wrapping.

## 5. Voice decode precedence

At tuple path `[5]`:

1. require a RuntimeValue Variant;
2. require outer Option owner;
3. require outer ordinal;
4. require exact outer name;
5. require outer payload presence;
6. for Some, require inner Variant;
7. require exact inner nominal owner;
8. require inner ordinal;
9. require exact inner name;
10. require inner payload presence;
11. for Id, require EntityRef and domain-valid ID;
12. publish voice state.

The path is extended through variant payload steps at each nested boundary.

## 6. General nominal Variant validation

For `RuntimeCheckedType::Variant`, the retained accepted G1 behavior is
normative:

1. actual value must be Variant;
2. exact `RuntimeVariantIdentity::Nominal` owner;
3. ordinal in range;
4. exact case name at that ordinal;
5. exact payload-presence relation;
6. recursive payload checked type.

The previous owner-only branch is not accepted.

## 7. Option and Result validation

Option:

- owner `Option`;
- Some ordinal 0/name Some/required payload matching item;
- None ordinal 1/name None/no payload.

Result:

- owner `Result`;
- Ok ordinal 0/name Ok/required payload matching ok type;
- Err ordinal 1/name Err/required payload matching error type.

No case accepts an unexpected payload or missing payload.

## 8. Choice runtime selection

`RuntimeCheckedType::Choice` is not boolean `any()` in authority-bearing
validation.

For one concrete value:

1. evaluate every alternative in source order under one shared work budget;
2. retain typed branch failures with their branch index;
3. count successful alternatives;
4. exactly one success selects that branch;
5. zero successes returns `RuntimeCheckedTypeError::ChoiceNoMatch`;
6. two or more successes returns
   `RuntimeCheckedTypeError::ChoiceAmbiguous` with the first two successful
   indices.

This prevents a shallow primitive branch from hiding a more specific branch
and prevents branch-order-dependent acceptance.

## 9. Choice error evidence

`ChoiceNoMatch` retains one ordered summary per branch. Higher boundaries may
limit display text, but the typed source includes:

- value path;
- checked-type path;
- branch index;
- branch mismatch kind.

`ChoiceAmbiguous` retains the first and second matching indices. It is reported
before operation-specific domain publication.

## 10. Choice admission traversal

Generation admission never chooses a Choice branch. It traverses every
alternative and unions every nominal key. A malformed later branch fails even
when an earlier branch is well formed.

## 11. Clear and normalize

- normalize uses the unique runtime-selected Choice branch;
- clear is allowed only when exactly one branch is selected for the current
  value and that branch has a deterministic clear rule;
- if the cleared result would match multiple branches, clear fails before
  publication.

## 12. Canonical bytes

Voice uses the existing nested RuntimeValue variant codec. Choice itself has no
runtime wrapper and therefore no Choice tag in RuntimeValue bytes; it is only a
checked-type predicate.

Canonical checked-type bytes preserve Choice source order. Reordering Choice
alternatives changes the generation contract and generation identity.
