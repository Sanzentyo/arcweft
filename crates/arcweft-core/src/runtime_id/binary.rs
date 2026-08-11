//! Canonical fixed-width binary primitives for runtime identities.

use super::{ExecutionInstanceId, RuntimeIdCursor};
use std::num::{NonZeroU32, NonZeroU64};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum RuntimeIdentityBinaryError {
    #[error("runtime identity binary length is {actual}; expected {expected}")]
    WrongLength { expected: usize, actual: usize },
    #[error("runtime identity binary value must be nonzero")]
    Zero,
    #[error("runtime cursor binary tag {tag} is unknown")]
    UnknownCursorTag { tag: u8 },
}

pub(crate) fn encode_nonzero_u32(value: NonZeroU32) -> [u8; 4] {
    value.get().to_le_bytes()
}

pub(crate) fn decode_nonzero_u32(bytes: &[u8]) -> Result<NonZeroU32, RuntimeIdentityBinaryError> {
    let bytes: [u8; 4] = bytes
        .try_into()
        .map_err(|_| RuntimeIdentityBinaryError::WrongLength {
            expected: 4,
            actual: bytes.len(),
        })?;
    NonZeroU32::new(u32::from_le_bytes(bytes)).ok_or(RuntimeIdentityBinaryError::Zero)
}

pub(crate) fn encode_nonzero_u64(value: NonZeroU64) -> [u8; 8] {
    value.get().to_le_bytes()
}

pub(crate) fn decode_nonzero_u64(bytes: &[u8]) -> Result<NonZeroU64, RuntimeIdentityBinaryError> {
    let bytes: [u8; 8] = bytes
        .try_into()
        .map_err(|_| RuntimeIdentityBinaryError::WrongLength {
            expected: 8,
            actual: bytes.len(),
        })?;
    NonZeroU64::new(u64::from_le_bytes(bytes)).ok_or(RuntimeIdentityBinaryError::Zero)
}

pub(crate) fn encode_execution_instance_id(value: ExecutionInstanceId) -> [u8; 8] {
    encode_nonzero_u64(value.get())
}

pub(crate) fn decode_execution_instance_id(
    bytes: &[u8],
) -> Result<ExecutionInstanceId, RuntimeIdentityBinaryError> {
    decode_nonzero_u64(bytes).map(ExecutionInstanceId::from_allocated)
}

pub(crate) fn encode_cursor(value: RuntimeIdCursor) -> Vec<u8> {
    match value {
        RuntimeIdCursor::Next(next) => {
            let mut bytes = Vec::with_capacity(9);
            bytes.push(0);
            bytes.extend_from_slice(&encode_nonzero_u64(next));
            bytes
        }
        RuntimeIdCursor::Exhausted => vec![1],
    }
}

pub(crate) fn decode_cursor(bytes: &[u8]) -> Result<RuntimeIdCursor, RuntimeIdentityBinaryError> {
    let Some((&tag, payload)) = bytes.split_first() else {
        return Err(RuntimeIdentityBinaryError::WrongLength {
            expected: 1,
            actual: 0,
        });
    };
    match tag {
        0 => decode_nonzero_u64(payload).map(RuntimeIdCursor::Next),
        1 if payload.is_empty() => Ok(RuntimeIdCursor::Exhausted),
        1 => Err(RuntimeIdentityBinaryError::WrongLength {
            expected: 1,
            actual: bytes.len(),
        }),
        tag => Err(RuntimeIdentityBinaryError::UnknownCursorTag { tag }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_and_cursor_binary_goldens_are_exact() {
        let execution = ExecutionInstanceId::from_allocated(NonZeroU64::MIN);
        assert_eq!(
            encode_execution_instance_id(execution),
            [1, 0, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(
            decode_execution_instance_id(&[1, 0, 0, 0, 0, 0, 0, 0]).unwrap(),
            execution
        );
        assert_eq!(
            encode_cursor(RuntimeIdCursor::initial()),
            vec![0, 1, 0, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(decode_cursor(&[1]).unwrap(), RuntimeIdCursor::Exhausted);
    }

    #[test]
    fn scalar_and_cursor_binary_reject_zero_length_tag_and_trailing_errors() {
        assert_eq!(
            decode_execution_instance_id(&[0; 8]),
            Err(RuntimeIdentityBinaryError::Zero)
        );
        assert!(matches!(
            decode_execution_instance_id(&[1; 7]),
            Err(RuntimeIdentityBinaryError::WrongLength { .. })
        ));
        assert!(matches!(
            decode_execution_instance_id(&[1; 9]),
            Err(RuntimeIdentityBinaryError::WrongLength { .. })
        ));
        assert_eq!(
            decode_cursor(&[2]),
            Err(RuntimeIdentityBinaryError::UnknownCursorTag { tag: 2 })
        );
        assert!(matches!(
            decode_cursor(&[1, 0]),
            Err(RuntimeIdentityBinaryError::WrongLength { .. })
        ));
    }
}
