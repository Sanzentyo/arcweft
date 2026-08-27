//! Canonical binary representation for diagnostic ownership identities.

use super::{RuntimeOwnedSlotId, RuntimeValuePath, RuntimeValuePathError, RuntimeValuePathSegment};
use crate::{
    awbc::schema::AwbcRegisterId,
    runtime_id::{
        ExecutionInstanceId, RuntimeCaptureSlotId, RuntimeChildInstanceId, RuntimeChildPacketId,
        RuntimeCleanupScopeId, RuntimeCleanupSlotId, RuntimeClosureInstanceId,
        RuntimeFiberInstanceId, RuntimeFrameInstanceId, RuntimeFrameLocalId, RuntimeLocalSlotId,
        RuntimeMailboxInstanceId, RuntimeMailboxLaneId, RuntimeTransferInstanceId,
        RuntimeTransferPacketId,
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
    if tag > 7 {
        return Err(RuntimeOwnershipBinaryError::UnknownOwnedSlotTag { tag });
    }
    let execution = ExecutionInstanceId::from_allocated(reader.nonzero_u64()?);
    let slot = match tag {
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
            | RuntimeValuePathSegment::TupleColumn(index) => {
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
            RuntimeValuePathSegment::FunctionCapture(capture) => {
                push_u32(&mut bytes, capture.get());
            }
            RuntimeValuePathSegment::ReductionCommandPayload(index)
            | RuntimeValuePathSegment::AgentEmbeddedValue(index) => {
                bytes.extend_from_slice(&index.to_le_bytes());
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
            6 => RuntimeValuePathSegment::FunctionCapture(
                RuntimeCaptureSlotId::from_accepted_ordinal(reader.nonzero_u32()?),
            ),
            7 => RuntimeValuePathSegment::VariantPayload,
            8 => RuntimeValuePathSegment::IteratorRemainder(reader.u64()?),
            9 => RuntimeValuePathSegment::IteratorWitnessState,
            10 => RuntimeValuePathSegment::OpaquePayload,
            11 => RuntimeValuePathSegment::ReductionState,
            12 => RuntimeValuePathSegment::ReductionCommandPayload(reader.u32()?),
            13 => RuntimeValuePathSegment::AgentEmbeddedValue(reader.u32()?),
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

fn push_u64(bytes: &mut Vec<u8>, value: NonZeroU64) {
    bytes.extend_from_slice(&value.get().to_le_bytes());
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
        ];
        for (json, expected) in goldens {
            let slot: RuntimeOwnedSlotId = self::json(json);
            let encoded = encode_owned_slot(slot);
            assert_eq!(hex(&encoded), expected);
            assert_eq!(decode_owned_slot(&encoded).unwrap(), slot);
        }
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
                r#"[{"kind":"function_capture","capture":3},{"kind":"iterator_remainder","index":"6"}]"#,
                "020000000603000000080600000000000000",
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
            decode_owned_slot(&[8]),
            Err(RuntimeOwnershipBinaryError::UnknownOwnedSlotTag { tag: 8 })
        ));
        assert!(decode_owned_slot(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0]).is_err());
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
