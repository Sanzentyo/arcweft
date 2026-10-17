//! Width-preserving primitive and numeric source fields.

use super::{Reader, RuntimeLiteralSourceError as Error, Writer};
use crate::math::{
    RuntimeDenseNumericElements, RuntimeDenseNumericPayload, RuntimeDenseNumericType,
    RuntimeFloatWidth,
};
use crate::value::{
    RuntimeInt, RuntimeRange, RuntimeRangeIterator, RuntimeSignedIntWidth, RuntimeUInt,
    RuntimeUnsignedIntWidth,
};

impl Writer<'_> {
    pub(super) fn signed(&mut self, value: RuntimeInt) -> Result<(), Error> {
        self.tag(value.width().semantic_tag())?;
        self.bytes(&value.as_i128().to_le_bytes())
    }

    pub(super) fn unsigned(&mut self, value: RuntimeUInt) -> Result<(), Error> {
        self.tag(value.width().semantic_tag())?;
        self.bytes(&value.as_u128().to_le_bytes())
    }

    pub(super) fn range(&mut self, value: &RuntimeRange) -> Result<(), Error> {
        if value.signed_width().is_none() && value.unsigned_width().is_none() {
            return Err(Error::Invalid {
                kind: "range bound widths",
                offset: self.length,
            });
        }
        match value {
            RuntimeRange::Int {
                start,
                end,
                inclusive,
            } => {
                self.tag(0)?;
                for value in [start, end] {
                    self.boolean(value.is_some())?;
                    if let Some(value) = value {
                        self.signed(*value)?;
                    }
                }
                self.boolean(*inclusive)
            }
            RuntimeRange::UInt {
                start,
                end,
                inclusive,
            } => {
                self.tag(1)?;
                for value in [start, end] {
                    self.boolean(value.is_some())?;
                    if let Some(value) = value {
                        self.unsigned(*value)?;
                    }
                }
                self.boolean(*inclusive)
            }
        }
    }

    pub(super) fn cursor(&mut self, value: &RuntimeRangeIterator) -> Result<(), Error> {
        if value.validated_end().is_none() {
            return Err(Error::Invalid {
                kind: "range iterator cursor",
                offset: self.length,
            });
        }
        match value {
            RuntimeRangeIterator::Int {
                width,
                current,
                end,
                inclusive,
                done,
            } => {
                self.tag(0)?;
                self.tag(width.semantic_tag())?;
                self.bytes(&current.to_le_bytes())?;
                self.bytes(&end.to_le_bytes())?;
                self.boolean(*inclusive)?;
                self.boolean(*done)
            }
            RuntimeRangeIterator::UInt {
                width,
                current,
                end,
                inclusive,
                done,
            } => {
                self.tag(1)?;
                self.tag(width.semantic_tag())?;
                self.bytes(&current.to_le_bytes())?;
                self.bytes(&end.to_le_bytes())?;
                self.boolean(*inclusive)?;
                self.boolean(*done)
            }
        }
    }

    pub(super) fn numeric(&mut self, value: &RuntimeDenseNumericPayload) -> Result<(), Error> {
        self.bytes(&value.ty().semantic_parts())?;
        self.counts.array::<u64>(value.dimensions().len())?;
        self.count(value.dimensions().len())?;
        for dimension in value.dimensions() {
            self.u64(*dimension)?;
        }
        self.count(value.elements().len())?;
        match value.elements() {
            RuntimeDenseNumericElements::F32(values) => {
                self.counts.array::<u32>(values.len())?;
                for value in values {
                    self.u32(*value)?;
                }
            }
            RuntimeDenseNumericElements::F64(values) => {
                self.counts.array::<u64>(values.len())?;
                for value in values {
                    self.u64(*value)?;
                }
            }
        }
        Ok(())
    }
}

