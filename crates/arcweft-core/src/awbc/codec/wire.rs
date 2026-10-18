use super::{AwbcCodecError, AwbcDecodeBudget};
use crate::canonical_varint::{CanonicalVarintError, CanonicalVarintWidth, encode_u32, encode_u64};
use crate::runtime_id::{RuntimeIdError, RuntimeIdFamily, RuntimeIdPath, RuntimeIdSegment};
use crate::value::RuntimeEntityIdentityVisitor;

pub(super) trait Wire: Sized {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError>;
    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError>;
}

#[derive(Default)]
pub(super) struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    pub(super) fn with_capacity(capacity: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(capacity),
        }
    }

    pub(super) fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    pub(super) const fn len(&self) -> usize {
        self.bytes.len()
    }

    pub(super) fn transaction<T>(
        &mut self,
        write: impl FnOnce(&mut Self) -> Result<T, AwbcCodecError>,
    ) -> Result<T, AwbcCodecError> {
        let checkpoint = self.bytes.len();
        match write(self) {
            Ok(value) => Ok(value),
            Err(error) => {
                self.bytes.truncate(checkpoint);
                Err(error)
            }
        }
    }

    pub(super) fn write_u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    pub(super) fn write_bytes(&mut self, value: &[u8]) {
        self.bytes.extend_from_slice(value);
    }

    pub(super) fn write_u16_le(&mut self, value: u16) {
        self.write_bytes(&value.to_le_bytes());
    }

    pub(super) fn write_f32_bits(&mut self, value: u32) {
        self.write_bytes(&value.to_le_bytes());
    }

    pub(super) fn write_u64_le(&mut self, value: u64) {
        self.write_bytes(&value.to_le_bytes());
    }

    pub(super) fn write_f64_bits(&mut self, value: u64) {
        self.write_bytes(&value.to_le_bytes());
    }

    pub(super) fn write_i32_le(&mut self, value: i32) {
        self.write_bytes(&value.to_le_bytes());
    }

    pub(super) fn write_u32_var(&mut self, value: u32) {
        let (encoded, length) = encode_u32(value);
        self.write_bytes(&encoded[..length]);
    }

    pub(super) fn write_u64_var(&mut self, value: u64) {
        let (encoded, length) = encode_u64(value);
        self.write_bytes(&encoded[..length]);
    }

    pub(super) fn write_len(&mut self, len: usize) -> Result<(), AwbcCodecError> {
        let len = u32::try_from(len).map_err(|_| AwbcCodecError::LengthOverflow)?;
        self.write_u32_var(len);
        Ok(())
    }

    pub(super) fn write_table<T: Wire>(&mut self, values: &[T]) -> Result<(), AwbcCodecError> {
        self.write_len(values.len())?;
        for value in values {
            value.write_wire(self)?;
        }
        Ok(())
    }

    pub(super) fn write_str(&mut self, value: &str) -> Result<(), AwbcCodecError> {
        self.write_len(value.len())?;
        self.write_bytes(value.as_bytes());
        Ok(())
    }

    pub(super) fn write_registry_key(
        &mut self,
        key: &arcweft_id::LifetimeRegistryKey,
    ) -> Result<(), AwbcCodecError> {
        self.write_str(key.scope().as_str())?;
        self.write_len(key.segments().len())?;
        for name in key.segments() {
            self.write_str(name.as_str())?;
        }
        Ok(())
    }

    pub(super) fn write_runtime_id_path(
        &mut self,
        family: RuntimeIdFamily,
        path: &RuntimeIdPath,
    ) -> Result<(), AwbcCodecError> {
        let segments = path.segments();
        let count = u32::try_from(segments.len()).map_err(|_| AwbcCodecError::LengthOverflow)?;
        if segments.is_empty() {
            return Err(AwbcCodecError::InvalidMetadata {
                kind: "runtime identity path",
                message: RuntimeIdError::Empty { family }.to_string(),
                offset: self.len(),
            });
        }
        for (index, segment) in segments.iter().enumerate() {
            RuntimeIdPath::preflight_stored_source_segment(
                family,
                index,
                segments.len(),
                segment.as_str(),
            )
            .map_err(|error| AwbcCodecError::InvalidMetadata {
                kind: "runtime identity path",
                message: error.to_string(),
                offset: self.len(),
            })?;
            u32::try_from(segment.as_str().len()).map_err(|_| AwbcCodecError::LengthOverflow)?;
        }
        self.write_u32_var(count);
        for segment in segments {
            self.write_str(segment.as_str())?;
        }
        Ok(())
    }

    pub(super) fn patch_u64_le(&mut self, offset: usize, value: u64) -> Result<(), AwbcCodecError> {
        let end = offset
            .checked_add(std::mem::size_of::<u64>())
            .ok_or(AwbcCodecError::LengthOverflow)?;
        let destination = self
            .bytes
            .get_mut(offset..end)
            .ok_or(AwbcCodecError::LengthOverflow)?;
        destination.copy_from_slice(&value.to_le_bytes());
        Ok(())
    }
}

