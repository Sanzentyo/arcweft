//! Borrowed metadata and iterative construction share the same node grammar.

use super::{Counts, DOMAIN, Error, Footprint, RuntimeSchemaSourceLimits};
use crate::entry::RuntimeNominalTypeId;
use crate::entry::schema::{
    RuntimeBuiltinSchema, RuntimeBytesFormat, RuntimeEnumRepr, RuntimeEnumTagStyle, RuntimeMapKind,
    RuntimeNominalSchemaIdentity, RuntimeSchemaField, RuntimeSchemaValueField,
    RuntimeSchemaVariant, RuntimeTypeSchema,
};
use crate::pattern::{
    RuntimeBuiltinVariantIdentity, RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner,
    RuntimeOpaqueTypeProducerId, RuntimeSemanticTypeId,
};
use crate::value::{RuntimeOpaquePersistence, RuntimeOpaqueValueClass, RuntimeRecordFieldId};

pub(super) struct Frame<'a> {
    node: Node<'a>,
    children: Vec<RuntimeTypeSchema>,
}
struct Node<'a> {
    tag: u8,
    metadata: &'a [u8],
    children: usize,
}

pub(super) fn inspect(bytes: &[u8], limits: RuntimeSchemaSourceLimits) -> Result<Footprint, Error> {
    let mut reader = Cursor::new(bytes);
    reader.header()?;
    let mut counts = Counts::new(limits);
    let mut parents = Vec::<usize>::new();
    loop {
        counts.node(parents.len().checked_add(1).ok_or(Error::Arithmetic)?)?;
        let node = reader.node(&mut counts)?;
        if node.children != 0 {
            parents.push(node.children);
            continue;
        }
        loop {
            match parents.last_mut() {
                Some(remaining) => {
                    *remaining -= 1;
                    if *remaining != 0 {
                        break;
                    }
                    parents.pop();
                }
                None => {
                    reader.finish()?;
                    return Ok(counts.footprint);
                }
            }
        }
    }
}

