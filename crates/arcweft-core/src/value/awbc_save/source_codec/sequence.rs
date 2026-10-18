//! Physical values, homogeneous storage and recursive column source fields.

use super::super::{
    AwbcRuntimeRecordSeqFieldSnapshot, AwbcRuntimeSeqSnapshot as Sequence,
    AwbcRuntimeValueSnapshot as Value,
};
use super::{Reader, RuntimeLiteralSourceError as Error, Writer};
use crate::time::LogicalDuration;
use crate::value::{
    DenseSeq, DenseSeqStorage, RuntimeEntityReference, RuntimeISizeValue, RuntimeSeq,
    RuntimeSequenceView, RuntimeUSizeValue,
};

impl Writer<'_> {
    fn dense_slice<T>(
        &mut self,
        values: &[T],
        mut write: impl FnMut(&mut Self, &T) -> Result<(), Error>,
    ) -> Result<(), Error> {
        self.counts.array::<T>(values.len())?;
        self.count(values.len())?;
        for value in values {
            write(self, value)?;
        }
        Ok(())
    }

    pub(super) fn sequence(
        &mut self,
        value: RuntimeSequenceView<'_>,
        depth: usize,
    ) -> Result<(), Error> {
        self.node(depth)?;
        match value {
            value @ (RuntimeSequenceView::Live(RuntimeSeq::Values(_))
            | RuntimeSequenceView::Snapshot(Sequence::Values(_))
            | RuntimeSequenceView::DialogueBindings(_)
            | RuntimeSequenceView::DialogueEffects(_)) => {
                let count = value.len().map_err(|_| Error::Arithmetic)?;
                self.tag(0)?;
                self.counts.array::<Value>(count)?;
                self.count(count)?;
                for index in 0..count {
                    self.value(
                        value
                            .value_view(index)
                            .expect("value sequence owns every row"),
                        depth + 1,
                    )?;
                }
            }
            RuntimeSequenceView::Live(RuntimeSeq::Dense(value))
            | RuntimeSequenceView::Snapshot(Sequence::Dense(value)) => {
                self.tag(1)?;
                self.dense(value)?;
            }
            RuntimeSequenceView::Bytes(bytes) => {
                self.tag(1)?;
                self.tag(16)?;
                self.dense_slice(bytes, |writer, value| {
                    writer.bytes(std::slice::from_ref(value))
                })?;
            }
            value @ (RuntimeSequenceView::Live(
                RuntimeSeq::TupleColumns(_) | RuntimeSeq::RecordColumns(_),
            )
            | RuntimeSequenceView::Snapshot(
                Sequence::TupleColumns { .. } | Sequence::RecordColumns { .. },
            )) => {
                let columns = value
                    .columns()
                    .expect("column storage has its physical inventory");
                self.tag(if columns.is_record() { 3 } else { 2 })?;
                let length = value.len().map_err(|_| Error::Arithmetic)?;
                self.u64(u64::try_from(length).map_err(|_| Error::Arithmetic)?)?;
                self.counts.elements(length)?;
                if columns.is_record() {
                    self.counts
                        .array::<AwbcRuntimeRecordSeqFieldSnapshot>(columns.len())?;
                } else {
                    self.counts.array::<Sequence>(columns.len())?;
                }
                self.count(columns.len())?;
                for index in 0..columns.len() {
                    let (field, values) = columns
                        .get(index)
                        .expect("column inventory owns every child");
                    if let Some((field, name)) = field {
                        self.field(field)?;
                        self.string(name)?;
                    }
                    self.sequence(values, depth + 1)?;
                }
            }
        }
        Ok(())
    }

    fn dense(&mut self, value: &DenseSeq) -> Result<(), Error> {
        match value {
            DenseSeq::Units(count) => {
                self.tag(0)?;
                self.counts.elements(*count)?;
                self.count(*count)
            }
            DenseSeq::I8(values) => {
                self.tag(1)?;
                self.dense_slice(values.as_slice(), |writer, value| {
                    writer.bytes(&value.to_le_bytes())
                })
            }
            DenseSeq::I16(values) => {
                self.tag(2)?;
                self.dense_slice(values.as_slice(), |writer, value| {
                    writer.bytes(&value.to_le_bytes())
                })
            }
            DenseSeq::I32(values) => {
                self.tag(3)?;
                self.dense_slice(values.as_slice(), |writer, value| {
                    writer.bytes(&value.to_le_bytes())
                })
            }
            DenseSeq::I64(values) => {
                self.tag(4)?;
                self.dense_slice(values.as_slice(), |writer, value| {
                    writer.bytes(&value.to_le_bytes())
                })
            }
            DenseSeq::I128(values) => {
                self.tag(5)?;
                self.dense_slice(values.as_slice(), |writer, value| {
                    writer.bytes(&value.to_le_bytes())
                })
            }
            DenseSeq::ISize(values) => {
                self.tag(6)?;
                self.dense_slice(values.as_slice(), |writer, value| {
                    writer.bytes(&value.get().to_le_bytes())
                })
            }
            DenseSeq::U8(values) => {
                self.tag(7)?;
                self.dense_slice(values.as_slice(), |writer, value| {
                    writer.bytes(&value.to_le_bytes())
                })
            }
            DenseSeq::U16(values) => {
                self.tag(8)?;
                self.dense_slice(values.as_slice(), |writer, value| {
                    writer.bytes(&value.to_le_bytes())
                })
            }
            DenseSeq::U32(values) => {
                self.tag(9)?;
                self.dense_slice(values.as_slice(), |writer, value| {
                    writer.bytes(&value.to_le_bytes())
                })
            }
            DenseSeq::U64(values) => {
                self.tag(10)?;
                self.dense_slice(values.as_slice(), |writer, value| {
                    writer.bytes(&value.to_le_bytes())
                })
            }
            DenseSeq::U128(values) => {
                self.tag(11)?;
                self.dense_slice(values.as_slice(), |writer, value| {
                    writer.bytes(&value.to_le_bytes())
                })
            }
            DenseSeq::USize(values) => {
                self.tag(12)?;
                self.dense_slice(values.as_slice(), |writer, value| {
                    writer.bytes(&value.get().to_le_bytes())
                })
            }
            DenseSeq::F32(values) => {
                self.tag(13)?;
                self.dense_slice(values.as_slice(), |writer, value| {
                    writer.bytes(&value.to_bits().to_le_bytes())
                })
            }
            DenseSeq::F64(values) => {
                self.tag(14)?;
                self.dense_slice(values.as_slice(), |writer, value| {
                    writer.bytes(&value.to_bits().to_le_bytes())
                })
            }
            DenseSeq::Bool(values) => {
                self.tag(15)?;
                self.dense_slice(values.as_slice(), |writer, value| {
                    writer.bytes(&[u8::from(*value)])
                })
            }
            DenseSeq::Bytes(values) => {
                self.tag(16)?;
                self.dense_slice(values.as_slice(), |writer, value| {
                    writer.bytes(&value.to_le_bytes())
                })
            }
            DenseSeq::Chars(values) => {
                self.tag(17)?;
                self.dense_slice(values.as_slice(), |writer, value| {
                    writer.bytes(&u32::from(*value).to_le_bytes())
                })
            }
            DenseSeq::Durations(values) => {
                self.tag(18)?;
                self.dense_slice(values.as_slice(), |writer, value| {
                    writer.bytes(&value.as_nanos().to_le_bytes())
                })
            }
            DenseSeq::Strings(values) => {
                self.tag(19)?;
                self.dense_slice(values.as_slice(), |writer, value| writer.string(value))
            }
            DenseSeq::EntityRefs(values) => {
                self.tag(20)?;
                self.dense_slice(values.as_slice(), |writer, value| writer.entity(value))
            }
        }
    }
}