impl<'a> Reader<'a, '_> {
    fn signed_width(&mut self) -> Result<RuntimeSignedIntWidth, Error> {
        let offset = self.offset;
        let tag = self.tag()?;
        RuntimeSignedIntWidth::from_semantic_tag(tag).ok_or(Error::Tag {
            kind: "signed width",
            tag,
            offset,
        })
    }

    fn unsigned_width(&mut self) -> Result<RuntimeUnsignedIntWidth, Error> {
        let offset = self.offset;
        let tag = self.tag()?;
        RuntimeUnsignedIntWidth::from_semantic_tag(tag).ok_or(Error::Tag {
            kind: "unsigned width",
            tag,
            offset,
        })
    }

    pub(super) fn signed(&mut self) -> Result<RuntimeInt, Error> {
        let width = self.signed_width()?;
        let offset = self.offset;
        let value = i128::from_le_bytes(
            self.take(16)?
                .try_into()
                .expect("sixteen signed source bytes"),
        );
        RuntimeInt::from_i128(width, value).ok_or(Error::Invalid {
            kind: "signed integer width",
            offset,
        })
    }

    pub(super) fn unsigned(&mut self) -> Result<RuntimeUInt, Error> {
        let width = self.unsigned_width()?;
        let offset = self.offset;
        let value = self.u128()?;
        RuntimeUInt::from_u128(width, value).ok_or(Error::Invalid {
            kind: "unsigned integer width",
            offset,
        })
    }

    pub(super) fn range(&mut self) -> Result<RuntimeRange, Error> {
        let offset = self.offset;
        let value = match self.tag()? {
            0 => RuntimeRange::Int {
                start: if self.boolean()? {
                    Some(self.signed()?)
                } else {
                    None
                },
                end: if self.boolean()? {
                    Some(self.signed()?)
                } else {
                    None
                },
                inclusive: self.boolean()?,
            },
            1 => RuntimeRange::UInt {
                start: if self.boolean()? {
                    Some(self.unsigned()?)
                } else {
                    None
                },
                end: if self.boolean()? {
                    Some(self.unsigned()?)
                } else {
                    None
                },
                inclusive: self.boolean()?,
            },
            tag => {
                return Err(Error::Tag {
                    kind: "range",
                    tag,
                    offset,
                });
            }
        };
        if value.signed_width().is_none() && value.unsigned_width().is_none() {
            return Err(Error::Invalid {
                kind: "range bound widths",
                offset,
            });
        }
        Ok(value)
    }

    pub(super) fn cursor(&mut self) -> Result<RuntimeRangeIterator, Error> {
        let offset = self.offset;
        let value = match self.tag()? {
            0 => RuntimeRangeIterator::Int {
                width: self.signed_width()?,
                current: i128::from_le_bytes(
                    self.take(16)?.try_into().expect("sixteen cursor bytes"),
                ),
                end: i128::from_le_bytes(self.take(16)?.try_into().expect("sixteen bound bytes")),
                inclusive: self.boolean()?,
                done: self.boolean()?,
            },
            1 => RuntimeRangeIterator::UInt {
                width: self.unsigned_width()?,
                current: self.u128()?,
                end: self.u128()?,
                inclusive: self.boolean()?,
                done: self.boolean()?,
            },
            tag => {
                return Err(Error::Tag {
                    kind: "range iterator",
                    tag,
                    offset,
                });
            }
        };
        if value.validated_end().is_none() {
            return Err(Error::Invalid {
                kind: "range iterator cursor",
                offset,
            });
        }
        Ok(value)
    }

    pub(super) fn numeric(
        &mut self,
        materialize: bool,
    ) -> Result<Option<RuntimeDenseNumericPayload>, Error> {
        let offset = self.offset;
        let parts = [self.tag()?, self.tag()?];
        let ty = RuntimeDenseNumericType::from_semantic_parts(parts).ok_or(Error::Invalid {
            kind: "dense numeric container/width",
            offset,
        })?;
        let rank = self.count()?;
        let start = self.offset;
        let dimensions = self.primitives(rank, 8, materialize, |_, chunk| {
            Ok(u64::from_le_bytes(
                chunk.try_into().expect("eight dimension bytes"),
            ))
        })?;
        let dimensions_source: &'a [u8] = &self.bytes[start..self.offset];
        let count = self.count()?;
        let elements = match ty.scalar_width() {
            RuntimeFloatWidth::F32 => self
                .primitives(count, 4, materialize, |_, chunk| {
                    Ok(u32::from_le_bytes(
                        chunk.try_into().expect("four numeric bytes"),
                    ))
                })?
                .map(RuntimeDenseNumericElements::F32),
            RuntimeFloatWidth::F64 => self
                .primitives(count, 8, materialize, |_, chunk| {
                    Ok(u64::from_le_bytes(
                        chunk.try_into().expect("eight numeric bytes"),
                    ))
                })?
                .map(RuntimeDenseNumericElements::F64),
        };
        RuntimeDenseNumericPayload::validate_shape_values(
            ty,
            dimensions_source.chunks_exact(8).map(|chunk| {
                u64::from_le_bytes(chunk.try_into().expect("admitted dimension width"))
            }),
            count,
        )
        .map_err(|source| Error::Dense { offset, source })?;
        if materialize {
            RuntimeDenseNumericPayload::try_new(
                ty,
                dimensions.expect("retained dimensions"),
                elements.expect("retained numeric elements"),
            )
            .map(Some)
            .map_err(|source| Error::Dense { offset, source })
        } else {
            Ok(None)
        }
    }
}