impl RuntimeEntityIdentityVisitor for Writer {
    type Error = AwbcCodecError;

    fn tag(&mut self, tag: u8) -> Result<(), Self::Error> {
        self.write_u8(tag);
        Ok(())
    }
    fn text(&mut self, text: &str) -> Result<(), Self::Error> {
        self.write_str(text)
    }
    fn digest(&mut self, digest: &[u8; 32]) -> Result<(), Self::Error> {
        self.write_bytes(digest);
        Ok(())
    }
    fn path(&mut self, family: RuntimeIdFamily, path: &RuntimeIdPath) -> Result<(), Self::Error> {
        self.write_runtime_id_path(family, path)
    }
}

pub(super) struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
    budget: AwbcDecodeBudget,
    collection_items: usize,
    string_bytes: usize,
    depth: usize,
}

impl<'a> Reader<'a> {
    pub(super) const fn new(bytes: &'a [u8], budget: &AwbcDecodeBudget) -> Self {
        Self {
            bytes,
            offset: 0,
            budget: *budget,
            collection_items: 0,
            string_bytes: 0,
            depth: 0,
        }
    }

    pub(super) const fn offset(&self) -> usize {
        self.offset
    }

    pub(super) const fn budget(&self) -> AwbcDecodeBudget {
        self.budget
    }

