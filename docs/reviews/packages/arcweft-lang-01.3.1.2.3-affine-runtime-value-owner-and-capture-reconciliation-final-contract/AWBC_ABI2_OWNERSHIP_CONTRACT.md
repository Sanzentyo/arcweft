# AWBC ABI-2 ownership contract

This file closes the generic ownership behavior assumed by the accepted codec-8 Stream contracts. It changes the existing AWBC owners in place. There is one register/frame model for all runtime values; Stream does not gain a separate register file, affine side table, or verifier.

## 1. Version and allocation

The accepted parent values remain:

```text
AWBC_ABI_VERSION = 2
AWBC_CODEC_VERSION = 8
```

The accepted Stream instructions remain:

```text
0x27 OpenStream
0x28 FinishStream
0x29 ApplyExternalStreamGroup
```

Lang-01.3.1.2.3 allocates the next generic instruction:

```rust
// Existing arcweft-core::awbc::schema owners, extended in place.
pub enum AwbcInstruction {
    // existing variants
    CopyValue {
        dst: AwbcRegisterId,
        src: AwbcRegisterId,
    },
}
```

```text
opcode 0x2a
wire   2a <dst: canonical unsigned base-128 varu32>
          <src: canonical unsigned base-128 varu32>
```

This supersedes only the .2.1 statement that `0x2a` is unknown. `0x2b..=0x7f` remain unknown. The codec rejects unknown opcodes, noncanonical varints, truncation, trailing bytes, and budget excess. No codec-7 or provisional codec-8 reader recognizes `0x2a`.

`Move` and `Drop=0x1f` retain their existing opcode numbers. Their semantics are corrected below; no duplicate replacement instruction is allocated.

