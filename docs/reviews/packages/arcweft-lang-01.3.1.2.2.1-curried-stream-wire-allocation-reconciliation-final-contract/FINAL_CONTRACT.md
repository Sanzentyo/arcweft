# FINAL CONTRACT — Lang-01.3.1.2.2.1

## 1. Scope and authority

This contract narrowly reconciles Lang-01.3.1.2.1 and Lang-01.3.1.2.2. It
preserves the accepted shared callable resolver/accounting, typed group
coordinates, canonical curried argument product, direct suspension,
`RuntimeFunctionValue::ExternalStreamPartial`, affine Stream table,
replay/tombstone policy, generator classification, and Source-elimination
direction. Only the contradictory wire allocation, duplicate owner spellings,
flat/general callable boundary seam, and their dependent rows are corrected.

Current production evidence was inspected at pushed `main` Git commit
`0b7e095f4193b9f7fbbc95cc350a626a8a63640a`. The matching Jujutsu Git-backend commit identity is the same object
ID and is exactly addressable as `commit_id("0b7e095f4193b9f7fbbc95cc350a626a8a63640a")`.

Normative keywords MUST, MUST NOT, SHALL, and SHALL NOT are binding.

## 2. Closed codec-8 allocation

The complete non-terminator Stream instruction allocation is:

| Opcode | Instruction | Status |
| ---: | --- | --- |
| `0x27` | `OpenStream` | retained parent lifecycle meaning; child payload replaces flat payload |
| `0x28` | `FinishStream` | retained parent lifecycle meaning and payload |
| `0x29` | `ApplyExternalStreamGroup` | child group-application meaning moved to next unused byte |

The implementation SHALL add all three rows to the owning `AwbcOpcode` and
`AwbcInstruction` implementations in one codec-8 cut. It SHALL NOT implement
dispatch with an ad hoc helper, extension trait, compatibility table, or
second reader.

This allocation is frozen for the inspected main. If a later pushed main has
consumed `0x29` before implementation, implementation is blocked and requires
a new explicit design reconciliation; implementers SHALL NOT locally
renumber any row.

Unassigned bytes `0x2a..=0x7f` remain unknown instruction opcodes. They have no
implicit or reserved instruction meaning.

## 3. Removed and retained instruction bytes

Codec 8 SHALL reject exactly these removed instruction bytes as unknown:

- `0x1c` — removed `StreamYield` instruction;
- `0x1d` — removed `StreamClose` instruction;
- `0x1e` — removed `SourceClose` instruction; and
- `0x20` — removed `SourceYield` instruction.

Existing `0x1f=Drop`, `0x21=AssignField`, `0x22=CallTraitMethod`,
`0x23=RegisterCleanup`, `0x24=CancelCleanup`, `0x25=MakeFunction`, and
`0x26=ApplyFunction` remain valid. In particular, child statements and tests
treating `0x22`/`0x23` as removed Source instructions are superseded.

Old function-kind tags 3, 4, and 5 remain unknown under the parent codec-8
lifecycle contract; this correction does not change that separate tag family.

## 4. Exact Rust instruction variants

The owning enum is changed in place and lists the new variants in numeric
opcode order:

```rust
pub enum AwbcInstruction {
    // existing retained variants through ApplyFunction
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

pub struct AwbcExternalStreamGroupArguments {
    pub coordinates: Vec<AwbcCallableParameterCoordinate>,
    pub values: Vec<AwbcExternalStreamArgumentOperand>,
}

pub struct AwbcCallableParameterCoordinate {
    pub group: u16,
    pub parameter: u16,
}

pub enum AwbcExternalStreamArgumentOperand {
    Explicit { value: AwbcRegisterId },
    Defaulted { default: AwbcDigest, value: AwbcRegisterId },
    OmittedOptional,
    RestPositional { value: AwbcRegisterId },
    RestNamed { value: AwbcRegisterId },
}

pub enum AwbcStreamProducerOutcome {
    Complete,
    Fail { error: AwbcRegisterId },
    Cancelled,
}
```

No deprecated or alternate variant exists.

## 5. Exact binary encoding

All instruction fields are encoded in the Rust declaration order above.

- opcode and nested enum tags: one `u8`;
- `u16`: fixed two-byte little-endian, using the current codec `Wire for u16`;
- register/table IDs and vector lengths: current canonical unsigned base-128
  `u32` varint;
- `u64`: fixed eight-byte little-endian;
- digest: 32 raw bytes;
- vectors: canonical varint length followed by elements.

`OpenStream` and `ApplyExternalStreamGroup` are:

```text
opcode
dst
callee
definition
signature
group:u16-le
coordinate_count
  repeated { coordinate.group:u16-le, coordinate.parameter:u16-le }
value_count
  repeated operand-tag + operand-payload
```

`FinishStream` is:

```text
0x28
stream
outcome-tag
  Complete(0): no payload
  Fail(1): error register
  Cancelled(2): no payload
```