    pub(super) fn finish(&self) -> Result<(), AwbcCodecError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(AwbcCodecError::TrailingBytes {
                count: self.bytes.len() - self.offset,
            })
        }
    }

    pub(super) fn read_u8(&mut self) -> Result<u8, AwbcCodecError> {
        let value = self
            .bytes
            .get(self.offset)
            .copied()
            .ok_or(AwbcCodecError::Truncated {
                offset: self.offset,
            })?;
        self.offset += 1;
        Ok(value)
    }

    pub(super) fn read_exact(&mut self, len: usize) -> Result<&'a [u8], AwbcCodecError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(AwbcCodecError::LengthOverflow)?;
        if end > self.bytes.len() {
            return Err(AwbcCodecError::Truncated {
                offset: self.offset,
            });
        }
        let value = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(value)
    }

    pub(super) fn read_u16_le(&mut self) -> Result<u16, AwbcCodecError> {
        Ok(u16::from_le_bytes(
            self.read_exact(2)?
                .try_into()
                .expect("fixed wire width checked"),
        ))
    }

    pub(super) fn read_f32_bits(&mut self) -> Result<u32, AwbcCodecError> {
        Ok(u32::from_le_bytes(
            self.read_exact(4)?
                .try_into()
                .expect("fixed wire width checked"),
        ))
    }

    pub(super) fn read_u64_le(&mut self) -> Result<u64, AwbcCodecError> {
        Ok(u64::from_le_bytes(
            self.read_exact(8)?
                .try_into()
                .expect("fixed wire width checked"),
        ))
    }

    pub(super) fn read_f64_bits(&mut self) -> Result<u64, AwbcCodecError> {
        Ok(u64::from_le_bytes(
            self.read_exact(8)?
                .try_into()
                .expect("fixed wire width checked"),
        ))
    }

    pub(super) fn read_i32_le(&mut self) -> Result<i32, AwbcCodecError> {
        Ok(i32::from_le_bytes(
            self.read_exact(4)?
                .try_into()
                .expect("fixed wire width checked"),
        ))
    }

    pub(super) fn read_u32_var(&mut self) -> Result<u32, AwbcCodecError> {
        self.read_unsigned_var(CanonicalVarintWidth::U32)
            .map(|value| u32::try_from(value).expect("u32 grammar bounded the payload"))
    }

    pub(super) fn read_u64_var(&mut self) -> Result<u64, AwbcCodecError> {
        self.read_unsigned_var(CanonicalVarintWidth::U64)
    }

    fn read_unsigned_var(&mut self, width: CanonicalVarintWidth) -> Result<u64, AwbcCodecError> {
        let start = self.offset;
        let (value, consumed) = match width.decode(&self.bytes[start..]) {
            Ok(value) => value,
            Err(CanonicalVarintError::Truncated { consumed }) => {
                return Err(AwbcCodecError::Truncated {
                    offset: start + consumed,
                });
            }
            Err(CanonicalVarintError::NonCanonical) => {
                return Err(AwbcCodecError::NonCanonicalVarint { offset: start });
            }
        };
        self.offset = self
            .offset
            .checked_add(consumed)
            .ok_or(AwbcCodecError::LengthOverflow)?;
        Ok(value)
    }

    pub(super) fn read_len(&mut self) -> Result<usize, AwbcCodecError> {
        usize::try_from(self.read_u32_var()?).map_err(|_| AwbcCodecError::LengthOverflow)
    }

    pub(super) fn read_table<T: Wire>(
        &mut self,
        budget_name: &'static str,
        limit: usize,
    ) -> Result<Vec<T>, AwbcCodecError> {
        let len = self.read_len()?;
        Self::check_limit(budget_name, len, limit)?;
        self.read_items(len)
    }

    pub(super) fn read_string_table(
        &mut self,
        limit: usize,
    ) -> Result<Vec<String>, AwbcCodecError> {
        let len = self.read_len()?;
        Self::check_limit("strings", len, limit)?;
        self.read_nested(|reader| (0..len).map(|_| String::read_wire(reader)).collect())
    }

    pub(super) fn read_items<T: Wire>(&mut self, len: usize) -> Result<Vec<T>, AwbcCodecError> {
        self.add_collection_items(len)?;
        self.read_nested(|reader| (0..len).map(|_| T::read_wire(reader)).collect())
    }

    fn add_collection_items(&mut self, len: usize) -> Result<(), AwbcCodecError> {
        let next = self
            .collection_items
            .checked_add(len)
            .ok_or(AwbcCodecError::LengthOverflow)?;
        Self::check_limit("collection_items", next, self.budget.collection_items)?;
        self.collection_items = next;
        Ok(())
    }

    pub(super) fn read_nested<T>(
        &mut self,
        read: impl FnOnce(&mut Self) -> Result<T, AwbcCodecError>,
    ) -> Result<T, AwbcCodecError> {
        self.enter_nesting()?;
        let result = read(self);
        self.leave_nesting();
        result
    }

    pub(super) fn add_string_bytes(&mut self, len: usize) -> Result<(), AwbcCodecError> {
        let next = self
            .string_bytes
            .checked_add(len)
            .ok_or(AwbcCodecError::LengthOverflow)?;
        Self::check_limit("string_bytes", next, self.budget.string_bytes)?;
        self.string_bytes = next;
        Ok(())
    }

    pub(super) fn read_str(&mut self) -> Result<&'a str, AwbcCodecError> {
        let len = self.read_len()?;
        self.add_string_bytes(len)?;
        self.read_utf8(len)
    }

    fn read_utf8(&mut self, len: usize) -> Result<&'a str, AwbcCodecError> {
        let offset = self.offset();
        let bytes = self.read_exact(len)?;
        std::str::from_utf8(bytes).map_err(|_| AwbcCodecError::InvalidUtf8 { offset })
    }

    pub(super) fn read_runtime_id_path(
        &mut self,
        family: RuntimeIdFamily,
        kind: &'static str,
    ) -> Result<RuntimeIdPath, AwbcCodecError> {
        let offset = self.offset();
        let count = self.read_len()?;
        if count == 0 {
            return Err(AwbcCodecError::InvalidMetadata {
                kind,
                message: RuntimeIdError::Empty { family }.to_string(),
                offset,
            });
        }
        self.add_collection_items(count)?;
        let start = self.offset();
        // The complete borrowed path and its cumulative allocation allowance
        // pass before retaining the first segment or allocating the slot Vec.
        self.read_nested(|reader| {
            for index in 0..count {
                let segment_offset = reader.offset();
                let text = reader.read_str()?;
                RuntimeIdPath::preflight_stored_source_segment(family, index, count, text)
                    .map_err(|error| AwbcCodecError::InvalidMetadata {
                        kind,
                        message: error.to_string(),
                        offset: segment_offset,
                    })?;
            }
            Ok(())
        })?;
        let end = self.offset();
        count
            .checked_mul(std::mem::size_of::<RuntimeIdSegment>())
            .filter(|bytes| isize::try_from(*bytes).is_ok())
            .ok_or(AwbcCodecError::LengthOverflow)?;
        // Reread only this admitted immutable span using the same primitive
        // grammar. Collection/string counters stay fully precharged; no new
        // Reader or allowance is created and no quota is rewound.
        self.offset = start;
        let mut segments = Vec::<RuntimeIdSegment>::with_capacity(count);
        for index in 0..count {
            let length = self.read_len()?;
            let text = self.read_utf8(length)?;
            let segment =
                RuntimeIdPath::preflight_stored_source_segment(family, index, count, text)
                    .map_err(|error| AwbcCodecError::InvalidMetadata {
                        kind,
                        message: error.to_string(),
                        offset,
                    })?;
            segments.push(segment.materialize());
        }
        debug_assert_eq!(self.offset(), end);
        RuntimeIdPath::from_source_segment_storage(family, segments).map_err(|error| {
            AwbcCodecError::InvalidMetadata {
                kind,
                message: error.to_string(),
                offset,
            }
        })
    }

    pub(super) fn read_registry_key(
        &mut self,
    ) -> Result<arcweft_id::LifetimeRegistryKey, AwbcCodecError> {
        let scope_offset = self.offset();
        let scope = self.read_str()?;
        let count = self.read_len()?;
        self.add_collection_items(count)?;
        count
            .checked_mul(size_of::<arcweft_id::DeclarationName>())
            .filter(|bytes| isize::try_from(*bytes).is_ok())
            .ok_or(AwbcCodecError::LengthOverflow)?;
        let start = self.offset();
        // Bound the whole borrowed key before retaining names. The second and
        // third passes reread only this immutable span; no quota is reset.
        self.read_nested(|reader| {
            for _ in 0..count {
                reader.read_str()?;
            }
            Ok(())
        })?;
        let end = self.offset();
        arcweft_id::DeclarationName::validate_source(scope).map_err(|source| {
            AwbcCodecError::InvalidRegistryKey {
                offset: scope_offset,
                source,
            }
        })?;
        self.offset = start;
        for _ in 0..count {
            let offset = self.offset();
            let len = self.read_len()?;
            let name = self.read_utf8(len)?;
            arcweft_id::DeclarationName::validate_source(name)
                .map_err(|source| AwbcCodecError::InvalidRegistryKey { offset, source })?;
        }
        self.offset = start;
        let mut segments = Vec::with_capacity(count);
        for _ in 0..count {
            let offset = self.offset();
            let len = self.read_len()?;
            let name = self.read_utf8(len)?;
            segments.push(
                arcweft_id::DeclarationName::try_new(name)
                    .map_err(|source| AwbcCodecError::InvalidRegistryKey { offset, source })?,
            );
        }
        debug_assert_eq!(self.offset(), end);
        let scope = arcweft_id::LifetimeScopeKind::parse(scope).map_err(|source| {
            AwbcCodecError::InvalidRegistryKey {
                offset: scope_offset,
                source,
            }
        })?;
        Ok(arcweft_id::LifetimeRegistryKey::new(
            scope,
            segments.into_boxed_slice(),
        ))
    }

    pub(super) fn check_limit(
        budget: &'static str,
        actual: usize,
        limit: usize,
    ) -> Result<(), AwbcCodecError> {
        if actual > limit {
            Err(AwbcCodecError::BudgetExceeded {
                budget,
                actual,
                limit,
            })
        } else {
            Ok(())
        }
    }

    fn enter_nesting(&mut self) -> Result<(), AwbcCodecError> {
        self.depth = self
            .depth
            .checked_add(1)
            .ok_or(AwbcCodecError::LengthOverflow)?;
        if self.depth > self.budget.nesting_depth {
            self.depth -= 1;
            return Err(AwbcCodecError::NestingDepthExceeded {
                limit: self.budget.nesting_depth,
            });
        }
        Ok(())
    }

    fn leave_nesting(&mut self) {
        self.depth -= 1;
    }
}

