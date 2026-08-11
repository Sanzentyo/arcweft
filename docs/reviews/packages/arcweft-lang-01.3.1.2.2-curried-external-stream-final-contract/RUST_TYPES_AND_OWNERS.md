# Rust-shaped types and owners

All names below are normative. Parent-contract types included as opaque newtypes
are shown so this correction can be implemented without guessing ownership.

## 1. Core scalar identities

Owner: `arcweft-core`.

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct RuntimeCallableGroupIndex(u16);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct RuntimeCallableParameterIndex(u16);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeCallableParameterCoordinate {
    pub group: RuntimeCallableGroupIndex,
    pub parameter: RuntimeCallableParameterIndex,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct RuntimeCallableGroupCount(u16);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeCallableDeclarationDigest([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeExternalStreamSignatureFingerprint([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeDefaultExpressionFingerprint([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeTypeLayoutHash([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeValueDigest([u8; 32]);
```

`RuntimeCallableGroupIndex::try_from_usize` and
`RuntimeCallableParameterIndex::try_from_usize` are inherent checked constructors.
The compiler invokes them when projecting sema indices. No extension trait or
free-standing unchecked conversion is accepted.

Parent Stream identities remain their parent owners:

```rust
pub struct StreamDefinitionId([u8; 32]);
pub struct StreamInstanceId(u64);
pub struct GenerationId(u64);
```

## 2. Checked group-aware signature

Owner: `arcweft-core`; produced by `arcweft-compiler`; stored by RuntimePlan and
AWBC.

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeExternalStreamCallableSignature {
    pub definition: StreamDefinitionId,
    pub declaration: RuntimeCallableDeclarationDigest,
    pub groups: Vec<RuntimeExternalStreamParameterGroup>,
    pub result: RuntimeExternalStreamResult,
    pub effects: RuntimeEffectSetFingerprint,
    pub provider_abi: RuntimeProviderAbiFingerprint,
    pub fingerprint: RuntimeExternalStreamSignatureFingerprint,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeExternalStreamParameterGroup {
    pub index: RuntimeCallableGroupIndex,
    pub kind: RuntimeCallableGroupKind,
    pub parameters: Vec<RuntimeExternalStreamParameter>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeCallableGroupKind {
    Initial,
    Curried,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeExternalStreamParameter {
    pub coordinate: RuntimeCallableParameterCoordinate,
    pub name: Option<RuntimeParameterName>,
    pub passing: RuntimeParameterPassing,
    pub presence: RuntimeParameterPresence,
    pub ty: RuntimeTypeLayoutHash,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeParameterPassing {
    PositionalOnly,
    PositionalOrNamed,
    NamedOnly,
    RestPositional,
    RestNamed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeParameterPresence {
    Required,
    Optional,
    Defaulted {
        default: RuntimeDefaultExpressionFingerprint,
    },
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeParameterName(Box<str>);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeExternalStreamResult {
    pub item: RuntimeTypeLayoutHash,
    pub error: RuntimeTypeLayoutHash,
}
```

Signature validation is an inherent method on
`RuntimeExternalStreamCallableSignature`. It enforces:

- 1..=16 groups;
- contiguous group indices starting at zero;
- group 0 is `Initial`, all later groups are `Curried`;
- at most 128 parameters total;
- contiguous parameter indices within each group;
- every stored coordinate matches its vector positions;
- unique parameter names within the name-visible namespace of one group;
- at most one positional-rest and one named-rest parameter per group;
- a rest parameter uses `Required` presence and its declared type is the canonical
  rest aggregate element/value type; and
- the supplied fingerprint equals the canonical fingerprint defined in
  `FINGERPRINT_AND_HOT_RELOAD.md`.

## 3. Values and argument cells

Owner: `arcweft-core`.

```rust
#[derive(Debug, PartialEq)]
pub struct RuntimeCheckedArgumentValue {
    pub ty: RuntimeTypeLayoutHash,
    pub value: RuntimeValue,
    pub digest: RuntimeValueDigest,
}

#[derive(Debug, PartialEq)]
pub enum RuntimeExternalStreamArgumentValue {
    Explicit(RuntimeCheckedArgumentValue),
    Defaulted {
        default: RuntimeDefaultExpressionFingerprint,
        value: RuntimeCheckedArgumentValue,
    },
    OmittedOptional,
    RestPositional {
        item_ty: RuntimeTypeLayoutHash,
        items: Vec<RuntimeCheckedArgumentValue>,
    },
    RestNamed {
        value_ty: RuntimeTypeLayoutHash,
        entries: Vec<RuntimeNamedRestArgument>,
    },
}

#[derive(Debug, PartialEq)]
pub struct RuntimeNamedRestArgument {
    pub name: RuntimeParameterName,
    pub value: RuntimeCheckedArgumentValue,
}
```

`RuntimeCheckedArgumentValue` is not an alternate payload domain. Its `value` is
the existing runtime value and `ty`/`digest` are checked evidence used at the
boundary. Its duplication behavior delegates to the existing ABI-2 affine-value
owner; it does not unconditionally implement `Clone`.

Disposition legality is exact:

| Passing/presence | Accepted argument value |
| --- | --- |
| non-rest + required | `Explicit` |
| non-rest + optional | `Explicit` or `OmittedOptional` |
| non-rest + defaulted | `Explicit` or `Defaulted` with matching fingerprint |
| positional-rest + required | `RestPositional` |
| named-rest + required | `RestNamed` |
| every other combination | rejected |

## 4. Prefix/full argument product

Owner: `arcweft-core`.

```rust
#[derive(Debug, PartialEq)]
pub struct RuntimeExternalStreamArgumentProduct {
    pub definition: StreamDefinitionId,
    pub declaration: RuntimeCallableDeclarationDigest,
    pub generation: GenerationId,
    pub signature: RuntimeExternalStreamSignatureFingerprint,
    pub completed_groups: RuntimeCallableGroupCount,
    pub coordinates: Vec<RuntimeCallableParameterCoordinate>,
    pub values: Vec<RuntimeExternalStreamArgumentValue>,
}
```

Inherent methods:

```rust
impl RuntimeExternalStreamArgumentProduct {
    pub fn empty_for(
        signature: &RuntimeExternalStreamCallableSignature,
        generation: GenerationId,
    ) -> Self;

    pub fn validate_prefix(
        &self,
        signature: &RuntimeExternalStreamCallableSignature,
        live_generations: &RuntimeLiveGenerationSet,
    ) -> Result<RuntimeValueOwnership, RuntimeExternalStreamArgumentError>;

    pub fn validate_complete(
        &self,
        signature: &RuntimeExternalStreamCallableSignature,
        live_generations: &RuntimeLiveGenerationSet,
    ) -> Result<RuntimeValueOwnership, RuntimeExternalStreamArgumentError>;

    pub fn try_join_next_group(
        &self,
        signature: &RuntimeExternalStreamCallableSignature,
        group: RuntimeCallableGroupIndex,
        arguments: RuntimeExternalStreamEvaluatedGroup,
        live_generations: &RuntimeLiveGenerationSet,
    ) -> Result<Self, RuntimeExternalStreamArgumentError>;
}
```

`try_join_next_group` builds and validates a new owner before replacing the old
owner. It never sorts an untrusted product into validity. Incoming coordinates
must already be canonical; a reordered input is rejected.

## 5. Evaluation plan

Owner: `arcweft-core` RuntimePlan types; produced only by `arcweft-compiler` from
accepted sema facts.

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeExternalStreamGroupApplicationPlan {
    pub definition: StreamDefinitionId,
    pub declaration: RuntimeCallableDeclarationDigest,
    pub signature: RuntimeExternalStreamSignatureFingerprint,
    pub group: RuntimeCallableGroupIndex,
    pub authored_evaluation: Vec<RuntimeExternalStreamAuthoredArgumentPlan>,
    pub canonical_slots: Vec<RuntimeExternalStreamArgumentSlotPlan>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeExternalStreamAuthoredArgumentPlan {
    pub source_ordinal: u16,
    pub coordinate: RuntimeCallableParameterCoordinate,
    pub expression: RuntimeExpr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeExternalStreamArgumentSlotPlan {
    pub coordinate: RuntimeCallableParameterCoordinate,
    pub source: RuntimeExternalStreamArgumentSourcePlan,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeExternalStreamArgumentSourcePlan {
    Authored { source_ordinal: u16 },
    Defaulted {
        default: RuntimeDefaultExpressionFingerprint,
        expression: RuntimeExpr,
    },
    OmittedOptional,
    RestPositional { source_ordinals: Vec<u16> },
    RestNamed { source_ordinals: Vec<u16> },
}
```

`authored_evaluation` is strictly increasing by source ordinal. `canonical_slots`
is strictly increasing by coordinate. Every authored ordinal is referenced exactly
once by one canonical slot. The two orders are intentionally distinct.

## 6. Function-value owner

Owner: the existing `arcweft-core::value::RuntimeFunctionValue` type.

```rust
#[derive(Debug, PartialEq)]
pub enum RuntimeFunctionValue {
    Closure(RuntimeClosureValue),
    ExternalStreamPartial(RuntimeExternalStreamPartialFunction),
}

#[derive(Debug, PartialEq)]
pub struct RuntimeClosureValue {
    pub params: Vec<String>,
    pub body: RuntimeFunctionBody,
    pub captures: Vec<RuntimeBinding>,
}

#[derive(Debug, PartialEq)]
pub struct RuntimeExternalStreamPartialFunction {
    pub definition: StreamDefinitionId,
    pub declaration: RuntimeCallableDeclarationDigest,
    pub generation: GenerationId,
    pub signature: RuntimeExternalStreamSignatureFingerprint,
    pub next_group: RuntimeCallableGroupIndex,
    pub captured: RuntimeExternalStreamArgumentProduct,
    pub ownership: RuntimeValueOwnership,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeValueOwnership {
    Unrestricted,
    Affine,
}
```

Inherent behavior is implemented on `RuntimeFunctionValue` and
`RuntimeExternalStreamPartialFunction`, not in an external trait:

```rust
pub enum RuntimeExternalStreamApplication {
    Partial(RuntimeExternalStreamPartialFunction),
    Open(RuntimeStreamOpenCommit),
}

impl RuntimeFunctionValue {
    pub fn ownership(&self) -> RuntimeValueOwnership;

    pub fn try_apply_external_stream_group(
        self,
        definition: &RuntimeStreamDefinition,
        plan: &RuntimeExternalStreamGroupApplicationPlan,
        evaluated: RuntimeExternalStreamEvaluatedGroup,
        runtime: &mut RuntimeStreamOpenTransaction<'_>,
    ) -> Result<RuntimeExternalStreamApplication, RuntimeFunctionApplicationError>;
}
```

The transaction is prepared and validated before it receives mutable access to the
live instance table/request batch. For an unrestricted partial the runtime may
produce a duplicable equivalent according to existing value rules. For an affine
partial, `self` is the sole transfer owner.

## 7. Open request and atomic commit

Owner: `arcweft-core`; adapters only serialize/execute it.

```rust
#[derive(Debug, PartialEq)]
pub enum RuntimeStreamRequest {
    Open(RuntimeStreamOpenRequest),
    Close(RuntimeStreamCloseRequest),
}

#[derive(Debug, PartialEq)]
pub struct RuntimeStreamOpenRequest {
    pub definition: StreamDefinitionId,
    pub declaration: RuntimeCallableDeclarationDigest,
    pub generation: GenerationId,
    pub instance: StreamInstanceId,
    pub signature: RuntimeExternalStreamSignatureFingerprint,
    pub capability: RuntimeCapabilityId,
    pub operation: RuntimeOperationId,
    pub arguments: RuntimeExternalStreamArgumentProduct,
    pub policy: StreamPolicy,
}

#[derive(Debug, PartialEq)]
pub struct RuntimeStreamOpenCommit {
    pub handle: RuntimeStreamHandle,
    pub instance_state: StreamInstanceState,
    pub request: RuntimeStreamOpenRequest,
}
```

`RuntimeStreamOpenCommit::try_prepare` validates everything and reserves no live
ID. The runtime's commit method then allocates an ID and inserts/appends all three
outputs in one non-fallible mutation block. If ID allocation can fail, it is
performed before any table/request mutation.

## 8. Typed errors

Owner: the type whose invariant failed. The closed minimum set is:

```rust
pub enum RuntimeExternalStreamArgumentError {
    WrongDefinition,
    ForeignDeclaration,
    StaleGeneration,
    SignatureMismatch,
    GroupOutOfRange,
    GroupNotNext,
    CompletedGroupCountMismatch,
    CoordinateValueLengthMismatch,
    MissingCoordinate(RuntimeCallableParameterCoordinate),
    DuplicateCoordinate(RuntimeCallableParameterCoordinate),
    UnknownCoordinate(RuntimeCallableParameterCoordinate),
    OutOfOrderCoordinate {
        previous: RuntimeCallableParameterCoordinate,
        actual: RuntimeCallableParameterCoordinate,
    },
    IllegalDisposition(RuntimeCallableParameterCoordinate),
    DefaultFingerprintMismatch(RuntimeCallableParameterCoordinate),
    TypeMismatch(RuntimeCallableParameterCoordinate),
    MalformedPositionalRest(RuntimeCallableParameterCoordinate),
    DuplicateNamedRestEntry {
        coordinate: RuntimeCallableParameterCoordinate,
        name: RuntimeParameterName,
    },
    OutOfOrderNamedRestEntry(RuntimeCallableParameterCoordinate),
    LimitExceeded(RuntimeExternalStreamArgumentLimit),
    AffineOwnership(RuntimeAffineOwnershipError),
}

pub enum RuntimeFunctionApplicationError {
    NotExternalStreamCallable,
    Argument(RuntimeExternalStreamArgumentError),
    InstanceIdOverflow,
}
```

Errors carry typed identities/coordinates. Display text is diagnostic only and is
never parsed to select behavior.
