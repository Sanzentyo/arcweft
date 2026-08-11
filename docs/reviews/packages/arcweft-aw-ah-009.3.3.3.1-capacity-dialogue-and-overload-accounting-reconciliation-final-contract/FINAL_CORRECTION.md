# Final correction

## 1. Normative vocabulary

The family evidence ledger uses two independent axes.

### Current authority class

```text
RejectingSchema
IntentionallyUnchecked
PendingAuthority
Removed
```

- `RejectingSchema` means the current final owner can produce both a clean
  accepted case and a typed schema rejection/poison case through the shared
  resolver and checker-owned facts.
- `IntentionallyUnchecked` means the current final owner intentionally checks
  argument expressions without an expected type and therefore has an accepted
  case plus a clean-recovery case, not a manufactured schema rejection.
- `PendingAuthority` means the accepted final owner is not yet production
  authority. Legacy or provisional behavior cannot earn final-model credit.
- `Removed` means the family is absent from the final enum/inventory.

### Final completion disposition

```text
Credited
PendingAuthority
PendingRemoval
Removed
```

Current execution and final completion are not aliases. A row may be observable
in current production while remaining `PendingRemoval` or `PendingAuthority`
for final completion.

## 2. Exact replacement for CapacityMethod

Replace any parent or returned-package wording that calls CapacityMethod a
rejecting family, uses `reserve` as its representative rejection, treats spread
as invalid, or credits the current string dispatcher/homogeneous `_` schema.
The replacement is:

> Before the AW-AH-009.3.3.4 production authority switch,
> `CallableFamily::CapacityMethod` is `PendingAuthority`. The legacy
> `well_known_static_capacity_method_type(&str)` success path, text-derived
> generic identity, bare-`Vec` `_` placeholder, and current
> `homogeneous(arity, Named("_"), ...)` schema are implementation drift and
> contribute no final migration evidence.
>
> The authority transition is the accepted typed route from the existing
> ordinary call surface through source-backed authored type identity and nominal
> resolution to `CallCallee::AssociatedType`, one `resolve_call_target`
> invocation, the existing `CapacityMethodId`, checker-owned call-target facts,
> and native semantic signature projection. The same compiling cut deletes the
> old dispatcher and every text/label reader named by AW-AH-009.3.3.4.
>
> After that switch, CapacityMethod is `IntentionallyUnchecked`. Its argument
> contract is the accepted `variadic_unchecked` schema: zero, one, multiple,
> positional, named, spread, and recovered entries are admitted by the family
> schema and each contained expression is checked without an expected type.
> Capacity-owned shape validation does not fabricate a negative spread case.

The current selected-call ordering remains unchanged: typed environment methods
precede Capacity; Capacity precedes associated traits and data-last fallback.
Value-first versus typed associated-receiver resolution is owned by
AW-AH-009.3.3.4 and is not reopened here.

## 3. Exact replacement for Dialogue

Replace any parent or returned-package wording that treats the current
`SpeakerLine`, string content call, or frozen `HirDialogue` carrier as final
Dialogue migration evidence. The replacement is:

> Before the AW-AH-009.4.2/.4.3 public authority switch,
> `CallableFamily::Dialogue` is `PendingAuthority`. No accepted or negative
> final Dialogue case is credited from `SpeakerLine`, `SpeakerPreset`, callee
> strings, `.say` suffixes, or the frozen content carrier. A pre-switch ordinary
> `Expr::Call` fixture that lacks final `CharacterFactory` or
> `CharacterReconfigure` candidate authority also cannot satisfy the row.
>
> Dialogue becomes `RejectingSchema` only in the same compiling public switch
> that installs all of the following: the Proof attached syntax/HIR expression
> arena and source map; AW-AH-009.4.2 bracket/colon typed content application;
> AW-AH-009.4.3 accepted project line identity and collision transaction; final
> sema and runtime-plan publication; and deletion of all frozen
> Speaker/string/`HirDialogue` readers together with `SpeakerCallableId`,
> `CallableFamily::Speaker`, and Speaker/SpeakerPreset semantic and runtime
> identities.
>
> After the switch, final `CharacterFactory` and `CharacterReconfigure` use the
> existing ordinary `Expr::Call`. `ContentApplication` is the distinct typed
> bracket/colon HIR operation and is never encoded as `Expr::Call`.

### Representative section-19 Dialogue pair

The exact final representative pair is `CharacterReconfigure`:

