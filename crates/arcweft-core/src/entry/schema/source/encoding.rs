//! Counted emission borrows the original typed schema and its shared traversal.

use super::{Counts, DOMAIN, Error, Footprint, RuntimeSchemaSourceLimits, charge, limit};
use crate::entry::schema::{RuntimeEnumTagStyle, RuntimeTypeSchema, traversal::SchemaPath};
use crate::task::semantic::TaskSemanticMeter;

pub(crate) struct PreparedRuntimeSchemaSourceEncoding<'a> {
    schema: &'a RuntimeTypeSchema,
    limits: RuntimeSchemaSourceLimits,
    bytes: usize,
    footprint: Footprint,
}
impl PreparedRuntimeSchemaSourceEncoding<'_> {
    pub(crate) const fn encoded_bytes(&self) -> usize {
        self.bytes
    }
    pub(crate) const fn storage_bytes(&self) -> usize {
        self.footprint.storage
    }
    pub(crate) const fn elements(&self) -> usize {
        self.footprint.elements
    }
    pub(crate) fn append_to(self, output: &mut Vec<u8>) -> Result<(), Error> {
        let remaining = output.capacity() - output.len();
        if remaining < self.bytes {
            return Err(Error::OutputCapacity {
                required: self.bytes,
                remaining,
            });
        }
        let mut writer = Writer {
            output: Some(output),
            meter: None,
            bytes: 0,
            limits: self.limits,
            counts: Counts::new(self.limits),
        };
        writer.emit(self.schema)?;
        debug_assert_eq!(writer.bytes, self.bytes);
        debug_assert_eq!(writer.counts.footprint, self.footprint);
        Ok(())
    }
}
impl RuntimeTypeSchema {
    pub(crate) fn prepare_source_encoding(
        &self,
        limits: RuntimeSchemaSourceLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<PreparedRuntimeSchemaSourceEncoding<'_>, Error> {
        meter.status()?;
        let result = (|| {
            let mut writer = Writer {
                output: None,
                meter: Some(&mut *meter),
                bytes: 0,
                limits,
                counts: Counts::new(limits),
            };
            writer.emit(self)?;
            Ok(PreparedRuntimeSchemaSourceEncoding {
                schema: self,
                limits,
                bytes: writer.bytes,
                footprint: writer.counts.footprint,
            })
        })();
        result.map_err(|error: Error| error.reject(meter))
    }
}
struct Writer<'a> {
    output: Option<&'a mut Vec<u8>>,
    meter: Option<&'a mut TaskSemanticMeter>,
    bytes: usize,
    limits: RuntimeSchemaSourceLimits,
    counts: Counts,
}
impl Writer<'_> {
    fn raw(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.bytes = self
            .bytes
            .checked_add(bytes.len())
            .ok_or(Error::Arithmetic)?;
        limit("encoded bytes", self.bytes, self.limits.encoded_bytes)?;
        if let Some(meter) = self.meter.as_mut() {
            charge(meter, bytes.len())?;
        }
        if let Some(output) = self.output.as_mut() {
            output.extend_from_slice(bytes);
        }
        Ok(())
    }
    fn byte(&mut self, byte: u8) -> Result<(), Error> {
        self.raw(&[byte])
    }
    fn u32(&mut self, value: u32) -> Result<(), Error> {
        self.raw(&value.to_le_bytes())
    }
    fn count(&mut self, value: usize) -> Result<(), Error> {
        self.u32(u32::try_from(value).map_err(|_| Error::Arithmetic)?)
    }
    fn text(&mut self, value: &str) -> Result<(), Error> {
        self.counts.text(value.len())?;
        self.count(value.len())?;
        self.raw(value.as_bytes())
    }
    fn schemas(&mut self, count: usize) -> Result<(), Error> {
        self.counts.array::<RuntimeTypeSchema>(count)?;
        self.count(count)
    }
    fn emit(&mut self, schema: &RuntimeTypeSchema) -> Result<(), Error> {
        self.raw(DOMAIN)?;
        self.byte(1)?;
        schema.walk(&mut SchemaPath::root("source"), |schema, depth, _| {
            self.counts.node(depth)?;
            self.node(schema)
        })
    }
    fn node(&mut self, schema: &RuntimeTypeSchema) -> Result<(), Error> {
        use RuntimeTypeSchema as S;
        match schema {
            S::Unit => self.byte(1)?,
            S::Bool => self.byte(2)?,
            S::I8 => self.byte(3)?,
            S::I16 => self.byte(4)?,
            S::I32 => self.byte(5)?,
            S::I64 => self.byte(6)?,
            S::I128 => self.byte(7)?,
            S::ISize => self.byte(8)?,
            S::U8 => self.byte(9)?,
            S::U16 => self.byte(10)?,
            S::U32 => self.byte(11)?,
            S::U64 => self.byte(12)?,
            S::U128 => self.byte(13)?,
            S::USize => self.byte(14)?,
            S::F32 => self.byte(15)?,
            S::F64 => self.byte(16)?,
            S::String => self.byte(17)?,
            S::Char => self.byte(18)?,
            S::Bytes { format } => {
                self.byte(19)?;
                self.byte(format.semantic_tag())?;
            }
            S::Builtin(builtin) => {
                self.byte(20)?;
                self.byte(builtin.owner().semantic_tag())?;
                self.schemas(builtin.payloads().len())?;
            }
            S::Seq(_) => self.byte(21)?,
            S::Map { kind, .. } => {
                self.byte(22)?;
                self.byte(kind.semantic_tag())?;
            }
            S::Record {
                name,
                fields,
                deny_unknown_fields,
            } => {
                self.byte(23)?;
                self.text(name)?;
                self.byte(u8::from(*deny_unknown_fields))?;
                self.counts
                    .array::<super::super::RuntimeSchemaField>(fields.len())?;
                self.count(fields.len())?;
                for field in fields {
                    self.text(&field.rust_name)?;
                    self.text(&field.wire_name)?;
                    self.byte(u8::from(field.has_default))?;
                    self.byte(u8::from(field.skip))?;
                    self.byte(u8::from(field.bytes_format.is_some()))?;
                    if let Some(format) = field.bytes_format {
                        self.byte(format.semantic_tag())?;
                    }
                }
            }
            S::Enum {
                name,
                variants,
                tag,
                repr,
            } => {
                self.byte(24)?;
                self.text(name)?;
                match tag {
                    RuntimeEnumTagStyle::External => self.byte(0)?,
                    RuntimeEnumTagStyle::Internal { tag } => {
                        self.byte(1)?;
                        self.text(tag)?;
                    }
                    RuntimeEnumTagStyle::Adjacent { tag, content } => {
                        self.byte(2)?;
                        self.text(tag)?;
                        self.text(content)?;
                    }
                }
                self.byte(u8::from(repr.is_some()))?;
                if let Some(repr) = repr {
                    self.byte(repr.semantic_tag())?;
                }
                self.counts
                    .array::<super::super::RuntimeSchemaVariant>(variants.len())?;
                self.count(variants.len())?;
                for variant in variants {
                    self.text(&variant.rust_name)?;
                    self.text(&variant.wire_name)?;
                    self.byte(u8::from(variant.payload.is_some()))?;
                    self.byte(u8::from(variant.discriminant.is_some()))?;
                    if let Some(value) = variant.discriminant {
                        self.raw(&value.to_le_bytes())?;
                    }
                }
            }
            S::Named(name) => {
                self.byte(25)?;
                self.text(name)?;
            }
            S::Tuple(items) => {
                self.byte(26)?;
                self.schemas(items.len())?;
            }
            S::RecordValue { fields } => {
                self.byte(28)?;
                self.counts
                    .array::<super::super::RuntimeSchemaValueField>(fields.len())?;
                self.count(fields.len())?;
                for field in fields {
                    self.u32(field.field().get().get())?;
                    self.text(field.name())?;
                }
            }
            S::ExactOpaque { owner, arguments } => {
                crate::entry::RuntimeNominalTypeId::validate_source(owner.producer().as_str())?;
                self.byte(29)?;
                self.text(owner.producer().as_str())?;
                self.raw(owner.semantic_identity().as_bytes())?;
                self.byte(owner.admission().encoded())?;
                self.byte(owner.value_class().semantic_tag())?;
                self.byte(owner.persistence().semantic_tag())?;
                self.schemas(arguments.len())?;
            }
            S::NominalRef(identity) => {
                crate::entry::RuntimeNominalTypeId::validate_source(identity.nominal().as_str())?;
                self.byte(30)?;
                self.text(identity.nominal().as_str())?;
                self.raw(identity.semantic_identity().as_bytes())?;
            }
            S::Never => self.byte(31)?,
            S::Duration => self.byte(32)?,
            S::Progress => self.byte(33)?,
            S::EntityReference => self.byte(34)?,
            S::AgentValue => self.byte(35)?,
            S::Choice(items) => {
                self.byte(36)?;
                self.schemas(items.len())?;
            }
            S::Array { length, .. } => {
                self.byte(37)?;
                self.raw(&length.to_le_bytes())?;
            }
            S::Color => self.byte(38)?,
            S::ExecutableRef(identity) => {
                self.byte(39)?;
                self.raw(identity.as_bytes())?;
            }
            S::DenseNumeric(dense) => {
                self.byte(40)?;
                self.raw(&dense.semantic_parts())?;
            }
        }
        Ok(())
    }
}
