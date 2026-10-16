//! Source policy fields; logical type identity remains on the type projection.

use super::{Reader, RuntimeTypeSourceError, Writer};
use crate::entry::schema::{
    RuntimeBytesFormat, RuntimeCodecUse, RuntimeEnumRepr, RuntimeEnumTagStyle,
    RuntimeFieldCodecUse, RuntimeVariantCodecUse,
};
use arcweft_id::runtime_program::RuntimePureProgramId;

impl Writer<'_> {
    fn policies(
        &mut self,
        items: &[RuntimeCodecUse],
        depth: usize,
    ) -> Result<(), RuntimeTypeSourceError> {
        self.counts.array::<RuntimeCodecUse>(items.len())?;
        self.count(items.len())?;
        for item in items {
            self.policy(item, depth)?;
        }
        Ok(())
    }

    pub(super) fn policy(
        &mut self,
        policy: &RuntimeCodecUse,
        depth: usize,
    ) -> Result<(), RuntimeTypeSourceError> {
        use RuntimeCodecUse as P;
        self.counts.policy_depth(depth)?;
        self.counts.elements(1)?;
        self.counts.policy_admission()?;
        match policy {
            P::Plain => self.tag(0)?,
            P::Bytes { format } => {
                self.tag(1)?;
                self.tag(format.semantic_tag())?;
            }
            P::Unary { item } => {
                self.tag(2)?;
                self.counts.storage(size_of::<P>())?;
                self.policy(item, depth + 1)?;
            }
            P::Newtype { inner } => {
                self.tag(3)?;
                self.counts.storage(size_of::<P>())?;
                self.policy(inner, depth + 1)?;
            }
            P::Tuple { items } => {
                self.tag(4)?;
                self.policies(items, depth + 1)?;
            }
            P::Map { key, value } => {
                self.tag(5)?;
                self.counts.storage(2 * size_of::<P>())?;
                self.policy(key, depth + 1)?;
                self.policy(value, depth + 1)?;
            }
            P::RecordFields { fields } => {
                self.tag(6)?;
                self.policies(fields, depth + 1)?;
            }
            P::Record {
                name,
                deny_unknown_fields,
                fields,
            } => {
                self.tag(7)?;
                self.string(name)?;
                self.tag(u8::from(*deny_unknown_fields))?;
                self.counts.array::<RuntimeFieldCodecUse>(fields.len())?;
                self.count(fields.len())?;
                for field in fields {
                    self.string(&field.wire_name)?;
                    self.tag(u8::from(field.has_default))?;
                    self.tag(u8::from(field.default_program.is_some()))?;
                    if let Some(program) = field.default_program {
                        self.bytes(&program.as_bytes())?;
                    }
                    self.tag(u8::from(field.skip))?;
                    self.tag(u8::from(field.bytes_format.is_some()))?;
                    if let Some(format) = field.bytes_format {
                        self.tag(format.semantic_tag())?;
                    }
                    self.policy(&field.value, depth + 1)?;
                }
            }
            P::Enum {
                name,
                tag,
                repr,
                cases,
            } => {
                self.tag(8)?;
                self.string(name)?;
                match tag {
                    RuntimeEnumTagStyle::External => self.tag(0)?,
                    RuntimeEnumTagStyle::Internal { tag } => {
                        self.tag(1)?;
                        self.string(tag)?;
                    }
                    RuntimeEnumTagStyle::Adjacent { tag, content } => {
                        self.tag(2)?;
                        self.string(tag)?;
                        self.string(content)?;
                    }
                }
                self.tag(u8::from(repr.is_some()))?;
                if let Some(repr) = repr {
                    self.tag(repr.semantic_tag())?;
                }
                self.counts.array::<RuntimeVariantCodecUse>(cases.len())?;
                self.count(cases.len())?;
                for case in cases {
                    self.string(&case.wire_name)?;
                    self.tag(u8::from(case.discriminant.is_some()))?;
                    if let Some(discriminant) = case.discriminant {
                        self.bytes(&discriminant.to_le_bytes())?;
                    }
                    self.tag(u8::from(case.payload.is_some()))?;
                    if let Some(payload) = &case.payload {
                        self.policy(payload, depth + 1)?;
                    }
                }
            }
            P::Builtin { payloads } => {
                self.tag(9)?;
                self.policies(payloads, depth + 1)?;
            }
            P::Choice { alternatives } => {
                self.tag(10)?;
                self.policies(alternatives, depth + 1)?;
            }
            P::Opaque { arguments } => {
                self.tag(11)?;
                self.policies(arguments, depth + 1)?;
            }
            P::NominalRef => self.tag(12)?,
        }
        Ok(())
    }
}

impl Reader<'_, '_> {
    fn policies(
        &mut self,
        materialize: bool,
        depth: usize,
    ) -> Result<Option<Box<[RuntimeCodecUse]>>, RuntimeTypeSourceError> {
        let count = self.count()?;
        self.counts.array::<RuntimeCodecUse>(count)?;
        let mut items = materialize.then(|| Vec::with_capacity(count));
        for _ in 0..count {
            let item = self.policy(materialize, depth)?;
            if let Some(items) = &mut items {
                items.push(item.expect("retaining checked codec source"));
            }
        }
        Ok(items.map(Vec::into_boxed_slice))
    }

    fn bytes_format(&mut self) -> Result<RuntimeBytesFormat, RuntimeTypeSourceError> {
        let tag = self.tag()?;
        RuntimeBytesFormat::from_semantic_tag(tag).ok_or(RuntimeTypeSourceError::Tag {
            kind: "bytes format",
            tag,
        })
    }

