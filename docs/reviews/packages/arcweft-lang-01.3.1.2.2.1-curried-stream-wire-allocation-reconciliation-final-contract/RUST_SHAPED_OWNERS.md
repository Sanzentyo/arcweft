# Rust-shaped owners and exact replacement boundary

These declarations are design shapes, not a repository patch. Names and field
order are normative.

## 1. Parent identity owners retained

```rust
pub struct RuntimeStreamDefinitionId(pub u32);       // static table index
pub struct RuntimeStreamDefinitionKey(pub [u8; 32]); // stable semantic key
pub struct StreamGeneration(pub u64);
pub struct StreamInstanceOrdinal(pub u64);

pub struct StreamInstanceKey {
    pub definition_key: RuntimeStreamDefinitionKey,
    pub generation: StreamGeneration,
    pub ordinal: StreamInstanceOrdinal,
}

// Existing owner in arcweft-core::entry.
pub struct TypeLayoutHash([u8; 32]);
```

`StreamDefinitionId`, `GenerationId`, `StreamInstanceId`, and
`RuntimeTypeLayoutHash` are not declared or aliased.

## 2. Sole group coordinate types

```rust
#[repr(transparent)]
pub struct RuntimeCallableGroupIndex(u16);

#[repr(transparent)]
pub struct RuntimeCallableParameterIndex(u16);

pub struct RuntimeCallableParameterCoordinate {
    pub group: RuntimeCallableGroupIndex,
    pub parameter: RuntimeCallableParameterIndex,
}

#[repr(transparent)]
pub struct RuntimeCallableGroupCount(u16);
```

Checked conversion and advancement are inherent methods on these types. There
is no unchecked free helper or extension trait.

## 3. In-place general callable owner

Owner: `arcweft-core::entry`.

```rust
pub struct RuntimeCallableBoundarySignature {
    pub callable: RuntimeCallableId,
    pub contract: CallableContractHash,
    pub groups: Vec<RuntimeCallableParameterGroup>,
    pub result: RuntimeValueTypeContract,
    pub effects: RuntimeEffectSetId,
}

pub struct RuntimeCallableParameterGroup {
    pub index: RuntimeCallableGroupIndex,
    pub kind: RuntimeCallableGroupKind,
    pub parameters: Vec<RuntimeCallableParameter>,
}

pub enum RuntimeCallableGroupKind {
    Initial,
    Curried,
}

pub struct RuntimeCallableParameter {
    pub coordinate: RuntimeCallableParameterCoordinate,
    pub name: Option<RuntimeParameterName>,
    pub ty: RuntimeValueTypeContract,
    pub passing: RuntimeParameterPassing,
    pub presence: RuntimeParameterPresence,
}

pub enum RuntimeParameterPresence {
    Required,
    Optional,
    Defaulted { default: RuntimeDefaultExpressionFingerprint },
}
```

Exact replacement:

- remove `RuntimeParameterIndex(u32)`;
- replace `RuntimeCallableParameter.index` with `coordinate`;
- replace `RuntimeCallableBoundarySignature.parameters` with `groups`;
- enrich the existing `RuntimeParameterPresence::Defaulted` variant in place
  with its fingerprint;
- do not add the child `RuntimeExternalStreamCallableSignature`/group/parameter/
  result family.

`RuntimeStreamDefinition.callable` continues to point at this sole owner.
External origin metadata retains module/module ABI/capability/operation and
adds the accepted declaration digest and external signature fingerprint. The
definition's item/error contracts are the only Stream result shape; no
`RuntimeExternalStreamResult` is introduced.

## 4. Sole canonical external product

```rust
pub struct RuntimeExternalStreamArgumentProduct {
    pub definition: RuntimeStreamDefinitionKey,
    pub declaration: RuntimeCallableDeclarationDigest,
    pub generation: StreamGeneration,
    pub signature: RuntimeExternalStreamSignatureFingerprint,
    pub completed_groups: RuntimeCallableGroupCount,
    pub coordinates: Vec<RuntimeCallableParameterCoordinate>,
    pub values: Vec<RuntimeExternalStreamArgumentValue>,
}
```

This exact owner is retained across RuntimePlan evaluation, partial capture,
final Open, host serialization, save/restore, and fingerprinting. It is never
flattened into `RuntimeResolvedArguments`, `RuntimeHostResolvedArguments`, or
`Vec<RuntimePayload>`.

## 5. Sole function-value owner

Owner: existing `arcweft-core::value::RuntimeFunctionValue`, changed in place.

```rust
pub enum RuntimeFunctionValue {
    Closure(RuntimeClosureValue),
    ExternalStreamPartial(RuntimeExternalStreamPartialFunction),
}

pub struct RuntimeExternalStreamPartialFunction {
    pub definition: RuntimeStreamDefinitionKey,
    pub declaration: RuntimeCallableDeclarationDigest,
    pub generation: StreamGeneration,
    pub signature: RuntimeExternalStreamSignatureFingerprint,
    pub next_group: RuntimeCallableGroupIndex,
    pub captured: RuntimeExternalStreamArgumentProduct,
    pub ownership: RuntimeValueOwnership,
}
```

Apply/ownership/snapshot behavior is inherent on this owner. No auxiliary
external partial enum or trait-based behavior dispatch is permitted.

## 6. Shared Open request owner

Owner: parent `arcweft-core::stream::RuntimeStreamRequest`, changed in place.

```rust
pub enum RuntimeStreamRequest {
    Open {
        request: StreamRequestId,
        instance: StreamInstanceKey,
        definition: RuntimeStreamDefinitionKey,
        declaration: RuntimeCallableDeclarationDigest,
        signature: RuntimeExternalStreamSignatureFingerprint,
        module: RuntimeExternalModuleId,
        module_abi: RuntimeExternalModuleAbiHash,
        capability: RuntimeCapabilityId,
        operation: RuntimeOperationId,
        arguments: RuntimeExternalStreamArgumentProduct,
        item_layout: TypeLayoutHash,
        error_layout: TypeLayoutHash,
        policy: ResolvedExternalStreamPolicy,
    },
    Close {
        request: StreamRequestId,
        instance: StreamInstanceKey,
        reason: StreamCloseReason,
    },
}
```

Invariants:

- `definition == instance.definition_key`;
- `arguments.definition == definition`;
- `arguments.generation == instance.generation`;
- declaration/signature/module ABI match the loaded external definition;
- every transmitted argument value is host-payload eligible and contains no
  affine Stream handle or runtime-only owner;
- adapters serialize this owner directly and add no endpoint DTO.

## 7. AWBC instruction owner

Owner: existing `arcweft-core::awbc::schema::AwbcInstruction` and
`AwbcOpcode`, both changed in place.

```rust
pub enum AwbcInstruction {
    OpenStream {
        dst: AwbcRegisterId,
        callee: AwbcRegisterId,
        definition: AwbcStreamDefinitionId,
        signature: AwbcCallableSignatureId,
        group: u16,
        arguments: AwbcExternalStreamGroupArguments,
    },
    FinishStream {
        stream: AwbcRegisterId,
        outcome: AwbcStreamProducerOutcome,
    },
    ApplyExternalStreamGroup {
        dst: AwbcRegisterId,
        callee: AwbcRegisterId,
        definition: AwbcStreamDefinitionId,
        signature: AwbcCallableSignatureId,
        group: u16,
        arguments: AwbcExternalStreamGroupArguments,
    },
}
```

`AwbcCallableSignature`/group/parameter rows are a serialized projection of
`RuntimeCallableBoundarySignature`. They are not a second parameter-schema
authority.