pub(super) fn materialize(
    bytes: &[u8],
    limits: RuntimeSchemaSourceLimits,
    footprint: Footprint,
) -> RuntimeTypeSchema {
    let mut reader = Cursor::new(bytes);
    reader.header().expect("preflighted schema header");
    let mut counts = Counts::new(limits);
    let mut frames = Vec::<Frame<'_>>::with_capacity(footprint.depth);
    loop {
        counts
            .node(frames.len() + 1)
            .expect("preflighted schema depth and node storage");
        let node = reader
            .node(&mut counts)
            .expect("immutable preflighted schema metadata");
        let children = Vec::with_capacity(node.children);
        frames.push(Frame { node, children });
        loop {
            let frame = frames.last().expect("one active source node");
            if frame.children.len() != frame.node.children {
                break;
            }
            let frame = frames.pop().expect("completed source node");
            let value = frame.node.materialize(frame.children);
            match frames.last_mut() {
                Some(parent) => parent.children.push(value),
                None => {
                    reader.finish().expect("preflighted complete schema packet");
                    debug_assert_eq!(counts.footprint, footprint);
                    return value;
                }
            }
        }
    }
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn raw(&mut self, count: usize) -> Result<&'a [u8], Error> {
        let end = self.offset.checked_add(count).ok_or(Error::Arithmetic)?;
        let bytes = self.bytes.get(self.offset..end).ok_or(Error::Truncated {
            offset: self.offset,
        })?;
        self.offset = end;
        Ok(bytes)
    }
    fn byte(&mut self) -> Result<u8, Error> {
        Ok(self.raw(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(
            self.raw(4)?.try_into().expect("source u32 width"),
        ))
    }
    fn u64(&mut self) -> Result<u64, Error> {
        Ok(u64::from_le_bytes(
            self.raw(8)?.try_into().expect("source u64 width"),
        ))
    }
    fn digest(&mut self) -> Result<[u8; 32], Error> {
        Ok(self.raw(32)?.try_into().expect("source digest width"))
    }
    fn string(&mut self) -> Result<&'a str, Error> {
        let length = self.u32()? as usize;
        Ok(std::str::from_utf8(self.raw(length)?)?)
    }
    fn text(&mut self, counts: &mut Counts) -> Result<&'a str, Error> {
        let value = self.string()?;
        counts.text(value.len())?;
        Ok(value)
    }
    fn flag(&mut self) -> Result<bool, Error> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            tag => Err(Error::Tag { kind: "flag", tag }),
        }
    }
    fn tag<T>(
        &mut self,
        kind: &'static str,
        decode: impl FnOnce(u8) -> Option<T>,
    ) -> Result<T, Error> {
        let tag = self.byte()?;
        decode(tag).ok_or(Error::Tag { kind, tag })
    }
    fn bytes_format(&mut self) -> Result<RuntimeBytesFormat, Error> {
        self.tag("bytes format", RuntimeBytesFormat::from_semantic_tag)
    }
    fn field(&mut self) -> Result<RuntimeRecordFieldId, Error> {
        let actual = self.u32()?;
        let index = actual.checked_sub(1).ok_or(Error::Field { actual })?;
        RuntimeRecordFieldId::try_from_zero_based_ordinal(index as usize)
            .map_err(|_| Error::Field { actual })
    }
    fn enum_tag(&mut self, counts: &mut Counts) -> Result<(), Error> {
        match self.byte()? {
            0 => (),
            1 => {
                self.text(counts)?;
            }
            2 => {
                self.text(counts)?;
                self.text(counts)?;
            }
            tag => {
                return Err(Error::Tag {
                    kind: "enum tagging",
                    tag,
                });
            }
        }
        Ok(())
    }
    fn repr(&mut self) -> Result<Option<RuntimeEnumRepr>, Error> {
        if self.flag()? {
            self.tag("enum repr", RuntimeEnumRepr::from_semantic_tag)
                .map(Some)
        } else {
            Ok(None)
        }
    }
    fn format_option(&mut self) -> Result<Option<RuntimeBytesFormat>, Error> {
        if self.flag()? {
            self.bytes_format().map(Some)
        } else {
            Ok(None)
        }
    }
    fn discriminant(&mut self) -> Result<Option<i128>, Error> {
        if self.flag()? {
            Ok(Some(i128::from_le_bytes(
                self.raw(16)?.try_into().expect("source i128 width"),
            )))
        } else {
            Ok(None)
        }
    }
    fn schemas(&mut self, counts: &mut Counts) -> Result<usize, Error> {
        let count = self.u32()? as usize;
        counts.array::<RuntimeTypeSchema>(count)?;
        Ok(count)
    }
    fn header(&mut self) -> Result<(), Error> {
        if self.raw(DOMAIN.len())? != DOMAIN {
            return Err(Error::Domain);
        }
        match self.byte()? {
            1 => Ok(()),
            actual => Err(Error::Version { actual }),
        }
    }
    fn finish(&self) -> Result<(), Error> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(Error::Trailing {
                offset: self.offset,
            })
        }
    }
    fn node(&mut self, counts: &mut Counts) -> Result<Node<'a>, Error> {
        let tag = self.byte()?;
        let start = self.offset;
        let children = match tag {
            1..=18 | 31..=35 | 38 => 0,
            19 => {
                self.bytes_format()?;
                0
            }
            20 => {
                let owner = self.tag(
                    "builtin owner",
                    RuntimeBuiltinVariantIdentity::from_wire_tag,
                )?;
                let count = self.schemas(counts)?;
                RuntimeBuiltinSchema::check_payload_count(owner, count)?;
                count
            }
            21 => 1,
            22 => {
                self.tag("map kind", RuntimeMapKind::from_semantic_tag)?;
                2
            }
            23 => {
                self.text(counts)?;
                self.flag()?;
                let count = self.u32()? as usize;
                counts.array::<RuntimeSchemaField>(count)?;
                for _ in 0..count {
                    self.text(counts)?;
                    self.text(counts)?;
                    self.flag()?;
                    self.flag()?;
                    self.format_option()?;
                }
                count
            }
            24 => {
                self.text(counts)?;
                self.enum_tag(counts)?;
                self.repr()?;
                let count = self.u32()? as usize;
                counts.array::<RuntimeSchemaVariant>(count)?;
                let mut children = 0usize;
                for _ in 0..count {
                    self.text(counts)?;
                    self.text(counts)?;
                    if self.flag()? {
                        children = children.checked_add(1).ok_or(Error::Arithmetic)?;
                    }
                    self.discriminant()?;
                }
                children
            }
            25 => {
                self.text(counts)?;
                0
            }
            26 | 36 => self.schemas(counts)?,
            28 => {
                let count = self.u32()? as usize;
                counts.array::<RuntimeSchemaValueField>(count)?;
                for _ in 0..count {
                    self.field()?;
                    self.text(counts)?;
                }
                count
            }
            29 => {
                RuntimeNominalTypeId::validate_source(self.text(counts)?)?;
                self.digest()?;
                self.tag("opaque admission", RuntimeOpaqueTypeAdmission::from_encoded)?;
                self.tag(
                    "opaque value class",
                    RuntimeOpaqueValueClass::from_semantic_tag,
                )?;
                self.tag(
                    "opaque persistence",
                    RuntimeOpaquePersistence::from_semantic_tag,
                )?;
                self.schemas(counts)?
            }
            30 => {
                RuntimeNominalTypeId::validate_source(self.text(counts)?)?;
                self.digest()?;
                0
            }
            37 => {
                self.u64()?;
                1
            }
            39 => {
                self.digest()?;
                0
            }
            40 => {
                let parts = [self.byte()?, self.byte()?];
                if crate::math::RuntimeDenseNumericType::from_semantic_parts(parts).is_none() {
                    return Err(Error::Tag {
                        kind: "dense numeric shape",
                        tag: parts[1],
                    });
                }
                0
            }
            tag => {
                return Err(Error::Tag {
                    kind: "schema",
                    tag,
                });
            }
        };
        Ok(Node {
            tag,
            metadata: &self.bytes[start..self.offset],
            children,
        })
    }
}

