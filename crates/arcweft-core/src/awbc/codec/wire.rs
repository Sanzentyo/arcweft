use super::{AwbcCodecError, AwbcDecodeBudget};

pub(super) trait Wire: Sized {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError>;
    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError>;
}

#[derive(Default)]
pub(super) struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    pub(super) fn finish(self) -> Vec<u8> {
        self.bytes
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

    pub(super) fn write_u32_le(&mut self, value: u32) {
        self.write_bytes(&value.to_le_bytes());
    }

    pub(super) fn write_u64_le(&mut self, value: u64) {
        self.write_bytes(&value.to_le_bytes());
    }

    pub(super) fn write_i32_le(&mut self, value: i32) {
        self.write_bytes(&value.to_le_bytes());
    }

    pub(super) fn write_u32_var(&mut self, mut value: u32) {
        while value >= 0x80 {
            let low_bits = u8::try_from(value & 0x7f).expect("varint low 7 bits always fit in u8");
            self.write_u8(low_bits | 0x80);
            value >>= 7;
        }
        self.write_u8(u8::try_from(value).expect("final varint byte is below 0x80"));
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

    pub(super) fn read_u32_le(&mut self) -> Result<u32, AwbcCodecError> {
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

    pub(super) fn read_i32_le(&mut self) -> Result<i32, AwbcCodecError> {
        Ok(i32::from_le_bytes(
            self.read_exact(4)?
                .try_into()
                .expect("fixed wire width checked"),
        ))
    }

    pub(super) fn read_u32_var(&mut self) -> Result<u32, AwbcCodecError> {
        let start = self.offset;
        let mut value = 0_u32;
        for shift in (0..35).step_by(7) {
            let byte = self.read_u8()?;
            if shift == 28 && byte > 0x0f {
                return Err(AwbcCodecError::NonCanonicalVarint { offset: start });
            }
            value |= u32::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                let canonical_len = if value == 0 {
                    1
                } else {
                    ((32 - value.leading_zeros()) as usize).div_ceil(7)
                };
                if self.offset - start != canonical_len {
                    return Err(AwbcCodecError::NonCanonicalVarint { offset: start });
                }
                return Ok(value);
            }
        }
        Err(AwbcCodecError::NonCanonicalVarint { offset: start })
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
        self.enter_nesting()?;
        let result = (0..len).map(|_| String::read_wire(self)).collect();
        self.leave_nesting();
        result
    }

    pub(super) fn read_items<T: Wire>(&mut self, len: usize) -> Result<Vec<T>, AwbcCodecError> {
        let next = self
            .collection_items
            .checked_add(len)
            .ok_or(AwbcCodecError::LengthOverflow)?;
        Self::check_limit("collection_items", next, self.budget.collection_items)?;
        self.collection_items = next;
        self.enter_nesting()?;
        let result = (0..len).map(|_| T::read_wire(self)).collect();
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
        writer.write_len(self.len())?;
        writer.write_bytes(self.as_bytes());
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let len = reader.read_len()?;
        reader.add_string_bytes(len)?;
        let offset = reader.offset();
        let bytes = reader.read_exact(len)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| AwbcCodecError::InvalidUtf8 { offset })
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

macro_rules! wire_enum {
    ($ty:ty, $kind:literal, {$($tag:literal => $variant:path),+ $(,)?}) => {
        impl Wire for $ty {
            fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
                let tag = match self {
                    $($variant => $tag,)+
                };
                writer.write_u8(tag);
                Ok(())
            }

            fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
                let offset = reader.offset();
                match reader.read_u8()? {
                    $($tag => Ok($variant),)+
                    tag => Err(AwbcCodecError::UnknownTag {
                        kind: $kind,
                        tag,
                        offset,
                    }),
                }
            }
        }
    };
}

pub(super) use wire_enum;
pub(super) use wire_id;