impl Reader<'_, '_> {
    pub(super) fn sequence(
        &mut self,
        materialize: bool,
        depth: usize,
    ) -> Result<Option<Sequence>, Error> {
        self.node(depth)?;
        let offset = self.offset;
        match self.tag()? {
            0 => {
                let count = self.count()?;
                self.counts.array::<Value>(count)?;
                let mut values = materialize.then(|| Vec::with_capacity(count));
                for _ in 0..count {
                    let value = self.value(materialize, depth + 1)?;
                    if let Some(values) = &mut values {
                        values.push(value.expect("retained sequence item"));
                    }
                }
                Ok(values.map(Sequence::Values))
            }
            1 => Ok(self.dense(materialize)?.map(Sequence::Dense)),
            2 => {
                let len = self.u64()?;
                self.counts
                    .elements(usize::try_from(len).map_err(|_| Error::Arithmetic)?)?;
                let count = self.count()?;
                self.counts.array::<Sequence>(count)?;
                let mut columns = materialize.then(|| Vec::with_capacity(count));
                for _ in 0..count {
                    let column = self.sequence(materialize, depth + 1)?;
                    if let Some(columns) = &mut columns {
                        columns.push(column.expect("retained column"));
                    }
                }
                Ok(columns.map(|columns| Sequence::TupleColumns { len, columns }))
            }
            3 => {
                let len = self.u64()?;
                self.counts
                    .elements(usize::try_from(len).map_err(|_| Error::Arithmetic)?)?;
                let count = self.count()?;
                self.counts
                    .array::<AwbcRuntimeRecordSeqFieldSnapshot>(count)?;
                let mut fields = materialize.then(|| Vec::with_capacity(count));
                for _ in 0..count {
                    let field = self.field()?;
                    let name = self.string()?;
                    let values = self.sequence(materialize, depth + 1)?;
                    if let Some(fields) = &mut fields {
                        fields.push(AwbcRuntimeRecordSeqFieldSnapshot {
                            field,
                            name: name.to_owned(),
                            values: values.expect("retained record column"),
                        });
                    }
                }
                Ok(fields.map(|fields| Sequence::RecordColumns { len, fields }))
            }
            tag => Err(Error::Tag {
                kind: "sequence storage",
                tag,
                offset,
            }),
        }
    }

