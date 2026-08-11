\
# FINAL CONTRACT — curried callable group validation

## 1. Normative status

This document is the complete correction for the curried-group ownership seam. It supersedes only prior clauses that require a schema-less `CurriedCallableId` constructor to prove schema membership or that allow more than one successful representation of a curried resolved callable. All unrelated AW-AH-009.3.3 substrate and later-cut obligations remain unchanged.

The keywords **MUST**, **MUST NOT**, **SHALL**, and **SHALL NOT** are normative.

## 2. Selected ownership model

The selected model is **context-free ID plus resolver validation**.

- `CurriedCallableId` owns structural identity only.
- `CallableSignatureSchema` owns group membership.
- `ResolvedCallable::try_new` is the one schema-owning publication boundary that validates a successful curried result.
- The shared resolver SHALL construct every curried success through that boundary after accepted request/world validation.

No global catalog, thread-local world, schema pointer, world lease, formatted signature, or hidden lookup is added to the identity type.

## 3. Exact public Rust surface

### 3.1 Preserved ID shape and constructor signature

```rust
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CurriedCallableId {
    base: Box<CallableCandidateId>,
    next_group: CallableGroupIndex,
}

impl CurriedCallableId {
    pub fn try_new(
        base: CallableCandidateId,
        next_group: CallableGroupIndex,
    ) -> Result<Self, CallableIdentityError>;

    pub const fn base(&self) -> &CallableCandidateId;
    pub const fn next_group(&self) -> CallableGroupIndex;
}
```

No overload, compatibility constructor, unchecked public constructor, alias, or deprecated spelling is permitted.

### 3.2 Corrected identity error

`CallableIdentityError::MissingGroup` SHALL be deleted directly. It SHALL NOT remain as a deprecated variant or compatibility alias.

The exact replacement is:

```rust
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum CallableIdentityError {
    #[error(transparent)]
    Scalar(#[from] CallableScalarError),

    #[error("callable {base:?} cannot use group {group:?} as a curried next group")]
    InvalidCurriedGroup {
        base: Box<CallableCandidateId>,
        group: CallableGroupIndex,
    },

    #[error("callable {base:?} cannot be curried")]
    InvalidCurriedBase {
        base: Box<CallableCandidateId>,
    },

    // Existing non-curried variants remain unchanged.
}
```

`InvalidCurriedGroup` is emitted by this constructor **only when `next_group.get() == 0`**. It does not claim that any nonzero group exists in a schema.

Identity-construction errors have no `CallableDiagnosticCode`; resolver-visible group failures use the existing resolver error below.

### 3.3 Preserved resolver surfaces

These existing public shapes remain unchanged:

```rust
pub enum CallableCandidateId {
    // existing variants
    Curried(CurriedCallableId),
    // existing variants
}

pub enum CallableInstantiation {
    // existing variants
    Curried {
        base: CallableCandidateId,
        group: CallableGroupIndex,
    },
    // existing variants
}

pub fn ResolvedCallable::try_new(
    id: CallableCandidateId,
    origin: SignatureOrigin,
    schema: Arc<CallableSignatureSchema>,
    instantiation: CallableInstantiation,
    equivalent_sources: Vec<EquivalentCallableSource>,
    authority: Option<CallableAuthorityRank>,
    limits: &CallableLimits,
) -> Result<ResolvedCallable, ResolveCallError>;
```

The function signature is preserved. Its curried validation and error specificity are corrected.

### 3.4 Resolver error and diagnostic mapping

The existing error and diagnostic code remain the exact schema-membership failure:

```rust
pub enum ResolveCallError {
    // existing variants
    InvalidCallGroup {
        candidate: CallableCandidateId,
        group: CallableGroupIndex,
    },
    InvalidResolvedCallable,
    // existing variants
}

pub enum CallableDiagnosticCode {
    // existing variants
    InvalidCallGroup,
    // existing variants
}
```

The existing mapping SHALL remain:

```rust
ResolveCallError::InvalidCallGroup { .. }
    => CallableDiagnosticCode::InvalidCallGroup
```

For a curried group failure, `candidate` SHALL be the unwrapped base candidate, never the `CallableCandidateId::Curried` wrapper.

## 4. Constructor invariants and precedence

`CurriedCallableId::try_new(base, next_group)` SHALL evaluate the following rules in order:

1. If `base` is `CallableCandidateId::Curried(_)` or `CallableCandidateId::DataLast(_)`, return:

   ```rust
   Err(CallableIdentityError::InvalidCurriedBase {
       base: Box::new(base),
   })
   ```

