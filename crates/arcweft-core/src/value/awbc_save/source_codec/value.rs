//! Exhaustive source grammar over the existing snapshot value algebra.

use super::super::{
    AwbcRuntimeCommandSnapshot, AwbcRuntimeFieldSnapshot, AwbcRuntimeIteratorSnapshot,
    AwbcRuntimeNominalFieldSnapshot, AwbcRuntimeNominalRecordSnapshot, AwbcRuntimeOpaqueSnapshot,
    AwbcRuntimeReductionSnapshot, AwbcRuntimeValueSnapshot as Value,
};
use super::{Reader, RuntimeLiteralSourceError as Error, Writer};
use crate::entry::TypeLayoutHash;
use crate::pattern::RuntimeSemanticTypeId;
use crate::time::LogicalDuration;
use crate::value::{
    RuntimeColor, RuntimeIteratorKind, RuntimeIteratorView, RuntimeScalarView, RuntimeValueView,
};

impl Writer<'_> {
    pub(super) fn value(&mut self, value: RuntimeValueView<'_>, depth: usize) -> Result<(), Error> {
        self.node(depth)?;
        match value {
            RuntimeValueView::Scalar(value) => self.scalar(value),
            RuntimeValueView::DenseNumeric(value) => {
                self.tag(6)?;
                self.numeric(value)
            }
            RuntimeValueView::Range(value) => {
                self.tag(12)?;
                self.range(value)
            }
            RuntimeValueView::Iterator(value) => {
                self.tag(13)?;
                self.iterator(value, depth)
            }
            RuntimeValueView::Tuple(values) => {
                self.tag(15)?;
                self.counts.array::<Value>(values.len())?;
                self.count(values.len())?;
                for value in values.iter() {
                    self.value(value, depth + 1)?;
                }
                Ok(())
            }
            RuntimeValueView::Sequence(value) => {
                self.tag(16)?;
                self.sequence(value, depth + 1)
            }
            RuntimeValueView::Record(fields) => {
                self.tag(17)?;
                self.counts
                    .array::<AwbcRuntimeFieldSnapshot>(fields.len())?;
                self.count(fields.len())?;
                for index in 0..fields.len() {
                    let (field, name, value) =
                        fields.get(index).expect("record view owns each field");
                    self.field(field)?;
                    self.string(name)?;
                    self.value(value, depth + 1)?;
                }
                Ok(())
            }
            RuntimeValueView::NominalRecord(value) => {
                self.tag(18)?;
                self.nominal_id(value.type_id())?;
                self.bytes(value.semantic_identity().as_bytes())?;
                self.bytes(value.layout().as_bytes())?;
                self.binding(value.type_instantiation(), depth + 1)?;
                let fields = value.fields();
                self.counts
                    .array::<AwbcRuntimeNominalFieldSnapshot>(fields.len())?;
                self.count(fields.len())?;
                for index in 0..fields.len() {
                    self.field(
                        value
                            .field_id(index)
                            .expect("nominal view owns each field ID"),
                    )?;
                    self.value(
                        fields.get(index).expect("nominal view owns each field"),
                        depth + 1,
                    )?;
                }
                Ok(())
            }
            RuntimeValueView::Opaque(value) => {
                self.tag(19)?;
                self.opaque_header(
                    value.producer(),
                    value.semantic_identity(),
                    value.value_class(),
                    value.persistence(),
                )?;
                self.counts.array::<Value>(1)?;
                self.value(value.payload(), depth + 1)
            }
            RuntimeValueView::Reduction(value) => {
                self.tag(20)?;
                self.opaque_owner(value.owner())?;
                self.counts.array::<Value>(1)?;
                self.value(value.state(), depth + 1)?;
                let commands = value.commands();
                self.counts
                    .array::<AwbcRuntimeCommandSnapshot>(commands.len())?;
                self.count(commands.len())?;
                for command in commands.iter() {
                    self.constructor(command.constructor())?;
                    self.target(command.target())?;
                    self.counts.array::<Value>(1)?;
                    self.value(command.payload(), depth + 1)?;
                }
                Ok(())
            }
            RuntimeValueView::Agent(value) => {
                self.tag(21)?;
                self.agent(value, depth)
            }
            RuntimeValueView::Variant(value) => {
                self.tag(22)?;
                self.variant_owner(value.owner())?;
                self.u32(value.ordinal())?;
                self.string(value.name())?;
                self.binding(value.type_instantiation(), depth + 1)?;
                let payload = value.payload();
                self.boolean(payload.is_some())?;
                if let Some(payload) = payload {
                    self.counts.array::<Value>(1)?;
                    self.value(payload, depth + 1)?;
                }
                Ok(())
            }
            RuntimeValueView::Need(_) => Err(Error::RuntimeCarrier {
                kind: "Need handle",
                offset: self.length,
            }),
            RuntimeValueView::Callable(_) => Err(Error::RuntimeCarrier {
                kind: "callable",
                offset: self.length,
            }),
        }
    }

    fn scalar(&mut self, value: RuntimeScalarView<'_>) -> Result<(), Error> {
        match value {
            RuntimeScalarView::Unit => self.tag(0),
            RuntimeScalarView::Bool(value) => {
                self.tag(1)?;
                self.boolean(value)
            }
            RuntimeScalarView::Int(value) => {
                self.tag(2)?;
                self.signed(value)
            }
            RuntimeScalarView::UInt(value) => {
                self.tag(3)?;
                self.unsigned(value)
            }
            RuntimeScalarView::F32(value) => {
                self.tag(4)?;
                self.u32(value.to_bits())
            }
            RuntimeScalarView::F64(value) => {
                self.tag(5)?;
                self.u64(value.to_bits())
            }
            RuntimeScalarView::String(value) => {
                self.tag(7)?;
                self.string(value)
            }
            RuntimeScalarView::Color(value) => {
                self.tag(8)?;
                self.bytes(&value.rgba8())
            }
            RuntimeScalarView::Char(value) => {
                self.tag(9)?;
                self.u32(u32::from(value))
            }
            RuntimeScalarView::Duration(value) => {
                self.tag(10)?;
                self.u64(value.as_nanos())
            }
            RuntimeScalarView::Progress { ratio, label } => {
                self.tag(11)?;
                crate::value::Progress::new(ratio).map_err(|source| Error::Progress {
                    offset: self.length,
                    source,
                })?;
                self.u32(ratio.to_bits())?;
                self.boolean(label.is_some())?;
                if let Some(label) = label {
                    self.string(label)?;
                }
                Ok(())
            }
            RuntimeScalarView::EntityRef(value) => {
                self.tag(14)?;
                self.entity(value)
            }
        }
    }

    fn iterator(&mut self, iterator: RuntimeIteratorView<'_>, depth: usize) -> Result<(), Error> {
        match iterator.kind() {
            RuntimeIteratorKind::Values(items) => {
                self.tag(0)?;
                self.counts.array::<Value>(items.len())?;
                self.count(items.len())?;
                for item in items.iter() {
                    self.value(item, depth + 1)?;
                }
                Ok(())
            }
            RuntimeIteratorKind::Range(value) => {
                self.tag(1)?;
                self.cursor(value)
            }
            RuntimeIteratorKind::Witness(_) => Err(Error::RuntimeCarrier {
                kind: "iterator witness",
                offset: self.length,
            }),
        }
    }
}

