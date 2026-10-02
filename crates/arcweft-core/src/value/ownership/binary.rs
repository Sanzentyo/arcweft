//! Canonical binary representation for diagnostic ownership identities.

use super::{RuntimeOwnedSlotId, RuntimeValuePath, RuntimeValuePathError, RuntimeValuePathSegment};
use crate::{
    awbc::{
        fiber::FiberCursor,
        schema::{AwbcRegisterId, AwbcScopeId, AwbcTaskPlanId},
    },
    runtime_id::{
        ExecutionInstanceId, RuntimeCaptureSlotId, RuntimeChildInstanceId, RuntimeChildPacketId,
        RuntimeCleanupScopeId, RuntimeCleanupSlotId, RuntimeClosureInstanceId,
        RuntimeFiberInstanceId, RuntimeFormatAttemptId, RuntimeFrameInstanceId,
        RuntimeFrameLocalId, RuntimeLocalSlotId, RuntimeMailboxInstanceId, RuntimeMailboxLaneId,
        RuntimePersistentFiberId, RuntimeTransferInstanceId, RuntimeTransferPacketId,
        binary::{RuntimeIdentityBinaryError, decode_nonzero_u32, decode_nonzero_u64},
    },
    value::RuntimeRecordFieldId,
};
use std::num::{NonZeroU32, NonZeroU64};
use thiserror::Error;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(super) enum RuntimeOwnershipBinaryError {
    #[error("runtime ownership binary ended before the selected value was complete")]
    UnexpectedEnd,
    #[error("runtime ownership binary has trailing bytes")]
    TrailingBytes,
    #[error("runtime owned-slot binary tag {tag} is unknown")]
    UnknownOwnedSlotTag { tag: u8 },
    #[error("runtime AWBC cleanup scope presence tag {tag} is invalid")]
    InvalidCleanupScopeTag { tag: u8 },
    #[error("runtime value-path binary tag {tag} is unknown")]
    UnknownPathSegmentTag { tag: u8 },
    #[error(transparent)]
    Identity(#[from] RuntimeIdentityBinaryError),
    #[error(transparent)]
    Path(#[from] RuntimeValuePathError),
}

pub(super) fn encode_owned_slot(slot: RuntimeOwnedSlotId) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.push(slot.canonical_tag());
    match slot {
        RuntimeOwnedSlotId::ProgramResult { execution, fiber } => {
            push_u64(&mut bytes, execution.get());
            bytes.extend_from_slice(&fiber.get().to_le_bytes());
        }
        RuntimeOwnedSlotId::EnvironmentLocal { execution, local } => {
            push_u64(&mut bytes, execution.get());
            push_u64(&mut bytes, local.get());
        }
        RuntimeOwnedSlotId::ClosureCapture {
            execution,
            closure,
            capture,
        } => {
            push_u64(&mut bytes, execution.get());
            push_u64(&mut bytes, closure.get());
            push_u32(&mut bytes, capture.get());
        }
        RuntimeOwnedSlotId::AwbcRegister {
            execution,
            fiber,
            frame,
            register,
        } => {
            push_u64(&mut bytes, execution.get());
            push_u64(&mut bytes, fiber.get());
            push_u64(&mut bytes, frame.get());
            bytes.extend_from_slice(&register.0.to_le_bytes());
        }
        RuntimeOwnedSlotId::AwbcFrameLocal {
            execution,
            fiber,
            frame,
            local,
        } => {
            push_u64(&mut bytes, execution.get());
            push_u64(&mut bytes, fiber.get());
            push_u64(&mut bytes, frame.get());
            push_u32(&mut bytes, local.get());
        }
        RuntimeOwnedSlotId::AwbcFormatOperand {
            execution,
            fiber,
            frame,
            site,
            ordinal,
        } => {
            push_u64(&mut bytes, execution.get());
            push_u64(&mut bytes, fiber.get());
            push_u64(&mut bytes, frame.get());
            push_raw_u32(&mut bytes, site.function.0);
            push_raw_u32(&mut bytes, site.block.0);
            push_raw_u32(&mut bytes, site.instruction_offset);
            push_raw_u32(&mut bytes, ordinal);
        }
        RuntimeOwnedSlotId::AwbcCleanupArg {
            execution,
            fiber,
            frame,
            scope,
            cleanup_ordinal,
            arg_ordinal,
        } => {
            push_u64(&mut bytes, execution.get());
            push_u64(&mut bytes, fiber.get());
            push_u64(&mut bytes, frame.get());
            match scope {
                None => bytes.push(0),
                Some(scope) => {
                    bytes.push(1);
                    push_raw_u32(&mut bytes, scope.0);
                }
            }
            push_raw_u32(&mut bytes, cleanup_ordinal);
            push_raw_u32(&mut bytes, arg_ordinal);
        }
        RuntimeOwnedSlotId::AwbcAwaitManyResult {
            execution,
            fiber,
            frame,
            plan,
            index,
        }
        | RuntimeOwnedSlotId::AwbcAwaitManyItem {
            execution,
            fiber,
            frame,
            plan,
            index,
        } => {
            push_u64(&mut bytes, execution.get());
            push_u64(&mut bytes, fiber.get());
            push_u64(&mut bytes, frame.get());
            push_raw_u32(&mut bytes, plan.0);
            push_raw_u32(&mut bytes, index);
        }
        RuntimeOwnedSlotId::AwbcLineObservationArg {
            execution,
            fiber,
            frame,
            site,
            ordinal,
        } => {
            push_u64(&mut bytes, execution.get());
            push_u64(&mut bytes, fiber.get());
            push_u64(&mut bytes, frame.get());
            push_raw_u32(&mut bytes, site.function.0);
            push_raw_u32(&mut bytes, site.block.0);
            push_raw_u32(&mut bytes, site.instruction_offset);
            push_raw_u32(&mut bytes, ordinal);
        }
        RuntimeOwnedSlotId::AwbcDialogueResultObservation {
            execution,
            fiber,
            frame,
            site,
        } => {
            push_u64(&mut bytes, execution.get());
            push_u64(&mut bytes, fiber.get());
            push_u64(&mut bytes, frame.get());
            push_raw_u32(&mut bytes, site.function.0);
            push_raw_u32(&mut bytes, site.block.0);
            push_raw_u32(&mut bytes, site.instruction_offset);
        }
        RuntimeOwnedSlotId::AwbcEffectObservationArg {
            execution,
            fiber,
            frame,
            site,
            effect_ordinal,
            arg_ordinal,
        } => {
            push_u64(&mut bytes, execution.get());
            push_u64(&mut bytes, fiber.get());
            push_u64(&mut bytes, frame.get());
            push_raw_u32(&mut bytes, site.function.0);
            push_raw_u32(&mut bytes, site.block.0);
            push_raw_u32(&mut bytes, site.instruction_offset);
            push_raw_u32(&mut bytes, effect_ordinal);
            push_raw_u32(&mut bytes, arg_ordinal);
        }
        RuntimeOwnedSlotId::NativeFormatOperand {
            execution,
            fiber,
            frame,
            attempt,
            ordinal,
        } => {
            push_u64(&mut bytes, execution.get());
            push_raw_u64(&mut bytes, fiber.get());
            push_raw_u32(&mut bytes, frame);
            push_raw_u32(&mut bytes, attempt.get().get());
            push_raw_u32(&mut bytes, ordinal);
        }
        RuntimeOwnedSlotId::MailboxLane {
            execution,
            mailbox,
            lane,
        } => {
            push_u64(&mut bytes, execution.get());
            push_u64(&mut bytes, mailbox.get());
            push_u32(&mut bytes, lane.get());
        }
        RuntimeOwnedSlotId::ChildPacket {
            execution,
            child,
            packet,
        } => {
            push_u64(&mut bytes, execution.get());
            push_u64(&mut bytes, child.get());
            push_u32(&mut bytes, packet.get());
        }
        RuntimeOwnedSlotId::TransferPacket {
            execution,
            transfer,
            packet,
        } => {
            push_u64(&mut bytes, execution.get());
            push_u64(&mut bytes, transfer.get());
            push_u32(&mut bytes, packet.get());
        }
        RuntimeOwnedSlotId::CleanupSlot {
            execution,
            scope,
            slot,
        } => {
            push_u64(&mut bytes, execution.get());
            push_u64(&mut bytes, scope.get());
            push_u32(&mut bytes, slot.get());
        }
    }
    bytes
}

pub(super) fn decode_owned_slot(
    bytes: &[u8],
) -> Result<RuntimeOwnedSlotId, RuntimeOwnershipBinaryError> {
    let mut reader = Reader::new(bytes);
    let tag = reader.u8()?;
    if tag > 16 {
        return Err(RuntimeOwnershipBinaryError::UnknownOwnedSlotTag { tag });
    }
    let execution = ExecutionInstanceId::from_allocated(reader.nonzero_u64()?);
    let slot = match tag {
        16 => RuntimeOwnedSlotId::ProgramResult {
            execution,
            fiber: RuntimePersistentFiberId::from_allocated(reader.u64()?),
        },
        0 => RuntimeOwnedSlotId::EnvironmentLocal {
            execution,
            local: RuntimeLocalSlotId::from_allocated(reader.nonzero_u64()?),
        },
        1 => RuntimeOwnedSlotId::ClosureCapture {
            execution,
            closure: RuntimeClosureInstanceId::from_allocated(reader.nonzero_u64()?),
            capture: RuntimeCaptureSlotId::from_accepted_ordinal(reader.nonzero_u32()?),
        },
        2 => RuntimeOwnedSlotId::AwbcRegister {
            execution,
            fiber: RuntimeFiberInstanceId::from_allocated(reader.nonzero_u64()?),
            frame: RuntimeFrameInstanceId::from_allocated(reader.nonzero_u64()?),
            register: AwbcRegisterId(reader.u32()?),
        },
        3 => RuntimeOwnedSlotId::AwbcFrameLocal {
            execution,
            fiber: RuntimeFiberInstanceId::from_allocated(reader.nonzero_u64()?),
            frame: RuntimeFrameInstanceId::from_allocated(reader.nonzero_u64()?),
            local: RuntimeFrameLocalId::from_accepted_ordinal(reader.nonzero_u32()?),
        },
        4 => RuntimeOwnedSlotId::MailboxLane {
            execution,
            mailbox: RuntimeMailboxInstanceId::from_allocated(reader.nonzero_u64()?),
            lane: RuntimeMailboxLaneId::from_accepted_ordinal(reader.nonzero_u32()?),
        },
        5 => RuntimeOwnedSlotId::ChildPacket {
            execution,
            child: RuntimeChildInstanceId::from_allocated(reader.nonzero_u64()?),
            packet: RuntimeChildPacketId::from_accepted_ordinal(reader.nonzero_u32()?),
        },
        6 => RuntimeOwnedSlotId::TransferPacket {
            execution,
            transfer: RuntimeTransferInstanceId::from_allocated(reader.nonzero_u64()?),
            packet: RuntimeTransferPacketId::from_accepted_ordinal(reader.nonzero_u32()?),
        },
        7 => RuntimeOwnedSlotId::CleanupSlot {
            execution,
            scope: RuntimeCleanupScopeId::from_allocated(reader.nonzero_u64()?),
            slot: RuntimeCleanupSlotId::from_accepted_ordinal(reader.nonzero_u32()?),
        },
        8 => RuntimeOwnedSlotId::AwbcFormatOperand {
            execution,
            fiber: RuntimeFiberInstanceId::from_allocated(reader.nonzero_u64()?),
            frame: RuntimeFrameInstanceId::from_allocated(reader.nonzero_u64()?),
            site: FiberCursor {
                function: crate::awbc::schema::AwbcFunctionId(reader.u32()?),
                block: crate::awbc::schema::AwbcBlockId(reader.u32()?),
                instruction_offset: reader.u32()?,
            },
            ordinal: reader.u32()?,
        },
        9 => RuntimeOwnedSlotId::NativeFormatOperand {
            execution,
            fiber: RuntimePersistentFiberId::from_allocated(reader.u64()?),
            frame: reader.u32()?,
            attempt: RuntimeFormatAttemptId::from_accepted_ordinal(reader.nonzero_u32()?),
            ordinal: reader.u32()?,
        },
        10 => {
            let fiber = RuntimeFiberInstanceId::from_allocated(reader.nonzero_u64()?);
            let frame = RuntimeFrameInstanceId::from_allocated(reader.nonzero_u64()?);
            let scope = match reader.u8()? {
                0 => None,
                1 => Some(AwbcScopeId(reader.u32()?)),
                tag => return Err(RuntimeOwnershipBinaryError::InvalidCleanupScopeTag { tag }),
            };
            RuntimeOwnedSlotId::AwbcCleanupArg {
                execution,
                fiber,
                frame,
                scope,
                cleanup_ordinal: reader.u32()?,
                arg_ordinal: reader.u32()?,
            }
        }
        11 => RuntimeOwnedSlotId::AwbcAwaitManyResult {
            execution,
            fiber: RuntimeFiberInstanceId::from_allocated(reader.nonzero_u64()?),
            frame: RuntimeFrameInstanceId::from_allocated(reader.nonzero_u64()?),
            plan: AwbcTaskPlanId(reader.u32()?),
            index: reader.u32()?,
        },
        12 => RuntimeOwnedSlotId::AwbcAwaitManyItem {
            execution,
            fiber: RuntimeFiberInstanceId::from_allocated(reader.nonzero_u64()?),
            frame: RuntimeFrameInstanceId::from_allocated(reader.nonzero_u64()?),
            plan: AwbcTaskPlanId(reader.u32()?),
            index: reader.u32()?,
        },
        13 => RuntimeOwnedSlotId::AwbcLineObservationArg {
            execution,
            fiber: RuntimeFiberInstanceId::from_allocated(reader.nonzero_u64()?),
            frame: RuntimeFrameInstanceId::from_allocated(reader.nonzero_u64()?),
            site: FiberCursor {
                function: crate::awbc::schema::AwbcFunctionId(reader.u32()?),
                block: crate::awbc::schema::AwbcBlockId(reader.u32()?),
                instruction_offset: reader.u32()?,
            },
            ordinal: reader.u32()?,
        },
        14 => RuntimeOwnedSlotId::AwbcDialogueResultObservation {
            execution,
            fiber: RuntimeFiberInstanceId::from_allocated(reader.nonzero_u64()?),
            frame: RuntimeFrameInstanceId::from_allocated(reader.nonzero_u64()?),
            site: FiberCursor {
                function: crate::awbc::schema::AwbcFunctionId(reader.u32()?),
                block: crate::awbc::schema::AwbcBlockId(reader.u32()?),
                instruction_offset: reader.u32()?,
            },
        },
        15 => RuntimeOwnedSlotId::AwbcEffectObservationArg {
            execution,
            fiber: RuntimeFiberInstanceId::from_allocated(reader.nonzero_u64()?),
            frame: RuntimeFrameInstanceId::from_allocated(reader.nonzero_u64()?),
            site: FiberCursor {
                function: crate::awbc::schema::AwbcFunctionId(reader.u32()?),
                block: crate::awbc::schema::AwbcBlockId(reader.u32()?),
                instruction_offset: reader.u32()?,
            },
            effect_ordinal: reader.u32()?,
            arg_ordinal: reader.u32()?,
        },
        _ => unreachable!("owned-slot tag was validated above"),
    };
    reader.finish()?;
    Ok(slot)
}

pub(super) fn encode_value_path(path: &RuntimeValuePath) -> Vec<u8> {
    let mut bytes = Vec::new();
    let count = u32::try_from(path.segments().len())
        .expect("runtime value paths are limited to 64 segments");
    bytes.extend_from_slice(&count.to_le_bytes());
    for segment in path.segments() {
        bytes.push(segment.canonical_tag());
        match *segment {
            RuntimeValuePathSegment::TupleElement(index)
            | RuntimeValuePathSegment::TupleColumn(index)
            | RuntimeValuePathSegment::ReductionCommandPayload(index)
            | RuntimeValuePathSegment::AgentEmbeddedValue(index)
            | RuntimeValuePathSegment::CallableRetained(index) => {
                bytes.extend_from_slice(&index.to_le_bytes());
            }
            RuntimeValuePathSegment::SequenceElement(index)
            | RuntimeValuePathSegment::IteratorRemainder(index) => {
                bytes.extend_from_slice(&index.to_le_bytes());
            }
            RuntimeValuePathSegment::RecordField(field)
            | RuntimeValuePathSegment::RecordColumn(field)
            | RuntimeValuePathSegment::NominalRecordField(field) => {
                push_u32(&mut bytes, field.get());
            }
            RuntimeValuePathSegment::VariantPayload
            | RuntimeValuePathSegment::IteratorWitnessState
            | RuntimeValuePathSegment::OpaquePayload
            | RuntimeValuePathSegment::ReductionState => {}
        }
    }
    bytes
}

pub(super) fn decode_value_path(
    bytes: &[u8],
) -> Result<RuntimeValuePath, RuntimeOwnershipBinaryError> {
    let mut reader = Reader::new(bytes);
    let count = reader.u32()?;
    let capacity = usize::try_from(count).map_err(|_| RuntimeValuePathError::TooDeep {
        maximum: super::MAX_RUNTIME_VALUE_PATH_SEGMENTS,
        actual: usize::MAX,
    })?;
    if count > super::MAX_RUNTIME_VALUE_PATH_SEGMENTS {
        return Err(RuntimeValuePathError::TooDeep {
            maximum: super::MAX_RUNTIME_VALUE_PATH_SEGMENTS,
            actual: capacity,
        }
        .into());
    }
    let mut segments = Vec::with_capacity(capacity);
    for _ in 0..count {
        let tag = reader.u8()?;
        let segment = match tag {
            0 => RuntimeValuePathSegment::TupleElement(reader.u32()?),
            1 => RuntimeValuePathSegment::SequenceElement(reader.u64()?),
            2 => RuntimeValuePathSegment::TupleColumn(reader.u32()?),
            3 => RuntimeValuePathSegment::RecordField(record_field(reader.nonzero_u32()?)?),
            4 => RuntimeValuePathSegment::RecordColumn(record_field(reader.nonzero_u32()?)?),
            5 => RuntimeValuePathSegment::NominalRecordField(record_field(reader.nonzero_u32()?)?),
            7 => RuntimeValuePathSegment::VariantPayload,
            8 => RuntimeValuePathSegment::IteratorRemainder(reader.u64()?),
            9 => RuntimeValuePathSegment::IteratorWitnessState,
            10 => RuntimeValuePathSegment::OpaquePayload,
            11 => RuntimeValuePathSegment::ReductionState,
            12 => RuntimeValuePathSegment::ReductionCommandPayload(reader.u32()?),
            13 => RuntimeValuePathSegment::AgentEmbeddedValue(reader.u32()?),
            15 => RuntimeValuePathSegment::CallableRetained(reader.u32()?),
            tag => return Err(RuntimeOwnershipBinaryError::UnknownPathSegmentTag { tag }),
        };
        segments.push(segment);
    }
    reader.finish()?;
    RuntimeValuePath::try_from_segments(segments).map_err(Into::into)
}

fn record_field(raw: NonZeroU32) -> Result<RuntimeRecordFieldId, RuntimeOwnershipBinaryError> {
    let zero_based = usize::try_from(raw.get() - 1)
        .expect("u32 record field ordinals fit every supported target");
    RuntimeRecordFieldId::try_from_zero_based_ordinal(zero_based)
        .map_err(|_| RuntimeValuePathError::InvalidRecordFieldIdentity.into())
}

fn push_u32(bytes: &mut Vec<u8>, value: NonZeroU32) {
    bytes.extend_from_slice(&value.get().to_le_bytes());
}

fn push_raw_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(bytes: &mut Vec<u8>, value: NonZeroU64) {
    bytes.extend_from_slice(&value.get().to_le_bytes());
}

fn push_raw_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

struct Reader<'a> {
    remaining: &'a [u8],
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { remaining: bytes }
    }

    fn take<const N: usize>(&mut self) -> Result<[u8; N], RuntimeOwnershipBinaryError> {
        let Some((value, remaining)) = self.remaining.split_at_checked(N) else {
            return Err(RuntimeOwnershipBinaryError::UnexpectedEnd);
        };
        self.remaining = remaining;
        Ok(value.try_into().expect("slice length was checked"))
    }

    fn u8(&mut self) -> Result<u8, RuntimeOwnershipBinaryError> {
        Ok(self.take::<1>()?[0])
    }

    fn u32(&mut self) -> Result<u32, RuntimeOwnershipBinaryError> {
        Ok(u32::from_le_bytes(self.take()?))
    }

    fn u64(&mut self) -> Result<u64, RuntimeOwnershipBinaryError> {
        Ok(u64::from_le_bytes(self.take()?))
    }

    fn nonzero_u32(&mut self) -> Result<NonZeroU32, RuntimeOwnershipBinaryError> {
        decode_nonzero_u32(&self.take::<4>()?).map_err(Into::into)
    }

    fn nonzero_u64(&mut self) -> Result<NonZeroU64, RuntimeOwnershipBinaryError> {
        decode_nonzero_u64(&self.take::<8>()?).map_err(Into::into)
    }

    fn finish(self) -> Result<(), RuntimeOwnershipBinaryError> {
        if self.remaining.is_empty() {
            Ok(())
        } else {
            Err(RuntimeOwnershipBinaryError::TrailingBytes)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::DeserializeOwned;

    fn json<T: DeserializeOwned>(value: &str) -> T {
        serde_json::from_str(value).unwrap()
    }

    fn hex(bytes: &[u8]) -> String {
        use std::fmt::Write as _;

        bytes.iter().fold(
            String::with_capacity(bytes.len() * 2),
            |mut output, byte| {
                write!(output, "{byte:02x}").expect("writing to a String cannot fail");
                output
            },
        )
    }

    #[test]
    fn owned_slot_binary_goldens_round_trip() {
        let goldens = [
            (
                r#"{"kind":"environment_local","execution":"1","local":"2"}"#,
                "0001000000000000000200000000000000",
            ),
            (
                r#"{"kind":"closure_capture","execution":"1","closure":"2","capture":3}"#,
                "010100000000000000020000000000000003000000",
            ),
            (
                r#"{"kind":"awbc_register","execution":"1","fiber":"2","frame":"3","register":4}"#,
                "0201000000000000000200000000000000030000000000000004000000",
            ),
            (
                r#"{"kind":"awbc_frame_local","execution":"1","fiber":"2","frame":"3","local":4}"#,
                "0301000000000000000200000000000000030000000000000004000000",
            ),
            (
                r#"{"kind":"mailbox_lane","execution":"1","mailbox":"2","lane":3}"#,
                "040100000000000000020000000000000003000000",
            ),
            (
                r#"{"kind":"child_packet","execution":"1","child":"2","packet":3}"#,
                "050100000000000000020000000000000003000000",
            ),
            (
                r#"{"kind":"transfer_packet","execution":"1","transfer":"2","packet":3}"#,
                "060100000000000000020000000000000003000000",
            ),
            (
                r#"{"kind":"cleanup_slot","execution":"1","scope":"2","slot":3}"#,
                "070100000000000000020000000000000003000000",
            ),
            (
                r#"{"kind":"awbc_format_operand","execution":"1","fiber":"2","frame":"3","site":{"function":4,"block":5,"instruction_offset":6},"ordinal":7}"#,
                "0801000000000000000200000000000000030000000000000004000000050000000600000007000000",
            ),
            (
                r#"{"kind":"native_format_operand","execution":"1","fiber":2,"frame":3,"attempt":4,"ordinal":5}"#,
                "0901000000000000000200000000000000030000000400000005000000",
            ),
            (
                r#"{"kind":"awbc_cleanup_arg","execution":"1","fiber":"2","frame":"3","scope":null,"cleanup_ordinal":4,"arg_ordinal":5}"#,
                "0a010000000000000002000000000000000300000000000000000400000005000000",
            ),
            (
                r#"{"kind":"awbc_cleanup_arg","execution":"1","fiber":"2","frame":"3","scope":6,"cleanup_ordinal":4,"arg_ordinal":5}"#,
                "0a01000000000000000200000000000000030000000000000001060000000400000005000000",
            ),
            (
                r#"{"kind":"awbc_await_many_result","execution":"1","fiber":"2","frame":"3","plan":4,"index":5}"#,
                "0b0100000000000000020000000000000003000000000000000400000005000000",
            ),
            (
                r#"{"kind":"awbc_await_many_item","execution":"1","fiber":"2","frame":"3","plan":4,"index":5}"#,
                "0c0100000000000000020000000000000003000000000000000400000005000000",
            ),
            (
                r#"{"kind":"awbc_line_observation_arg","execution":"1","fiber":"2","frame":"3","site":{"function":4,"block":5,"instruction_offset":6},"ordinal":7}"#,
                "0d01000000000000000200000000000000030000000000000004000000050000000600000007000000",
            ),
            (
                r#"{"kind":"awbc_dialogue_result_observation","execution":"1","fiber":"2","frame":"3","site":{"function":4,"block":5,"instruction_offset":6}}"#,
                "0e010000000000000002000000000000000300000000000000040000000500000006000000",
            ),
            (
                r#"{"kind":"awbc_effect_observation_arg","execution":"1","fiber":"2","frame":"3","site":{"function":4,"block":5,"instruction_offset":6},"effect_ordinal":7,"arg_ordinal":8}"#,
                "0f0100000000000000020000000000000003000000000000000400000005000000060000000700000008000000",
            ),
        ];
        for (json, expected) in goldens {
            let slot: RuntimeOwnedSlotId = self::json(json);
            let encoded = encode_owned_slot(slot);
            assert_eq!(hex(&encoded), expected);
            assert_eq!(decode_owned_slot(&encoded).unwrap(), slot);
        }
        let result: RuntimeOwnedSlotId =
            self::json(r#"{"kind":"program_result","execution":"1","fiber":0}"#);
        let bytes = encode_owned_slot(result);
        assert_eq!(hex(&bytes), "1001000000000000000000000000000000");
        assert_eq!(decode_owned_slot(&bytes).unwrap(), result);
        assert_eq!(
            serde_json::to_string(&result).unwrap(),
            r#"{"kind":"program_result","execution":"1","fiber":0}"#
        );
    }

    #[test]
    fn value_path_binary_goldens_round_trip() {
        let goldens = [
            ("[]", "00000000"),
            (
                r#"[{"kind":"record_field","field":2},{"kind":"sequence_element","index":"4"},{"kind":"variant_payload"}]"#,
                "03000000030200000001040000000000000007",
            ),
            (
                r#"[{"kind":"callable_retained","index":2},{"kind":"iterator_remainder","index":"6"}]"#,
                "020000000f02000000080600000000000000",
            ),
            (r#"[{"kind":"iterator_witness_state"}]"#, "0100000009"),
            (r#"[{"kind":"opaque_payload"}]"#, "010000000a"),
        ];
        for (json, expected) in goldens {
            let path: RuntimeValuePath = self::json(json);
            let encoded = encode_value_path(&path);
            assert_eq!(hex(&encoded), expected);
            assert_eq!(decode_value_path(&encoded).unwrap(), path);
        }
    }

    #[test]
    fn binary_decoders_reject_unknown_zero_truncated_and_trailing_forms() {
        assert!(matches!(
            decode_owned_slot(&[17]),
            Err(RuntimeOwnershipBinaryError::UnknownOwnedSlotTag { tag: 17 })
        ));
        let mut invalid_cleanup_scope = encode_owned_slot(self::json(
            r#"{"kind":"awbc_cleanup_arg","execution":"1","fiber":"2","frame":"3","scope":null,"cleanup_ordinal":4,"arg_ordinal":5}"#,
        ));
        invalid_cleanup_scope[25] = 2;
        assert!(matches!(
            decode_owned_slot(&invalid_cleanup_scope),
            Err(RuntimeOwnershipBinaryError::InvalidCleanupScopeTag { tag: 2 })
        ));
        assert!(decode_owned_slot(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0]).is_err());
        assert!(matches!(
            decode_value_path(&[1, 0, 0, 0, 16]),
            Err(RuntimeOwnershipBinaryError::UnknownPathSegmentTag { tag: 16 })
        ));
        assert!(matches!(
            decode_value_path(&[1, 0, 0, 0, 6]),
            Err(RuntimeOwnershipBinaryError::UnknownPathSegmentTag { tag: 6 })
        ));
        assert!(matches!(
            decode_value_path(&[1, 0, 0, 0, 14]),
            Err(RuntimeOwnershipBinaryError::UnknownPathSegmentTag { tag: 14 })
        ));
        assert!(decode_value_path(&[1, 0, 0, 0, 3, 0, 0, 0, 0]).is_err());
        assert!(decode_value_path(&[2, 0, 0, 0, 7]).is_err());
        assert!(matches!(
            decode_value_path(&[0, 0, 0, 0, 0]),
            Err(RuntimeOwnershipBinaryError::TrailingBytes)
        ));
    }
}