## 2. Static register facts

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AwbcRegisterState {
    Uninitialized,
    Live {
        ty: AwbcTypeId,
        ownership: RuntimeValueOwnership,
    },
    Moved {
        ty: AwbcTypeId,
    },
    Dropped {
        ty: AwbcTypeId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AwbcOwnershipState {
    pub registers: Box<[AwbcRegisterState]>,
    pub cleanup: AwbcCleanupState,
    pub transaction: AwbcOwnershipTransactionState,
}
```

The verifier computes static ownership from final runtime type/layout tables. A type is statically unrestricted only when its complete closed layout cannot contain an affine leaf. Open/opaque/generic runtime types that may contain a handle are affine unless admitted by exact typed evidence. Runtime checks recompute actual value ownership and reject a mismatch before mutation.

`Moved` and `Dropped` are distinct for diagnostics and cleanup verification. A later explicit initialization may overwrite a terminal slot only where the existing instruction contract declares that register as a destination and the verifier proves no cleanup obligation remains.

## 3. Fundamental instruction transitions

### 3.1 `CopyValue`

Preconditions:

```text
src = Live { ty=T, ownership=Unrestricted }
dst = Uninitialized or an explicitly dead/reusable destination
src != dst
no pending ownership transaction
```

Postcondition:

```text
src remains Live { T, Unrestricted }
dst becomes Live { T, Unrestricted }
```

Runtime calls `try_duplicate_unrestricted` into a staged value. If actual ownership is affine, value/layout validation fails, or destination is invalid, the instruction traps before changing either register, cleanup state, table, or observation stream.

### 3.2 existing `Move`

Preconditions:

```text
src = Live { ty=T, ownership=O }
dst is empty/reusable
src != dst
```

Postcondition:

```text
src = Moved { T }
dst = Live { T, O }
```

The VM removes the original value from `src`; it never calls `clone`. Affine token and Stream lease move with the value unchanged. A source registered for cleanup transfers that cleanup obligation to the destination or is re-registered exactly as required by the existing frame layout; two obligations never point to one owner.

### 3.3 existing `Drop=0x1f`

Preconditions:

```text
reg = Live { ty=T, ownership=O }
no active borrow or conflicting transfer
prepared table-aware drop succeeds
```

Postcondition:

```text
reg = Dropped { T }
all nested affine releases committed exactly once
cleanup obligation discharged
```

`Drop` uses the generic prepared drop and Stream table owner. It does not Rust-drop a live handle, rotate a lease, or defer uniqueness to a later verifier pass.

## 4. Operand-use rule

Each instruction schema classifies each register operand as exactly one of:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AwbcOperandUse {
    Borrow,
    Copy,
    Consume,
    Destination,
}
```

This classification is inherent metadata on `AwbcInstruction`/the owning opcode enum, not an extension trait or external match table. `Copy` operands lower to or internally use the same checked `CopyValue` semantics; by-value reusable source code normally receives an explicit `CopyValue` from the lowerer. `Consume` transitions source to `Moved`. `Borrow` cannot escape the instruction or safe point. `Destination` must be empty/reusable.

The final general rules are:

- scalar tests, branch conditions, equality, tag/length inspection: Borrow;
- aggregate/function/variant/sequence construction operands: Consume;
- by-value call callee/arguments and return value: Consume;
- closure/fiber capture: per typed `Copy | Move` capture plan;
- cleanup registration: Borrow slot identity and establish obligation, not value duplication;
- Stream Apply/Open callee and all register-bearing argument operands: Consume;
- `OmittedOptional`: no register/no owner;
- an authored reuse is represented by `CopyValue` before the consuming operation.

## 5. Aggregate and function instructions

Every existing make-tuple/record/sequence/variant/function/call/partial instruction is migrated so its by-value operands are removed from source registers and installed in the destination/frame. Internal helper code may not preserve sources through `.clone()`.

A constructor preflights all source facts/destination/capacity, then takes operands in encoded declaration order and publishes one aggregate. On trap, no source moves. The destination ownership is the join of operand ownership.

`MakeFunction` consumes/stages exactly the capture plan encoded/referenced by final RuntimePlan/AWBC capture metadata. It has no ambient environment input. A `Copy` capture requires statically unrestricted source and retains it; a `Move` capture consumes it. Capture destination order is exact `RuntimeCaptureSlot` order.

Ordinary `ApplyFunction`/call consumes the callee value and by-value arguments into a frame. Where callable invocation semantically retains/reuses an unrestricted function, lowering emits `CopyValue` first. Return consumes one callee-frame register into the caller destination before frame cleanup.

## 6. External Stream instruction transitions

The accepted exact wire fields remain unchanged.

### 6.1 `ApplyExternalStreamGroup = 0x29`

```text
inputs:
  callee register                          Consume
  each Explicit value register            Consume
  each Defaulted value register           Consume
  each RestPositional aggregate register  Consume
  each RestNamed aggregate register       Consume
  OmittedOptional                          no register
output:
  dst -> one new RuntimeFunctionValue::ExternalStreamPartial
```

Verifier requires a non-final exact next group, matching definition/signature/coordinate table, canonical operand vector, and empty destination. It computes result ownership as the join of old captured product and new cells. The runtime validates and stages the full product before any take. A failure leaves callee/argument registers live and destination empty.

### 6.2 `OpenStream = 0x27`

The same register-use rule applies, but group must be the final group and destination receives the unique `StreamHandle`. The non-fallible commit atomically:

- consumes callee/argument registers;
- allocates `StreamInstanceKey` ordinal/request ID/lease/generic affine owner token;
- inserts the sole instance-table entry;
- appends the core Open request;
- installs the handle in `dst`;
- records cleanup obligation/observation.

Any metadata, payload eligibility, limit, generation, destination, owner-uniqueness, or table preparation error traps before every listed mutation.

### 6.3 `FinishStream = 0x28`

This remains the parent producer-side instruction. The stream/producer owner is consumed or terminally transitioned exactly as the parent lifecycle contract specifies; outcome error register is consumed for `Fail`. It cannot duplicate the consumer handle or create a second register model.

## 7. Branch and match dataflow

For every control-flow edge the verifier propagates the complete register state vector plus cleanup obligations. At a join:

```text
same type + same liveness + same ownership -> accepted
Live on one edge, Moved/Dropped on another -> rejected
```

The only accepted normalization is an explicit `Drop` inserted on a predecessor when the register is dead after the join, producing terminal facts on every edge. The verifier never inserts/assumes an implicit copy or revives a moved register.

Match tag/shape selection borrows the scrutinee. Arm binding instructions then carry explicit moves/copies. All arm-local live owners are dropped or transferred before the join. Loop headers apply the same exact-state rule to entry and back-edges.

## 8. Cleanup facts

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AwbcCleanupObligation {
    pub slot: AwbcOwnedSlot,
    pub ty: AwbcTypeId,
    pub ownership: RuntimeValueOwnership,
    pub registered_at: AwbcInstructionAddress,
}
```

`RegisterCleanup` establishes one obligation for a live owned slot. Move transfers the obligation to the destination according to frame layout. `Drop`, return transfer, or other terminal owner handoff discharges it. `CancelCleanup` is valid only when ownership was transferred to an authority that now owns cleanup; it does not silently forget a live affine value.

Every normal/return/break/cancel/trap edge has a verifier-proven cleanup sequence. Cleanup order is reverse registration. Duplicate obligation, missing obligation for an affine owner, cleanup of moved slot, or a live obligation at frame destruction is a verifier error.

## 9. Safe points and suspension

A local safe point requires:

- instruction boundary with `transaction == None`;
- no live instruction borrow;
- every value in a register/frame/mailbox/owned packet;
- exact cleanup obligations;
- exact frame/cursor identity;
- no partially assembled aggregate/capture/application/Open commit.

A global checkpoint additionally requires whole-execution snapshot eligibility and all child/mailbox/table relations. `NextStream`/`YieldStream` retain accepted safe-point tags. `CopyValue` itself is not a safe point and cannot suspend between staging and install.

## 10. Child-fiber exchange

`SpawnFiber` consumes/copies only the registers named by its typed capture plan. The verifier checks source modes, destination child-frame layout, lexical task scope, join destination, and no duplicate affine owner. Parent environment/register file is not cloned.

At runtime, source transitions, child ID/frame/capture packet, scope membership, join/mailbox state, and scheduler observation commit atomically. On failure all parent registers remain as before and no child/scope/observation exists.

Returning/joining a child uses an owned transfer packet. A child cannot return the same affine owner through two lanes. Cancellation cleanup drops remaining child-owned values before parent scope terminalization.

## 11. Interpreter/compiled-region parity

The interpreter, AOT/JIT accelerator, and product-step/compiled-region bridge call the same `RuntimeValueSlot`, prepared transfer, prepared drop, and Stream table commit APIs. Compiled code may manipulate unboxed unrestricted scalars internally only where the typed layout proves no runtime owner; at every deoptimization/safe-point/region boundary it materializes the same ownership facts.

The compiled-region exchange shape is ownership-bearing and non-Clone:

```rust
#[derive(Debug)]
pub struct RuntimeCompiledRegionExchange {
    pub register_updates: Vec<RuntimeOwnedRegisterUpdate>,
    pub frame_updates: Vec<RuntimeOwnedFrameUpdate>,
    pub control: RuntimeCompiledControlTransfer,
}
```

An update carries owned values or terminal facts, not cloned `FiberState`. Core validates the complete candidate against the expected pre-state and commits one replacement. A stale/malformed exchange is rejected with the old state unchanged.

Parity means identical:

- returned values and errors;
- register/fiber liveness facts;
- owner IDs and Stream key/lease/table relation;
- request/observation ordering;
- cleanup/drop order;
- safe-point and snapshot eligibility.

## 12. Trap atomicity

Instruction execution uses:

```text
decode -> verify static facts -> runtime preflight -> stage -> record/recheck source revisions and owner sets -> commit
```

Decode/verifier failure executes nothing. Runtime preflight/staging failure leaves register bytes/values, frame/fiber state, table, request/event queues, mailboxes, cleanup, and observations unchanged. Commit has no error branch.

In particular:

- `CopyValue` stages before destination mutation;
- `Move` validates destination/cleanup before source take;
- constructor/call/capture stages every Copy and records/rechecks every Move source revision and owner set before the first take;
- Drop validates every nested owner/table row before slot transition;
- Apply/Open validate the complete canonical product and Open transaction before consuming any register;
- compiled exchange validates the complete candidate before state replacement.

## 13. Verifier errors

The existing verifier error owner gains typed variants equivalent to:

```rust
pub enum AwbcOwnershipVerificationError {
    ReadFromUninitialized { register: AwbcRegisterId },
    UseAfterMove { register: AwbcRegisterId },
    UseAfterDrop { register: AwbcRegisterId },
    DestinationLive { register: AwbcRegisterId },
    CopyRequiresUnrestricted { register: AwbcRegisterId, ty: AwbcTypeId },
    MoveSourceEqualsDestination { register: AwbcRegisterId },
    OperandUseMismatch { instruction: AwbcInstructionAddress },
    OwnershipTypeMismatch { register: AwbcRegisterId },
    JoinStateMismatch { block: AwbcBlockId, register: AwbcRegisterId },
    DuplicateAffineOwnerPath { instruction: AwbcInstructionAddress },
    MissingCleanup { slot: AwbcOwnedSlot },
    DuplicateCleanup { slot: AwbcOwnedSlot },
    CleanupOfNonLiveSlot { slot: AwbcOwnedSlot },
    LiveCleanupAtFrameExit { slot: AwbcOwnedSlot },
    UnsafePointDuringOwnershipTransaction { point: AwbcResumePointId },
    InvalidCapturePlan { plan: AwbcCapturePlanId },
    InvalidChildTransfer { instruction: AwbcInstructionAddress },
}
```

Errors carry typed IDs/addresses; display strings are never parsed.

## 14. Exact codec rows

| Opcode | Variant | Binary order | Ownership effect |
|---:|---|---|---|
| existing | `Move { dst, src }` | unchanged | consume `src`, install `dst` |
| `0x1f` | `Drop { register }` | unchanged | prepared language drop, terminal slot |
| `0x27` | `OpenStream { dst, callee, definition, signature, group, arguments }` | accepted .2.1 bytes | consume callee/args; create affine handle |
| `0x28` | `FinishStream { stream, outcome }` | accepted .2.1 bytes | parent producer terminal transfer |
| `0x29` | `ApplyExternalStreamGroup { dst, callee, definition, signature, group, arguments }` | accepted .2.1 bytes | consume callee/args; create partial |
| `0x2a` | `CopyValue { dst, src }` | opcode, dst varu32, src varu32 | retain src, install checked copy |

The exact group-coordinate/operand tags and producer-outcome tags remain those in .2.1. `0x2b..=0x7f`, removed `0x1c/0x1d/0x1e/0x20`, and every unknown nested tag are hard errors.

## 15. Protected publication rule

Generic ownership facts can be implemented in G3 against the current/pre-ABI2 internal schema, but `CopyValue=0x2a` is externally published only in the protected P6+C4 ABI2/codec8 cut together with all parent tables/tags/opcodes/verifier/VM/lowerer/codegen changes. There is no main/release commit with codec 8 missing ownership verification or with two meanings/readers.