impl Wire for bool {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(u8::from(*self));
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        match reader.read_u8()? {
            0 => Ok(false),
            1 => Ok(true),
            tag => Err(AwbcCodecError::UnknownTag {
                kind: "bool",
                tag,
                offset,
            }),
        }
    }
}

impl Wire for u8 {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(*self);
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        reader.read_u8()
    }
}

impl Wire for u16 {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u16_le(*self);
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        reader.read_u16_le()
    }
}

impl Wire for u32 {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u32_var(*self);
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        reader.read_u32_var()
    }
}

impl Wire for u64 {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u64_le(*self);
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        reader.read_u64_le()
    }
}

impl Wire for i32 {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_i32_le(*self);
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        reader.read_i32_le()
    }
}

impl Wire for [u8; 16] {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_bytes(self);
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(reader
            .read_exact(16)?
            .try_into()
            .expect("fixed wire width checked"))
    }
}

impl Wire for [u8; 32] {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_bytes(self);
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(reader
            .read_exact(32)?
            .try_into()
            .expect("fixed wire width checked"))
    }
}

impl Wire for String {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_str(self)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        reader.read_str().map(str::to_owned)
    }
}

impl<T: Wire> Wire for Option<T> {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            None => writer.write_u8(0),
            Some(value) => {
                writer.write_u8(1);
                value.write_wire(writer)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        match reader.read_u8()? {
            0 => Ok(None),
            1 => T::read_wire(reader).map(Some),
            tag => Err(AwbcCodecError::UnknownTag {
                kind: "option",
                tag,
                offset,
            }),
        }
    }
}