2. Otherwise, if `next_group.get() == 0`, return:

   ```rust
   Err(CallableIdentityError::InvalidCurriedGroup {
       base: Box::new(base),
       group: next_group,
   })
   ```

3. Otherwise, return `Ok(CurriedCallableId { .. })` without consulting or storing a schema.

The wrapper error precedes the zero-group error when both conditions are present. This preserves current constructor ordering and makes recursive wrapper growth the primary structural defect.

A nonzero `next_group` MUST be structurally constructible even when no schema is available. Construction is not resolution and does not authorize publication.

## 5. Canonical successful resolved representation

A successful curried `ResolvedCallable` SHALL use exactly this pair:

```rust
id = CallableCandidateId::Curried(curried_id)

instantiation = CallableInstantiation::Curried {
    base: curried_id.base().clone(),
    group: curried_id.next_group(),
}
```

The following MUST all hold:

1. `curried_id.base() == base`.
2. `curried_id.next_group() == group`.
3. `schema.group(group).is_some()`.
4. `origin_matches(curried_id.base(), origin, authority)` under the existing origin rules.
5. The stored schema is the complete base schema supplied to `ResolvedCallable::try_new`; it is not sliced, rebuilt, or replaced by a group-only schema.
6. Existing equivalent-source uniqueness and candidate-limit invariants hold unchanged.

`ResolvedCallable::id().family()` continues to derive the base family through the existing `CallableCandidateId::family` behavior.

## 6. Noncanonical pair rejection

Each of the following SHALL return `ResolveCallError::InvalidResolvedCallable` before schema-membership classification:

- a base `CallableCandidateId` paired with `CallableInstantiation::Curried`;
- a `CallableCandidateId::Curried` paired with any non-curried instantiation;
- a curried ID whose embedded base differs from the instantiation base;
- a curried ID whose `next_group` differs from the instantiation group;
- a recursively wrapped or otherwise structurally impossible curried identity reaching the boundary through internal corruption.

In particular, this current alternate success shape is prohibited and SHALL be removed:

```rust
(id, CallableInstantiation::Curried { base, group })
    if id == base && schema.group(*group).is_some()
```

There is one successful representation, not two competing resolver products.

## 7. Schema-aware group validation

To preserve the current substrate's error precedence, `ResolvedCallable::try_new` SHALL first complete its existing candidate-count, origin/authority, canonical-instantiation, and equivalent-source uniqueness validation. Only when those existing structural checks pass SHALL a canonical curried pair be classified for schema membership. It SHALL then execute:

```rust
if schema.group(group).is_none() {
    return Err(ResolveCallError::InvalidCallGroup {
        candidate: base.clone(),
        group,
    });
}
```

This includes:

- one-over (`group.get() == schema.groups().len()`),
- any larger representable nonzero group,
- any nonzero hole received from corrupt internal state, even though public schema construction already requires contiguous groups.

The missing group SHALL NOT be collapsed to `InvalidResolvedCallable`, `ResourceExhausted`, unknown-call, ambiguity, or catalog-missing behavior.

## 8. Required internal validation shape

The implementation MAY keep the private `instantiation_matches` helper, but its semantics SHALL be equivalent to:

```rust
fn instantiation_matches(
    id: &CallableCandidateId,
    instantiation: &CallableInstantiation,
) -> bool {
    match (id, instantiation) {
        (
            CallableCandidateId::Curried(id),
            CallableInstantiation::Curried { base, group },
        ) => id.base() == base && id.next_group() == *group,

        // Existing non-curried arms remain unchanged.

        _ => false,
    }
}
```

Schema membership is a separately classified final pre-publication check so that it can return `ResolveCallError::InvalidCallGroup`. Existing malformed-product checks retain `InvalidResolvedCallable` precedence when more than one invariant is broken. No public helper, extension trait, wrapper type, or second resolved product is introduced.

## 9. Shared-resolver construction and error mapping

When the shared resolver has a base candidate and a requested continuation group, it SHALL:

1. operate only under the already accepted request/world lease and cancellation/work context from AW-AH-009.3.2;
2. call the exact two-argument `CurriedCallableId::try_new`;
3. map `InvalidCurriedGroup { group, .. }` inline to:

   ```rust
   ResolveCallError::InvalidCallGroup {
       candidate: base.clone(),
       group,
   }
   ```

4. treat `InvalidCurriedBase` as an internal invariant failure and return `ResolveCallError::InvalidResolvedCallable`;
5. construct the canonical `CallableCandidateId::Curried` plus matching `CallableInstantiation::Curried` pair;
6. call `ResolvedCallable::try_new` with the full base schema;
7. publish `ResolveCallOutcome::Resolved` only from that successful result.