Operand tags are `0 Explicit`, `1 Defaulted`, `2 OmittedOptional`,
`3 RestPositional`, and `4 RestNamed`. Unknown tags reject at the tag byte.
Coordinates and values SHALL pass independent decode budgets before they are
paired; unequal lengths reject. The decoder/verifier SHALL NOT sort or repair
malformed vectors.

## 6. Verifier and execution rules

Verification order is header/version, table budgets/ranges, signature/group
structure, parameter legality/fingerprints, function/frame/register structure,
then instruction-specific rules.

Common rules for Open/Apply:

1. `definition` exists and has external origin.
2. `signature` is exactly the callable signature referenced by that definition.
3. `callee` has static type
   `ExternalStreamCallable { definition, next_group: group }`.
4. `group` is in range.
5. Coordinates equal the complete declared coordinate list for the group,
   including an empty list for an empty group; they are unique and strictly
   increasing.
6. Coordinate/value lengths are equal and within the 128-cell bound.
7. Each disposition is legal for the declared passing/presence mode.
8. Each register has the exact value/rest aggregate type.
9. Each default digest equals the parameter metadata.

Apply-specific rules:

- `group + 1 < group_count`;
- `dst` has type
  `ExternalStreamCallable { definition, next_group: group + 1 }`;
- execution joins exactly this group into the captured canonical product,
  advances once, emits no Stream request, and allocates no instance.

Open-specific rules:

- `group + 1 == group_count`;
- `dst` has the exact `StreamHandle { item, error }` type from the definition;
- prefix plus current group forms a complete product;
- after all dynamic generation, affine, payload, capacity, and allocation
  checks pass, instance ordinal/key allocation, Opening state insertion,
  handle production, and one Open request append form one non-fallible atomic
  commit.

Finish-specific static rules:

- owning function kind is `GeneratorProducer`;
- owning function has both `MAY_SUSPEND` and `OWNS_STREAM_PRODUCER`;
- `stream` is a matching `StreamHandle` register;
- `Fail.error` has the definition error type;
- no reachable path can commit a second producer terminal outcome.

Finish-specific dynamic ownership rules:

- current `FiberState.producer_stream` exists;
- its key equals the `stream` value key;
- the sole table entry reciprocally names
  `StreamProducerOwner::Fiber { fiber: current_fiber, lease }` with the same
  producer lease and is `LocalRunning`;
- the loaded definition's producer function is the current producer function;
- failure of any check traps/rejects before terminal mutation.

`Complete`, `Fail`, and `Cancelled` then use the retained parent lifecycle,
replay, terminal, and tombstone behavior. No new scheduler or lifecycle table
is introduced.

## 7. Identity reconciliation

The final owners are:

| Meaning | Final type |
| --- | --- |
| RuntimePlan/AWBC table index | `RuntimeStreamDefinitionId(u32)` / `AwbcStreamDefinitionId(u32)` |
| Stable semantic definition identity | `RuntimeStreamDefinitionKey([u8;32])` |
| Loaded/runtime generation | `StreamGeneration(u64)` |
| Allocation cursor component | `StreamInstanceOrdinal(u64)` |
| Complete live identity | `StreamInstanceKey { definition_key, generation, ordinal }` |
| Runtime type layout | existing `TypeLayoutHash([u8;32])` |

The child spellings `StreamDefinitionId`, `GenerationId`, `StreamInstanceId`,
and `RuntimeTypeLayoutHash` SHALL NOT be declared as types, aliases, serde
aliases, or adapter wrappers.

`RuntimeExternalStreamArgumentProduct.definition` and
`RuntimeExternalStreamPartialFunction.definition` are
`RuntimeStreamDefinitionKey`. Their generation fields are `StreamGeneration`.

The parent `RuntimeStreamDefinitionKey` domain remains
`arcweft.stream.definition.v1 `, but transcript step 8 is replaced exactly by:

```text
group_count: canonical u32 varint
for each group in group-index order:
  group.index: u16 little-endian
  group.kind: u8 (0 Initial, 1 Curried)
  parameter_count: canonical u32 varint
  for each parameter in parameter-index order:
    coordinate.group: u16 little-endian
    coordinate.parameter: u16 little-endian
    optional_name: existing Option/string encoding
    type_layout_hash: 32 raw bytes
    passing: u8 (0..4)
    presence: u8 (0 Required, 1 Optional, 2 Defaulted)
    if Defaulted: default_expression_fingerprint: 32 raw bytes
```

Parent transcript steps 1--7 and 9 remain unchanged. The old flat worked
transcript byte count/hash is superseded. This nested transcript, not a flat
equivalent, determines the stable definition key.
`RuntimeStreamRequest::Open.instance` is `StreamInstanceKey`; it has no bare
ordinal identity and no redundant top-level generation field. Its argument
product generation MUST equal `instance.generation`, and its definition key
MUST equal `instance.definition_key`.