- **Accepted case** — an ordinary `Expr::Call` whose target is an accepted
  `CharacterDialogue` value, whose shared-resolver primary candidate is
  `CallableCandidateId::Dialogue(DialogueCallableId::CharacterReconfigure)`,
  and whose arguments contain a valid exact/dependent standard named field
  (use `look` bound to the value's retained character identity) plus one valid
  `OpenChecked` custom named field. The checker publishes a selected
  `CheckedCallTarget`, retains the same character identity, and returns the
  final `CharacterDialogue` semantic type.
- **Negative case** — the same typed target and candidate with one authored
  `CallArg::Spread`. The final schema's `SpreadArgumentPolicy::Reject` produces
  `CallableDiagnosticCode::UnsupportedSpread`; the checker publishes a rejected
  target that still identifies `CharacterReconfigure`. The argument expression
  is physically evaluated according to the candidate/pass accounting rules, but
  only the deterministic recovery projection is retained.

`ContentApplication` has a separate mandatory activation test: a bracket and a
colon source form must lower to the accepted typed HIR content-application
variant, publish `DialogueCallableId::ContentApplication`, produce the accepted
line identity and `DialogueLine` result, and contain no ordinary-call encoding.
It is not the representative rejection pair because its grammar/HIR authority
must remain distinct from ordinary call schema validation.

## 4. Exact Speaker transition

Before the final Dialogue switch, current production may retain the existing
Speaker accepted/clean-recovery pair solely as a typed regression observation.
Its current authority class is `IntentionallyUnchecked`; its final completion
disposition is `PendingRemoval`.

Speaker receives no final-model row or case credit in any phase. The Dialogue
switch atomically removes:

```text
SpeakerCallableId
CallableFamily::Speaker
TypeKind::Speaker
TypeKind::SpeakerPreset
SpeakerRef
SpeakerPreset
DialogueSpeakerPreset
DialogueCalleeIdentity::Speaker
DialogueCalleeIdentity::SpeakerPreset
DialogueCallableId::SpeakerLine
all frozen Speaker/string/HirDialogue readers
```

There is no valid inventory in which Speaker and final Dialogue both receive
matrix credit.

## 5. Exact overload-accounting replacement

Replace broad claims that `argument_expression_checks == exactly once` with the
following two names and definitions.

### `physical_candidate_argument_evaluations`

A bounded operational multiset of actual candidate-specific argument-slot
checks. One element is emitted immediately before the checker physically
invokes the expression/slot checker under a concrete candidate and evaluation
pass. It includes speculative probes and any selected or rejected recovery
replay. It is not rolled back with semantic candidate state and is not inferred
from resolver or work-meter counts.

### `retained_argument_inference_facts`

The final multiset projection of existing `CheckedCallArgumentSlotFact` values
published in the call's committed or deterministic recovery
`CheckedCallTarget`. It is a retained semantic-state measure, not an execution
counter. Unselected probe facts and rolled-back judgments, substitutions,
lowering evidence, captures, effects, diagnostics, and nested facts are absent.

The accepted per-candidate contextual algorithm remains unchanged. A candidate
may contextualize the same source expression differently from another
candidate. A unique winner is replayed and its replay facts are retained.
Ambiguity retains the deterministic primary tied probe projection. Multiple
rejections retain the stable primary probe projection; a singleton rejection
uses the rejected-recovery replay. Terminal cancellation, deadline, or work
failure retains no call-target inference facts, while already completed
physical events remain operational evidence.

No equality is required between the two cardinalities. For one ordinary
argument and two viable candidates with a unique winner, the normal current
algorithm produces three physical evaluations (two probes and one selected
replay) and one retained inference fact. A fixed literal spread with `k` logical
slots produces `3k` physical slot evaluations and `k` retained facts under the
same two-candidate/one-winner shape.

## 6. Normative precedence for parent sections

1. AW-AH-009.3.3 section 23 remains authoritative for candidate-specific
   contextual transactions, ranking, rollback, and replay.
2. AW-AH-009.3.3 section 36 item 4 is corrected to require exactly one retained
   inference fact per retained logical argument slot, not one physical traversal
   of the source expression.
3. AW-AH-009.3 TEST_MATRIX section 19 is replaced by the phase-aware family
   table and both evidence axes in this package.
4. The AW-AH-009.3.3.3 return remains authoritative for Drop, Promotion, the 18
   stable rejecting families, the general case taxonomy, and retained-fact
   semantics except where this package corrects the counter name.
5. The returned CapacityMethod and Dialogue rows are superseded. Speaker is
   retained only as current-phase observation and `PendingRemoval`.
6. AW-AH-009.3.3.4 controls Capacity. AW-AH-009.4/.4.2/.4.3 control Dialogue.
7. Curried-group validation, typed external publication, the shared resolver,
   candidate order, checker facts, native cache, and signature projection are
   unchanged.

## 7. Section-19 completion rule

The replacement text and staged 23-family classification ledger are complete
and may be recorded as `STAGED_CLASSIFICATION_COMPLETE`. The AW-AH-009.3
end-to-end family-matrix acceptance gate itself must remain open whenever any
row is `PendingAuthority` or `PendingRemoval`.

Therefore:

- pre-capacity: final family-matrix acceptance is open for Capacity, Dialogue,
  and Speaker deletion;
- post-capacity/pre-Dialogue: only Dialogue authority and Speaker deletion keep
  the final gate open;
- final: the gate may close only after the 22-family inventory, 19/3 classes,
  44 final cases, final Dialogue evidence, and typed absence of Speaker all pass.

This distinction closes the design correction without relabelling partial
implementation as end-to-end completion.

## 8. Compiling switch obligations

### Capacity switch

The same compiling cut must:

1. install the accepted typed associated receiver/callee owner;
2. resolve aliases, qualification, and generic identity without display strings;
3. route registered and non-registered checking through the shared resolver;
4. publish checker facts and native signature parity;
5. use `variadic_unchecked` exactly;
6. delete `well_known_static_capacity_method_type`, generic text slicing, the
   bare-`Vec` `_` placeholder, the early success branch, and every static-capacity
   label reader;
7. prove one call registration, one shared resolver invocation, and zero old
   dispatches.

### Dialogue switch

The same compiling cut must:

1. expose the accepted attached syntax/HIR/project owner required by Proof;
2. install AW-AH-009.4.2 bracket/colon content application in that arena;
3. install AW-AH-009.4.3 line identity and project collision acceptance;
4. publish final `CharacterFactory`, `CharacterReconfigure`, and
   `ContentApplication` semantic identities and runtime-plan behavior;
5. delete every frozen Speaker/string/`HirDialogue` reader and all Speaker IDs,
   types, enum variants, and runtime carriers;
6. update the family inventory from 23 to 22 in the same typed transition;
7. activate final Dialogue evidence only after all preceding conditions hold.
