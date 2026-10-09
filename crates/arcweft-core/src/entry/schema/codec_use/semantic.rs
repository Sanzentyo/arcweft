//! Executable codec roles read from the existing occurrence policy owner.
//! Wire names are contract fields; diagnostic/source labels are not read here.

use super::{RuntimeCodecUse, RuntimeNominalCodecUses};
use crate::plan::body_semantic::{RuntimeBodySemanticContext, RuntimeBodySemanticError};
use crate::task::semantic::TaskSemanticEncoder;

impl RuntimeNominalCodecUses {
    pub(crate) fn try_visit_semantic_child_counts<E>(
        &self,
        visitor: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<(), E> {
        self.body.try_visit_semantic_child_counts(visitor)?;
        visitor(self.arguments.len())?;
        for argument in &self.arguments {
            argument.try_visit_semantic_child_counts(visitor)?;
        }
        Ok(())
    }

    pub(crate) fn encode_executable_policy(
        &self,
        context: &RuntimeBodySemanticContext<'_>,
        encoder: &mut TaskSemanticEncoder<'_>,
    ) -> Result<(), RuntimeBodySemanticError> {
        encoder.status()?;
        self.body.encode_executable_policy(context, encoder)?;
        encoder.count(self.arguments.len());
        for (ordinal, argument) in self.arguments.iter().enumerate() {
            encoder.enter_element();
            encoder.status()?;
            encoder.count(ordinal);
            argument.encode_executable_policy(context, encoder)?;
        }
        encoder.status().map_err(Into::into)
    }
}

impl RuntimeCodecUse {
    /// Direct widths from the same borrowed grammar as validation and encoding.
    pub(crate) fn try_visit_semantic_child_counts<E>(
        &self,
        visitor: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<(), E> {
        use super::traversal::CodecEvent;
        for event in super::traversal::CodecEvents::new(self) {
            let count = match event {
                CodecEvent::Node { node, .. } => match node {
                    Self::Unary { .. } | Self::Newtype { .. } => 1,
                    Self::Map { .. } => 2,
                    Self::Tuple { items }
                    | Self::RecordFields { fields: items }
                    | Self::Builtin { payloads: items }
                    | Self::Choice {
                        alternatives: items,
                    }
                    | Self::Opaque { arguments: items } => items.len(),
                    Self::Record { fields, .. } => fields.len(),
                    Self::Enum { cases, .. } => cases.len(),
                    Self::Plain | Self::Bytes { .. } | Self::NominalRef => 0,
                },
                CodecEvent::Field { .. } => 1,
                CodecEvent::Case { case, .. } => usize::from(case.payload.is_some()),
                CodecEvent::Item { .. } => continue,
            };
            visitor(count)?;
        }
        Ok(())
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one exhaustive codec algebra with source-ordered field/case roles and one iterative work stack"
    )]
    pub(crate) fn encode_executable_policy(
        &self,
        context: &RuntimeBodySemanticContext<'_>,
        encoder: &mut TaskSemanticEncoder<'_>,
    ) -> Result<(), RuntimeBodySemanticError> {
        let mut events = super::traversal::CodecEvents::new(self);
        loop {
            encoder.status()?;
            let Some(event) = events.next() else {
                break;
            };
            match event {
                super::traversal::CodecEvent::Item { ordinal } => {
                    encoder.enter_element();
                    encoder.status()?;
                    encoder.count(ordinal);
                }
                super::traversal::CodecEvent::Field { ordinal, field } => {
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
                }
                super::traversal::CodecEvent::Case { ordinal, case } => {
                    encoder.enter_element();
                    encoder.status()?;
                    encoder.count(ordinal);
                    encoder.string(&case.wire_name);
                    encoder.tag(u8::from(case.discriminant.is_some()));
                    if let Some(discriminant) = case.discriminant {
                        encoder.scalar_u128(u128::from_le_bytes(discriminant.to_le_bytes()));
                    }
                    encoder.tag(u8::from(case.payload.is_some()));
                }
                super::traversal::CodecEvent::Node { node, .. } => {
                    encoder.enter_element();
                    encoder.status()?;
                    match node {
                        Self::Plain => encoder.tag(0),
                        Self::Bytes { format } => {
                            encoder.tag(1);
                            encoder.tag(format.semantic_tag());
                        }
                        Self::Unary { .. } => {
                            encoder.tag(2);
                        }
                        Self::Newtype { .. } => {
                            encoder.tag(3);
                        }
                        Self::Tuple { items } => {
                            encoder.tag(4);
                            encoder.count(items.len());
                        }
                        Self::Map { .. } => {
                            encoder.tag(5);
                        }
                        Self::RecordFields { fields } => {
                            encoder.tag(6);
                            encoder.count(fields.len());
                        }
                        Self::Record {
                            name: _,
                            deny_unknown_fields,
                            fields,
                        } => {
                            encoder.tag(7);
                            encoder.tag(u8::from(*deny_unknown_fields));
                            encoder.count(fields.len());
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
                        }
                        Self::Builtin { payloads } => {
                            encoder.tag(9);
                            encoder.count(payloads.len());
                        }
                        Self::Choice { alternatives } => {
                            encoder.tag(10);
                            encoder.count(alternatives.len());
                        }
                        Self::Opaque { arguments } => {
                            encoder.tag(11);
                            encoder.count(arguments.len());
                        }
                        Self::NominalRef => encoder.tag(12),
                    }
                }
            }
        }
        encoder.status().map_err(Into::into)
    }
}

#[cfg(test)]
mod tests;