There SHALL be no blanket `From<CallableIdentityError> for ResolveCallError`, because the mapping is domain-specific and must retain the unwrapped base candidate.

No failed curried construction or missing group may fall back to a base-ID success, a legacy checker success, a signature-only resolver, or a second resolver.

## 10. Project, standard, and adapter ownership

The same final boundary applies uniformly:

- project base: `CallableCandidateId::Project(..)`;
- standard base: `CallableCandidateId::Environment(..)` whose owner is `EnvironmentCallableOwner::Standard(..)`;
- adapter base: `CallableCandidateId::Environment(..)` whose owner is `EnvironmentCallableOwner::Adapter(..)`.

No family-specific bypass or duplicate group-membership check is added. Provider identity changes neither the error variant nor the success representation.

## 11. Corrupt-world rule

Public catalog and schema constructors continue to prevent ordinary malformed publication. Defensive resolution SHALL nevertheless assume that internal accepted-world state could be inconsistent.

A preconstructed nonzero `CurriedCallableId` whose group is absent from the supplied record schema MUST be rejected by `ResolvedCallable::try_new` with `InvalidCallGroup`. The shared resolver MUST propagate that rejection as `ResolveCallOutcome::Rejected`. It MUST NOT repair the product, synthesize a group, slice another schema, retry another provider, or invoke the old resolver.

Tests may use crate-private typed fixtures to inject inconsistent resolver input. They SHALL NOT add an unchecked public constructor or inspect source text.

## 12. Rejected alternatives

### 12.1 Schema argument on the identity constructor — rejected

```rust
pub fn CurriedCallableId::try_new(
    base: CallableCandidateId,
    next_group: CallableGroupIndex,
    schema: &CallableSignatureSchema,
) -> Result<Self, CallableIdentityError>;
```

Rejected because it changes the fixed API, makes an identity constructor catalog-policy-dependent, couples construction to accepted-world evidence, and duplicates validation already owned by `ResolvedCallable::try_new`.

### 12.2 Separate validated curried resolver product — rejected

Rejected because `ResolvedCallable` already owns ID, origin, full schema, instantiation, equivalent sources, and authority. A second validated product would duplicate the successful representation or require an unnecessary conversion path.

### 12.3 Global or thread-local schema lookup — rejected

Rejected because identity construction would become world-dependent, hidden borrowing would cross the accepted-world boundary, deterministic tests would require ambient state, and corrupt-world behavior could bypass the explicit resolver contract.

## 13. Preserved substrate

Absent a separate concrete defect, the following remain unchanged:

- `CurriedCallableId` fields and accessors;
- `CallableGroupIndex` representation and constructors;
- `CallableCandidateId::Curried` and family unwrapping;
- `CallableInstantiation::Curried` fields;
- `ResolvedCallable` fields and public constructor signature;
- `CallableSignatureSchema` representation, contiguous-group constructor, and `group` accessor;
- `SignatureOrigin`, authority, equivalent-source, and limit contracts;
- accepted request/world validation;
- catalog publication and family schemas;
- old resolver behavior until its scheduled removal cut.

The only substrate redesign is the minimum correction demanded by two concrete flaws: a schema-membership-named identity error without schema evidence, and a duplicate successful curried representation.

## 14. Deletion rule

Temporary duplicate group-existence logic in the old checker SHALL remain only while that checker is the sole production resolver for the affected family. It SHALL be deleted in the same reviewable cut that proves all of the following:

1. the shared resolver is the sole successful production route for the migrated family;
2. every curried result passes through `ResolvedCallable::try_new`;
3. project, standard, adapter, positive multi-group, one-over, and corrupt-world tests pass;
4. checker target facts and query/signature facts consume the same resolved candidate ID;
5. no fallback to the old route remains.

The deletion proof SHALL use typed outcomes, call-target facts, tests, and structured dependency evidence—not source gates or spelling scans.

## 15. Explicit prohibitions

The implementation SHALL NOT introduce:

- a compatibility constructor or method;
- an alias or deprecated error variant;
- a dual reader or dual product;
- a second successful resolver;
- a source gate;
- an extension trait or ad hoc wrapper to avoid editing Arcweft-owned enums/types;
- a global catalog lookup or thread-local accepted world;
- schema/catalog/world data inside `CurriedCallableId`;
- a CSS or Takumi path;
- a bypass of accepted request/world validation.
