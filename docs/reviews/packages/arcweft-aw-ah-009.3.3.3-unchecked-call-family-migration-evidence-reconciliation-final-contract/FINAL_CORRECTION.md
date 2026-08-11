# Final correction

## 1. Scope and precedence

This document is the normative correction to AW-AH-009.3.3
`TEST_MATRIX.md` section 19, “Final migration evidence.” Replace that section in
full with section 2 below.

This correction has precedence over any parent wording that requires a
rejected call from a family whose accepted production argument contract cannot
reject authored arguments. It does not change any production candidate,
schema, validator, diagnostic, result, resolver precedence, checker fact,
signature projection, work accounting rule, Dialogue owner, curried group, or
external project publication rule.

## 2. Exact replacement for AW-AH-009.3.3 `TEST_MATRIX.md` section 19

### 19. Final migration evidence

The final migration audit is driven by the current typed
`CallableFamily::ALL` array. Every family is classified exactly once by the
following test-only evidence class:

```rust
#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MigrationValidatorClass {
    RejectingSchema,
    IntentionallyUnchecked,
}
```

The exhaustive classification is owned by an inherent `CallableFamily`
implementation in the enum's owning module. It must not be an extension trait,
string table, file scan, symbol-name scan, or display-label parser.

```rust
#[cfg(test)]
impl CallableFamily {
    pub(crate) const fn migration_validator_class(self) -> MigrationValidatorClass {
        match self {
            Self::Fx
            | Self::EnumConstructor
            | Self::ResultConstructor
            | Self::OptionConstructor
            | Self::Builtin
            | Self::Agent
            | Self::Presentation
            | Self::Dialogue
            | Self::Project
            | Self::Environment
            | Self::Lexical
            | Self::FunctionValue
            | Self::CollectionMethod
            | Self::PresentationHandleMethod
            | Self::IntegerMethod
            | Self::DomainMethod
            | Self::TraitMethod
            | Self::DataLast
            | Self::CapacityMethod
            | Self::StageMethod => MigrationValidatorClass::RejectingSchema,
            Self::Drop | Self::Promotion | Self::Speaker => {
                MigrationValidatorClass::IntentionallyUnchecked
            }
        }
    }
}
```

The classification is scoped to section-19 **argument migration evidence**. A
`RejectingSchema` family may contain individual unchecked parameters or
individual unchecked members; it belongs to this class because at least one
current production member has a reachable typed argument-mapping or
family-validator rejection. `IntentionallyUnchecked` is the stronger claim
that the family's section-19 production candidates deliberately expose the
unchecked variadic argument contract and no family argument rejection is
reachable without changing accepted semantics.

Every family has exactly two matrix cases:

| Validator class | Required first case | Required second case |
|---|---|---|
| `RejectingSchema` | `Accepted` | `RejectedOrPoisoned` |
| `IntentionallyUnchecked` | `Accepted` | `CleanRecovery` |

With the current 23-family inventory this is exactly 46 cases: 23 accepted,
20 rejected-or-poisoned, and 3 clean-recovery.

#### 19.1 Normative case meanings

`Accepted` means all of the following:

1. target resolution retains a candidate whose typed `family()` is the row's
   family;
2. checker facts are `CallTargetFact::Selected` for that exact candidate;
3. aggregate call poison is `CallPoison::Clean`;
4. the committed result and effects are the candidate's production result and
   effects;
5. there is no error-severity callable diagnostic attributable to argument
   mapping or family validation;
6. public signature projection retains the same primary candidate whenever the
   current production surface is signature-query applicable.

`RejectedOrPoisoned` means all of the following:

1. the callee/receiver resolves to one or more bounded candidates in the row's
   family before argument validation;
2. a current production mapping, exact-type, spread, group, or family-validator
   rule rejects or poisons the authored call;
3. checker facts are either:
   - `CallTargetFact::Rejected { candidates }` with the deterministically first
     retained candidate in the row's family and aggregate
     `CallPoison::Rejected`; or
   - `CallTargetFact::Selected` for that family with the exact documented
     recovered/rejected poison when a current family validator deliberately
     keeps a selected recovery result;
4. at least one typed callable diagnostic identifies the genuine production
   failure;
5. every authored argument/recovery slot retained in facts was checked exactly
   once in the committed or diagnostic-retention transaction;
6. signature projection, when applicable, presents the same deterministic
   primary candidate and callable diagnostic. For a rejected candidate set,
   “primary” is the first candidate in its production deterministic order and
   is the signature help active signature at index zero; it is not a checker
   success selection.

`CleanRecovery` means all of the following:

1. the callee/receiver resolves to the exact family candidate;
2. at least one authored argument expression takes the ordinary semantic
   recovery path and yields no inferred type;
3. the unchecked schema maps/checks that expression with
   `expected() == None` exactly once;
4. target facts remain `CallTargetFact::Selected`;
5. aggregate call, argument, and slot poison remain `CallPoison::Clean`;
6. candidate identity, result, effects, and signature projection are identical
   to the clean accepted case;
7. no `CallableDiagnostic` is manufactured. Any normal non-callable-expression
   diagnostic emitted by expression checking remains outside the callable
   argument diagnostic set.

Clean recovery is a specialized accepted disposition, but it occupies the
second matrix slot for an intentionally unchecked family. It may not also be
counted as that family's first `Accepted` fixture.

#### 19.1.1 No genuine unchecked-family negative exists

At the inspected revision there is no genuine family argument negative for any
of the three intentionally unchecked families:

- `DropCallableId::Drop` accepts every authored positional, named, and spread
  argument through the variadic-unchecked policy and retains result `Unit`;
- `PromotionCallableId::{Promote, PromoteUnchecked, Assume}` accept authored
  arguments through the same policy and retain their documented `Promoted`,
  `Promoted`, and `Unit` results;
- character-speaker and speaker-preset candidates accept authored arguments
  through the same policy and retain `SpeakerPreset(Character)`.

A misspelled promotion function is `Missing`; a value that does not resolve as
a speaker is `Missing` or `NonCallable`; an unsupported receiver/method is an
unknown method; and cancellation, limits, or corrupt state are terminal. None
has an unchecked-family candidate that rejected arguments, so none is a family
negative. This correction therefore does not identify or authorize a rejected
fixture for these families.

#### 19.2 Dispositions that cannot satisfy a family row

The following are typed, valid outcomes but are not family negative evidence:

- `UnknownTarget`: `CallTargetFact::Missing`, because no family candidate was
  resolved;
- `NonCallableTarget`: `CallTargetFact::NonCallable`, because the resolved
  lexical/project value is not a callable family candidate;
- `UnsupportedSurface`: no current production callable carrier reaches that
  family; an old or synthetic surface may not be restored to create evidence;
- `TerminalQueryError`: cancellation, deadline, work/limit exhaustion, world or
  source identity failure, corrupt accepted state, or signature arithmetic
  failure; no partial family fact/help may be published.

An unknown callee named like a family member, an unknown method on the wrong
receiver, a non-callable lexical shadow, or removed Dialogue syntax therefore
cannot be relabelled as a `RejectedOrPoisoned` case for that family.

#### 19.3 Counter assertions

The final matrix preserves these exact assertions:

```text
shared_resolver_invocations == number_of_call_expressions
old_dispatch_calls == 0
checker.primary_candidate == signature.primary_candidate
argument_expression_checks == exactly_once_per_committed_or_recovery_argument
```

They have the following normative interpretation:

- `shared_resolver_invocations` is charged at the one authoritative
  `resolve_call_target` entry for each matrix call expression. Candidate probes,
  selected replay, fact projection, and signature projection do not increment
  it. Every fixture isolates one target call and prebinds any supporting typed
  values so nested helper calls cannot alter the count.
- `old_dispatch_calls` is a typed test-only dispatcher-boundary observation.
  It is not a source search. No legacy success branch, signature-only resolver,
  or second dispatch is permitted.
- `checker.primary_candidate` is the selected candidate for `Selected`, or the
  first deterministically ordered retained candidate for `Rejected` or
  `Ambiguous`. It is absent for `Missing`, `NonCallable`, and terminal error.
- `signature.primary_candidate` is the candidate at the public help result's
  active signature. For a rejected candidate set, active index zero reflects
  deterministic UI focus and does not turn rejection into selection.
- `argument_expression_checks` is a transaction-aware typed multiset keyed by
  committed `TypeExpressionId`. Speculative candidate transactions roll their
  counts back; the selected replay or rejected-diagnostic retention commits one
  count for every published argument slot. A clean-recovery unresolved slot is
  counted once exactly like a typed slot.

Consequently:

- an `Accepted` case performs one resolver invocation, zero old dispatches,
  candidate parity, and exactly-one committed argument checks;
- a `RejectedOrPoisoned` case performs one resolver invocation, zero old
  dispatches, deterministic retained-candidate parity, and exactly-one
  diagnostic-retention argument checks;
- a `CleanRecovery` case performs one resolver invocation, zero old dispatches,
  selected-candidate parity, and exactly-one recovery argument checks, while
  retaining clean callable poison.

#### 19.4 Completion and drift rules

The migration is complete only when all of the following hold:

1. `CallableFamily::ALL.len() == 23` at the inspected revision;
2. the classification has exactly 23 distinct entries in the exact `ALL` order;
3. the class counts are exactly 20 rejecting and 3 intentionally unchecked;
4. the intentionally unchecked set is exactly
   `{Drop, Promotion, Speaker}`;
5. each family has one accepted case and exactly the class-required second
   case, with no duplicate case kind;
6. all 46 cases use typed candidate IDs, typed schemas, checker facts, signature
   results, diagnostics, poison, and expression IDs;
7. a new `CallableFamily` variant causes the exhaustive inherent match to fail
   compilation until explicitly classified;
8. a changed validator/schema category causes the schema-contract or case
   outcome test to fail until production contract and matrix are deliberately
   updated together;
9. `Curried` continues to report its base family and creates no extra row;
10. Project external bindings use the AW-AH-009.3.3.2 typed path publication;
11. Dialogue evidence uses only its current authoritative typed surface. No
    superseded speaker/content-call carrier or fake `Expr::Call` is permitted.

The counters are test-only typed instrumentation. They do not scan source and
do not ship as a production semantic branch.

## 3. Required implementation ownership

The correction is implemented in tests and enum-owned test classification only:

- place `migration_validator_class` in the existing inherent
  `CallableFamily` implementation under `#[cfg(test)]`;
- place the case table and test-only audit recorder under the existing sema
  signature/checker test modules;
- use existing `CallableSignatureSchema`, `CallableValidator`,
  `CallTargetFact`, `CallPoison`, `CallableDiagnosticCode`,
  `TypeExpressionId`, and public signature query results;
- do not add a production resolver branch, semantic diagnostic, compatibility
  API, extension trait, string family key, source scanner, or repository-file
  scanner.

No production behavior change is required or authorized.