    fn dense(&mut self, materialize: bool) -> Result<Option<DenseSeq>, Error> {
        let offset = self.offset;
        let tag = self.tag()?;
        if tag > 20 {
            return Err(Error::Tag {
                kind: "dense storage",
                tag,
                offset,
            });
        }
        let count = self.count()?;
        let value = match tag {
            0 => {
                self.counts.elements(count)?;
                materialize.then_some(DenseSeq::Units(count))
            }
            1 => self
                .primitives::<i8>(count, 1, materialize, |_, chunk| {
                    Ok(i8::from_le_bytes(
                        chunk.try_into().expect("one signed byte"),
                    ))
                })?
                .map(|values| DenseSeq::I8(DenseSeqStorage::new(values))),
            2 => self
                .primitives::<i16>(count, 2, materialize, |_, chunk| {
                    Ok(i16::from_le_bytes(
                        chunk.try_into().expect("two signed bytes"),
                    ))
                })?
                .map(|values| DenseSeq::I16(DenseSeqStorage::new(values))),
            3 => self
                .primitives::<i32>(count, 4, materialize, |_, chunk| {
                    Ok(i32::from_le_bytes(
                        chunk.try_into().expect("four signed bytes"),
                    ))
                })?
                .map(|values| DenseSeq::I32(DenseSeqStorage::new(values))),
            4 => self
                .primitives::<i64>(count, 8, materialize, |_, chunk| {
                    Ok(i64::from_le_bytes(
                        chunk.try_into().expect("eight signed bytes"),
                    ))
                })?
                .map(|values| DenseSeq::I64(DenseSeqStorage::new(values))),
            5 => self
                .primitives::<i128>(count, 16, materialize, |_, chunk| {
                    Ok(i128::from_le_bytes(
                        chunk.try_into().expect("sixteen signed bytes"),
                    ))
                })?
                .map(|values| DenseSeq::I128(DenseSeqStorage::new(values))),
            6 => self
                .primitives::<RuntimeISizeValue>(count, 8, materialize, |_, chunk| {
                    Ok(RuntimeISizeValue::new(i64::from_le_bytes(
                        chunk.try_into().expect("eight portable isize bytes"),
                    )))
                })?
                .map(|values| DenseSeq::ISize(DenseSeqStorage::new(values))),
            7 => self
                .primitives::<u8>(count, 1, materialize, |_, chunk| {
                    Ok(u8::from_le_bytes(
                        chunk.try_into().expect("one unsigned byte"),
                    ))
                })?
                .map(|values| DenseSeq::U8(DenseSeqStorage::new(values))),
            8 => self
                .primitives::<u16>(count, 2, materialize, |_, chunk| {
                    Ok(u16::from_le_bytes(
                        chunk.try_into().expect("two unsigned bytes"),
                    ))
                })?
                .map(|values| DenseSeq::U16(DenseSeqStorage::new(values))),
            9 => self
                .primitives::<u32>(count, 4, materialize, |_, chunk| {
                    Ok(u32::from_le_bytes(
                        chunk.try_into().expect("four unsigned bytes"),
                    ))
                })?
                .map(|values| DenseSeq::U32(DenseSeqStorage::new(values))),
            10 => self
                .primitives::<u64>(count, 8, materialize, |_, chunk| {
                    Ok(u64::from_le_bytes(
                        chunk.try_into().expect("eight unsigned bytes"),
                    ))
                })?
                .map(|values| DenseSeq::U64(DenseSeqStorage::new(values))),
            11 => self
                .primitives::<u128>(count, 16, materialize, |_, chunk| {
                    Ok(u128::from_le_bytes(
                        chunk.try_into().expect("sixteen unsigned bytes"),
                    ))
                })?
                .map(|values| DenseSeq::U128(DenseSeqStorage::new(values))),
            12 => self
                .primitives::<RuntimeUSizeValue>(count, 8, materialize, |_, chunk| {
                    Ok(RuntimeUSizeValue::new(u64::from_le_bytes(
                        chunk.try_into().expect("eight portable usize bytes"),
                    )))
                })?
                .map(|values| DenseSeq::USize(DenseSeqStorage::new(values))),
            13 => self
                .primitives::<f32>(count, 4, materialize, |_, chunk| {
                    Ok(f32::from_bits(u32::from_le_bytes(
                        chunk.try_into().expect("four float bits"),
                    )))
                })?
                .map(|values| DenseSeq::F32(DenseSeqStorage::new(values))),
            14 => self
                .primitives::<f64>(count, 8, materialize, |_, chunk| {
                    Ok(f64::from_bits(u64::from_le_bytes(
                        chunk.try_into().expect("eight float bits"),
                    )))
                })?
                .map(|values| DenseSeq::F64(DenseSeqStorage::new(values))),
            15 => self
                .primitives::<bool>(count, 1, materialize, |offset, chunk| match chunk[0] {
                    0 => Ok(false),
                    1 => Ok(true),
                    tag => Err(Error::Tag {
                        kind: "dense boolean",
                        tag,
                        offset,
                    }),
                })?
                .map(|values| DenseSeq::Bool(DenseSeqStorage::new(values))),
            16 => self
                .primitives::<u8>(count, 1, materialize, |_, chunk| {
                    Ok(u8::from_le_bytes(chunk.try_into().expect("one byte")))
                })?
                .map(|values| DenseSeq::Bytes(DenseSeqStorage::new(values))),
            17 => self
                .primitives::<char>(count, 4, materialize, |offset, chunk| {
                    char::from_u32(u32::from_le_bytes(
                        chunk.try_into().expect("four character bytes"),
                    ))
                    .ok_or(Error::Invalid {
                        kind: "dense character",
                        offset,
                    })
                })?
                .map(|values| DenseSeq::Chars(DenseSeqStorage::new(values))),
            18 => self
                .primitives::<LogicalDuration>(count, 8, materialize, |_, chunk| {
                    Ok(LogicalDuration::from_nanos(u64::from_le_bytes(
                        chunk.try_into().expect("eight duration bytes"),
                    )))
                })?
                .map(|values| DenseSeq::Durations(DenseSeqStorage::new(values))),
            19 => {
                self.counts.array::<String>(count)?;
                let mut values = materialize.then(|| Vec::with_capacity(count));
                for _ in 0..count {
                    let value = self.string()?;
                    if let Some(values) = &mut values {
                        values.push(value.to_owned());
                    }
                }
                values.map(|values| DenseSeq::Strings(DenseSeqStorage::new(values)))
            }
            20 => {
                self.counts.array::<RuntimeEntityReference>(count)?;
                let mut values = materialize.then(|| Vec::with_capacity(count));
                for _ in 0..count {
                    let value = self.entity(materialize)?;
                    if let Some(values) = &mut values {
                        values.push(value.expect("retained dense entity"));
                    }
                }
                values.map(|values| DenseSeq::EntityRefs(DenseSeqStorage::new(values)))
            }
            tag => {
                return Err(Error::Tag {
                    kind: "dense storage",
                    tag,
                    offset,
                });
            }
        };
        Ok(value)
    }
}