Static RuntimePlan, AWBC, and bundle records contain definition index/key
mappings, not live instance keys. Runtime allocation forms the first
`StreamInstanceKey`; host requests, live state, save, and restore carry that
exact key. Restore validates it against the definition mapping loaded from the
bundle rather than substituting a current generation or new ordinal.

## 8. Sole group-aware callable boundary owner

The final semantic/runtime owner is the parent type
`arcweft-core::entry::RuntimeCallableBoundarySignature`. It is changed in
place. Its old flat `parameters` field and `RuntimeParameterIndex` are removed.

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

Group 0 is `Initial`; groups 1.. are `Curried`; indices are contiguous; total
parameters are at most 128; group count is 1..=16. Each stored coordinate
equals its vector position. These are inherent validation methods on the
owning types.

The child `RuntimeExternalStreamCallableSignature`,
`RuntimeExternalStreamParameterGroup`, `RuntimeExternalStreamParameter`, and
`RuntimeExternalStreamResult` are not implemented. External binding facts are
retained by the existing `RuntimeStreamDefinition` and its External origin:
the definition keeps the sole boundary signature, item/error contracts,
module ABI, declaration digest, and external signature fingerprint. The
callable result MUST equal the definition's derived Stream handle type.

The compiler performs one projection:

```text
accepted sema CallableSignatureSchema + CallTargetFacts coordinates
  -> RuntimeCallableBoundarySignature
  -> RuntimePlan/AWBC metadata and RuntimeExternalStreamArgumentProduct
```

There is no name re-binding, flat external parameter vector, or flattening
adapter. Parent `RuntimeResolvedArguments<T>` may remain for unrelated flat
boundaries, but it is not accepted by any external Stream definition, Open
constructor, host request, AWBC instruction, save snapshot, or adapter.

`AwbcCallableSignature`, group rows, and parameter rows are the canonical codec
projection of this owner, not a second semantic schema. Ordinary frame
`AwbcSignature` remains the frame-local ABI shape selected by the child
contract.

## 9. Sole function-value and argument-product owners

Existing `arcweft-core::value::RuntimeFunctionValue` changes in place to the
closed enum:

```rust
pub enum RuntimeFunctionValue {
    Closure(RuntimeClosureValue),
    ExternalStreamPartial(RuntimeExternalStreamPartialFunction),
}
```

Behavior such as ownership and group application is implemented as inherent
methods on this owner and its owned partial variant, not an extension trait or
external match helper. Because an external partial may capture affine values,
the public function-value owner does not expose an unconditional duplicating
`Clone`; transfer, snapshot-candidate copying, and unrestricted duplication use
the retained affine-aware value owner and its checked operations.

The sole group product is:

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

It is used unchanged across partial capture, final Open, host serialization,
save, restore, and fingerprinting. Adapters serialize the shared typed owner
directly and do not rebuild or flatten it.

## 10. Compile-clean implementation interleave

The only accepted interleave for Lang-01.3.1.2.1 Cuts 3--8 and
Lang-01.3.1.2.2 Cuts 1--6 is:

1. **P3** — land shared accepted-sema external binding evidence only. No
   Stream runtime or wire publication.
2. **P4 + C1** — in one compile-clean core cut, land parent identities,
   lifecycle/table owners, the grouped in-place callable boundary, the sole
   canonical product, and the one `RuntimeFunctionValue` enum. Update every
   core constructor/match/serde traversal in the cut. No flat external product
   or second function-value shape may compile afterward.
3. **P5 + C2** — land RuntimePlan definition tables and the sole compiler
   projection from accepted sema groups/coordinates. No runtime name lookup or
   flattening projection exists.
4. **C3 over P4/P5** — land structured group application and atomic final Open
   on the parent table/lifecycle. Product codec remains version 7 during this
   cut; no codec-8 reader/writer is published.
5. **P6 + C4** — one non-separable ABI-2/codec-8 cut installs all tables,
   runtime/constant tags, `0x27/0x28/0x29`, removed-byte rejection, verifier,
   VM, lowering, and compiled-region parity. ABI/codec constants change only
   in this complete cut.
6. **P7 + C5** — land the shared host request/serde owner using
   `StreamInstanceKey` and the canonical product. Native/Web/Agent add no DTO.
7. **P8 + C6** — land bundle/save/restore/hot-reload fingerprints, partial
   snapshots, generation pins, and blockers, then run the full matrix.

Steps 5--7 form the parent protected migration group and SHALL merge to main
only after all three are complete and validated. Review commits may exist, but
no pushed/released main state may advertise codec 8 with an incomplete table,
flat external product, or two function-value shapes.

## 11. Compatibility and non-goals

There is no compatibility alias, shim, migration adapter, dual reader/writer,
serde alias, endpoint DTO, feature/source gate, removed-syntax diagnostic,
CSS path, or Takumi path. `arcweft-core` and data-format owners remain Sans I/O.
This contract does not redesign callable selection/accounting, direct
suspension, coordinate semantics, affine lifecycle, queue/replay/tombstone
policy, generator classification, or Source-elimination direction.