impl Reader<'_, '_> {
    pub(super) fn value(
        &mut self,
        materialize: bool,
        depth: usize,
    ) -> Result<Option<Value>, Error> {
        self.node(depth)?;
        let offset = self.offset;
        let value = match self.tag()? {
            0 => materialize.then_some(Value::Unit),
            1 => {
                let value = self.boolean()?;
                materialize.then_some(Value::Bool(value))
            }
            2 => {
                let value = self.signed()?;
                materialize.then_some(Value::Int(value))
            }
            3 => {
                let value = self.unsigned()?;
                materialize.then_some(Value::UInt(value))
            }
            4 => {
                let value = self.u32()?;
                materialize.then_some(Value::F32(value))
            }
            5 => {
                let value = self.u64()?;
                materialize.then_some(Value::F64(value))
            }
            6 => self.numeric(materialize)?.map(Value::DenseNumeric),
            7 => {
                let value = self.string()?;
                materialize.then(|| Value::String(value.to_owned()))
            }
            8 => {
                let value =
                    RuntimeColor::from_rgba8(self.take(4)?.try_into().expect("four color bytes"));
                materialize.then_some(Value::Color(value))
            }
            9 => {
                let offset = self.offset;
                let value = char::from_u32(self.u32()?).ok_or(Error::Invalid {
                    kind: "character",
                    offset,
                })?;
                materialize.then_some(Value::Char(value))
            }
            10 => {
                let value = LogicalDuration::from_nanos(self.u64()?);
                materialize.then_some(Value::Duration(value))
            }
            11 => {
                let offset = self.offset;
                let ratio = f32::from_bits(self.u32()?);
                crate::value::Progress::new(ratio)
                    .map_err(|source| Error::Progress { offset, source })?;
                let label = if self.boolean()? {
                    Some(self.string()?)
                } else {
                    None
                };
                materialize.then(|| Value::Progress {
                    ratio,
                    label: label.map(str::to_owned),
                })
            }
            12 => {
                let value = self.range()?;
                materialize.then_some(Value::Range(value))
            }
            13 => self.iterator(materialize, depth)?.map(Value::Iterator),
            14 => self.entity(materialize)?.map(Value::EntityRef),
            15 => {
                let count = self.count()?;
                self.counts.array::<Value>(count)?;
                let mut values = materialize.then(|| Vec::with_capacity(count));
                for _ in 0..count {
                    let value = self.value(materialize, depth + 1)?;
                    if let Some(values) = &mut values {
                        values.push(value.expect("retained tuple item"));
                    }
                }
                values.map(Value::Tuple)
            }
            16 => self.sequence(materialize, depth + 1)?.map(Value::Seq),
            17 => {
                let count = self.count()?;
                self.counts.array::<AwbcRuntimeFieldSnapshot>(count)?;
                let mut fields = materialize.then(|| Vec::with_capacity(count));
                for _ in 0..count {
                    let field = self.field()?;
                    let name = self.string()?;
                    let value = self.value(materialize, depth + 1)?;
                    if let Some(fields) = &mut fields {
                        fields.push(AwbcRuntimeFieldSnapshot {
                            field,
                            name: name.to_owned(),
                            value: value.expect("retained record field"),
                        });
                    }
                }
                fields.map(Value::Record)
            }
            18 => {
                let type_id = self.nominal_id(materialize)?;
                let semantic_identity = RuntimeSemanticTypeId::from_bytes(self.digest()?);
                let layout = TypeLayoutHash::from_bytes(self.digest()?);
                let type_instantiation = self.binding(materialize, depth + 1)?;
                let count = self.count()?;
                self.counts
                    .array::<AwbcRuntimeNominalFieldSnapshot>(count)?;
                let mut fields = materialize.then(|| Vec::with_capacity(count));
                for _ in 0..count {
                    let field = self.field()?;
                    let value = self.value(materialize, depth + 1)?;
                    if let Some(fields) = &mut fields {
                        fields.push(AwbcRuntimeNominalFieldSnapshot {
                            field,
                            value: value.expect("retained nominal field"),
                        });
                    }
                }
                fields.map(|fields| {
                    Value::NominalRecord(AwbcRuntimeNominalRecordSnapshot {
                        type_id: type_id.expect("retained nominal header"),
                        semantic_identity,
                        layout,
                        type_instantiation,
                        fields,
                    })
                })
            }
            19 => {
                let (producer, semantic_identity, value_class, persistence) =
                    self.opaque_header(materialize)?;
                self.counts.array::<Value>(1)?;
                let payload = self.value(materialize, depth + 1)?;
                payload.map(|payload| {
                    Value::Opaque(AwbcRuntimeOpaqueSnapshot {
                        producer: producer.expect("retained opaque producer"),
                        semantic_identity,
                        value_class,
                        persistence,
                        payload: Box::new(payload),
                    })
                })
            }
            20 => {
                let owner = self.opaque_owner(materialize)?;
                self.counts.array::<Value>(1)?;
                let state = self.value(materialize, depth + 1)?;
                let count = self.count()?;
                self.counts.array::<AwbcRuntimeCommandSnapshot>(count)?;
                let mut commands = materialize.then(|| Vec::with_capacity(count));
                for _ in 0..count {
                    let constructor = self.constructor(materialize)?;
                    let target = self.target(materialize)?;
                    self.counts.array::<Value>(1)?;
                    let payload = self.value(materialize, depth + 1)?;
                    if let Some(commands) = &mut commands {
                        commands.push(AwbcRuntimeCommandSnapshot {
                            constructor: constructor.expect("retained command constructor"),
                            target: target.expect("retained command target"),
                            payload: Box::new(payload.expect("retained command payload")),
                        });
                    }
                }
                commands.map(|commands| {
                    Value::Reduction(AwbcRuntimeReductionSnapshot {
                        owner: owner.expect("retained reduction owner"),
                        state: Box::new(state.expect("retained reduction state")),
                        commands,
                    })
                })
            }
            21 => self.agent(materialize, depth)?.map(Value::Agent),
            22 => {
                let owner = self.variant_owner(materialize)?;
                let ordinal = self.u32()?;
                let name = self.string()?;
                let type_instantiation = self.binding(materialize, depth + 1)?;
                let payload = if self.boolean()? {
                    self.counts.array::<Value>(1)?;
                    self.value(materialize, depth + 1)?.map(Box::new)
                } else {
                    None
                };
                materialize.then(|| Value::Variant {
                    owner: owner.expect("retained variant owner"),
                    ordinal,
                    name: name.to_owned(),
                    type_instantiation,
                    payload,
                })
            }
            23 => {
                return Err(Error::RuntimeCarrier {
                    kind: "Need handle",
                    offset,
                });
            }
            24 => {
                return Err(Error::RuntimeCarrier {
                    kind: "callable",
                    offset,
                });
            }
            tag => {
                return Err(Error::Tag {
                    kind: "value",
                    tag,
                    offset,
                });
            }
        };
        Ok(value)
    }

    fn iterator(
        &mut self,
        materialize: bool,
        depth: usize,
    ) -> Result<Option<AwbcRuntimeIteratorSnapshot>, Error> {
        let offset = self.offset;
        match self.tag()? {
            0 => {
                let count = self.count()?;
                self.counts.array::<Value>(count)?;
                let mut items = materialize.then(|| Vec::with_capacity(count));
                for _ in 0..count {
                    let value = self.value(materialize, depth + 1)?;
                    if let Some(items) = &mut items {
                        items.push(value.expect("retained iterator item"));
                    }
                }
                Ok(items.map(|items| AwbcRuntimeIteratorSnapshot::Values { items }))
            }
            1 => {
                let value = self.cursor()?;
                Ok(materialize.then_some(AwbcRuntimeIteratorSnapshot::Range(value)))
            }
            2 => Err(Error::RuntimeCarrier {
                kind: "iterator witness",
                offset,
            }),
            tag => Err(Error::Tag {
                kind: "iterator",
                tag,
                offset,
            }),
        }
    }
}