impl Node<'_> {
    fn materialize(self, children: Vec<RuntimeTypeSchema>) -> RuntimeTypeSchema {
        self.build(children)
            .expect("complete immutable source metadata and child inventory preflighted")
    }
    fn build(self, children: Vec<RuntimeTypeSchema>) -> Result<RuntimeTypeSchema, Error> {
        use RuntimeTypeSchema as S;
        let mut reader = Cursor::new(self.metadata);
        let mut children = children.into_iter();
        let mut child = || children.next().expect("preflighted direct schema child");
        let schema = match self.tag {
            1 => S::Unit,
            2 => S::Bool,
            3 => S::I8,
            4 => S::I16,
            5 => S::I32,
            6 => S::I64,
            7 => S::I128,
            8 => S::ISize,
            9 => S::U8,
            10 => S::U16,
            11 => S::U32,
            12 => S::U64,
            13 => S::U128,
            14 => S::USize,
            15 => S::F32,
            16 => S::F64,
            17 => S::String,
            18 => S::Char,
            19 => S::Bytes {
                format: reader.bytes_format()?,
            },
            20 => {
                let owner = reader.tag(
                    "builtin owner",
                    RuntimeBuiltinVariantIdentity::from_wire_tag,
                )?;
                let count = reader.u32()? as usize;
                S::Builtin(RuntimeBuiltinSchema::try_new(
                    owner,
                    (0..count).map(|_| child()).collect::<Box<[_]>>(),
                )?)
            }
            21 => S::Seq(Box::new(child())),
            22 => S::Map {
                kind: reader.tag("map kind", RuntimeMapKind::from_semantic_tag)?,
                key: Box::new(child()),
                value: Box::new(child()),
            },
            23 => {
                let name = reader.string()?.to_owned();
                let deny_unknown_fields = reader.flag()?;
                let count = reader.u32()? as usize;
                let mut fields = Vec::with_capacity(count);
                for _ in 0..count {
                    fields.push(RuntimeSchemaField {
                        rust_name: reader.string()?.to_owned(),
                        wire_name: reader.string()?.to_owned(),
                        has_default: reader.flag()?,
                        skip: reader.flag()?,
                        bytes_format: reader.format_option()?,
                        schema: child(),
                    });
                }
                S::Record {
                    name,
                    deny_unknown_fields,
                    fields,
                }
            }
            24 => {
                let name = reader.string()?.to_owned();
                let tag = match reader.byte()? {
                    0 => RuntimeEnumTagStyle::External,
                    1 => RuntimeEnumTagStyle::Internal {
                        tag: reader.string()?.to_owned(),
                    },
                    2 => RuntimeEnumTagStyle::Adjacent {
                        tag: reader.string()?.to_owned(),
                        content: reader.string()?.to_owned(),
                    },
                    _ => unreachable!("preflighted enum tag"),
                };
                let repr = reader.repr()?;
                let count = reader.u32()? as usize;
                let mut variants = Vec::with_capacity(count);
                for _ in 0..count {
                    let rust_name = reader.string()?.to_owned();
                    let wire_name = reader.string()?.to_owned();
                    let payload = if reader.flag()? { Some(child()) } else { None };
                    variants.push(RuntimeSchemaVariant {
                        rust_name,
                        wire_name,
                        payload,
                        discriminant: reader.discriminant()?,
                    });
                }
                S::Enum {
                    name,
                    tag,
                    repr,
                    variants,
                }
            }
            25 => S::Named(reader.string()?.to_owned()),
            26 => {
                let count = reader.u32()? as usize;
                S::Tuple((0..count).map(|_| child()).collect())
            }
            28 => {
                let count = reader.u32()? as usize;
                let mut fields = Vec::with_capacity(count);
                for _ in 0..count {
                    fields.push(RuntimeSchemaValueField::new(
                        reader.field()?,
                        reader.string()?.to_owned(),
                        child(),
                    ));
                }
                S::RecordValue {
                    fields: fields.into_boxed_slice(),
                }
            }
            29 => {
                let producer = RuntimeOpaqueTypeProducerId::try_new(reader.string()?)?;
                let semantic = RuntimeSemanticTypeId::from_bytes(reader.digest()?);
                let admission =
                    reader.tag("opaque admission", RuntimeOpaqueTypeAdmission::from_encoded)?;
                let class = reader.tag(
                    "opaque value class",
                    RuntimeOpaqueValueClass::from_semantic_tag,
                )?;
                let persistence = reader.tag(
                    "opaque persistence",
                    RuntimeOpaquePersistence::from_semantic_tag,
                )?;
                let count = reader.u32()? as usize;
                S::ExactOpaque {
                    owner: RuntimeOpaqueTypeOwner::with_admission(
                        producer,
                        semantic,
                        admission,
                        class,
                        persistence,
                    ),
                    arguments: (0..count).map(|_| child()).collect(),
                }
            }
            30 => S::NominalRef(RuntimeNominalSchemaIdentity::new(
                RuntimeNominalTypeId::try_new(reader.string()?)?,
                RuntimeSemanticTypeId::from_bytes(reader.digest()?),
            )),
            31 => S::Never,
            32 => S::Duration,
            33 => S::Progress,
            34 => S::EntityReference,
            35 => S::AgentValue,
            36 => {
                let count = reader.u32()? as usize;
                S::Choice((0..count).map(|_| child()).collect())
            }
            37 => {
                let length = reader.u64()?;
                S::Array {
                    length,
                    item: Box::new(child()),
                }
            }
            38 => S::Color,
            39 => S::ExecutableRef(RuntimeSemanticTypeId::from_bytes(reader.digest()?)),
            40 => S::DenseNumeric(
                crate::math::RuntimeDenseNumericType::from_semantic_parts([
                    reader.byte()?,
                    reader.byte()?,
                ])
                .expect("preflighted dense numeric shape"),
            ),
            _ => unreachable!("preflighted schema tag"),
        };
        debug_assert!(children.next().is_none());
        reader.finish()?;
        Ok(schema)
    }
}