    pub(super) fn policy(
        &mut self,
        materialize: bool,
        depth: usize,
    ) -> Result<Option<RuntimeCodecUse>, RuntimeTypeSourceError> {
        use RuntimeCodecUse as P;
        self.counts.policy_depth(depth)?;
        self.counts.elements(1)?;
        self.counts.policy_admission()?;
        Ok(match self.tag()? {
            0 => materialize.then_some(P::Plain),
            1 => {
                let format = self.bytes_format()?;
                materialize.then_some(P::Bytes { format })
            }
            2 => {
                self.counts.storage(size_of::<P>())?;
                self.policy(materialize, depth + 1)?.map(|item| P::Unary {
                    item: Box::new(item),
                })
            }
            3 => {
                self.counts.storage(size_of::<P>())?;
                self.policy(materialize, depth + 1)?
                    .map(|inner| P::Newtype {
                        inner: Box::new(inner),
                    })
            }
            4 => self
                .policies(materialize, depth + 1)?
                .map(|items| P::Tuple { items }),
            5 => {
                self.counts.storage(2 * size_of::<P>())?;
                let key = self.policy(materialize, depth + 1)?;
                let value = self.policy(materialize, depth + 1)?;
                key.zip(value).map(|(key, value)| P::Map {
                    key: Box::new(key),
                    value: Box::new(value),
                })
            }
            6 => self
                .policies(materialize, depth + 1)?
                .map(|fields| P::RecordFields { fields }),
            7 => {
                let name = self.string()?;
                let deny_unknown_fields = self.boolean()?;
                let count = self.count()?;
                self.counts.array::<RuntimeFieldCodecUse>(count)?;
                let mut fields = materialize.then(|| Vec::with_capacity(count));
                for _ in 0..count {
                    let wire_name = self.string()?;
                    let has_default = self.boolean()?;
                    let default_program = if self.boolean()? {
                        Some(RuntimePureProgramId::from_checked_digest(self.digest()?))
                    } else {
                        None
                    };
                    let skip = self.boolean()?;
                    let bytes_format = if self.boolean()? {
                        Some(self.bytes_format()?)
                    } else {
                        None
                    };
                    let value = self.policy(materialize, depth + 1)?;
                    if let Some(fields) = &mut fields {
                        fields.push(RuntimeFieldCodecUse {
                            wire_name: wire_name.to_owned(),
                            has_default,
                            default_program,
                            skip,
                            bytes_format,
                            value: value.expect("retaining field codec source"),
                        });
                    }
                }
                fields.map(|fields| P::Record {
                    name: name.to_owned(),
                    deny_unknown_fields,
                    fields: fields.into_boxed_slice(),
                })
            }
            8 => {
                let name = self.string()?;
                // String ownership is deferred with the rest of the enum.
                let (style, tag_name, content_name) = match self.tag()? {
                    0 => (0, None, None),
                    1 => (1, Some(self.string()?), None),
                    2 => (2, Some(self.string()?), Some(self.string()?)),
                    tag => {
                        return Err(RuntimeTypeSourceError::Tag {
                            kind: "enum tag style",
                            tag,
                        });
                    }
                };
                let repr = if self.boolean()? {
                    let tag = self.tag()?;
                    Some(RuntimeEnumRepr::from_semantic_tag(tag).ok_or(
                        RuntimeTypeSourceError::Tag {
                            kind: "enum repr",
                            tag,
                        },
                    )?)
                } else {
                    None
                };
                let count = self.count()?;
                self.counts.array::<RuntimeVariantCodecUse>(count)?;
                let mut cases = materialize.then(|| Vec::with_capacity(count));
                for _ in 0..count {
                    let wire_name = self.string()?;
                    let discriminant = if self.boolean()? {
                        Some(i128::from_le_bytes(
                            self.take(16)?
                                .try_into()
                                .expect("16-byte case discriminant"),
                        ))
                    } else {
                        None
                    };
                    let payload = if self.boolean()? {
                        self.policy(materialize, depth + 1)?
                    } else {
                        None
                    };
                    if let Some(cases) = &mut cases {
                        cases.push(RuntimeVariantCodecUse {
                            wire_name: wire_name.to_owned(),
                            discriminant,
                            payload,
                        });
                    }
                }
                cases.map(|cases| P::Enum {
                    name: name.to_owned(),
                    tag: match style {
                        0 => RuntimeEnumTagStyle::External,
                        1 => RuntimeEnumTagStyle::Internal {
                            tag: tag_name.expect("internal source tag").to_owned(),
                        },
                        2 => RuntimeEnumTagStyle::Adjacent {
                            tag: tag_name.expect("adjacent source tag").to_owned(),
                            content: content_name.expect("adjacent source content").to_owned(),
                        },
                        _ => unreachable!("checked enum tag style"),
                    },
                    repr,
                    cases: cases.into_boxed_slice(),
                })
            }
            9 => self
                .policies(materialize, depth + 1)?
                .map(|payloads| P::Builtin { payloads }),
            10 => self
                .policies(materialize, depth + 1)?
                .map(|alternatives| P::Choice { alternatives }),
            11 => self
                .policies(materialize, depth + 1)?
                .map(|arguments| P::Opaque { arguments }),
            12 => materialize.then_some(P::NominalRef),
            tag => {
                return Err(RuntimeTypeSourceError::Tag {
                    kind: "codec policy",
                    tag,
                });
            }
        })
    }
}
