use super::error::SectionCodecError;

pub(crate) fn u32_from_usize(value: usize) -> Result<u32, SectionCodecError> {
    u32::try_from(value).map_err(|_| SectionCodecError::LengthOverflow)
}

pub(crate) fn usize_from_u32(value: u32) -> Result<usize, SectionCodecError> {
    usize::try_from(value).map_err(|_| SectionCodecError::LengthOverflow)
}

pub(crate) fn u64_from_usize(value: usize) -> Result<u64, SectionCodecError> {
    u64::try_from(value).map_err(|_| SectionCodecError::LengthOverflow)
}

pub(crate) fn usize_from_u64(value: u64) -> Result<usize, SectionCodecError> {
    usize::try_from(value).map_err(|_| SectionCodecError::LengthOverflow)
}

pub(crate) fn read_array<const N: usize>(
    bytes: &[u8],
    offset: usize,
) -> Result<[u8; N], SectionCodecError> {
    let end = offset
        .checked_add(N)
        .ok_or(SectionCodecError::LengthOverflow)?;
    bytes
        .get(offset..end)
        .and_then(|slice| slice.try_into().ok())
        .ok_or(SectionCodecError::Truncated)
}

pub(crate) fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, SectionCodecError> {
    read_array::<4>(bytes, offset).map(u32::from_le_bytes)
}

pub(crate) fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, SectionCodecError> {
    read_array::<8>(bytes, offset).map(u64::from_le_bytes)
}

pub(crate) struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    pub(crate) fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }

    pub(crate) fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], SectionCodecError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(SectionCodecError::LengthOverflow)?;
        let slice = self
            .bytes
            .get(self.offset..end)
            .ok_or(SectionCodecError::Truncated)?;
        self.offset = end;
        Ok(slice)
    }

    pub(crate) fn read_u8(&mut self) -> Result<u8, SectionCodecError> {
        self.read_bytes(1).map(|bytes| bytes[0])
    }

    pub(crate) fn read_u16(&mut self) -> Result<u16, SectionCodecError> {
        self.read_bytes(2)
            .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    pub(crate) fn read_u32(&mut self) -> Result<u32, SectionCodecError> {
        self.read_bytes(4)
            .map(|bytes| u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
}
