//! Executable codec roles read from the existing occurrence policy owner.
//! Wire names are contract fields; diagnostic/source labels are not read here.

use super::{RuntimeCodecUse, RuntimeFieldCodecUse, RuntimeVariantCodecUse};
use crate::plan::body_semantic::{RuntimeBodySemanticContext, RuntimeBodySemanticError};
use crate::task::semantic::TaskSemanticEncoder;

impl RuntimeCodecUse {
    #[expect(
        clippy::too_many_lines,
        reason = "one exhaustive codec algebra with source-ordered field/case roles and one iterative work stack"
    )]
    pub(crate) fn encode_executable_policy(
        &self,
        context: &RuntimeBodySemanticContext<'_>,
        encoder: &mut TaskSemanticEncoder<'_>,
    ) -> Result<(), RuntimeBodySemanticError> {
        enum Work<'a> {
            Node(&'a RuntimeCodecUse),
            Nodes(std::iter::Enumerate<std::slice::Iter<'a, RuntimeCodecUse>>),
            Fields(std::iter::Enumerate<std::slice::Iter<'a, RuntimeFieldCodecUse>>),
            Cases(std::iter::Enumerate<std::slice::Iter<'a, RuntimeVariantCodecUse>>),
        }
        let mut work = vec![Work::Node(self)];
        while let Some(next) = work.pop() {
            encoder.status()?;
            match next {
                Work::Nodes(mut nodes) => {
                    if let Some((ordinal, node)) = nodes.next() {
                        encoder.enter_element();
                        encoder.status()?;
                        encoder.count(ordinal);
                        work.push(Work::Nodes(nodes));
                        work.push(Work::Node(node));
                    }
                }
                Work::Fields(mut fields) => {
                    if let Some((ordinal, field)) = fields.next() {
                        encoder.enter_element();
                        encoder.status()?;
                        encoder.count(ordinal);
                        encoder.string(&field.wire_name);
                        encoder.tag(u8::from(field.has_default));
                        encoder.tag(u8::from(field.default_program.is_some()));
                        if let Some(program) = field.default_program {
                            context.write_pure_program_reference(encoder, program)?;
                        }
                        encoder.tag(u8::from(field.skip));
                        encoder.tag(u8::from(field.bytes_format.is_some()));
                        if let Some(format) = field.bytes_format {
                            encoder.tag(format.semantic_tag());
                        }
                        work.push(Work::Fields(fields));
                        work.push(Work::Node(&field.value));
                    }
                }
                Work::Cases(mut cases) => {
                    if let Some((ordinal, case)) = cases.next() {
                        encoder.enter_element();
                        encoder.status()?;
                        encoder.count(ordinal);
                        encoder.string(&case.wire_name);
                        encoder.tag(u8::from(case.discriminant.is_some()));
                        if let Some(discriminant) = case.discriminant {
                            encoder.scalar_u128(u128::from_le_bytes(discriminant.to_le_bytes()));
                        }
                        encoder.tag(u8::from(case.payload.is_some()));
                        work.push(Work::Cases(cases));
                        if let Some(payload) = &case.payload {
                            work.push(Work::Node(payload));
                        }
                    }
                }
                Work::Node(node) => {
                    encoder.enter_element();
                    encoder.status()?;
                    match node {
                        Self::Plain => encoder.tag(0),
                        Self::Bytes { format } => {
                            encoder.tag(1);
                            encoder.tag(format.semantic_tag());
                        }
                        Self::Unary { item } => {
                            encoder.tag(2);
                            work.push(Work::Node(item));
                        }
                        Self::Newtype { inner } => {
                            encoder.tag(3);
                            work.push(Work::Node(inner));
                        }
                        Self::Tuple { items } => {
                            encoder.tag(4);
                            encoder.count(items.len());
                            work.push(Work::Nodes(items.iter().enumerate()));
                        }
                        Self::Map { key, value } => {
                            encoder.tag(5);
                            work.push(Work::Node(value));
                            work.push(Work::Node(key));
                        }
                        Self::RecordFields { fields } => {
                            encoder.tag(6);
                            encoder.count(fields.len());
                            work.push(Work::Nodes(fields.iter().enumerate()));
                        }
                        Self::Record {
                            name: _,
                            deny_unknown_fields,
                            fields,
                        } => {
                            encoder.tag(7);
                            encoder.tag(u8::from(*deny_unknown_fields));
                            encoder.count(fields.len());
                            work.push(Work::Fields(fields.iter().enumerate()));
                        }
                        Self::Enum {
                            name: _,
                            tag,
                            repr,
                            cases,
                        } => {
                            encoder.tag(8);
                            match tag {
                                crate::entry::schema::RuntimeEnumTagStyle::External => {
                                    encoder.tag(0);
                                }
                                crate::entry::schema::RuntimeEnumTagStyle::Internal { tag } => {
                                    encoder.tag(1);
                                    encoder.string(tag);
                                }
                                crate::entry::schema::RuntimeEnumTagStyle::Adjacent {
                                    tag,
                                    content,
                                } => {
                                    encoder.tag(2);
                                    encoder.string(tag);
                                    encoder.string(content);
                                }
                            }
                            encoder.tag(u8::from(repr.is_some()));
                            if let Some(repr) = repr {
                                encoder.tag(repr.semantic_tag());
                            }
                            encoder.count(cases.len());
                            work.push(Work::Cases(cases.iter().enumerate()));
                        }
                        Self::Builtin { payloads } => {
                            encoder.tag(9);
                            encoder.count(payloads.len());
                            work.push(Work::Nodes(payloads.iter().enumerate()));
                        }
                        Self::Choice { alternatives } => {
                            encoder.tag(10);
                            encoder.count(alternatives.len());
                            work.push(Work::Nodes(alternatives.iter().enumerate()));
                        }
                        Self::Opaque { arguments } => {
                            encoder.tag(11);
                            encoder.count(arguments.len());
                            work.push(Work::Nodes(arguments.iter().enumerate()));
                        }
                        Self::NominalRef => encoder.tag(12),
                    }
                }
            }
        }
        encoder.status().map_err(Into::into)
    }
}