impl<T: Wire> Wire for Vec<T> {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_table(self)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let len = reader.read_len()?;
        reader.read_items(len)
    }
}

macro_rules! wire_id {
    ($($name:ty),+ $(,)?) => {
        $(
            impl Wire for $name {
                fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
                    self.0.write_wire(writer)
                }

                fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
                    u32::read_wire(reader).map(Self)
                }
            }
        )+
    };
}

pub(super) use wire_id;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writer_uses_only_the_canonical_encoded_prefix() {
        let mut writer = Writer::default();
        writer.write_u32_var(128);
        writer.write_u8(0xaa);

        assert_eq!(writer.into_bytes(), vec![0x80, 0x01, 0xaa]);
    }

    #[test]
    fn writer_transaction_rolls_back_every_partial_byte() {
        let mut writer = Writer::with_capacity(20);
        writer.write_bytes(b"prefix");
        let result: Result<(), AwbcCodecError> = writer.transaction(|writer| {
            writer.write_bytes(b"partial");
            Err(AwbcCodecError::LengthOverflow)
        });

        assert_eq!(result, Err(AwbcCodecError::LengthOverflow));
        assert_eq!(writer.into_bytes(), b"prefix");
    }

    #[test]
    fn writer_patches_the_reserved_envelope_length_in_place() {
        let mut writer = Writer::with_capacity(20);
        writer.write_bytes(&[0; 20]);
        writer.write_bytes(&[1, 2, 3]);
        writer.patch_u64_le(12, 3).expect("patch envelope length");

        let bytes = writer.into_bytes();
        assert_eq!(&bytes[12..20], &3_u64.to_le_bytes());
        assert_eq!(&bytes[20..], &[1, 2, 3]);
    }

    #[test]
    fn reader_borrows_utf8_directly_from_the_input_slice() {
        let bytes = [3, b'a', b'w', b'b'];
        let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
        let value = reader.read_str().expect("borrow canonical UTF-8");

        assert_eq!(value, "awb");
        assert_eq!(value.as_ptr(), bytes[1..].as_ptr());
        reader.finish().expect("consume borrowed string");
    }

    #[test]
    fn borrowed_string_reader_retains_budget_and_utf8_fail_closed_rules() {
        let budget = AwbcDecodeBudget {
            string_bytes: 2,
            ..AwbcDecodeBudget::default()
        };
        let mut over_budget = Reader::new(&[3, b'a', b'w', b'b'], &budget);
        assert_eq!(
            over_budget
                .read_str()
                .expect_err("borrowed path still enforces byte budget"),
            AwbcCodecError::BudgetExceeded {
                budget: "string_bytes",
                actual: 3,
                limit: 2,
            }
        );

        let mut invalid_utf8 = Reader::new(&[1, 0xff], &AwbcDecodeBudget::default());
        assert_eq!(
            invalid_utf8
                .read_str()
                .expect_err("borrowed path still validates UTF-8"),
            AwbcCodecError::InvalidUtf8 { offset: 1 }
        );
    }

    #[test]
    fn reader_maps_truncation_to_the_consumed_byte_offset() {
        let mut reader = Reader::new(&[0xaa, 0x80], &AwbcDecodeBudget::default());

        assert_eq!(reader.read_u8(), Ok(0xaa));

        assert_eq!(
            reader.read_u32_var(),
            Err(AwbcCodecError::Truncated { offset: 2 })
        );
    }

    #[test]
    fn reader_maps_fifth_byte_overflow_and_sixth_byte_continuation() {
        for bytes in [
            [0xff, 0xff, 0xff, 0xff, 0x10, 0],
            [0x80, 0x80, 0x80, 0x80, 0x80, 0],
        ] {
            let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
            assert_eq!(
                reader.read_u32_var(),
                Err(AwbcCodecError::NonCanonicalVarint { offset: 0 })
            );
        }
    }

    #[test]
    fn reader_consumes_only_the_canonical_prefix_before_trailing_bytes() {
        let mut reader = Reader::new(&[0x80, 0x01, 0xaa], &AwbcDecodeBudget::default());

        assert_eq!(reader.read_u32_var(), Ok(128));
        assert_eq!(reader.offset(), 2);
        assert_eq!(reader.read_u8(), Ok(0xaa));
        assert_eq!(reader.offset(), 3);
    }
}
